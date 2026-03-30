use std::sync::Arc;

use tokio::sync::mpsc;

use super::conversation_export::{
    build_export_filename, render_export_content, resolve_export_directory,
    resolve_unique_export_path, write_export_file_retrying_collisions, EXPORT_DIR_SETTING_KEY,
    EXPORT_FORMAT_SETTING_KEY,
};
use super::view_command::ErrorSeverity;
use super::{ChatPresenter, ViewCommand};
use crate::models::ConversationExportFormat;
use crate::services::{AppSettingsService, ConversationService};

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
            .send(ViewCommand::ShowNotification {
                message: format!(
                    "Conversation saved as {} ({})",
                    path.display(),
                    format.display_label()
                ),
            })
            .await;
        let _ = view_tx
            .send(ViewCommand::ShowConversationExportFormat { format })
            .await;
    }
}
