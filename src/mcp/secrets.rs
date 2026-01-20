use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("Keyfile not found: {0}")]
    KeyfileNotFound(PathBuf),
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Secret not found for MCP {0}")]
    SecretNotFound(Uuid),
}

pub struct SecretsManager {
    secrets_dir: PathBuf,
}

impl SecretsManager {
    #[must_use]
    pub const fn new(secrets_dir: PathBuf) -> Self {
        Self { secrets_dir }
    }

    /// Store an API key for an MCP (single env var)
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be written.
    pub fn store_api_key(&self, mcp_id: Uuid, key: &str) -> Result<(), SecretsError> {
        self.store_api_key_named(mcp_id, "default", key)
    }

    /// Store a named API key for an MCP (for MCPs with multiple env vars)
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be written.
    pub fn store_api_key_named(
        &self,
        mcp_id: Uuid,
        var_name: &str,
        key: &str,
    ) -> Result<(), SecretsError> {
        fs::create_dir_all(&self.secrets_dir)?;

        let filename = if var_name == "default" {
            format!("mcp_{mcp_id}.key")
        } else {
            format!("mcp_{mcp_id}_{var_name}.key")
        };
        let path = self.secrets_dir.join(filename);

        fs::write(&path, key)?;

        // Set permissions to 600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, permissions)?;
        }

        Ok(())
    }

    /// Load an API key for an MCP
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be loaded.
    pub fn load_api_key(&self, mcp_id: Uuid) -> Result<String, SecretsError> {
        self.load_api_key_named(mcp_id, "default")
    }

    /// Load a named API key for an MCP
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be loaded.
    pub fn load_api_key_named(&self, mcp_id: Uuid, var_name: &str) -> Result<String, SecretsError> {
        let filename = if var_name == "default" {
            format!("mcp_{mcp_id}.key")
        } else {
            format!("mcp_{mcp_id}_{var_name}.key")
        };
        let path = self.secrets_dir.join(filename);

        if !path.exists() {
            return Err(SecretsError::SecretNotFound(mcp_id));
        }

        let key = fs::read_to_string(&path)?;
        Ok(key.trim().to_string())
    }

    /// Delete an API key for an MCP
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if key deletion fails.
    pub fn delete_api_key(&self, mcp_id: Uuid) -> Result<(), SecretsError> {
        // Delete all keys for this MCP (default and named)
        let pattern = format!("mcp_{mcp_id}");
        if let Ok(entries) = fs::read_dir(&self.secrets_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_key = std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("key"));
                if name.starts_with(&pattern) && is_key {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    /// Read a keyfile from a path
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the keyfile cannot be read.
    pub fn read_keyfile(&self, path: &Path) -> Result<String, SecretsError> {
        if !path.exists() {
            return Err(SecretsError::KeyfileNotFound(path.to_path_buf()));
        }

        match fs::read_to_string(path) {
            Ok(content) => Ok(content.trim().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Err(SecretsError::PermissionDenied(path.to_path_buf()))
            }
            Err(e) => Err(SecretsError::Io(e)),
        }
    }
}
