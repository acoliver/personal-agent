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
    pub fn new(secrets_dir: PathBuf) -> Self {
        Self { secrets_dir }
    }

    /// Store an API key for an MCP (single env var)
    pub fn store_api_key(&self, mcp_id: Uuid, key: &str) -> Result<(), SecretsError> {
        self.store_api_key_named(mcp_id, "default", key)
    }

    /// Store a named API key for an MCP (for MCPs with multiple env vars)
    pub fn store_api_key_named(
        &self,
        mcp_id: Uuid,
        var_name: &str,
        key: &str,
    ) -> Result<(), SecretsError> {
        fs::create_dir_all(&self.secrets_dir)?;

        let filename = if var_name == "default" {
            format!("mcp_{}.key", mcp_id)
        } else {
            format!("mcp_{}_{}.key", mcp_id, var_name)
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
    pub fn load_api_key(&self, mcp_id: Uuid) -> Result<String, SecretsError> {
        self.load_api_key_named(mcp_id, "default")
    }

    /// Load a named API key for an MCP
    pub fn load_api_key_named(&self, mcp_id: Uuid, var_name: &str) -> Result<String, SecretsError> {
        let filename = if var_name == "default" {
            format!("mcp_{}.key", mcp_id)
        } else {
            format!("mcp_{}_{}.key", mcp_id, var_name)
        };
        let path = self.secrets_dir.join(filename);

        if !path.exists() {
            return Err(SecretsError::SecretNotFound(mcp_id));
        }

        let key = fs::read_to_string(&path)?;
        Ok(key.trim().to_string())
    }

    /// Delete an API key for an MCP
    pub fn delete_api_key(&self, mcp_id: Uuid) -> Result<(), SecretsError> {
        // Delete all keys for this MCP (default and named)
        let pattern = format!("mcp_{}", mcp_id);
        if let Ok(entries) = fs::read_dir(&self.secrets_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&pattern) && name.ends_with(".key") {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    /// Read a keyfile from a path
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_and_load_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();
        let key = "test-api-key-12345";

        manager.store_api_key(mcp_id, key).unwrap();
        let loaded = manager.load_api_key(mcp_id).unwrap();

        assert_eq!(loaded, key);
    }

    #[test]
    fn test_store_and_load_named_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();
        let key1 = "api-key-1";
        let key2 = "api-key-2";

        manager
            .store_api_key_named(mcp_id, "CLIENT_ID", key1)
            .unwrap();
        manager
            .store_api_key_named(mcp_id, "CLIENT_SECRET", key2)
            .unwrap();

        let loaded1 = manager.load_api_key_named(mcp_id, "CLIENT_ID").unwrap();
        let loaded2 = manager.load_api_key_named(mcp_id, "CLIENT_SECRET").unwrap();

        assert_eq!(loaded1, key1);
        assert_eq!(loaded2, key2);
    }

    #[test]
    fn test_load_nonexistent_key() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();
        let result = manager.load_api_key(mcp_id);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecretsError::SecretNotFound(_)
        ));
    }

    #[test]
    fn test_delete_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();
        let key = "test-key";

        manager.store_api_key(mcp_id, key).unwrap();
        assert!(manager.load_api_key(mcp_id).is_ok());

        manager.delete_api_key(mcp_id).unwrap();
        assert!(manager.load_api_key(mcp_id).is_err());
    }

    #[test]
    fn test_delete_multiple_keys() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();

        manager.store_api_key(mcp_id, "key1").unwrap();
        manager
            .store_api_key_named(mcp_id, "CLIENT_ID", "key2")
            .unwrap();
        manager
            .store_api_key_named(mcp_id, "CLIENT_SECRET", "key3")
            .unwrap();

        manager.delete_api_key(mcp_id).unwrap();

        assert!(manager.load_api_key(mcp_id).is_err());
        assert!(manager.load_api_key_named(mcp_id, "CLIENT_ID").is_err());
        assert!(manager.load_api_key_named(mcp_id, "CLIENT_SECRET").is_err());
    }

    #[test]
    fn test_read_keyfile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let keyfile_path = temp_dir.path().join("test.key");
        fs::write(&keyfile_path, "test-keyfile-content\n").unwrap();

        let content = manager.read_keyfile(&keyfile_path).unwrap();
        assert_eq!(content, "test-keyfile-content");
    }

    #[test]
    fn test_read_nonexistent_keyfile() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let keyfile_path = temp_dir.path().join("nonexistent.key");
        let result = manager.read_keyfile(&keyfile_path);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecretsError::KeyfileNotFound(_)
        ));
    }

    #[test]
    #[cfg(unix)]
    fn test_api_key_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();
        let key = "test-key";

        manager.store_api_key(mcp_id, key).unwrap();

        let keyfile_path = temp_dir.path().join(format!("mcp_{}.key", mcp_id));
        let metadata = fs::metadata(keyfile_path).unwrap();
        let permissions = metadata.permissions();

        assert_eq!(permissions.mode() & 0o777, 0o600);
    }

    #[test]
    fn test_trim_whitespace_on_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SecretsManager::new(temp_dir.path().to_path_buf());

        let mcp_id = Uuid::new_v4();
        let key = "  test-key-with-whitespace  \n";

        manager.store_api_key(mcp_id, key).unwrap();
        let loaded = manager.load_api_key(mcp_id).unwrap();

        assert_eq!(loaded, "test-key-with-whitespace");
    }

    #[test]
    fn test_secrets_dir_created() {
        let temp_dir = TempDir::new().unwrap();
        let secrets_path = temp_dir.path().join("secrets");

        assert!(!secrets_path.exists());

        let manager = SecretsManager::new(secrets_path.clone());
        let mcp_id = Uuid::new_v4();

        manager.store_api_key(mcp_id, "test").unwrap();

        assert!(secrets_path.exists());
    }
}
