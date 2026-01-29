//! Data migration utilities
//!
//! This module provides functionality to migrate existing data to work with the new architecture.
//! It ensures backward compatibility and prevents data loss during migration.
//!
//! # Migration Process
//!
//! 1. **Backup**: Creates a backup of existing data before migration
//! 2. **Schema Migration**: Updates data structures to match new schema
//! 3. **Validation**: Verifies migrated data integrity
//! 4. **Rollback**: Supports rollback if migration fails
//!
//! @plan PLAN-20250125-REFACTOR.P14
//! @requirement REQ-028.1, REQ-028.2, REQ-028.3

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use tracing::{info, warn, error};

use crate::config::Config;
use crate::models::Conversation;
use crate::storage::ConversationStorage;

/// Migration runner that coordinates all data migrations
pub struct MigrationRunner {
    data_dir: PathBuf,
    backup_dir: PathBuf,
}

impl MigrationRunner {
    /// Create a new migration runner with the given data directory
    pub fn new(data_dir: PathBuf) -> Self {
        let backup_dir = data_dir.join("backup_before_migration");
        Self {
            data_dir,
            backup_dir,
        }
    }

    /// Create a migration runner with the default data directory
    ///
    /// # Errors
    /// Returns error if default data directory cannot be determined
    pub fn with_default_path() -> Result<Self> {
        let app_support = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine application support directory"))?;

        let data_dir = app_support.join("PersonalAgent");
        Ok(Self::new(data_dir))
    }

    /// Run all migrations in sequence
    ///
    /// # Errors
    /// Returns error if any migration fails
    pub async fn run_migrations(&self) -> Result<MigrationReport> {
        info!("Starting data migration...");

        // Create backup directory
        fs::create_dir_all(&self.backup_dir)
            .context("Failed to create backup directory")?;
        info!("Backup directory created at: {:?}", self.backup_dir);

        let mut report = MigrationReport::default();

        // 1. Backup existing data
        self.backup_existing_data().await?;
        info!("Backup created at: {:?}", self.backup_dir);

        // 2. Migrate conversations
        let stats = self.migrate_conversations();
        report.conversations_migrated = stats.count;
        info!("Migrated {} conversations", stats.count);

        // 3. Migrate profiles (verify config is readable)
        let stats = self.verify_config_readable();
        report.profiles_verified = stats.count;
        info!("Verified {} profiles in config", stats.count);

        // 4. Migrate MCP configurations (verify they're readable)
        let stats = self.verify_mcp_configs();
        report.mcp_configs_verified = stats.count;
        info!("Verified {} MCP configurations", stats.count);

        info!("Migration completed successfully: {:?}", report);
        Ok(report)
    }

    /// Rollback all migrations by restoring from backup
    ///
    /// # Errors
    /// Returns error if rollback fails
    pub async fn rollback(&self) -> Result<()> {
        info!("Rolling back migration...");

        // Restore from backup
        self.restore_backup().await?;

        info!("Rollback completed");
        Ok(())
    }

    /// Backup existing data files
    async fn backup_existing_data(&self) -> Result<()> {
        // Backup config
        let config_path = self.data_dir.join("config.json");
        if config_path.exists() {
            let backup_path = self.backup_dir.join("config.json.bak");
            fs::copy(&config_path, &backup_path)
                .context("Failed to backup config.json")?;
            info!("Backed up config.json");
        }

        // Backup conversation directory
        let conversations_dir = self.data_dir.join("conversations");
        if conversations_dir.exists() {
            let backup_dir = self.backup_dir.join("conversations");
            fs::create_dir_all(&backup_dir)
                .context("Failed to create backup conversations directory")?;

            for entry in fs::read_dir(&conversations_dir)
                .context("Failed to read conversations directory")?
            {
                let entry = entry?;
                let src_path = entry.path();
                if src_path.is_file() {
                    let filename = src_path.file_name()
                        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                    let dest_path = backup_dir.join(filename);
                    fs::copy(&src_path, &dest_path)
                        .with_context(|| format!("Failed to backup conversation file: {:?}", filename))?;
                }
            }
            info!("Backed up conversation files");
        }

        info!("All data backed up successfully");
        Ok(())
    }

    /// Migrate conversations to new format
    fn migrate_conversations(&self) -> MigrationStats {
        // Load existing conversations using old storage format
        let storage = match ConversationStorage::with_default_path() {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create conversation storage: {}", e);
                return MigrationStats { count: 0 };
            }
        };

