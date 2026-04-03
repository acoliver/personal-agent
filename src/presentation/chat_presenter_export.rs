use std::sync::Arc;

use tokio::sync::mpsc;

use super::conversation_export::{
    build_export_filename, render_export_content, resolve_export_directory,
    resolve_unique_export_path, validate_export_directory, write_export_file_retrying_collisions,
    EXPORT_DIR_SETTING_KEY, EXPORT_FORMAT_SETTING_KEY,
};
use super::view_command::ErrorSeverity;
use super::{ChatPresenter, ViewCommand};
use crate::models::ConversationExportFormat;
use crate::services::{AppSettingsService, ConversationService};
use crate::ui_gpui::error_log::{render_error_log_text, ErrorLogStore};

impl ChatPresenter {
    pub(crate) async fn emit_initial_export_format(
        app_settings_service: &Arc<dyn AppSettingsService>,
        current_export_format: &Arc<std::sync::Mutex<ConversationExportFormat>>,
        view_tx: &mpsc::Sender<ViewCommand>,
    ) {
        let view_tx = view_tx.clone();
        let format = Self::load_conversation_export_format(app_settings_service).await;
        *current_export_format
            .lock()
            .expect("export format mutex poisoned") = format;
        let _ = view_tx
            .send(ViewCommand::ShowConversationExportFormat { format })
            .await;

        let export_dir = app_settings_service
            .get_setting(EXPORT_DIR_SETTING_KEY)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let _ = view_tx
            .send(ViewCommand::ExportDirectoryLoaded { path: export_dir })
            .await;
    }

    pub(crate) async fn load_conversation_export_format(
        app_settings_service: &Arc<dyn AppSettingsService>,
    ) -> ConversationExportFormat {
        app_settings_service
            .get_setting(EXPORT_FORMAT_SETTING_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|raw| ConversationExportFormat::from_setting_value(&raw))
            .unwrap_or_default()
    }

    pub(crate) async fn handle_select_conversation_export_format(
        app_settings_service: &Arc<dyn AppSettingsService>,
        current_export_format: &Arc<std::sync::Mutex<ConversationExportFormat>>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        format: ConversationExportFormat,
    ) {
        *current_export_format
            .lock()
            .expect("export format mutex poisoned") = format;

        if let Err(error) = app_settings_service
            .set_setting(
                EXPORT_FORMAT_SETTING_KEY,
                format.as_setting_value().to_string(),
            )
            .await
        {
            tracing::warn!("Failed to persist export format setting: {error}");
            let _ = view_tx
                .send(ViewCommand::ShowError {
                    title: "Export Format".to_string(),
                    message: "Failed to persist export format preference".to_string(),
                    severity: ErrorSeverity::Warning,
                })
                .await;
        }

        let _ = view_tx
            .send(ViewCommand::ShowConversationExportFormat { format })
            .await;
    }

