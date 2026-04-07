//! Additional behavioral tests for `SettingsPresenter` backup handlers
//!
//! Tests error handling and edge cases in the presenter handlers.
//!
//! @requirement REQ-BACKUP-001

use async_trait::async_trait;
use personal_agent::backup::{BackupInfo, BackupResult, DatabaseBackupSettings, RestoreResult};
use personal_agent::events::types::UserEvent;
use personal_agent::presentation::ViewCommand;
use personal_agent::services::{BackupService, ServiceError, ServiceResult};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Mock backup service that simulates errors
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
struct FailingBackupService {
    fail_create: bool,
    fail_list: bool,
    fail_restore: bool,
    fail_get_settings: bool,
    fail_update_settings: bool,
}

impl FailingBackupService {
    const fn new() -> Self {
        Self {
            fail_create: false,
            fail_list: false,
            fail_restore: false,
            fail_get_settings: false,
            fail_update_settings: false,
        }
    }

    const fn with_fail_create(mut self) -> Self {
        self.fail_create = true;
        self
    }

    const fn with_fail_restore(mut self) -> Self {
        self.fail_restore = true;
        self
    }

    const fn with_fail_list(mut self) -> Self {
        self.fail_list = true;
        self
    }

    const fn with_fail_get_settings(mut self) -> Self {
        self.fail_get_settings = true;
        self
    }
}

#[async_trait]
impl BackupService for FailingBackupService {
    async fn create_backup(&self) -> ServiceResult<BackupResult> {
        if self.fail_create {
            Err(ServiceError::Storage("backup failed".into()))
        } else {
            Ok(BackupResult::Success {
                path: std::path::PathBuf::from("/backup.db.gz"),
                duration_ms: 100,
            })
        }
    }

    async fn list_backups(&self) -> ServiceResult<Vec<BackupInfo>> {
        if self.fail_list {
            Err(ServiceError::Storage("list failed".into()))
        } else {
            Ok(Vec::new())
        }
    }

    async fn restore_backup(&self, _path: &std::path::Path) -> ServiceResult<RestoreResult> {
        if self.fail_restore {
            Err(ServiceError::Storage("restore failed".into()))
        } else {
            Ok(RestoreResult::Success)
        }
    }

    async fn get_settings(&self) -> ServiceResult<DatabaseBackupSettings> {
        if self.fail_get_settings {
            Err(ServiceError::Storage("get settings failed".into()))
        } else {
            Ok(DatabaseBackupSettings::default())
        }
    }

    async fn update_settings(&self, _settings: DatabaseBackupSettings) -> ServiceResult<()> {
        if self.fail_update_settings {
            Err(ServiceError::Storage("update settings failed".into()))
        } else {
            Ok(())
        }
    }

    async fn get_last_backup_time(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(None)
    }

    async fn should_backup(&self) -> ServiceResult<bool> {
        Ok(false)
    }
}

/// Test: `handle_backup_user_event` handles `TriggerBackupNow`
#[tokio::test]
async fn handle_backup_user_event_trigger_backup_now() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::TriggerBackupNow,
    )
    .await;

    // Should receive BackupCompleted command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupCompleted { .. }));

    // The result should be success
    if let ViewCommand::BackupCompleted { result } = cmd {
        assert!(result.is_success());
    }
}

/// Test: `handle_backup_user_event` handles `create_backup` error
#[tokio::test]
async fn handle_backup_user_event_trigger_backup_now_error() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new().with_fail_create());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::TriggerBackupNow,
    )
    .await;

    // Should receive BackupCompleted with Failed result
    let cmd = rx.recv().await.expect("receive command");
    if let ViewCommand::BackupCompleted { result } = cmd {
        assert!(!result.is_success());
    } else {
        panic!("Expected BackupCompleted command");
    }
}

/// Test: `handle_backup_user_event` handles `RestoreBackup`
#[tokio::test]
async fn handle_backup_user_event_restore_backup() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::RestoreBackup {
            path: "/backup.db.gz".to_string(),
        },
    )
    .await;

    // Should receive RestoreCompleted command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::RestoreCompleted { .. }));

    // Should receive DatabaseRestored for success
    let cmd2 = rx.recv().await.expect("receive database restored");
    assert!(matches!(cmd2, ViewCommand::DatabaseRestored));

    // Should receive ShowNotification for success
    let cmd3 = rx.recv().await.expect("receive notification");
    assert!(matches!(cmd3, ViewCommand::ShowNotification { .. }));
}

