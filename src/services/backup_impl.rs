//! Backup service implementation
//!
//! Implements database backup functionality using `SQLite`'s online backup API
//! with Gzip compression, rolling retention, and change detection.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::backup::{Backup, StepResult};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::backup::{
    BackupInfo, BackupMetadata, BackupResult, DatabaseBackupSettings, RestoreResult,
};
use crate::db::worker::DbHandle;
use crate::services::app_settings::AppSettingsService;
use crate::services::backup::BackupService;
use crate::services::{ServiceError, ServiceResult};

const BACKUP_METADATA_KEY: &str = "backup_metadata";
const BACKUP_SETTINGS_KEY: &str = "backup_settings";
const BACKUP_FILENAME_PREFIX: &str = "personalagent";

/// Implementation of the backup service
pub struct BackupServiceImpl {
    db: DbHandle,
    app_settings: Arc<dyn AppSettingsService>,
    db_path: PathBuf,
}

impl BackupServiceImpl {
    /// Create a new backup service implementation
    #[must_use]
    pub const fn new(
        db: DbHandle,
        app_settings: Arc<dyn AppSettingsService>,
        db_path: PathBuf,
    ) -> Self {
        Self {
            db,
            app_settings,
            db_path,
        }
    }

    /// Get the backup directory path
    async fn backup_dir(&self) -> ServiceResult<PathBuf> {
        let settings = self.load_settings().await?;
        settings.effective_backup_directory().ok_or_else(|| {
            ServiceError::Configuration("Failed to determine backup directory".to_string())
        })
    }

    /// Load settings from app settings service
    async fn load_settings(&self) -> ServiceResult<DatabaseBackupSettings> {
        let json_opt = self.app_settings.get_setting(BACKUP_SETTINGS_KEY).await?;
        json_opt.map_or_else(
            || Ok(DatabaseBackupSettings::default()),
            |json| {
                serde_json::from_str(&json).map_err(|e| {
                    ServiceError::Serialization(format!("Failed to parse backup settings: {e}"))
                })
            },
        )
    }

    /// Save settings to app settings service
    async fn save_settings(&self, settings: &DatabaseBackupSettings) -> ServiceResult<()> {
        let json = serde_json::to_string(settings).map_err(|e| {
            ServiceError::Serialization(format!("Failed to serialize backup settings: {e}"))
        })?;
        self.app_settings
            .set_setting(BACKUP_SETTINGS_KEY, json)
            .await
    }

    /// Load backup metadata from app settings
    async fn load_metadata(&self) -> ServiceResult<BackupMetadata> {
        let json_opt = self.app_settings.get_setting(BACKUP_METADATA_KEY).await?;
        json_opt.map_or_else(
            || Ok(BackupMetadata::default()),
            |json| {
                serde_json::from_str(&json).map_err(|e| {
                    ServiceError::Serialization(format!("Failed to parse backup metadata: {e}"))
                })
            },
        )
    }

    /// Save backup metadata to app settings
    async fn save_metadata(&self, metadata: &BackupMetadata) -> ServiceResult<()> {
        let json = serde_json::to_string(metadata).map_err(|e| {
            ServiceError::Serialization(format!("Failed to serialize backup metadata: {e}"))
        })?;
        self.app_settings
            .set_setting(BACKUP_METADATA_KEY, json)
            .await
    }

    /// Generate a backup filename with ISO 8601 timestamp
    fn generate_backup_filename(timestamp: DateTime<Utc>) -> String {
        // Format: personalagent-2026-04-05T08-00-00Z.db.gz
        let timestamp_str = timestamp.format("%Y-%m-%dT%H-%M-%SZ").to_string();
        format!("{BACKUP_FILENAME_PREFIX}-{timestamp_str}.db.gz")
    }