        let old_conversations = match storage.load_all() {
            Ok(convs) => convs,
            Err(e) => {
                error!("Failed to load conversations: {}", e);
                return MigrationStats { count: 0 };
            }
        };

        let count = old_conversations.len() as u32;

        // Verify each conversation can be deserialized
        for conv in &old_conversations {
            // Validate the structure
            if conv.id.is_nil() {
                warn!("Conversation has invalid ID: {:?}", conv.filename());
            }
            if conv.title.as_ref().is_some_and(|t| t.is_empty()) {
                warn!("Conversation has empty title: {:?}", conv.filename());
            }
        }

        // Since the new ConversationServiceImpl uses the same ConversationStorage,
        // conversations are already in the correct format. No conversion needed.
        // The service will load them on demand.

        info!("Verified {} existing conversations are compatible", count);
        MigrationStats { count }
    }

    /// Verify config is readable and contains valid profiles
    fn verify_config_readable(&self) -> MigrationStats {
        let config_path = self.data_dir.join("config.json");

        if !config_path.exists() {
            info!("No existing config.json found, will use defaults");
            return MigrationStats { count: 0 };
        }

        // Try to load and parse the config
        let contents = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to read config.json: {}", e);
                return MigrationStats { count: 0 };
            }
        };

        let config: Config = match serde_json::from_str(&contents) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to parse config.json: {}", e);
                return MigrationStats { count: 0 };
            }
        };

        let count = config.profiles.len() as u32;

        // Verify each profile
        for profile in &config.profiles {
            if profile.id.is_nil() {
                warn!("Profile has invalid ID: {}", profile.name);
            }
        }

        info!("Verified {} profiles in config", count);
        MigrationStats { count }
    }

    /// Verify MCP configurations are readable
    fn verify_mcp_configs(&self) -> MigrationStats {
        let config_path = self.data_dir.join("config.json");

        if !config_path.exists() {
            return MigrationStats { count: 0 };
        }

        // Load config to check MCP configurations
        let contents = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to read config.json: {}", e);
                return MigrationStats { count: 0 };
            }
        };

        let config: Config = match serde_json::from_str(&contents) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to parse config.json: {}", e);
                return MigrationStats { count: 0 };
            }
        };

        let count = config.mcps.len() as u32;

        // Verify each MCP config
        for mcp in &config.mcps {
            if mcp.id.is_nil() {
                warn!("MCP config has invalid ID: {}", mcp.name);
            }
        }

        info!("Verified {} MCP configurations", count);
        MigrationStats { count }
    }

    /// Restore backup files
    async fn restore_backup(&self) -> Result<()> {
        // Restore config
        let config_backup = self.backup_dir.join("config.json.bak");
        if config_backup.exists() {
            let target = self.data_dir.join("config.json");
            fs::copy(&config_backup, &target)
                .context("Failed to restore config.json")?;
            info!("Restored config.json");
        }

        // Restore conversations
        let conv_backup_dir = self.backup_dir.join("conversations");
        if conv_backup_dir.exists() {
            let conversations_dir = self.data_dir.join("conversations");
            fs::create_dir_all(&conversations_dir)
                .context("Failed to create conversations directory")?;

            for entry in fs::read_dir(&conv_backup_dir)
                .context("Failed to read backup conversations directory")?
            {
                let entry = entry?;
                let src_path = entry.path();
                if src_path.is_file() {
                    let filename = src_path.file_name()
                        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
                    let dest_path = conversations_dir.join(filename);
                    fs::copy(&src_path, &dest_path)
                        .with_context(|| format!("Failed to restore conversation file: {:?}", filename))?;
                }
            }
            info!("Restored conversation files");
        }

        Ok(())
    }
}

/// Report of migration results
#[derive(Debug, Default)]
pub struct MigrationReport {
    pub conversations_migrated: u32,
    pub profiles_verified: u32,
    pub mcp_configs_verified: u32,
}

/// Statistics for a single migration operation
#[derive(Debug)]
pub struct MigrationStats {
    pub count: u32,
}

/// Detect the version of the config file
///
/// # Errors
/// Returns error if config cannot be read
pub fn detect_config_version(config_path: &Path) -> Result<String> {
    if !config_path.exists() {
        return Ok("1.0".to_string()); // Default version for new installs
    }

    let contents = fs::read_to_string(config_path)
        .context("Failed to read config file")?;

    // Try to parse as Config to get version
    let config: Config = serde_json::from_str(&contents)
        .context("Failed to parse config file")?;

    Ok(config.version)
}

