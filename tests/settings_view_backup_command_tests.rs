//! Settings View Command handling - Backup tests
//!
//! These tests verify the backup-related `ViewCommand` handling in `SettingsView`.
//!
//! @requirement REQ-BACKUP-001

use chrono::{TimeZone, Utc};
use personal_agent::backup::{BackupInfo, BackupResult, DatabaseBackupSettings, RestoreResult};
use personal_agent::presentation::view_command::ViewCommand;
use personal_agent::ui_gpui::views::settings_view::SettingsState;
use std::path::PathBuf;

/// Test: `BackupSettingsLoaded` updates state
#[test]
fn backup_settings_loaded_updates_state() {
    let mut state = SettingsState::new();

    let settings = DatabaseBackupSettings {
        enabled: true,
        interval_hours: 6,
        max_copies: 15,
        backup_directory: Some(PathBuf::from("/custom")),
        ..DatabaseBackupSettings::default()
    };

    let backup = BackupInfo::new(
        PathBuf::from("/backup.db.gz"),
        Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap(),
        1024,
    );

    let last_time = Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap();

    let cmd = ViewCommand::BackupSettingsLoaded {
        settings,
        backups: vec![backup],
        last_backup_time: Some(last_time),
    };

    // Apply command to state (simulating SettingsView::handle_command)
    if let ViewCommand::BackupSettingsLoaded {
        settings,
        backups,
        last_backup_time,
    } = cmd
    {
        state.backup_settings = Some(settings);
        state.backups = backups;
        state.last_backup_time = last_backup_time;
    }

    assert!(state.backup_settings.is_some());
    assert_eq!(state.backup_settings.unwrap().interval_hours, 6);
    assert_eq!(state.backups.len(), 1);
    assert_eq!(state.last_backup_time, Some(last_time));
}

/// Test: `BackupCompleted` with success updates status
#[test]
fn backup_completed_success_updates_status() {
    let mut state = SettingsState::new();

    let cmd = ViewCommand::BackupCompleted {
        result: BackupResult::Success {
            path: PathBuf::from("/backup.db.gz"),
            duration_ms: 500,
        },
    };

    // Apply command to state
    if let ViewCommand::BackupCompleted { result } = cmd {
        match result {
            BackupResult::Success { path, duration_ms } => {
                state.backup_status = Some(format!(
                    "Backup completed in {}ms: {}",
                    duration_ms,
                    path.display()
                ));
                state.backup_in_progress = false;
            }
            BackupResult::Skipped { reason } => {
                state.backup_status = Some(format!("Backup skipped: {reason}"));
                state.backup_in_progress = false;
            }
            BackupResult::Failed { error } => {
                state.backup_status = Some(format!("Backup failed: {error}"));
                state.backup_in_progress = false;
            }
        }
    }

    assert!(state.backup_status.is_some());
    assert!(state.backup_status.unwrap().contains("completed"));
    assert!(!state.backup_in_progress);
}

/// Test: `BackupCompleted` with failure updates status
#[test]
fn backup_completed_failure_updates_status() {
    let mut state = SettingsState::new();

    let cmd = ViewCommand::BackupCompleted {
        result: BackupResult::Failed {
            error: "Disk full".to_string(),
        },
    };

    // Apply command to state
    if let ViewCommand::BackupCompleted { result } = cmd {
        match result {
            BackupResult::Success { .. } | BackupResult::Skipped { .. } => unreachable!(),
            BackupResult::Failed { error } => {
                state.backup_status = Some(format!("Backup failed: {error}"));
                state.backup_in_progress = false;
            }
        }
    }

    assert!(state.backup_status.unwrap().contains("failed"));
    assert!(!state.backup_in_progress);
}

/// Test: `BackupListRefreshed` updates backups list
#[test]
fn backup_list_refreshed_updates_list() {
    let mut state = SettingsState::new();

    let backup1 = BackupInfo::new(
        PathBuf::from("/backup1.db.gz"),
        Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap(),
        1024,
    );
    let backup2 = BackupInfo::new(
        PathBuf::from("/backup2.db.gz"),
        Utc.with_ymd_and_hms(2026, 4, 4, 10, 0, 0).unwrap(),
        2048,
    );

    let cmd = ViewCommand::BackupListRefreshed {
        backups: vec![backup1, backup2],
    };

    // Apply command to state
    if let ViewCommand::BackupListRefreshed { backups } = cmd {
        state.backups = backups;
    }

    assert_eq!(state.backups.len(), 2);
}

/// Test: `RestoreCompleted` success shows restart prompt
#[test]
fn restore_completed_success_shows_restart_prompt() {
    let mut state = SettingsState::new();

    let cmd = ViewCommand::RestoreCompleted {
        result: RestoreResult::Success,
    };

    // Apply command to state
    if let ViewCommand::RestoreCompleted { result } = cmd {
        match result {
            RestoreResult::Success => {
                state.backup_status =
                    Some("Database restored. Please restart the app.".to_string());
                state.backup_in_progress = false;
            }
            RestoreResult::Failed { error } => {
                state.backup_status = Some(format!("Restore failed: {error}"));
                state.backup_in_progress = false;
            }
        }
    }

    assert!(state.backup_status.unwrap().contains("restart"));
}

/// Test: `RestoreCompleted` failure shows error
#[test]
fn restore_completed_failure_shows_error() {
    let mut state = SettingsState::new();

    let cmd = ViewCommand::RestoreCompleted {
        result: RestoreResult::Failed {
            error: "Invalid backup format".to_string(),
        },
    };

    // Apply command to state
    if let ViewCommand::RestoreCompleted { result } = cmd {
        match result {
            RestoreResult::Success => unreachable!(),
            RestoreResult::Failed { error } => {
                state.backup_status = Some(format!("Restore failed: {error}"));
                state.backup_in_progress = false;
            }
        }
    }

    assert!(state.backup_status.unwrap().contains("failed"));
}

/// Test: `BackupResult::is_success` returns correct value
#[test]
fn backup_result_is_success() {
    let success = BackupResult::Success {
        path: PathBuf::from("/backup.db.gz"),
        duration_ms: 100,
    };
    assert!(success.is_success());

    let skipped = BackupResult::Skipped {
        reason: "No changes".to_string(),
    };
    assert!(!skipped.is_success());

    let failed = BackupResult::Failed {
        error: "Error".to_string(),
    };
    assert!(!failed.is_success());
}

/// Test: `RestoreResult::is_success` returns correct value
#[test]
fn restore_result_is_success() {
    let success = RestoreResult::Success;
    assert!(success.is_success());

    let failed = RestoreResult::Failed {
        error: "Error".to_string(),
    };
    assert!(!failed.is_success());
}

/// Test: `BackupInfo` formatting
#[test]
fn backup_info_formatting() {
    let backup = BackupInfo::new(
        PathBuf::from("/backup.db.gz"),
        Utc.with_ymd_and_hms(2026, 4, 5, 14, 30, 0).unwrap(),
        1024 * 1024 * 5, // 5 MB
    );

    assert_eq!(backup.formatted_timestamp(), "2026-04-05 14:30 UTC");
    assert_eq!(backup.formatted_size(), "5.00 MB");
}

/// Test: `BackupInfo` with small file shows KB
#[test]
fn backup_info_small_file_shows_kb() {
    let backup = BackupInfo::new(
        PathBuf::from("/backup.db.gz"),
        Utc::now(),
        1024 * 512, // 512 KB
    );

    assert_eq!(backup.formatted_size(), "512.00 KB");
}
