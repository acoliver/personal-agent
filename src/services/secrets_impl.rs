use std::fs;
use std::path::PathBuf;
use async_trait::async_trait;

use crate::services::{ServiceError, ServiceResult};
use crate::services::secrets::SecretsService;

pub struct SecretsServiceImpl {
    secrets_dir: PathBuf,
}

impl SecretsServiceImpl {
    pub fn new(secrets_dir: PathBuf) -> Result<Self, ServiceError> {
        fs::create_dir_all(&secrets_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to create secrets directory: {}", e)))?;
        
        Ok(Self {
            secrets_dir,
        })
    }

    fn get_secret_path(&self, key: &str) -> PathBuf {
        self.secrets_dir.join(format!("{}.txt", key))
    }

    fn get_api_key_path(&self, provider: &str) -> PathBuf {
        self.secrets_dir.join(format!("api_key_{}.txt", provider))
    }

    fn validate_key(&self, key: &str) -> Result<(), ServiceError> {
        if key.is_empty() {
            return Err(ServiceError::Validation("Key cannot be empty".to_string()));
        }

        if key.contains('/') || key.contains('\\') || key.contains("..") {
            return Err(ServiceError::Validation("Key contains invalid characters".to_string()));
        }

        if key.chars().any(|c| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.') {
            return Err(ServiceError::Validation("Key contains invalid characters".to_string()));
        }

        Ok(())
    }
}

#[async_trait]
impl SecretsService for SecretsServiceImpl {
    async fn store(&self, key: String, value: String) -> ServiceResult<()> {
        self.validate_key(&key)?;

        let path = self.get_secret_path(&key);
        fs::write(&path, value)
            .map_err(|e| ServiceError::Storage(format!("Failed to write secret: {}", e)))
    }

    async fn get(&self, key: &str) -> ServiceResult<Option<String>> {
        self.validate_key(key)?;

        let path = self.get_secret_path(key);
        
        if !path.exists() {
            return Ok(None);
        }

        let value = fs::read_to_string(&path)
            .map_err(|e| ServiceError::Storage(format!("Failed to read secret: {}", e)))?;
        Ok(Some(value))
    }

    async fn delete(&self, key: &str) -> ServiceResult<()> {
        self.validate_key(key)?;

        let path = self.get_secret_path(key);
        
        if !path.exists() {
            return Err(ServiceError::NotFound(format!("Secret not found: {}", key)));
        }

        fs::remove_file(&path)
            .map_err(|e| ServiceError::Storage(format!("Failed to delete secret: {}", e)))
    }

    async fn list_keys(&self) -> ServiceResult<Vec<String>> {
        let mut keys = Vec::new();

        let entries = fs::read_dir(&self.secrets_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to read secrets directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| ServiceError::Storage(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("txt") {
                continue;
            }

            if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                // Skip API key files
                if file_stem.starts_with("api_key_") {
                    continue;
                }
                keys.push(file_stem.to_string());
            }
        }

        keys.sort();
        Ok(keys)
    }

    async fn exists(&self, key: &str) -> ServiceResult<bool> {
        self.validate_key(key)?;
        let path = self.get_secret_path(key);
        Ok(path.exists())
    }

    async fn store_api_key(&self, provider: String, api_key: String) -> ServiceResult<()> {
        self.validate_key(&provider)?;

        let path = self.get_api_key_path(&provider);
        fs::write(&path, api_key)
            .map_err(|e| ServiceError::Storage(format!("Failed to write API key: {}", e)))
    }

    async fn get_api_key(&self, provider: &str) -> ServiceResult<Option<String>> {
        self.validate_key(provider)?;

        let path = self.get_api_key_path(provider);
        
        if !path.exists() {
            return Ok(None);
        }

        let value = fs::read_to_string(&path)
            .map_err(|e| ServiceError::Storage(format!("Failed to read API key: {}", e)))?;
        Ok(Some(value))
    }

    async fn delete_api_key(&self, provider: &str) -> ServiceResult<()> {
        self.validate_key(provider)?;

        let path = self.get_api_key_path(provider);
        
        if !path.exists() {
            return Err(ServiceError::NotFound(format!("API key not found: {}", provider)));
        }

        fs::remove_file(&path)
            .map_err(|e| ServiceError::Storage(format!("Failed to delete API key: {}", e)))
    }
}
