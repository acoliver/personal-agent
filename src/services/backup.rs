//! Backup service trait
//!
//! Defines the interface for database backup operations including
//! creating backups, restoring, managing settings, and checking backup status.

use std::path::Path;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::backup::{BackupInfo, BackupResult, DatabaseBackupSettings, RestoreResult};
use crate::services::ServiceResult;

/// Backup service interface for database backup operations
#[async_trait]
pub trait BackupService: Send + Sync {
    /// Create a backup of the current database
    ///
    /// Uses `SQLite`'s online backup API with compression.
    /// Returns `BackupResult::Skipped` if no changes have occurred since last backup.
    async fn create_backup(&self) -> ServiceResult<BackupResult>;

    /// List all available backups
    ///
    /// Returns metadata about each backup file including path, timestamp, and size.
    async fn list_backups(&self) -> ServiceResult<Vec<BackupInfo>>;

    /// Restore the database from a backup file
    ///
    /// # Arguments
    /// * `path` - Path to the backup file to restore from
    ///
    /// # Errors
    /// Returns an error if the backup file is not found, is corrupted,
    /// or if the restore operation fails.
    async fn restore_backup(&self, path: &Path) -> ServiceResult<RestoreResult>;

    /// Get the current backup settings
    async fn get_settings(&self) -> ServiceResult<DatabaseBackupSettings>;

    /// Update the backup settings
    ///
    /// # Arguments
    /// * `settings` - New backup settings to save
    ///
    /// # Errors
    /// Returns an error if settings validation fails or persistence fails.
    async fn update_settings(&self, settings: DatabaseBackupSettings) -> ServiceResult<()>;

    /// Get the timestamp of the last successful backup
    ///
    /// Returns `None` if no backup has ever been created.
    async fn get_last_backup_time(&self) -> ServiceResult<Option<DateTime<Utc>>>;

    /// Check if a backup is needed based on settings and time elapsed
    ///
    /// Considers the backup interval, whether backups are enabled,
    /// and whether the database has been modified since the last backup.
    async fn should_backup(&self) -> ServiceResult<bool>;
}