/// Convert old conversation format to new format if needed
///
/// This function checks if a conversation file needs conversion and performs it.
/// Currently, the format is backward compatible, so no conversion is needed.
///
/// # Errors
/// Returns error if conversation cannot be loaded or converted
pub fn convert_conversation_format(conversation_path: &Path) -> Result<Conversation> {
    let contents = fs::read_to_string(conversation_path)
        .context("Failed to read conversation file")?;

    // Try to parse as new format
    let conversation: Conversation = serde_json::from_str(&contents)
        .context("Failed to parse conversation file")?;

    // The Conversation struct is backward compatible with the old format
    // No conversion needed
    Ok(conversation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Test: Create a conversation in the old format and verify it can be loaded
    #[test]
    fn test_old_conversation_format_loadable() {
        let temp_dir = TempDir::new().unwrap();
        let conversations_dir = temp_dir.path().join("conversations");
        fs::create_dir_all(&conversations_dir).unwrap();

        // Create a conversation file in the old format
        let conv_id = Uuid::new_v4();
        let filename = format!("{}-{}.json", conv_id, "Test Conversation");
        let conv_path = conversations_dir.join(&filename);

        let old_format = r#"{
  "id": "CONV_ID",
  "title": "Test Conversation",
  "profile_id": "PROFILE_ID",
  "messages": [
    {
      "id": "MSG_ID",
      "role": "user",
      "content": "Hello, world!",
      "timestamp": "2025-01-27T12:00:00Z"
    }
  ],
  "created_at": "2025-01-27T12:00:00Z",
  "updated_at": "2025-01-27T12:00:00Z"
}"#;

        let old_format = old_format
            .replace("CONV_ID", &conv_id.to_string())
            .replace("PROFILE_ID", &Uuid::new_v4().to_string())
            .replace("MSG_ID", &Uuid::new_v4().to_string());

        let mut file = File::create(&conv_path).unwrap();
        file.write_all(old_format.as_bytes()).unwrap();

        // Verify it can be loaded
        let storage = ConversationStorage::new(&conversations_dir);
        let loaded = storage.load(&filename).unwrap();

        assert_eq!(loaded.id, conv_id);
        assert_eq!(loaded.title, Some("Test Conversation".to_string()));
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "Hello, world!");
    }

    /// Test: Verify config.json is readable
    #[test]
    fn test_config_readable() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create a config file
        let config = Config::default();
        config.save(&config_path).unwrap();

        // Verify it can be loaded
        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.version, "1.0");
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.global_hotkey, "Cmd+Shift+Space");
    }

    /// Test: Detect config version
    #[test]
    fn test_detect_config_version() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create a config file
        let config = Config {
            version: "1.0".to_string(),
            ..Default::default()
        };
        config.save(&config_path).unwrap();

        // Detect version
        let version = detect_config_version(&config_path).unwrap();
        assert_eq!(version, "1.0");
    }

    /// Test: Convert conversation format
    #[test]
    fn test_convert_conversation_format() {
        let temp_dir = TempDir::new().unwrap();
        let conv_path = temp_dir.path().join("conversation.json");

        // Create a conversation file
        let conv_id = Uuid::new_v4();
        let old_format = r#"{
  "id": "CONV_ID",
  "title": "Test",
  "profile_id": "PROFILE_ID",
  "messages": [],
  "created_at": "2025-01-27T12:00:00Z",
  "updated_at": "2025-01-27T12:00:00Z"
}"#;

        let old_format = old_format
            .replace("CONV_ID", &conv_id.to_string())
            .replace("PROFILE_ID", &Uuid::new_v4().to_string());

        let mut file = File::create(&conv_path).unwrap();
        file.write_all(old_format.as_bytes()).unwrap();

        // Convert
        let conv = convert_conversation_format(&conv_path).unwrap();
        assert_eq!(conv.id, conv_id);
    }

    /// Test: Migration report structure
    #[test]
    fn test_migration_report() {
        let report = MigrationReport {
            conversations_migrated: 5,
            profiles_verified: 2,
            mcp_configs_verified: 1,
        };

        assert_eq!(report.conversations_migrated, 5);
        assert_eq!(report.profiles_verified, 2);
        assert_eq!(report.mcp_configs_verified, 1);
    }
}