/// Test: `handle_backup_user_event` handles restore error
#[tokio::test]
async fn handle_backup_user_event_restore_backup_error() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new().with_fail_restore());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::RestoreBackup {
            path: "/backup.db.gz".to_string(),
        },
    )
    .await;

    // Should receive RestoreCompleted with Failed result
    let cmd = rx.recv().await.expect("receive command");
    if let ViewCommand::RestoreCompleted { result } = cmd {
        assert!(!result.is_success());
    } else {
        panic!("Expected RestoreCompleted command");
    }
}

/// Test: `handle_backup_user_event` handles `RefreshBackupList`
#[tokio::test]
async fn handle_backup_user_event_refresh_backup_list() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::RefreshBackupList,
    )
    .await;

    // Should receive BackupListRefreshed command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupListRefreshed { .. }));
}

/// Test: `handle_backup_user_event` handles `RefreshBackupList` error
#[tokio::test]
async fn handle_backup_user_event_refresh_backup_list_error() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new().with_fail_list());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::RefreshBackupList,
    )
    .await;

    // Should receive BackupListRefreshed with empty list
    let cmd = rx.recv().await.expect("receive command");
    if let ViewCommand::BackupListRefreshed { backups } = cmd {
        assert!(backups.is_empty());
    } else {
        panic!("Expected BackupListRefreshed command");
    }
}

/// Test: `handle_backup_user_event` handles `SetBackupEnabled`
#[tokio::test]
async fn handle_backup_user_event_set_backup_enabled() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::SetBackupEnabled { enabled: false },
    )
    .await;

    // Should receive BackupSettingsLoaded command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupSettingsLoaded { .. }));
}

/// Test: `handle_backup_user_event` handles `SetBackupIntervalHours`
#[tokio::test]
async fn handle_backup_user_event_set_backup_interval_hours() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::SetBackupIntervalHours { hours: 24 },
    )
    .await;

    // Should receive BackupSettingsLoaded command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupSettingsLoaded { .. }));
}

/// Test: `handle_backup_user_event` handles `SetBackupMaxCopies`
#[tokio::test]
async fn handle_backup_user_event_set_backup_max_copies() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::SetBackupMaxCopies { copies: 50 },
    )
    .await;

    // Should receive BackupSettingsLoaded command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupSettingsLoaded { .. }));
}

/// Test: `handle_backup_user_event` handles `SetBackupDirectory`
#[tokio::test]
async fn handle_backup_user_event_set_backup_directory() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::handle_backup_user_event(
        &service,
        &tx,
        UserEvent::SetBackupDirectory {
            path: Some("/custom/backup".to_string()),
        },
    )
    .await;

    // Should receive BackupSettingsLoaded command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupSettingsLoaded { .. }));
}

/// Test: `emit_backup_settings_snapshot` sends correct commands
#[tokio::test]
async fn emit_backup_settings_snapshot_sends_commands() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(FailingBackupService::new());

    personal_agent::presentation::SettingsPresenter::emit_backup_settings_snapshot(&service, &tx)
        .await;

    // Should receive BackupSettingsLoaded command
    let cmd = rx.recv().await.expect("receive command");
    assert!(matches!(cmd, ViewCommand::BackupSettingsLoaded { .. }));
}

/// Test: `emit_backup_settings_snapshot` handles errors gracefully
#[tokio::test]
async fn emit_backup_settings_snapshot_handles_errors() {
    let (tx, mut rx) = broadcast::channel(16);
    let service: Arc<dyn BackupService> = Arc::new(
        FailingBackupService::new()
            .with_fail_list()
            .with_fail_get_settings(),
    );

    personal_agent::presentation::SettingsPresenter::emit_backup_settings_snapshot(&service, &tx)
        .await;

    // Should still receive BackupSettingsLoaded with defaults
    let cmd = rx.recv().await.expect("receive command");
    if let ViewCommand::BackupSettingsLoaded {
        settings,
        backups,
        last_backup_time,
    } = cmd
    {
        // Should have default settings when get_settings fails
        assert_eq!(settings.interval_hours, 12);
        assert!(backups.is_empty());
        assert!(last_backup_time.is_none());
    } else {
        panic!("Expected BackupSettingsLoaded command");
    }
}
