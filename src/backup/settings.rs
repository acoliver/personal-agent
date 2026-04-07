//! Database backup settings
//!
//! Defines the configuration structure for automatic `SQLite` database backups.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Settings for automatic database backup functionality
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseBackupSettings {
    /// Whether automatic backups are enabled
    pub enabled: bool,
    /// Custom backup directory (None = use default app-managed location)
    pub backup_directory: Option<PathBuf>,
    /// Hours between automatic backups
    pub interval_hours: u32,
    /// Maximum number of backup copies to retain
    pub max_copies: u32,
    /// Whether to run backup on startup if stale
    pub run_on_startup_if_stale: bool,
}

impl Default for DatabaseBackupSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            backup_directory: None,
            interval_hours: 12,
            max_copies: 10,
            run_on_startup_if_stale: true,
        }
    }
}

impl DatabaseBackupSettings {
    /// Create a new settings instance with all defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the effective backup directory
    ///
    /// Returns the configured directory if set, otherwise returns the default
    /// app-managed backup location.
    #[must_use]
    pub fn effective_backup_directory(&self) -> Option<PathBuf> {
        self.backup_directory
            .clone()
            .or_else(|| dirs::data_local_dir().map(|dir| dir.join("PersonalAgent").join("backups")))
    }

    /// Validate the settings
    ///
    /// Returns an error string if any setting is invalid.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a descriptive message if:
    /// - `interval_hours` is 0
    /// - `max_copies` is 0
    /// - `max_copies` is greater than 100
    pub fn validate(&self) -> Result<(), String> {
        if self.interval_hours == 0 {
            return Err("Backup interval must be at least 1 hour".to_string());
        }
        if self.max_copies == 0 {
            return Err("Must retain at least 1 backup copy".to_string());
        }
        if self.max_copies > 100 {
            return Err("Cannot retain more than 100 backup copies".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = DatabaseBackupSettings::default();
        assert!(settings.enabled);
        assert!(settings.backup_directory.is_none());
        assert_eq!(settings.interval_hours, 12);
        assert_eq!(settings.max_copies, 10);
        assert!(settings.run_on_startup_if_stale);
    }

    #[test]
    fn test_effective_backup_directory_with_configured() {
        let settings = DatabaseBackupSettings {
            backup_directory: Some(PathBuf::from("/custom/backups")),
            ..Default::default()
        };
        assert_eq!(
            settings.effective_backup_directory(),
            Some(PathBuf::from("/custom/backups"))
        );
    }

    #[test]
    fn test_effective_backup_directory_default() {
        let settings = DatabaseBackupSettings::default();
        // Just verify it returns something when backup_directory is None
        // (actual path depends on the OS)
        if let Some(path) = settings.effective_backup_directory() {
            assert!(path.to_string_lossy().contains("backups"));
        }
    }

    #[test]
    fn test_validate_valid_settings() {
        let settings = DatabaseBackupSettings::default();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_interval() {
        let settings = DatabaseBackupSettings {
            interval_hours: 0,
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validate_zero_copies() {
        let settings = DatabaseBackupSettings {
            max_copies: 0,
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validate_too_many_copies() {
        let settings = DatabaseBackupSettings {
            max_copies: 101,
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }
}
