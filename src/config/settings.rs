//! Configuration settings for `PersonalAgent`

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::models::ModelProfile;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub version: String,
    pub theme: String,
    pub global_hotkey: String,
    pub default_profile: Option<Uuid>,
    /// The currently active conversation ID (restored on app restart)
    #[serde(default)]
    pub active_conversation_id: Option<Uuid>,
    pub context_management: ContextManagement,
    pub profiles: Vec<ModelProfile>,
    #[serde(default)]
    pub mcps: Vec<crate::mcp::McpConfig>,
    /// Smithery API key or path to keyfile
    #[serde(default)]
    pub smithery_auth: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextManagement {
    pub trigger_threshold: f64,
    pub preserve_top: f64,
    pub preserve_bottom: f64,
    pub summary_target_ratio: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            theme: "dark".to_string(),
            global_hotkey: "Cmd+Shift+Space".to_string(),
            default_profile: None,
            active_conversation_id: None,
            context_management: ContextManagement::default(),
            profiles: Vec::new(),
            mcps: Vec::new(),
            smithery_auth: None,
        }
    }
}

impl Default for ContextManagement {
    fn default() -> Self {
        Self {
            trigger_threshold: 0.80,
            preserve_top: 0.20,
            preserve_bottom: 0.20,
            summary_target_ratio: 0.50,
        }
    }
}

impl Config {
    /// Load configuration from file, creating default if it doesn't exist
    ///
    /// # Errors
    /// Returns error if file cannot be read or parsed
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if path.exists() {
            let contents = fs::read_to_string(path)?;
            let config: Self = serde_json::from_str(&contents)?;
            Ok(config)
        } else {
            let config = Self::default();
            config.save(path)?;
            Ok(config)
        }
    }

    /// Save configuration to file with secure permissions
    ///
    /// # Errors
    /// Returns error if file cannot be written or permissions cannot be set
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to JSON
        let contents = serde_json::to_string_pretty(self)?;

        // Write to file
        fs::write(path, contents)?;

        // Set permissions to 600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, permissions)?;
        }

        Ok(())
    }

    /// Get the default config file path
    ///
    /// # Errors
    /// Returns error if application support directory cannot be determined
    pub fn default_path() -> Result<PathBuf> {
        let app_support = dirs::data_local_dir().ok_or_else(|| {
            AppError::Config("Could not determine application support directory".to_string())
        })?;

        Ok(app_support.join("PersonalAgent").join("config.json"))
    }

    /// Add a profile to the configuration
    pub fn add_profile(&mut self, profile: ModelProfile) {
        self.profiles.push(profile);
    }

    /// Remove a profile by ID
    ///
    /// # Errors
    /// Returns error if profile with given ID is not found
    pub fn remove_profile(&mut self, id: &Uuid) -> Result<()> {
        let index = self
            .profiles
            .iter()
            .position(|p| p.id == *id)
            .ok_or_else(|| AppError::ProfileNotFound(id.to_string()))?;

        self.profiles.remove(index);

        // Clear default profile if it was removed
        if self.default_profile == Some(*id) {
            self.default_profile = None;
        }

        Ok(())
    }

    /// Get a profile by ID
    ///
    /// # Errors
    /// Returns error if profile with given ID is not found
    pub fn get_profile(&self, id: &Uuid) -> Result<&ModelProfile> {
        self.profiles
            .iter()
            .find(|p| p.id == *id)
            .ok_or_else(|| AppError::ProfileNotFound(id.to_string()))
    }

    /// Get a mutable profile by ID
    ///
    /// # Errors
    /// Returns error if profile with given ID is not found
    pub fn get_profile_mut(&mut self, id: &Uuid) -> Result<&mut ModelProfile> {
        self.profiles
            .iter_mut()
            .find(|p| p.id == *id)
            .ok_or_else(|| AppError::ProfileNotFound(id.to_string()))
    }

    /// Update a profile
    ///
    /// # Errors
    /// Returns error if profile with given ID is not found
    pub fn update_profile(&mut self, profile: ModelProfile) -> Result<()> {
        let existing = self.get_profile_mut(&profile.id)?;
        *existing = profile;
        Ok(())
    }

    /// Add an MCP to the configuration
    pub fn add_mcp(&mut self, mcp: crate::mcp::McpConfig) {
        self.mcps.push(mcp);
    }

    /// Remove an MCP by ID
    ///
    /// # Errors
    /// Returns error if MCP with given ID is not found
    pub fn remove_mcp(&mut self, id: &Uuid) -> Result<()> {
        let index = self
            .mcps
            .iter()
            .position(|m| m.id == *id)
            .ok_or_else(|| AppError::Config(format!("MCP not found: {id}")))?;

        self.mcps.remove(index);
        Ok(())
    }

    /// Get enabled MCPs
    #[must_use]
    pub fn get_enabled_mcps(&self) -> Vec<&crate::mcp::McpConfig> {
        self.mcps.iter().filter(|m| m.enabled).collect()
    }
}
