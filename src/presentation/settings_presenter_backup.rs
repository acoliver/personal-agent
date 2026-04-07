//! Backup handlers for `SettingsPresenter`.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::settings_presenter::SettingsPresenter;
use super::view_command::ViewCommand;
use crate::events::types::UserEvent;
use crate::services::BackupService;

impl SettingsPresenter {
    /// Handle backup-related user events
    pub async fn handle_backup_user_event(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::TriggerBackupNow => {
                Self::on_trigger_backup_now(backup_service, view_tx).await;
            }
            UserEvent::SetBackupDirectory { path } => {
                Self::on_set_backup_directory(backup_service, view_tx, path).await;
            }
            UserEvent::RestoreBackup { path } => {
                Self::on_restore_backup(backup_service, view_tx, path).await;
            }
            UserEvent::RefreshBackupList => {
                Self::on_refresh_backup_list(backup_service, view_tx).await;
            }
            UserEvent::SetBackupEnabled { enabled } => {
                Self::on_set_backup_enabled(backup_service, view_tx, enabled).await;
            }
            UserEvent::SetBackupIntervalHours { hours } => {
                Self::on_set_backup_interval_hours(backup_service, view_tx, hours).await;
            }
            UserEvent::SetBackupMaxCopies { copies } => {
                Self::on_set_backup_max_copies(backup_service, view_tx, copies).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Emit the initial backup settings snapshot
    pub async fn emit_backup_settings_snapshot(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        tracing::info!("emit_backup_settings_snapshot: starting");

        let settings = Self::get_backup_settings_or_default(backup_service).await;
        let backups = Self::list_backups_or_empty(backup_service).await;
        let last_backup_time = Self::get_last_backup_time_or_none(backup_service).await;

        tracing::info!(
            "emit_backup_settings_snapshot: sending BackupSettingsLoaded with {} backups",
            backups.len()
        );
        let _ = view_tx.send(ViewCommand::BackupSettingsLoaded {
            settings,
            backups,
            last_backup_time,
        });
    }

    async fn get_backup_settings_or_default(
        backup_service: &Arc<dyn BackupService>,
    ) -> crate::backup::DatabaseBackupSettings {
        match backup_service.get_settings().await {
            Ok(s) => {
                tracing::info!("emit_backup_settings_snapshot: got settings {:?}", s);
                s
            }
            Err(e) => {
                tracing::warn!("Failed to load backup settings: {}", e);
                crate::backup::DatabaseBackupSettings::default()
            }
        }
    }

    async fn list_backups_or_empty(
        backup_service: &Arc<dyn BackupService>,
    ) -> Vec<crate::backup::BackupInfo> {
        match backup_service.list_backups().await {
            Ok(b) => {
                tracing::info!("emit_backup_settings_snapshot: listed {} backups", b.len());
                for backup in &b {
                    tracing::info!("  backup: {:?}", backup);
                }
                b
            }
            Err(e) => {
                tracing::warn!("Failed to list backups: {}", e);
                Vec::new()
            }
        }
    }

    async fn get_last_backup_time_or_none(
        backup_service: &Arc<dyn BackupService>,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        match backup_service.get_last_backup_time().await {
            Ok(t) => {
                tracing::info!("emit_backup_settings_snapshot: last_backup_time = {:?}", t);
                t
            }
            Err(e) => {
                tracing::warn!("Failed to get last backup time: {}", e);
                None
            }
        }
    }

    /// Handle `TriggerBackupNow` user event
    async fn on_trigger_backup_now(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        tracing::info!("Manual backup triggered");

        match backup_service.create_backup().await {
            Ok(result) => {
                tracing::info!("Backup result: {:?}", result);
                let _ = view_tx.send(ViewCommand::BackupCompleted {
                    result: result.clone(),
                });

                // Refresh backup list after successful backup
                if result.is_success() {
                    Self::on_refresh_backup_list(backup_service, view_tx).await;
                }
            }
            Err(e) => {
                tracing::error!("Backup failed: {}", e);
                let _ = view_tx.send(ViewCommand::BackupCompleted {
                    result: crate::backup::BackupResult::Failed {
                        error: format!("Service error: {e}"),
                    },
                });
            }
        }
    }

    /// Handle `SetBackupDirectory` user event
    async fn on_set_backup_directory(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        path: Option<String>,
    ) {
        tracing::info!("Setting backup directory: {:?}", path);

        let error = Self::update_backup_setting(backup_service, |s| {
            s.backup_directory = path.clone().map(std::path::PathBuf::from);
        })
        .await;

        if let Some(e) = error {
            tracing::warn!("Failed to set backup directory: {}", e);
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Backup Settings".to_string(),
                message: e,
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_backup_settings_snapshot(backup_service, view_tx).await;
    }

    /// Handle `RestoreBackup` user event
    async fn on_restore_backup(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        path: String,
    ) {
        tracing::info!("Restore backup requested: {}", path);

        let backup_path = std::path::PathBuf::from(&path);

        match backup_service.restore_backup(&backup_path).await {
            Ok(result) => {
                tracing::info!("Restore result: {:?}", result);
                let _ = view_tx.send(ViewCommand::RestoreCompleted {
                    result: result.clone(),
                });

                if result.is_success() {
                    // Notify that database was restored so UI can refresh
                    let _ = view_tx.send(ViewCommand::DatabaseRestored);
                    let _ = view_tx.send(ViewCommand::ShowNotification {
                        message: "Database restored successfully".to_string(),
                    });
                }
            }
            Err(e) => {
                tracing::error!("Restore failed: {}", e);
                let _ = view_tx.send(ViewCommand::RestoreCompleted {
                    result: crate::backup::RestoreResult::Failed {
                        error: format!("Service error: {e}"),
                    },
                });
            }
        }
    }

    /// Handle `RefreshBackupList` user event
    async fn on_refresh_backup_list(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        tracing::debug!("Refreshing backup list");

        match backup_service.list_backups().await {
            Ok(backups) => {
                let _ = view_tx.send(ViewCommand::BackupListRefreshed { backups });
            }
            Err(e) => {
                tracing::warn!("Failed to refresh backup list: {}", e);
                let _ = view_tx.send(ViewCommand::BackupListRefreshed {
                    backups: Vec::new(),
                });
            }
        }
    }

    /// Handle `SetBackupEnabled` user event
    async fn on_set_backup_enabled(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        enabled: bool,
    ) {
        tracing::info!("Setting backup enabled: {}", enabled);

        let result = Self::update_backup_setting(backup_service, |s| s.enabled = enabled).await;
        if let Some(e) = result {
            Self::emit_settings_error(view_tx, e);
            return;
        }

        Self::emit_backup_settings_snapshot(backup_service, view_tx).await;
    }

    /// Handle `SetBackupIntervalHours` user event
    async fn on_set_backup_interval_hours(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        hours: u32,
    ) {
        tracing::info!("Setting backup interval: {} hours", hours);

        let result =
            Self::update_backup_setting(backup_service, |s| s.interval_hours = hours.max(1)).await;
        if let Some(e) = result {
            Self::emit_settings_error(view_tx, e);
            return;
        }

        Self::emit_backup_settings_snapshot(backup_service, view_tx).await;
    }

    /// Handle `SetBackupMaxCopies` user event
    async fn on_set_backup_max_copies(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        copies: u32,
    ) {
        tracing::info!("Setting backup max copies: {}", copies);

        let result = Self::update_backup_setting(backup_service, |s| {
            s.max_copies = copies.clamp(1, 100);
        })
        .await;
        if let Some(e) = result {
            Self::emit_settings_error(view_tx, e);
            return;
        }

        Self::emit_backup_settings_snapshot(backup_service, view_tx).await;
    }

    /// Helper to update a backup setting with error handling.
    /// Returns Some(error) if the update failed.
    async fn update_backup_setting<F>(
        backup_service: &Arc<dyn BackupService>,
        modifier: F,
    ) -> Option<String>
    where
        F: FnOnce(&mut crate::backup::DatabaseBackupSettings),
    {
        let mut settings = match backup_service.get_settings().await {
            Ok(s) => s,
            Err(e) => return Some(format!("Failed to load settings: {e}")),
        };

        modifier(&mut settings);

        if let Err(e) = backup_service.update_settings(settings).await {
            return Some(format!("Failed to save settings: {e}"));
        }

        None
    }

    fn emit_settings_error(view_tx: &broadcast::Sender<ViewCommand>, error: String) {
        let _ = view_tx.send(ViewCommand::ShowError {
            title: "Backup Settings".to_string(),
            message: error,
            severity: super::view_command::ErrorSeverity::Warning,
        });
    }
}
