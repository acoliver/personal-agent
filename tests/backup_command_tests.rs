//! Tests for backup-related actions and commands
//!
//! Tests the `backup_actions` module and `ViewCommand` handling.
//!
//! @requirement REQ-BACKUP-001

use chrono::Utc;
use personal_agent::backup::{BackupInfo, BackupResult, DatabaseBackupSettings, RestoreResult};
use personal_agent::presentation::ViewCommand;
use std::path::PathBuf;

/// Test: `BackupSettingsLoaded` command structure
#[test]
fn backup_settings_loaded_command() {
    let settings = DatabaseBackupSettings::default();
    let backups = vec![BackupInfo {
        path: PathBuf::from("/test/backup.db.gz"),
        timestamp: Utc::now(),
        size_bytes: 1024,
    }];
    let last_backup_time = Some(Utc::now());

    let cmd = ViewCommand::BackupSettingsLoaded {
        settings,
        backups,
        last_backup_time,
    };

    match cmd {
        ViewCommand::BackupSettingsLoaded {
            settings: s,
            backups: b,
            last_backup_time: t,
        } => {
            assert!(s.enabled);
            assert_eq!(b.len(), 1);
            assert!(t.is_some());
        }
        _ => panic!("Expected BackupSettingsLoaded"),
    }
}

/// Test: `BackupCompleted` command with success
#[test]
fn backup_completed_command_success() {
    let result = BackupResult::Success {
        path: PathBuf::from("/test/backup.db.gz"),
        duration_ms: 100,
    };

    let cmd = ViewCommand::BackupCompleted { result };

    match cmd {
        ViewCommand::BackupCompleted { result: r } => {
            assert!(r.is_success());
        }
        _ => panic!("Expected BackupCompleted"),
    }
}

/// Test: `BackupCompleted` command with skipped
#[test]
fn backup_completed_command_skipped() {
    let result = BackupResult::Skipped {
        reason: "No changes".to_string(),
    };

    let cmd = ViewCommand::BackupCompleted { result };

    match cmd {
        ViewCommand::BackupCompleted { result: r } => {
            assert!(!r.is_success());
            assert!(matches!(r, BackupResult::Skipped { .. }));
        }
        _ => panic!("Expected BackupCompleted"),
    }
}

/// Test: `BackupCompleted` command with failed
#[test]
fn backup_completed_command_failed() {
    let result = BackupResult::Failed {
        error: "Error".to_string(),
    };

    let cmd = ViewCommand::BackupCompleted { result };

    match cmd {
        ViewCommand::BackupCompleted { result: r } => {
            assert!(!r.is_success());
            assert!(matches!(r, BackupResult::Failed { .. }));
        }
        _ => panic!("Expected BackupCompleted"),
    }
}

/// Test: `BackupListRefreshed` command
#[test]
fn backup_list_refreshed_command() {
    let backups = vec![
        BackupInfo {
            path: PathBuf::from("/test/backup1.db.gz"),
            timestamp: Utc::now(),
            size_bytes: 1024,
        },
        BackupInfo {
            path: PathBuf::from("/test/backup2.db.gz"),
            timestamp: Utc::now() - chrono::Duration::hours(1),
            size_bytes: 2048,
        },
    ];

    let cmd = ViewCommand::BackupListRefreshed { backups };

    match cmd {
        ViewCommand::BackupListRefreshed { backups: b } => {
            assert_eq!(b.len(), 2);
        }
        _ => panic!("Expected BackupListRefreshed"),
    }
}

/// Test: `RestoreCompleted` command with success
#[test]
fn restore_completed_command_success() {
    let result = RestoreResult::Success;

    let cmd = ViewCommand::RestoreCompleted { result };

    match cmd {
        ViewCommand::RestoreCompleted { result: r } => {
            assert!(matches!(r, RestoreResult::Success));
        }
        _ => panic!("Expected RestoreCompleted"),
    }
}

/// Test: `RestoreCompleted` command with failure
#[test]
fn restore_completed_command_failed() {
    let result = RestoreResult::Failed {
        error: "Restore failed".to_string(),
    };

    let cmd = ViewCommand::RestoreCompleted { result };

    match cmd {
        ViewCommand::RestoreCompleted { result: r } => match r {
            RestoreResult::Failed { error } => assert_eq!(error, "Restore failed"),
            RestoreResult::Success => panic!("Expected Failed"),
        },
        _ => panic!("Expected RestoreCompleted"),
    }
}

/// Test: `BackupResult::message` for success
#[test]
fn backup_result_message_success() {
    let result = BackupResult::Success {
        path: PathBuf::from("/test/backup.db.gz"),
        duration_ms: 100,
    };
    let msg = result.message();
    assert!(msg.contains("Success") || msg.contains("success") || msg.contains("backup"));
}

/// Test: `BackupResult::message` for skipped
#[test]
fn backup_result_message_skipped() {
    let result = BackupResult::Skipped {
        reason: "No changes".to_string(),
    };
    let msg = result.message();
    assert!(msg.contains("Skipped") || msg.contains("skipped") || msg.contains("No changes"));
}

/// Test: `BackupResult::message` for failed
#[test]
fn backup_result_message_failed() {
    let result = BackupResult::Failed {
        error: "Error".to_string(),
    };
    let msg = result.message();
    assert!(msg.contains("Failed") || msg.contains("failed") || msg.contains("Error"));
}

/// Test: `RestoreResult` message
#[test]
fn restore_result_message() {
    let success = RestoreResult::Success;
    let failed = RestoreResult::Failed {
        error: "Error".to_string(),
    };

    let success_msg = success.message();
    let failed_msg = failed.message();

    assert!(success_msg.contains("Success") || success_msg.contains("success"));
    assert!(
        failed_msg.contains("Failed")
            || failed_msg.contains("failed")
            || failed_msg.contains("Error")
    );
}
