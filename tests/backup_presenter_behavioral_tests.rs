//! Behavioral tests for backup settings presenter handlers
//!
//! These tests verify that backup-related `UserEvents` are properly handled
//! by the `SettingsPresenter`, producing the expected `ViewCommands`.
//!
//! @requirement REQ-BACKUP-001

use std::sync::Arc;

use async_trait::async_trait;
use personal_agent::backup::{BackupInfo, BackupResult, DatabaseBackupSettings, RestoreResult};
use personal_agent::events::types::UserEvent;
use personal_agent::presentation::settings_presenter::SettingsPresenter;
use personal_agent::presentation::view_command::ViewCommand;
use personal_agent::services::{BackupService, ServiceResult};
use std::path::{Path, PathBuf};
use tokio::sync::broadcast;

/// Mock backup service that returns configurable responses
struct MockBackupService {
    settings: std::sync::Mutex<DatabaseBackupSettings>,
    create_backup_result: std::sync::Mutex<BackupResult>,
    backups: std::sync::Mutex<Vec<BackupInfo>>,
    last_backup_time: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>,
}

impl MockBackupService {
    fn new() -> Self {
        Self {
            settings: std::sync::Mutex::new(DatabaseBackupSettings::default()),
            create_backup_result: std::sync::Mutex::new(BackupResult::Success {
                path: PathBuf::from("/test/backup.db.gz"),
                duration_ms: 100,
            }),
            backups: std::sync::Mutex::new(Vec::new()),
            last_backup_time: std::sync::Mutex::new(None),
        }
    }

    fn set_create_backup_result(&self, result: BackupResult) {
        *self.create_backup_result.lock().unwrap() = result;
    }

    fn set_settings(&self, settings: DatabaseBackupSettings) {
        *self.settings.lock().unwrap() = settings;
    }

    fn set_backups(&self, backups: Vec<BackupInfo>) {
        *self.backups.lock().unwrap() = backups;
    }

    fn set_last_backup_time(&self, time: Option<chrono::DateTime<chrono::Utc>>) {
        *self.last_backup_time.lock().unwrap() = time;
    }
}

#[async_trait]
impl BackupService for MockBackupService {
    async fn create_backup(&self) -> ServiceResult<BackupResult> {
        Ok(self.create_backup_result.lock().unwrap().clone())
    }

    async fn list_backups(&self) -> ServiceResult<Vec<BackupInfo>> {
        Ok(self.backups.lock().unwrap().clone())
    }

    async fn restore_backup(&self, _path: &Path) -> ServiceResult<RestoreResult> {
        Ok(RestoreResult::Success)
    }

    async fn get_settings(&self) -> ServiceResult<DatabaseBackupSettings> {
        Ok(self.settings.lock().unwrap().clone())
    }

    async fn update_settings(&self, settings: DatabaseBackupSettings) -> ServiceResult<()> {
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }

    async fn get_last_backup_time(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(*self.last_backup_time.lock().unwrap())
    }

    async fn should_backup(&self) -> ServiceResult<bool> {
        Ok(true)
    }
}

/// Test: `TriggerBackupNow` emits `BackupCompleted` and refreshes list
///
/// Behavior: When the user triggers a backup, the presenter emits a
/// `BackupCompleted` command and then refreshes the backup list.
#[tokio::test]
async fn trigger_backup_now_emits_completed_and_refreshes() {
    let mock: Arc<dyn BackupService> = Arc::new(MockBackupService::new());
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    // Call the handler directly
    SettingsPresenter::handle_backup_user_event(&mock, &tx, UserEvent::TriggerBackupNow).await;

    // Should receive BackupCompleted
    let cmd1 = rx.recv().await.expect("receive command 1");
    assert!(
        matches!(cmd1, ViewCommand::BackupCompleted { .. }),
        "Expected BackupCompleted, got {cmd1:?}"
    );

    // Should receive BackupListRefreshed
    let cmd2 = rx.recv().await.expect("receive command 2");
    assert!(
        matches!(cmd2, ViewCommand::BackupListRefreshed { .. }),
        "Expected BackupListRefreshed, got {cmd2:?}"
    );
}

/// Test: `SetBackupEnabled` updates settings and emits snapshot
///
/// Behavior: When backup is enabled/disabled, the setting is persisted
/// and a snapshot is emitted with the updated state.
#[tokio::test]
async fn set_backup_enabled_updates_and_emits() {
    let mock: Arc<dyn BackupService> = Arc::new(MockBackupService::new());
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::handle_backup_user_event(
        &mock,
        &tx,
        UserEvent::SetBackupEnabled { enabled: false },
    )
    .await;

    // Should receive BackupSettingsLoaded with updated settings
    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupSettingsLoaded { settings, .. } => {
            assert!(!settings.enabled, "Backup should be disabled");
        }
        _ => panic!("Expected BackupSettingsLoaded, got {cmd:?}"),
    }
}

/// Test: `SetBackupIntervalHours` clamps to minimum 1 hour
///
/// Behavior: Setting interval to 0 should be clamped to 1.
#[tokio::test]
async fn set_backup_interval_hours_clamps_to_minimum() {
    let mock: Arc<dyn BackupService> = Arc::new(MockBackupService::new());
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::handle_backup_user_event(
        &mock,
        &tx,
        UserEvent::SetBackupIntervalHours { hours: 0 },
    )
    .await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupSettingsLoaded { settings, .. } => {
            assert_eq!(settings.interval_hours, 1, "Should be clamped to 1 hour");
        }
        _ => panic!("Expected BackupSettingsLoaded, got {cmd:?}"),
    }
}

