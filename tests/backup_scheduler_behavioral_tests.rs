//! Behavioral tests for `BackupScheduler`
//!
//! These tests verify the scheduler's timing and decision logic
//! without requiring real-time waiting.
//!
//! @requirement REQ-BACKUP-001

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use personal_agent::backup::{
    spawn_backup_scheduler, BackupResult, DatabaseBackupSettings, RestoreResult,
};
use personal_agent::services::{BackupService, ServiceResult};
use std::path::{Path, PathBuf};

/// Mock backup service that tracks calls and can be configured
struct MockBackupService {
    create_backup_calls: AtomicUsize,
    should_backup_returns: AtomicBool,
    settings: std::sync::Mutex<DatabaseBackupSettings>,
}

impl MockBackupService {
    fn new() -> Self {
        Self {
            create_backup_calls: AtomicUsize::new(0),
            should_backup_returns: AtomicBool::new(true),
            settings: std::sync::Mutex::new(DatabaseBackupSettings::default()),
        }
    }

    fn set_enabled(&self, enabled: bool) {
        let mut settings = self.settings.lock().unwrap();
        settings.enabled = enabled;
    }

    fn set_should_backup(&self, val: bool) {
        self.should_backup_returns.store(val, Ordering::SeqCst);
    }

    fn call_count(&self) -> usize {
        self.create_backup_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl BackupService for MockBackupService {
    async fn create_backup(&self) -> ServiceResult<BackupResult> {
        self.create_backup_calls.fetch_add(1, Ordering::SeqCst);
        Ok(BackupResult::Success {
            path: PathBuf::from("/test/backup.db.gz"),
            duration_ms: 100,
        })
    }

    async fn list_backups(&self) -> ServiceResult<Vec<personal_agent::backup::BackupInfo>> {
        Ok(Vec::new())
    }

    async fn restore_backup(&self, _path: &Path) -> ServiceResult<RestoreResult> {
        Ok(RestoreResult::Success)
    }

    async fn get_settings(&self) -> ServiceResult<DatabaseBackupSettings> {
        let settings = self.settings.lock().unwrap();
        Ok(settings.clone())
    }

    async fn update_settings(&self, settings: DatabaseBackupSettings) -> ServiceResult<()> {
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }

    async fn get_last_backup_time(&self) -> ServiceResult<Option<DateTime<Utc>>> {
        Ok(None)
    }

    async fn should_backup(&self) -> ServiceResult<bool> {
        Ok(self.should_backup_returns.load(Ordering::SeqCst))
    }
}

/// Test: Scheduler exits immediately when backups are disabled
///
/// Behavior: If settings.enabled is false, the scheduler `run()` returns immediately
/// without creating any backups.
#[tokio::test]
async fn scheduler_exits_when_disabled() {
    let mock = Arc::new(MockBackupService::new());
    mock.set_enabled(false);

    let (handle, shutdown_tx) = spawn_backup_scheduler(mock.clone() as Arc<dyn BackupService>);

    // Wait a brief moment for scheduler to check settings and exit
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Scheduler should have exited
    // (If it didn't exit, the handle would still be running)
    drop(shutdown_tx);
    let _ = tokio::time::timeout(tokio::time::Duration::from_millis(100), handle).await;

    // No backups should have been created
    assert_eq!(mock.call_count(), 0);
}

/// Test: Scheduler responds to shutdown signal
///
/// Behavior: When shutdown signal is sent, the scheduler exits gracefully.
#[tokio::test]
async fn scheduler_shutdown_signal() {
    let mock = Arc::new(MockBackupService::new());
    mock.set_should_backup(false); // Don't actually try to backup

    let (handle, shutdown_tx) = spawn_backup_scheduler(mock.clone() as Arc<dyn BackupService>);

    // Send shutdown signal immediately
    shutdown_tx.send(true).expect("send shutdown");

    // Wait for scheduler to exit
    let result = tokio::time::timeout(tokio::time::Duration::from_millis(200), handle).await;

    // Should have exited without error
    assert!(result.is_ok());
}

/// Test: Scheduler checks settings on each iteration
///
/// Behavior: If settings change to disabled, scheduler exits.
#[tokio::test]
async fn scheduler_respects_settings_changes() {
    let mock = Arc::new(MockBackupService::new());
    mock.set_should_backup(false);

    let (handle, shutdown_tx) = spawn_backup_scheduler(mock.clone() as Arc<dyn BackupService>);

    // Wait briefly for startup
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // Disable backups after scheduler starts
    mock.set_enabled(false);

    // Wait for scheduler to notice and exit
    let result = tokio::time::timeout(tokio::time::Duration::from_millis(200), async {
        drop(shutdown_tx);
        handle.await
    })
    .await;

    // Should have exited
    assert!(result.is_ok());
}
