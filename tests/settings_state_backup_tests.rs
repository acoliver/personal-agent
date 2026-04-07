//! Settings State - Backup handling tests
//!
//! These tests verify the backup-related state transitions in `SettingsState`.
//!
//! @requirement REQ-BACKUP-001

use chrono::{TimeZone, Utc};
use personal_agent::backup::{BackupInfo, DatabaseBackupSettings};
use personal_agent::ui_gpui::views::settings_view::SettingsState;
use std::path::PathBuf;

/// Test: `SettingsState` default has no backup settings
#[test]
fn settings_state_default_has_no_backup_settings() {
    let state = SettingsState::new();
    assert!(
        state.backup_settings.is_none(),
        "Default state should have no backup settings"
    );
    assert!(
        state.backups.is_empty(),
        "Default state should have no backups"
    );
    assert!(
        state.last_backup_time.is_none(),
        "Default state should have no last backup time"
    );
    assert!(
        state.backup_status.is_none(),
        "Default state should have no backup status"
    );
    assert!(
        !state.backup_in_progress,
        "Default state should not have backup in progress"
    );
    assert!(
        state.selected_backup_id.is_none(),
        "Default state should have no selected backup"
    );
}

/// Test: `SettingsState` can store backup settings
#[test]
fn settings_state_can_store_backup_settings() {
    let mut state = SettingsState::new();
    let settings = DatabaseBackupSettings {
        enabled: true,
        interval_hours: 6,
        max_copies: 20,
        backup_directory: Some(PathBuf::from("/custom/backup")),
        ..DatabaseBackupSettings::default()
    };

    state.backup_settings = Some(settings);

    assert!(state.backup_settings.is_some());
    let stored = state.backup_settings.unwrap();
    assert!(stored.enabled);
    assert_eq!(stored.interval_hours, 6);
    assert_eq!(stored.max_copies, 20);
    assert_eq!(
        stored.backup_directory,
        Some(PathBuf::from("/custom/backup"))
    );
}

/// Test: `SettingsState` can store backup list
#[test]
fn settings_state_can_store_backup_list() {
    let mut state = SettingsState::new();

    let backup1 = BackupInfo::new(
        PathBuf::from("/backups/backup1.db.gz"),
        Utc.with_ymd_and_hms(2026, 4, 5, 10, 0, 0).unwrap(),
        1024 * 1024,
    );
    let backup2 = BackupInfo::new(
        PathBuf::from("/backups/backup2.db.gz"),
        Utc.with_ymd_and_hms(2026, 4, 4, 10, 0, 0).unwrap(),
        2048 * 1024,
    );

    state.backups = vec![backup1, backup2];

    assert_eq!(state.backups.len(), 2);
    assert_eq!(
        state.backups[0].path,
        PathBuf::from("/backups/backup1.db.gz")
    );
    assert_eq!(state.backups[1].size_bytes, 2048 * 1024);
}

/// Test: `SettingsState` tracks backup in progress
#[test]
fn settings_state_tracks_backup_in_progress() {
    let mut state = SettingsState::new();

    assert!(!state.backup_in_progress);

    state.backup_in_progress = true;
    assert!(state.backup_in_progress);

    state.backup_in_progress = false;
    assert!(!state.backup_in_progress);
}

/// Test: `SettingsState` stores last backup time
#[test]
fn settings_state_stores_last_backup_time() {
    let mut state = SettingsState::new();

    let now = Utc::now();
    state.last_backup_time = Some(now);

    assert!(state.last_backup_time.is_some());
    assert_eq!(state.last_backup_time.unwrap(), now);
}

/// Test: `SettingsState` stores backup status messages
#[test]
fn settings_state_stores_backup_status_messages() {
    let mut state = SettingsState::new();

    state.backup_status = Some("Backup completed successfully".to_string());
    assert_eq!(
        state.backup_status,
        Some("Backup completed successfully".to_string())
    );

    state.backup_status = Some("Backup failed: Disk full".to_string());
    assert_eq!(
        state.backup_status,
        Some("Backup failed: Disk full".to_string())
    );
}

/// Test: `SettingsState` tracks selected backup for restore
#[test]
fn settings_state_tracks_selected_backup() {
    let mut state = SettingsState::new();

    state.selected_backup_id = Some(0);
    assert_eq!(state.selected_backup_id, Some(0));

    state.selected_backup_id = Some(2);
    assert_eq!(state.selected_backup_id, Some(2));

    state.selected_backup_id = None;
    assert!(state.selected_backup_id.is_none());
}

/// Test: `SettingsState` with backup directory default
#[test]
fn settings_state_default_backup_directory_is_none() {
    let settings = DatabaseBackupSettings::default();
    assert!(
        settings.backup_directory.is_none(),
        "Default backup directory should be None (use default location)"
    );
}

/// Test: `SettingsState` enabled by default
#[test]
fn settings_state_backup_enabled_by_default() {
    let settings = DatabaseBackupSettings::default();
    assert!(settings.enabled, "Backups should be enabled by default");
    assert_eq!(
        settings.interval_hours, 12,
        "Default interval should be 12 hours"
    );
    assert_eq!(settings.max_copies, 10, "Default max copies should be 10");
}

/// Test: `SettingsState` validates backup settings
#[test]
fn settings_state_validates_backup_settings() {
    // Invalid interval (0)
    let settings = DatabaseBackupSettings {
        interval_hours: 0,
        ..DatabaseBackupSettings::default()
    };
    assert!(
        settings.validate().is_err(),
        "Zero interval should fail validation"
    );

    // Invalid max_copies (0)
    let settings = DatabaseBackupSettings {
        max_copies: 0,
        ..DatabaseBackupSettings::default()
    };
    assert!(
        settings.validate().is_err(),
        "Zero max_copies should fail validation"
    );

    // Valid settings
    let settings = DatabaseBackupSettings::default();
    assert!(
        settings.validate().is_ok(),
        "Default settings should validate"
    );
}
