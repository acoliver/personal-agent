//! Database backup module
//!
//! Provides automatic `SQLite` database backup functionality with:
//! - Configurable periodic backups
//! - Rolling retention policy
//! - Manual backup/restore support
//! - Startup recovery for corrupted databases

pub mod scheduler;
pub mod settings;
pub mod types;

pub use scheduler::{spawn_backup_scheduler, BackupScheduler};
pub use settings::DatabaseBackupSettings;
pub use types::{BackupInfo, BackupMetadata, BackupResult, RestoreResult};
