//! Database backup types
//!
//! Defines types for backup metadata, results, and related structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Information about a single backup file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupInfo {
    /// Full path to the backup file
    pub path: PathBuf,
    /// Timestamp when the backup was created
    pub timestamp: DateTime<Utc>,
    /// Size of the backup file in bytes
    pub size_bytes: u64,
}

impl BackupInfo {
    /// Create a new backup info instance
    #[must_use]
    pub const fn new(path: PathBuf, timestamp: DateTime<Utc>, size_bytes: u64) -> Self {
        Self {
            path,
            timestamp,
            size_bytes,
        }
    }

    /// Format the timestamp for display
    #[must_use]
    pub fn formatted_timestamp(&self) -> String {
        self.timestamp.format("%Y-%m-%d %H:%M UTC").to_string()
    }

    /// Format the size for display (human-readable)
    #[must_use]
    pub fn formatted_size(&self) -> String {
        format_size(self.size_bytes)
    }
}

/// Result of a backup operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackupResult {
    /// Backup was created successfully
    Success {
        /// Path to the created backup file
        path: PathBuf,
        /// Duration of the backup operation in milliseconds
        duration_ms: u64,
    },
    /// Backup was skipped (no changes since last backup)
    Skipped {
        /// Reason for skipping
        reason: String,
    },
    /// Backup failed
    Failed {
        /// Error message
        error: String,
    },
}

impl BackupResult {
    /// Check if the backup was successful
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Check if the backup was skipped
    #[must_use]
    pub const fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped { .. })
    }

    /// Check if the backup failed
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Get a display message for the result
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Success { path, duration_ms } => {
                format!(
                    "Backup created: {} (took {}ms)",
                    path.display(),
                    duration_ms
                )
            }
            Self::Skipped { reason } => format!("Backup skipped: {reason}"),
            Self::Failed { error } => format!("Backup failed: {error}"),
        }
    }
}

/// Result of a restore operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RestoreResult {
    /// Restore completed successfully
    Success,
    /// Restore failed
    Failed {
        /// Error message
        error: String,
    },
}

impl RestoreResult {
    /// Check if restore was successful
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Check if restore failed
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Get a display message for the result
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Success => "Restore completed successfully".to_string(),
            Self::Failed { error } => format!("Restore failed: {error}"),
        }
    }
}

/// Metadata stored for each successful backup
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupMetadata {
    /// Timestamp of the last successful backup
    pub last_backup_time: Option<DateTime<Utc>>,
    /// Timestamp of the database's last modification (change marker)
    pub last_db_modified: Option<DateTime<Utc>>,
    /// Path to the last created backup
    pub last_backup_path: Option<PathBuf>,
}

/// Format bytes to human-readable string
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let exp = (bytes.ilog2() / 10).min(UNITS.len() as u32 - 1);
    let value = bytes as f64 / (1024_f64.powi(exp as i32));
    format!("{:.2} {}", value, UNITS[exp as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_backup_info_formatted_timestamp() {
        let info = BackupInfo::new(
            PathBuf::from("/test/backup.db.gz"),
            Utc.with_ymd_and_hms(2026, 4, 5, 8, 30, 0).unwrap(),
            1024,
        );
        assert_eq!(info.formatted_timestamp(), "2026-04-05 08:30 UTC");
    }

    #[test]
    fn test_backup_info_formatted_size() {
        let test_cases = vec![
            (0, "0 B"),
            (512, "512.00 B"),
            (1024, "1.00 KB"),
            (1024 * 1024, "1.00 MB"),
            (1024 * 1024 * 1024, "1.00 GB"),
        ];
        for (bytes, expected) in test_cases {
            let info = BackupInfo::new(PathBuf::from("/test"), Utc::now(), bytes);
            assert_eq!(info.formatted_size(), expected);
        }
    }

    #[test]
    fn test_backup_result_is_success() {
        assert!(BackupResult::Success {
            path: PathBuf::from("/test"),
            duration_ms: 100,
        }
        .is_success());
        assert!(!BackupResult::Skipped {
            reason: "test".to_string(),
        }
        .is_success());
        assert!(!BackupResult::Failed {
            error: "test".to_string(),
        }
        .is_success());
    }

    #[test]
    fn test_backup_result_is_skipped() {
        assert!(BackupResult::Skipped {
            reason: "test".to_string(),
        }
        .is_skipped());
        assert!(!BackupResult::Success {
            path: PathBuf::from("/test"),
            duration_ms: 100,
        }
        .is_skipped());
    }

    #[test]
    fn test_backup_result_is_failed() {
        assert!(BackupResult::Failed {
            error: "test".to_string(),
        }
        .is_failed());
    }

    #[test]
    fn test_backup_result_message() {
        let success = BackupResult::Success {
            path: PathBuf::from("/backups/test.db.gz"),
            duration_ms: 150,
        };
        assert!(success.message().contains("Backup created"));
        assert!(success.message().contains("150ms"));

        let skipped = BackupResult::Skipped {
            reason: "No changes".to_string(),
        };
        assert_eq!(skipped.message(), "Backup skipped: No changes");

        let failed = BackupResult::Failed {
            error: "Disk full".to_string(),
        };
        assert_eq!(failed.message(), "Backup failed: Disk full");
    }

    #[test]
    fn test_restore_result() {
        assert!(RestoreResult::Success.is_success());
        assert!(!RestoreResult::Success.is_failed());

        let failed = RestoreResult::Failed {
            error: "test".to_string(),
        };
        assert!(failed.is_failed());
        assert!(!failed.is_success());

        assert_eq!(
            RestoreResult::Success.message(),
            "Restore completed successfully"
        );
        assert_eq!(
            RestoreResult::Failed {
                error: "test".to_string(),
            }
            .message(),
            "Restore failed: test"
        );
    }
}