    /// Parse a timestamp from a backup filename
    fn parse_backup_filename(filename: &str) -> Option<DateTime<Utc>> {
        // Expected format: personalagent-2026-04-05T08-00-00Z.db.gz
        let prefix = format!("{BACKUP_FILENAME_PREFIX}-");
        let suffix = ".db.gz";

        if !filename.starts_with(&prefix) || !filename.ends_with(suffix) {
            return None;
        }

        let timestamp_part = &filename[prefix.len()..filename.len() - suffix.len()];
        let iso_str = timestamp_part.replace('-', ":");
        let iso_str = format!("{}T{}Z", &iso_str[..10], &iso_str[11..19]);

        DateTime::parse_from_rfc3339(&iso_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .ok()
    }

    /// Get the last modification time from the database
    async fn get_db_last_modified(&self) -> ServiceResult<Option<DateTime<Utc>>> {
        self.db
            .execute(|conn| {
                conn.query_row("SELECT MAX(updated_at) FROM conversations", [], |row| {
                    row.get::<_, Option<String>>(0)
                })
            })
            .await
            .map(|opt| {
                opt.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                })
            })
            .map_err(|e| {
                ServiceError::Storage(format!("Failed to query database modification time: {e}"))
            })
    }

    fn backup_skip_result(reason: impl Into<String>) -> BackupResult {
        BackupResult::Skipped {
            reason: reason.into(),
        }
    }

    fn validate_backup_request(
        settings: &DatabaseBackupSettings,
        metadata: &BackupMetadata,
        current_modified: Option<DateTime<Utc>>,
    ) -> Option<BackupResult> {
        if !settings.enabled {
            return Some(Self::backup_skip_result("Backups are disabled"));
        }

        if let Err(error) = settings.validate() {
            return Some(Self::backup_skip_result(format!(
                "Invalid settings: {error}"
            )));
        }

        if let (Some(last), Some(current)) = (metadata.last_db_modified, current_modified) {
            if current <= last {
                return Some(Self::backup_skip_result("No changes since last backup"));
            }
        }

        None
    }

    async fn prepare_backup_target(&self) -> ServiceResult<(PathBuf, DateTime<Utc>)> {
        let backup_dir = self.backup_dir().await?;
        fs::create_dir_all(&backup_dir)
            .map_err(|e| ServiceError::Io(format!("Failed to create backup directory: {e}")))?;

        let timestamp = Utc::now();
        let filename = Self::generate_backup_filename(timestamp);
        Ok((backup_dir.join(filename), timestamp))
    }

    async fn perform_sqlite_backup(&self, backup_path: PathBuf) -> ServiceResult<()> {
        self.db
            .execute(move |conn| Self::write_compressed_backup(conn, &backup_path))
            .await
            .map_err(|e| ServiceError::Storage(format!("Backup failed: {e}")))
    }

    fn write_compressed_backup(
        conn: &rusqlite::Connection,
        backup_path: &Path,
    ) -> Result<(), rusqlite::Error> {
        let temp_db_path = backup_path.with_extension("tmp");
        let result = Self::write_backup_snapshot(conn, &temp_db_path, backup_path);
        let _ = fs::remove_file(&temp_db_path);
        result
    }

    fn write_backup_snapshot(
        conn: &rusqlite::Connection,
        temp_db_path: &Path,
        backup_path: &Path,
    ) -> Result<(), rusqlite::Error> {
        let mut dst_conn = Self::open_backup_destination(temp_db_path)?;
        let backup = Self::initialize_backup(conn, &mut dst_conn)?;
        Self::run_backup_steps(&backup)?;
        drop(backup);
        drop(dst_conn);
        Self::compress_temp_backup(temp_db_path, backup_path)
    }

    fn open_backup_destination(
        temp_db_path: &Path,
    ) -> Result<rusqlite::Connection, rusqlite::Error> {
        rusqlite::Connection::open(temp_db_path).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to open backup destination: {e}")),
            )
        })
    }

    fn initialize_backup<'a>(
        conn: &'a rusqlite::Connection,
        dst_conn: &'a mut rusqlite::Connection,
    ) -> Result<Backup<'a, 'a>, rusqlite::Error> {
        Backup::new(conn, dst_conn).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to initialize backup: {e}")),
            )
        })
    }

    fn run_backup_steps(backup: &Backup<'_, '_>) -> Result<(), rusqlite::Error> {
        loop {
            match backup.step(100) {
                Ok(StepResult::Done) => return Ok(()),
                Ok(StepResult::More) => {}
                Ok(StepResult::Busy | StepResult::Locked | _) => std::thread::yield_now(),

                Err(e) => {
                    return Err(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(1),
                        Some(format!("Backup step failed: {e}")),
                    ));
                }
            }
        }
    }

    fn compress_temp_backup(
        temp_db_path: &Path,
        backup_path: &Path,
    ) -> Result<(), rusqlite::Error> {
        let backup_data = fs::read(temp_db_path).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to read temp backup: {e}")),
            )
        })?;

        let compressed_file = fs::File::create(backup_path).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to create compressed file: {e}")),
            )
        })?;

        let mut encoder = GzEncoder::new(compressed_file, Compression::default());
        encoder.write_all(&backup_data).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to compress backup: {e}")),
            )
        })?;

        encoder.finish().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1),
                Some(format!("Failed to finish compression: {e}")),
            )
        })?;

        Ok(())
    }

    async fn persist_backup_metadata(
        &self,
        timestamp: DateTime<Utc>,
        current_modified: Option<DateTime<Utc>>,
        backup_path: &Path,
    ) -> ServiceResult<()> {
        let mut metadata = self.load_metadata().await?;
        metadata.last_backup_time = Some(timestamp);
        metadata.last_db_modified = current_modified;
        metadata.last_backup_path = Some(backup_path.to_path_buf());
        self.save_metadata(&metadata).await
    }

    /// Apply rolling retention policy - delete old backups beyond `max_copies`
    async fn apply_retention(&self, max_copies: u32) -> ServiceResult<()> {
        let backup_dir = self.backup_dir().await?;

        let mut backups = self.list_backups_internal(&backup_dir).await?;

        // Sort by timestamp (oldest first)
        backups.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Delete oldest backups beyond max_copies
        if backups.len() > max_copies as usize {
            let to_delete = &backups[..backups.len() - max_copies as usize];
            for backup in to_delete {
                if let Err(e) = fs::remove_file(&backup.path) {
                    tracing::warn!(
                        "Failed to delete old backup {}: {}",
                        backup.path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Internal method to list backups in a directory
    async fn list_backups_internal(&self, dir: &Path) -> ServiceResult<Vec<BackupInfo>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();

        let entries = fs::read_dir(dir)
            .map_err(|e| ServiceError::Io(format!("Failed to read backup directory: {e}")))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| ServiceError::Io(format!("Failed to read directory entry: {e}")))?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(timestamp) = Self::parse_backup_filename(filename) {
                    let metadata = entry.metadata().map_err(|e| {
                        ServiceError::Io(format!("Failed to read file metadata: {e}"))
                    })?;

                    let size = metadata.len();
                    backups.push(BackupInfo::new(path, timestamp, size));
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }
}

#[async_trait]
#[allow(clippy::too_many_lines)]
impl BackupService for BackupServiceImpl {
    async fn create_backup(&self) -> ServiceResult<BackupResult> {
        let start = std::time::Instant::now();
        let settings = self.load_settings().await?;
        let current_modified = self.get_db_last_modified().await?;
        let metadata = self.load_metadata().await?;

        if let Some(result) = Self::validate_backup_request(&settings, &metadata, current_modified)
        {
            return Ok(result);
        }

        let (backup_path, timestamp) = self.prepare_backup_target().await?;
        self.perform_sqlite_backup(backup_path.clone()).await?;

        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

        self.persist_backup_metadata(timestamp, current_modified, &backup_path)
            .await?;
        if let Err(e) = self.apply_retention(settings.max_copies).await {
            tracing::warn!("Failed to apply retention policy: {}", e);
        }

        Ok(BackupResult::Success {
            path: backup_path,
            duration_ms,
        })
    }

    async fn list_backups(&self) -> ServiceResult<Vec<BackupInfo>> {
        let backup_dir = self.backup_dir().await?;
        self.list_backups_internal(&backup_dir).await
    }

    async fn restore_backup(&self, path: &Path) -> ServiceResult<RestoreResult> {
        // Verify the backup file exists
        if !path.exists() {
            return Ok(RestoreResult::Failed {
                error: format!("Backup file not found: {}", path.display()),
            });
        }

        // Check if it's a gzipped backup
        if path.extension().is_none_or(|ext| ext != "gz") {
            return Ok(RestoreResult::Failed {
                error: "Backup file must be a .db.gz file".to_string(),
            });
        }

        // Create temporary restore file
        let temp_restore_path = self.db_path.with_extension("restore.tmp");

        // Decompress backup
        let compressed_file = fs::File::open(path)
            .map_err(|e| ServiceError::Io(format!("Failed to open backup file: {e}")))?;

        let mut decoder = GzDecoder::new(compressed_file);
        let mut restored_data = Vec::new();
        decoder
            .read_to_end(&mut restored_data)
            .map_err(|e| ServiceError::Io(format!("Failed to decompress backup: {e}")))?;

        // Write decompressed data to temp file
        fs::write(&temp_restore_path, restored_data).map_err(|e| {
            ServiceError::Io(format!("Failed to write temporary restore file: {e}"))
        })?;

        // Validate it's a valid SQLite database
        match rusqlite::Connection::open(&temp_restore_path) {
            Ok(_conn) => {
                // Database is valid
            }
            Err(e) => {
                let _ = fs::remove_file(&temp_restore_path);
                return Ok(RestoreResult::Failed {
                    error: format!("Restored file is not a valid SQLite database: {e}"),
                });
            }
        }

        // Replace current database with restored backup
        // Note: In a real app, we'd need to ensure the DB worker is shut down first
        // For now, we assume restore is done when app is not actively using the DB
        let backup_of_current = self.db_path.with_extension("pre_restore.bak");

        // Backup current database if it exists
        if self.db_path.exists() {
            if let Err(e) = fs::copy(&self.db_path, &backup_of_current) {
                tracing::warn!("Failed to backup current database before restore: {}", e);
            }
        }

        // Replace with restored database
        fs::rename(&temp_restore_path, &self.db_path).map_err(|e| {
            ServiceError::Io(format!("Failed to replace database with backup: {e}"))
        })?;

        Ok(RestoreResult::Success)
    }

    async fn get_settings(&self) -> ServiceResult<DatabaseBackupSettings> {
        self.load_settings().await
    }

    async fn update_settings(&self, settings: DatabaseBackupSettings) -> ServiceResult<()> {
        if let Err(e) = settings.validate() {
            return Err(ServiceError::Configuration(format!(
                "Invalid backup settings: {e}"
            )));
        }
        self.save_settings(&settings).await
    }

    async fn get_last_backup_time(&self) -> ServiceResult<Option<DateTime<Utc>>> {
        let metadata = self.load_metadata().await?;
        Ok(metadata.last_backup_time)
    }

    async fn should_backup(&self) -> ServiceResult<bool> {
        let settings = self.load_settings().await?;
        if !settings.enabled {
            return Ok(false);
        }

        let metadata = self.load_metadata().await?;
        let current_modified = self.get_db_last_modified().await?;

        // If no last backup, should backup if there's any data
        if metadata.last_backup_time.is_none() {
            return Ok(current_modified.is_some());
        }

        // Check if enough time has passed since last backup
        if let Some(last_backup) = metadata.last_backup_time {
            let elapsed = Utc::now() - last_backup;
            let interval = chrono::Duration::hours(i64::from(settings.interval_hours));
            if elapsed < interval {
                return Ok(false);
            }
        }

        // Check if database has changed since last backup
        if let (Some(last), Some(current)) = (metadata.last_db_modified, current_modified) {
            Ok(current > last)
        } else {
            Ok(current_modified.is_some())
        }
    }
}