    pub(crate) async fn handle_save_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        current_export_format: &Arc<std::sync::Mutex<ConversationExportFormat>>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        let active = match conversation_service.get_active().await {
            Ok(Some(id)) => id,
            Ok(None) => {
                let _ = view_tx
                    .send(ViewCommand::ShowNotification {
                        message: "No active conversation to save".to_string(),
                    })
                    .await;
                return;
            }
            Err(error) => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Save Conversation".to_string(),
                        message: format!("Failed to load active conversation: {error}"),
                        severity: ErrorSeverity::Error,
                    })
                    .await;
                return;
            }
        };

        let conversation = match conversation_service.load(active).await {
            Ok(conversation) => conversation,
            Err(error) => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Save Conversation".to_string(),
                        message: format!("Failed to load conversation for export: {error}"),
                        severity: ErrorSeverity::Error,
                    })
                    .await;
                return;
            }
        };

        let format = *current_export_format
            .lock()
            .expect("export format mutex poisoned");
        let export_body = match render_export_content(&conversation, format) {
            Ok(body) => body,
            Err(error) => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Save Conversation".to_string(),
                        message: error,
                        severity: ErrorSeverity::Error,
                    })
                    .await;
                return;
            }
        };

        let configured_export_dir = app_settings_service
            .get_setting(EXPORT_DIR_SETTING_KEY)
            .await
            .ok()
            .flatten();
        let export_dir = resolve_export_directory(configured_export_dir.as_deref());
        let filename = build_export_filename(&conversation, format);
        let initial_path = resolve_unique_export_path(&export_dir, &filename);

        let path = match write_export_file_retrying_collisions(initial_path, &export_body) {
            Ok(path) => path,
            Err(error) => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Save Conversation".to_string(),
                        message: format!("failed to write export file {filename}: {error}"),
                        severity: ErrorSeverity::Error,
                    })
                    .await;
                return;
            }
        };

        let _ = view_tx
            .send(ViewCommand::ExportCompleted {
                path: path.display().to_string(),
                format_label: format.display_label().to_string(),
            })
            .await;
        let _ = view_tx
            .send(ViewCommand::ShowConversationExportFormat { format })
            .await;
    }

    pub(crate) async fn handle_save_error_log(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        let entries = ErrorLogStore::global().entries();
        if entries.is_empty() {
            let _ = view_tx
                .send(ViewCommand::ShowNotification {
                    message: "No errors recorded".to_string(),
                })
                .await;
            return;
        }

        let export_body = render_error_log_text(&entries);

        let configured_export_dir = app_settings_service
            .get_setting(EXPORT_DIR_SETTING_KEY)
            .await
            .ok()
            .flatten();
        let export_dir = resolve_export_directory(configured_export_dir.as_deref());
        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let filename = format!("{timestamp}-error-log.txt");
        let initial_path = resolve_unique_export_path(&export_dir, &filename);

        let path = match write_export_file_retrying_collisions(initial_path, &export_body) {
            Ok(path) => path,
            Err(error) => {
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Save Error Log".to_string(),
                        message: format!("failed to write export file {filename}: {error}"),
                        severity: ErrorSeverity::Error,
                    })
                    .await;
                return;
            }
        };

        let _ = view_tx
            .send(ViewCommand::ErrorLogExportCompleted {
                path: path.display().to_string(),
            })
            .await;
    }

    pub(crate) async fn handle_set_export_directory(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        path: String,
    ) {
        let trimmed = path.trim().to_string();
        let trimmed = if trimmed.starts_with('~') {
            resolve_export_directory(Some(&trimmed))
                .to_string_lossy()
                .to_string()
        } else {
            trimmed
        };

        if trimmed.is_empty() {
            if let Err(error) = app_settings_service
                .set_setting(EXPORT_DIR_SETTING_KEY, String::new())
                .await
            {
                tracing::warn!("Failed to clear export directory setting: {error}");
                let _ = view_tx
                    .send(ViewCommand::ShowError {
                        title: "Export Directory".to_string(),
                        message: "Failed to reset export directory preference".to_string(),
                        severity: ErrorSeverity::Warning,
                    })
                    .await;
                return;
            }
            let _ = view_tx
                .send(ViewCommand::ExportDirectoryLoaded {
                    path: String::new(),
                })
                .await;
            let _ = view_tx
                .send(ViewCommand::ShowNotification {
                    message: "Export directory reset to system Downloads".to_string(),
                })
                .await;
            return;
        }

        if let Err(reason) = validate_export_directory(&trimmed) {
            let _ = view_tx
                .send(ViewCommand::ShowError {
                    title: "Export Directory".to_string(),
                    message: reason,
                    severity: ErrorSeverity::Warning,
                })
                .await;
            return;
        }

        if let Err(error) = app_settings_service
            .set_setting(EXPORT_DIR_SETTING_KEY, trimmed.clone())
            .await
        {
            tracing::warn!("Failed to persist export directory setting: {error}");
            let _ = view_tx
                .send(ViewCommand::ShowError {
                    title: "Export Directory".to_string(),
                    message: "Failed to persist export directory preference".to_string(),
                    severity: ErrorSeverity::Warning,
                })
                .await;
            return;
        }

        let _ = view_tx
            .send(ViewCommand::ExportDirectoryLoaded {
                path: trimmed.clone(),
            })
            .await;
        let _ = view_tx
            .send(ViewCommand::ShowNotification {
                message: format!("Export directory set to {trimmed}"),
            })
            .await;
    }
}
