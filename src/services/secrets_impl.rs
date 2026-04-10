use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use async_trait::async_trait;
use rand::random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::services::secrets::SecretsService;
use crate::services::{secure_store, ServiceError, ServiceResult};

const SECRET_INDEX_KEY: &str = "__secret_index__";
const SECRET_KEY_PREFIX: &str = "secret:";
const API_KEY_PREFIX: &str = "api_key:";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecretsBackendMode {
    KeyringPreferred,
    FileFallbackOnly,
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedSecretFile {
    version: u32,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

pub struct SecretsServiceImpl {
    secrets_dir: PathBuf,
    backend_mode: SecretsBackendMode,
}

impl SecretsServiceImpl {
    /// # Errors
    ///
    /// Returns `ServiceError` if the secrets directory cannot be created.
    pub fn new(secrets_dir: PathBuf) -> Result<Self, ServiceError> {
        Self::with_backend_mode(secrets_dir, SecretsBackendMode::KeyringPreferred)
    }

    /// # Errors
    ///
    /// Returns `ServiceError` if the secrets directory cannot be created.
    pub fn new_file_fallback_only(secrets_dir: PathBuf) -> Result<Self, ServiceError> {
        Self::with_backend_mode(secrets_dir, SecretsBackendMode::FileFallbackOnly)
    }

    fn with_backend_mode(
        secrets_dir: PathBuf,
        backend_mode: SecretsBackendMode,
    ) -> Result<Self, ServiceError> {
        fs::create_dir_all(&secrets_dir).map_err(|e| {
            ServiceError::Storage(format!("Failed to create secrets directory: {e}"))
        })?;

        Ok(Self {
            secrets_dir,
            backend_mode,
        })
    }

    fn get_secret_path(&self, key: &str) -> PathBuf {
        self.secrets_dir.join(format!("{key}.enc"))
    }

    fn get_api_key_path(&self, provider: &str) -> PathBuf {
        self.secrets_dir.join(format!("api_key_{provider}.enc"))
    }

    fn validate_key(key: &str) -> Result<(), ServiceError> {
        if key.is_empty() {
            return Err(ServiceError::Validation("Key cannot be empty".to_string()));
        }

        if key.contains('/') || key.contains('\\') || key.contains("..") {
            return Err(ServiceError::Validation(
                "Key contains invalid characters".to_string(),
            ));
        }

        if key
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.')
        {
            return Err(ServiceError::Validation(
                "Key contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }

    fn store_secret_value(
        &self,
        secure_store_key: &str,
        file_path: &Path,
        value: &str,
    ) -> ServiceResult<()> {
        match self.backend_mode {
            SecretsBackendMode::KeyringPreferred => {
                match secure_store::set_secret(secure_store_key, value) {
                    Ok(()) => Ok(()),
                    Err(_) => self.write_encrypted_file(file_path, value),
                }
            }
            SecretsBackendMode::FileFallbackOnly => self.write_encrypted_file(file_path, value),
        }
    }

    fn get_secret_value(
        &self,
        secure_store_key: &str,
        file_path: &Path,
    ) -> ServiceResult<Option<String>> {
        match self.backend_mode {
            SecretsBackendMode::KeyringPreferred => {
                match secure_store::get_secret(secure_store_key) {
                    Ok(Some(value)) => Ok(Some(value)),
                    Ok(None) | Err(_) => self.read_encrypted_file(file_path),
                }
            }
            SecretsBackendMode::FileFallbackOnly => self.read_encrypted_file(file_path),
        }
    }

    fn delete_secret_value(&self, secure_store_key: &str, file_path: &Path) -> ServiceResult<()> {
        let keyring_deleted = match self.backend_mode {
            SecretsBackendMode::KeyringPreferred => {
                secure_store::delete_secret(secure_store_key).is_ok()
            }
            SecretsBackendMode::FileFallbackOnly => false,
        };

        let file_deleted = if file_path.exists() {
            fs::remove_file(file_path)
                .map(|()| true)
                .map_err(|e| ServiceError::Storage(format!("Failed to delete secret: {e}")))?
        } else {
            false
        };

        if keyring_deleted || file_deleted {
            Ok(())
        } else {
            Err(ServiceError::NotFound(format!(
                "Secret not found: {}",
                file_path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("unknown")
            )))
        }
    }

    fn exists_secret_value(&self, secure_store_key: &str, file_path: &Path) -> bool {
        match self.backend_mode {
            SecretsBackendMode::KeyringPreferred => {
                match secure_store::has_secret(secure_store_key) {
                    Ok(true) => true,
                    Ok(false) | Err(_) => file_path.exists(),
                }
            }
            SecretsBackendMode::FileFallbackOnly => file_path.exists(),
        }
    }

    fn load_key_index(&self) -> ServiceResult<Vec<String>> {
        let index_path = self.get_secret_path(SECRET_INDEX_KEY);
        let secure_store_key = format!("{SECRET_KEY_PREFIX}{SECRET_INDEX_KEY}");
        let maybe_json = self.get_secret_value(&secure_store_key, &index_path)?;
        let mut keys = maybe_json.map_or_else(Vec::new, |json| {
            serde_json::from_str::<Vec<String>>(&json).unwrap_or_default()
        });
        keys.sort();
        keys.dedup();
        Ok(keys)
    }

    fn save_key_index(&self, keys: &[String]) -> ServiceResult<()> {
        let json = serde_json::to_string(keys).map_err(|e| {
            ServiceError::Serialization(format!("Failed to serialize key index: {e}"))
        })?;
        let index_path = self.get_secret_path(SECRET_INDEX_KEY);
        let secure_store_key = format!("{SECRET_KEY_PREFIX}{SECRET_INDEX_KEY}");
        self.store_secret_value(&secure_store_key, &index_path, &json)
    }

    fn add_to_key_index(&self, key: &str) -> ServiceResult<()> {
        let mut keys = self.load_key_index()?;
        if !keys.iter().any(|existing| existing == key) {
            keys.push(key.to_string());
            keys.sort();
            self.save_key_index(&keys)?;
        }
        Ok(())
    }

    fn remove_from_key_index(&self, key: &str) -> ServiceResult<()> {
        let mut keys = self.load_key_index()?;
        keys.retain(|existing| existing != key);
        self.save_key_index(&keys)
    }

    fn derive_encryption_key(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"personal-agent-secrets-fallback-v1");
        hasher.update(self.secrets_dir.to_string_lossy().as_bytes());

        if let Ok(user) = std::env::var("USER") {
            hasher.update(user.as_bytes());
        }
        if let Ok(username) = std::env::var("USERNAME") {
            hasher.update(username.as_bytes());
        }
        if let Ok(home) = std::env::var("HOME") {
            hasher.update(home.as_bytes());
        }

        let digest = hasher.finalize();
        let mut key = [0_u8; 32];
        key.copy_from_slice(&digest);
        key
    }

    fn write_encrypted_file(&self, path: &Path, value: &str) -> ServiceResult<()> {
        let key = self.derive_encryption_key();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce_bytes: [u8; 12] = random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, value.as_bytes()).map_err(|e| {
            ServiceError::Storage(format!(
                "Failed to encrypt secret for {}: {e}",
                path.display()
            ))
        })?;

        let payload = EncryptedSecretFile {
            version: 1,
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        };
        let serialized = serde_json::to_vec(&payload).map_err(|e| {
            ServiceError::Serialization(format!("Failed to serialize encrypted secret: {e}"))
        })?;

        fs::write(path, serialized)
            .map_err(|e| ServiceError::Storage(format!("Failed to write secret: {e}")))
    }

    fn read_encrypted_file(&self, path: &Path) -> ServiceResult<Option<String>> {
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path)
            .map_err(|e| ServiceError::Storage(format!("Failed to read secret: {e}")))?;
        let payload: EncryptedSecretFile = serde_json::from_slice(&bytes).map_err(|e| {
            ServiceError::Serialization(format!("Failed to parse encrypted secret: {e}"))
        })?;

        if payload.version != 1 {
            return Err(ServiceError::Storage(format!(
                "Unsupported encrypted secret version: {}",
                payload.version
            )));
        }

        if payload.nonce.len() != 12 {
            return Err(ServiceError::Storage(
                "Encrypted secret nonce must be 12 bytes".to_string(),
            ));
        }

        let key = self.derive_encryption_key();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&payload.nonce),
                payload.ciphertext.as_ref(),
            )
            .map_err(|e| {
                ServiceError::Storage(format!(
                    "Failed to decrypt secret for {}: {e}",
                    path.display()
                ))
            })?;

        String::from_utf8(plaintext).map(Some).map_err(|e| {
            ServiceError::Storage(format!("Decrypted secret was not valid UTF-8: {e}"))
        })
    }
}

#[async_trait]
impl SecretsService for SecretsServiceImpl {
    async fn store(&self, key: String, value: String) -> ServiceResult<()> {
        Self::validate_key(&key)?;

        let secure_store_key = format!("{SECRET_KEY_PREFIX}{key}");
        let path = self.get_secret_path(&key);
        self.store_secret_value(&secure_store_key, &path, &value)?;
        self.add_to_key_index(&key)
    }

    async fn get(&self, key: &str) -> ServiceResult<Option<String>> {
        Self::validate_key(key)?;

        let secure_store_key = format!("{SECRET_KEY_PREFIX}{key}");
        let path = self.get_secret_path(key);
        self.get_secret_value(&secure_store_key, &path)
    }

    async fn delete(&self, key: &str) -> ServiceResult<()> {
        Self::validate_key(key)?;

        let secure_store_key = format!("{SECRET_KEY_PREFIX}{key}");
        let path = self.get_secret_path(key);
        self.delete_secret_value(&secure_store_key, &path)?;
        self.remove_from_key_index(key)
    }

    async fn list_keys(&self) -> ServiceResult<Vec<String>> {
        self.load_key_index()
    }

    async fn exists(&self, key: &str) -> ServiceResult<bool> {
        Self::validate_key(key)?;

        let secure_store_key = format!("{SECRET_KEY_PREFIX}{key}");
        let path = self.get_secret_path(key);
        Ok(self.exists_secret_value(&secure_store_key, &path))
    }

    async fn store_api_key(&self, provider: String, api_key: String) -> ServiceResult<()> {
        Self::validate_key(&provider)?;

        let secure_store_key = format!("{API_KEY_PREFIX}{provider}");
        let path = self.get_api_key_path(&provider);
        self.store_secret_value(&secure_store_key, &path, &api_key)
    }

    async fn get_api_key(&self, provider: &str) -> ServiceResult<Option<String>> {
        Self::validate_key(provider)?;

        let secure_store_key = format!("{API_KEY_PREFIX}{provider}");
        let path = self.get_api_key_path(provider);
        self.get_secret_value(&secure_store_key, &path)
    }

    async fn delete_api_key(&self, provider: &str) -> ServiceResult<()> {
        Self::validate_key(provider)?;

        let secure_store_key = format!("{API_KEY_PREFIX}{provider}");
        let path = self.get_api_key_path(provider);
        self.delete_secret_value(&secure_store_key, &path)
            .map_err(|err| match err {
                ServiceError::NotFound(_) => {
                    ServiceError::NotFound(format!("API key not found: {provider}"))
                }
                other => other,
            })
    }
}
