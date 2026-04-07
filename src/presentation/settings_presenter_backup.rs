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

        // Get current settings
        let settings = match backup_service.get_settings().await {
            Ok(s) => {
                tracing::info!("emit_backup_settings_snapshot: got settings {:?}", s);
                s
            }
            Err(e) => {
                tracing::warn!("Failed to load backup settings: {}", e);
                crate::backup::DatabaseBackupSettings::default()
            }
        };

        // Get list of backups
        let backups = match backup_service.list_backups().await {
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
        };

        // Get last backup time
        let last_backup_time = match backup_service.get_last_backup_time().await {
            Ok(t) => {
                tracing::info!("emit_backup_settings_snapshot: last_backup_time = {:?}", t);
                t
            }
            Err(e) => {
                tracing::warn!("Failed to get last backup time: {}", e);
                None
            }
        };

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

        // Get current settings first
        let mut settings = match backup_service.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to load backup settings: {}", e);
                crate::backup::DatabaseBackupSettings::default()
            }
        };

        // Update the backup directory
        settings.backup_directory = path.map(std::path::PathBuf::from);

        // Validate and save
        if let Err(e) = settings.validate() {
            tracing::warn!("Invalid backup settings: {}", e);
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Backup Settings".to_string(),
                message: format!("Invalid settings: {e}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        if let Err(e) = backup_service.update_settings(settings.clone()).await {
            tracing::warn!("Failed to update backup settings: {}", e);
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Backup Settings".to_string(),
                message: format!("Failed to save settings: {e}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        // Refresh the backup settings display
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

        let mut settings = match backup_service.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to load backup settings: {}", e);
                crate::backup::DatabaseBackupSettings::default()
            }
        };

        settings.enabled = enabled;

        if let Err(e) = backup_service.update_settings(settings).await {
            tracing::warn!("Failed to update backup settings: {}", e);
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Backup Settings".to_string(),
                message: format!("Failed to save settings: {e}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
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

        let mut settings = match backup_service.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to load backup settings: {}", e);
                crate::backup::DatabaseBackupSettings::default()
            }
        };

        settings.interval_hours = hours.max(1); // Ensure at least 1 hour

        if let Err(e) = backup_service.update_settings(settings).await {
            tracing::warn!("Failed to update backup settings: {}", e);
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Backup Settings".to_string(),
                message: format!("Failed to save settings: {e}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
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

        let mut settings = match backup_service.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to load backup settings: {}", e);
                crate::backup::DatabaseBackupSettings::default()
            }
        };

        settings.max_copies = copies.clamp(1, 100); // Clamp between 1 and 100

        if let Err(e) = backup_service.update_settings(settings).await {
            tracing::warn!("Failed to update backup settings: {}", e);
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Backup Settings".to_string(),
                message: format!("Failed to save settings: {e}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        Self::emit_backup_settings_snapshot(backup_service, view_tx).await;
    }
}
