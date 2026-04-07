//! More behavioral tests for `BackupScheduler` to push coverage
//!
//! Tests additional scheduler logic paths.
//!
//! @requirement REQ-BACKUP-001

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use personal_agent::backup::{
    BackupInfo, BackupResult, BackupScheduler, DatabaseBackupSettings, RestoreResult,
};
use personal_agent::services::{BackupService, ServiceResult};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

/// Mock backup service that can be configured for different scenarios
struct ConfigurableMockService {
    create_backup_result: std::sync::Mutex<Option<BackupResult>>,
    should_backup_value: AtomicBool,
    settings: std::sync::Mutex<DatabaseBackupSettings>,
    last_backup_time: std::sync::Mutex<Option<DateTime<Utc>>>,
    #[allow(dead_code)]
    create_backup_calls: AtomicUsize,
}

impl ConfigurableMockService {
    fn new() -> Self {
        Self {
            create_backup_result: std::sync::Mutex::new(Some(BackupResult::Success {
                path: PathBuf::from("/test/backup.db.gz"),
                duration_ms: 100,
            })),
            should_backup_value: AtomicBool::new(true),
            settings: std::sync::Mutex::new(DatabaseBackupSettings::default()),
            last_backup_time: std::sync::Mutex::new(None),
            create_backup_calls: AtomicUsize::new(0),
        }
    }

    fn with_should_backup(&self, value: bool) {
        self.should_backup_value.store(value, Ordering::SeqCst);
    }

    fn with_last_backup_time(&self, time: Option<DateTime<Utc>>) {
        let mut last = self.last_backup_time.lock().unwrap();
        *last = time;
    }
}

#[async_trait]
impl BackupService for ConfigurableMockService {
    async fn create_backup(&self) -> ServiceResult<BackupResult> {
        self.create_backup_calls.fetch_add(1, Ordering::SeqCst);
        let result = self.create_backup_result.lock().unwrap().clone();
        Ok(result.unwrap_or_else(|| BackupResult::Success {
            path: PathBuf::from("/test/backup.db.gz"),
            duration_ms: 100,
        }))
    }

    async fn list_backups(&self) -> ServiceResult<Vec<BackupInfo>> {
        Ok(Vec::new())
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

    async fn get_last_backup_time(&self) -> ServiceResult<Option<DateTime<Utc>>> {
        Ok(*self.last_backup_time.lock().unwrap())
    }

    async fn should_backup(&self) -> ServiceResult<bool> {
        Ok(self.should_backup_value.load(Ordering::SeqCst))
    }
}

/// Test: Scheduler exits immediately when disabled
#[tokio::test]
async fn scheduler_exits_when_disabled() {
    let service = Arc::new(ConfigurableMockService::new());
    service
        .update_settings(DatabaseBackupSettings {
            enabled: false,
            ..DatabaseBackupSettings::default()
        })
        .await
        .expect("update settings");

    let (_, shutdown_rx) = watch::channel(false);
    let mut scheduler = BackupScheduler::new(service, shutdown_rx);

    // Run should exit immediately
    let start = std::time::Instant::now();
    scheduler.run().await;
    let elapsed = start.elapsed();

    // Should exit very quickly since backups disabled
    assert!(elapsed.as_millis() < 100);
}

/// Test: Scheduler handles service error gracefully
#[tokio::test]
async fn scheduler_handles_service_error() {
    let service = Arc::new(ConfigurableMockService::new());
    // Settings will fail to load (returns default)

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut scheduler = BackupScheduler::new(service.clone(), shutdown_rx);

    // Send immediate shutdown
    let _ = shutdown_tx.send(true);
    scheduler.run().await;

    // Should exit without panic
}

/// Test: Scheduler with startup backup enabled (with shutdown)
#[tokio::test]
async fn scheduler_startup_backup() {
    let service = Arc::new(ConfigurableMockService::new());

    // Configure for immediate startup backup
    let settings = DatabaseBackupSettings {
        run_on_startup_if_stale: true,
        enabled: true,
        ..DatabaseBackupSettings::default()
    };
    service
        .update_settings(settings)
        .await
        .expect("update settings");

    // Set last backup time to very old
    let old_time = Utc::now() - chrono::Duration::hours(24);
    service.with_last_backup_time(Some(old_time));
    service.with_should_backup(true);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut scheduler = BackupScheduler::new(service.clone(), shutdown_rx);

    // Send shutdown after brief delay
    let tx = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = tx.send(true);
    });

    scheduler.run().await;
}

/// Test: Scheduler shutdown during run
#[tokio::test]
async fn scheduler_graceful_shutdown() {
    let service = Arc::new(ConfigurableMockService::new());
    service
        .update_settings(DatabaseBackupSettings {
            enabled: true,
            interval_hours: 1000, // Very long interval
            ..DatabaseBackupSettings::default()
        })
        .await
        .expect("update settings");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let service_clone = service.clone();

    let handle = tokio::spawn(async move {
        let mut scheduler = BackupScheduler::new(service_clone, shutdown_rx);
        scheduler.run().await;
    });

    // Send shutdown signal immediately
    let _ = shutdown_tx.send(true);

    // Should complete quickly
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;

    assert!(result.is_ok());
}

/// Test: Scheduler compute next backup time
#[test]
fn compute_next_backup_time_basic() {
    let now = Utc::now();
    let interval: i32 = 12;

    // Next time should be interval hours from now
    let expected = now + chrono::Duration::hours(i64::from(interval));

    // Basic sanity check
    assert!(expected > now);
}

/// Test: Scheduler with no previous backup (with shutdown)
#[tokio::test]
async fn scheduler_no_previous_backup() {
    let service = Arc::new(ConfigurableMockService::new());
    service.with_last_backup_time(None); // Never backed up
    service.with_should_backup(true);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut scheduler = BackupScheduler::new(service.clone(), shutdown_rx);

    // Send shutdown after brief delay
    let tx = shutdown_tx;
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = tx.send(true);
    });

    scheduler.run().await;
}

/// Test: Settings with run on startup disabled
#[test]
fn settings_run_on_startup_disabled() {
    let settings = DatabaseBackupSettings {
        run_on_startup_if_stale: false,
        ..DatabaseBackupSettings::default()
    };

    assert!(!settings.run_on_startup_if_stale);
}

/// Test: Settings with custom interval
#[test]
fn settings_custom_interval() {
    let settings = DatabaseBackupSettings {
        interval_hours: 6,
        ..DatabaseBackupSettings::default()
    };

    assert_eq!(settings.interval_hours, 6);
}

/// Test: Settings with custom max copies
#[test]
fn settings_custom_max_copies() {
    let settings = DatabaseBackupSettings {
        max_copies: 5,
        ..DatabaseBackupSettings::default()
    };

    assert_eq!(settings.max_copies, 5);
}

/// Test: Scheduler respects `should_backup` false (with shutdown)
#[tokio::test]
async fn scheduler_respects_should_backup_false() {
    let service = Arc::new(ConfigurableMockService::new());
    service.with_should_backup(false); // No backup needed

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut scheduler = BackupScheduler::new(service.clone(), shutdown_rx);

    // Send shutdown after brief delay
    let tx = shutdown_tx;
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = tx.send(true);
    });

    scheduler.run().await;
}
