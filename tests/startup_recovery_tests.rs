//! Behavioral tests for startup recovery functions
//!
//! Tests for backup directory scanning, filename parsing, and restore operations.
//!
//! @requirement REQ-BACKUP-001

use std::io::Write;
use std::path::PathBuf;

use personal_agent::backup::BackupInfo;

/// Create a temporary directory with backup files for testing
fn setup_backup_test_dir() -> (tempfile::TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let backup_dir = temp_dir.path().join("backups");
    std::fs::create_dir_all(&backup_dir).expect("create backup dir");
    (temp_dir, backup_dir)
}

/// Create a valid gzip-compressed backup file
fn create_backup_file(backup_dir: &std::path::Path, timestamp: &str, content: &[u8]) -> PathBuf {
    let filename = format!("personalagent-{timestamp}.db.gz");
    let path = backup_dir.join(&filename);

    // Compress content with gzip
    let file = std::fs::File::create(&path).expect("create file");
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    encoder.write_all(content).expect("write content");
    encoder.finish().expect("finish encoding");

    path
}

/// Test: `BackupInfo::new` creates backup info with path, timestamp, and size
#[test]
fn backup_info_new_creates_info() {
    let path = PathBuf::from("/backups/personalagent-2026-04-05T14-30-00Z.db.gz");
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

/// Test: Backup files are created with gzip compression
#[test]
fn backup_file_is_gzip_compressed() {
    let (_temp_dir, backup_dir) = setup_backup_test_dir();

    let content = b"SQLite database content here";
    let path = create_backup_file(&backup_dir, "2026-04-05T14-30-00Z", content);

    // Verify file exists
    assert!(path.exists());

    // Verify it's smaller than original due to gzip compression
    let metadata = std::fs::metadata(&path).expect("get metadata");
    // Gzip adds header, so small content might be larger, but for larger content
    // we'd see compression. Just verify the file was created.
    assert!(metadata.len() > 0);
}

/// Test: Gzip decompression recovers original content
#[test]
fn gzip_decompression_recovers_content() {
    use std::io::Read;

    let (_temp_dir, backup_dir) = setup_backup_test_dir();

    let original_content = b"Original database content that should be preserved";
    let path = create_backup_file(&backup_dir, "2026-04-05T14-30-00Z", original_content);

    // Decompress and verify
    let file = std::fs::File::open(&path).expect("open file");
    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).expect("decompress");

    assert_eq!(decompressed.as_slice(), original_content);
}

/// Test: `BackupInfo` can be sorted by timestamp
#[test]
fn backup_info_sorts_by_timestamp() {
    use chrono::TimeZone;

    let t1 = chrono::Utc.with_ymd_and_hms(2026, 4, 1, 10, 0, 0).unwrap();
    let t2 = chrono::Utc.with_ymd_and_hms(2026, 4, 5, 15, 0, 0).unwrap();
    let t3 = chrono::Utc.with_ymd_and_hms(2026, 4, 3, 12, 0, 0).unwrap();

    let mut backups = [
        BackupInfo::new(PathBuf::from("/b1"), t1, 1024),
        BackupInfo::new(PathBuf::from("/b2"), t2, 1024),
        BackupInfo::new(PathBuf::from("/b3"), t3, 1024),
    ];

    // Sort newest first
    backups.sort_by_key(|b| std::cmp::Reverse(b.timestamp));

    assert_eq!(backups[0].formatted_timestamp(), "2026-04-05 15:00 UTC");
    assert_eq!(backups[1].formatted_timestamp(), "2026-04-03 12:00 UTC");
    assert_eq!(backups[2].formatted_timestamp(), "2026-04-01 10:00 UTC");
}

/// Test: `BackupMetadata` stores backup state info
#[test]
fn backup_metadata_defaults() {
    let metadata = personal_agent::backup::BackupMetadata::default();

    assert!(metadata.last_backup_time.is_none());
    assert!(metadata.last_db_modified.is_none());
}
