//! Behavioral tests for startup recovery functions
//!
//! These tests verify the database health check and backup recovery flow.
//!
//! @requirement REQ-BACKUP-001

use std::path::PathBuf;

use personal_agent::backup::BackupInfo;

/// Test: `BackupInfo::new` creates backup info
#[test]
fn backup_info_new_creates_info() {
    let path = PathBuf::from("/backup.db.gz");
    let timestamp = chrono::Utc::now();
    let size = 1024;

    let info = BackupInfo::new(path.clone(), timestamp, size);

    assert_eq!(info.path, path);
    assert_eq!(info.size_bytes, size);
}

/// Test: `BackupInfo::formatted_size` formats bytes correctly
#[test]
fn backup_info_formatted_size_bytes() {
    let info = BackupInfo::new(PathBuf::from("/backup"), chrono::Utc::now(), 512);
    assert_eq!(info.formatted_size(), "512.00 B");
}

/// Test: `BackupInfo::formatted_size` formats KB correctly
#[test]
fn backup_info_formatted_size_kb() {
    let info = BackupInfo::new(PathBuf::from("/backup"), chrono::Utc::now(), 1024);
    assert_eq!(info.formatted_size(), "1.00 KB");

    let info = BackupInfo::new(PathBuf::from("/backup"), chrono::Utc::now(), 1536);
    assert_eq!(info.formatted_size(), "1.50 KB");
}

/// Test: `BackupInfo::formatted_size` formats MB correctly
#[test]
fn backup_info_formatted_size_mb() {
    let info = BackupInfo::new(PathBuf::from("/backup"), chrono::Utc::now(), 1024 * 1024);
    assert_eq!(info.formatted_size(), "1.00 MB");

    let info = BackupInfo::new(
        PathBuf::from("/backup"),
        chrono::Utc::now(),
        1024 * 1024 * 5,
    );
    assert_eq!(info.formatted_size(), "5.00 MB");
}

/// Test: `BackupInfo::formatted_size` formats GB correctly
#[test]
fn backup_info_formatted_size_gb() {
    let info = BackupInfo::new(
        PathBuf::from("/backup"),
        chrono::Utc::now(),
        1024 * 1024 * 1024,
    );
    assert_eq!(info.formatted_size(), "1.00 GB");
}

/// Test: `BackupInfo::formatted_timestamp` produces UTC format
#[test]
fn backup_info_formatted_timestamp() {
    use chrono::TimeZone;

    let timestamp = chrono::Utc.with_ymd_and_hms(2026, 4, 5, 14, 30, 0).unwrap();
    let info = BackupInfo::new(PathBuf::from("/backup"), timestamp, 1024);

    assert_eq!(info.formatted_timestamp(), "2026-04-05 14:30 UTC");
}

/// Test: `DatabaseBackupSettings::default` has correct values
#[test]
fn backup_settings_default_values() {
    let settings = personal_agent::backup::DatabaseBackupSettings::default();

    assert!(settings.enabled, "Backups should be enabled by default");
    assert_eq!(settings.interval_hours, 12);
    assert_eq!(settings.max_copies, 10);
    assert!(
        settings.backup_directory.is_none(),
        "Default directory should be None (use default location)"
    );
    assert!(
        settings.run_on_startup_if_stale,
        "Should run on startup if stale by default"
    );
}

/// Test: `DatabaseBackupSettings::validate` rejects zero interval
#[test]
fn backup_settings_validate_zero_interval() {
    let settings = personal_agent::backup::DatabaseBackupSettings {
        interval_hours: 0,
        ..Default::default()
    };

    assert!(settings.validate().is_err());
}

/// Test: `DatabaseBackupSettings::validate` rejects zero `max_copies`
#[test]
fn backup_settings_validate_zero_max_copies() {
    let settings = personal_agent::backup::DatabaseBackupSettings {
        max_copies: 0,
        ..Default::default()
    };

    assert!(settings.validate().is_err());
}

/// Test: `DatabaseBackupSettings::validate` accepts valid settings
#[test]
fn backup_settings_validate_accepts_valid() {
    let settings = personal_agent::backup::DatabaseBackupSettings {
        enabled: true,
        interval_hours: 6,
        max_copies: 20,
        backup_directory: Some(PathBuf::from("/custom")),
        run_on_startup_if_stale: true,
    };

    assert!(settings.validate().is_ok());
}

/// Test: `BackupResult::Success` `is_success`
#[test]
fn backup_result_success_is_success() {
    let result = personal_agent::backup::BackupResult::Success {
        path: PathBuf::from("/backup"),
        duration_ms: 100,
    };
    assert!(result.is_success());
}

/// Test: `BackupResult::Skipped` is not `is_success`
#[test]
fn backup_result_skipped_not_success() {
    let result = personal_agent::backup::BackupResult::Skipped {
        reason: "No changes".to_string(),
    };
    assert!(!result.is_success());
}

/// Test: `BackupResult::Failed` is not `is_success`
#[test]
fn backup_result_failed_not_success() {
    let result = personal_agent::backup::BackupResult::Failed {
        error: "Error".to_string(),
    };
    assert!(!result.is_success());
}

/// Test: `RestoreResult::Success` `is_success`
#[test]
fn restore_result_success_is_success() {
    let result = personal_agent::backup::RestoreResult::Success;
    assert!(result.is_success());
}

/// Test: `RestoreResult::Failed` is not `is_success`
#[test]
fn restore_result_failed_not_success() {
    let result = personal_agent::backup::RestoreResult::Failed {
        error: "Error".to_string(),
    };
    assert!(!result.is_success());
}