/// Test: `SetBackupMaxCopies` clamps between 1 and 100
///
/// Behavior: Setting max copies outside the valid range should be clamped.
#[tokio::test]
async fn set_backup_max_copies_clamps_to_range() {
    let mock: Arc<dyn BackupService> = Arc::new(MockBackupService::new());
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    // Test upper bound
    SettingsPresenter::handle_backup_user_event(
        &mock,
        &tx,
        UserEvent::SetBackupMaxCopies { copies: 200 },
    )
    .await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupSettingsLoaded { settings, .. } => {
            assert_eq!(settings.max_copies, 100, "Should be clamped to 100");
        }
        _ => panic!("Expected BackupSettingsLoaded, got {cmd:?}"),
    }
}

/// Test: `RefreshBackupList` emits `BackupListRefreshed`
///
/// Behavior: When backup list is refreshed, the presenter emits the list.
#[tokio::test]
async fn refresh_backup_list_emits_list() {
    let mock_svc = MockBackupService::new();

    // Create test backups
    let backup1 = BackupInfo::new(PathBuf::from("/backup1.db.gz"), chrono::Utc::now(), 1024);
    let backup2 = BackupInfo::new(
        PathBuf::from("/backup2.db.gz"),
        chrono::Utc::now() - chrono::Duration::hours(1),
        2048,
    );
    mock_svc.set_backups(vec![backup1, backup2]);

    let mock: Arc<dyn BackupService> = Arc::new(mock_svc);
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::handle_backup_user_event(&mock, &tx, UserEvent::RefreshBackupList).await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupListRefreshed { backups } => {
            assert_eq!(backups.len(), 2, "Should have 2 backups");
        }
        _ => panic!("Expected BackupListRefreshed, got {cmd:?}"),
    }
}

/// Test: `RestoreBackup` emits `RestoreCompleted` with success
///
/// Behavior: When restore succeeds, `RestoreCompleted` is emitted.
#[tokio::test]
async fn restore_backup_emits_completed_on_success() {
    let mock: Arc<dyn BackupService> = Arc::new(MockBackupService::new());
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::handle_backup_user_event(
        &mock,
        &tx,
        UserEvent::RestoreBackup {
            path: "/test/backup.db.gz".to_string(),
        },
    )
    .await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::RestoreCompleted { result } => {
            assert!(result.is_success(), "Restore should succeed");
        }
        _ => panic!("Expected RestoreCompleted, got {cmd:?}"),
    }
}

/// Test: `SetBackupDirectory` validates and saves
///
/// Behavior: When setting backup directory, the setting is validated and saved.
#[tokio::test]
async fn set_backup_directory_updates_settings() {
    let mock: Arc<dyn BackupService> = Arc::new(MockBackupService::new());
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::handle_backup_user_event(
        &mock,
        &tx,
        UserEvent::SetBackupDirectory {
            path: Some("/custom/backup/dir".to_string()),
        },
    )
    .await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupSettingsLoaded { settings, .. } => {
            assert_eq!(
                settings.backup_directory,
                Some(PathBuf::from("/custom/backup/dir")),
                "Directory should be updated"
            );
        }
        _ => panic!("Expected BackupSettingsLoaded, got {cmd:?}"),
    }
}

/// Test: `emit_backup_settings_snapshot` emits complete state
///
/// Behavior: The snapshot includes settings, backups, and last backup time.
#[tokio::test]
async fn emit_backup_settings_snapshot_includes_all_state() {
    let mock_svc = MockBackupService::new();

    // Configure mock
    let settings = DatabaseBackupSettings {
        enabled: true,
        interval_hours: 6,
        max_copies: 10,
        ..DatabaseBackupSettings::default()
    };
    mock_svc.set_settings(settings.clone());
    mock_svc.set_last_backup_time(Some(chrono::Utc::now()));

    let backup = BackupInfo::new(PathBuf::from("/backup.db.gz"), chrono::Utc::now(), 1024);
    mock_svc.set_backups(vec![backup]);

    let mock: Arc<dyn BackupService> = Arc::new(mock_svc);
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::emit_backup_settings_snapshot(&mock, &tx).await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupSettingsLoaded {
            settings: loaded_settings,
            backups,
            last_backup_time,
        } => {
            assert_eq!(loaded_settings.interval_hours, 6);
            assert_eq!(backups.len(), 1);
            assert!(last_backup_time.is_some());
        }
        _ => panic!("Expected BackupSettingsLoaded, got {cmd:?}"),
    }
}

/// Test: Backup failure still emits `BackupCompleted`
///
/// Behavior: When backup fails, `BackupCompleted` is emitted with the failure.
#[tokio::test]
async fn backup_failure_emits_failed_result() {
    let mock_svc = MockBackupService::new();
    mock_svc.set_create_backup_result(BackupResult::Failed {
        error: "Disk full".to_string(),
    });

    let mock: Arc<dyn BackupService> = Arc::new(mock_svc);
    let (tx, mut rx) = broadcast::channel::<ViewCommand>(16);

    SettingsPresenter::handle_backup_user_event(&mock, &tx, UserEvent::TriggerBackupNow).await;

    let cmd = rx.recv().await.expect("receive command");
    match cmd {
        ViewCommand::BackupCompleted { result } => {
            assert!(matches!(result, BackupResult::Failed { .. }));
        }
        _ => panic!("Expected BackupCompleted, got {cmd:?}"),
    }
}
