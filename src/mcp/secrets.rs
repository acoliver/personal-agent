use crate::services::secure_store::{self, mcp_keys};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("Keychain error: {0}")]
    Keychain(String),
    #[error("Secret not found for MCP {0}")]
    SecretNotFound(Uuid),
}

impl From<secure_store::SecureStoreError> for SecretsError {
    fn from(e: secure_store::SecureStoreError) -> Self {
        match e {
            secure_store::SecureStoreError::NotFound(msg)
            | secure_store::SecureStoreError::Keychain(msg) => Self::Keychain(msg),
        }
    }
}

/// MCP secret storage backed by the OS keychain via `secure_store::mcp_keys`.
pub struct SecretsManager;

impl SecretsManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Store an API key for an MCP (single env var).
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be stored.
    pub fn store_api_key(&self, mcp_id: Uuid, key: &str) -> Result<(), SecretsError> {
        mcp_keys::store(mcp_id, key)?;
        Ok(())
    }

    /// Store a named API key for an MCP (for MCPs with multiple env vars).
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be stored.
    pub fn store_api_key_named(
        &self,
        mcp_id: Uuid,
        var_name: &str,
        key: &str,
    ) -> Result<(), SecretsError> {
        mcp_keys::store_named(mcp_id, var_name, key)?;
        Ok(())
    }

    /// Load an API key for an MCP.
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be loaded.
    pub fn load_api_key(&self, mcp_id: Uuid) -> Result<String, SecretsError> {
        mcp_keys::get(mcp_id)?.ok_or(SecretsError::SecretNotFound(mcp_id))
    }

    /// Load a named API key for an MCP.
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if the key cannot be loaded.
    pub fn load_api_key_named(&self, mcp_id: Uuid, var_name: &str) -> Result<String, SecretsError> {
        mcp_keys::get_named(mcp_id, var_name)?.ok_or(SecretsError::SecretNotFound(mcp_id))
    }

    /// Delete all keys for an MCP.
    ///
    /// # Errors
    ///
    /// Returns `SecretsError` if key deletion fails.
    pub fn delete_api_key(&self, mcp_id: Uuid) -> Result<(), SecretsError> {
        mcp_keys::delete(mcp_id)?;
        Ok(())
    }
}

impl Default for SecretsManager {
    fn default() -> Self {
        Self::new()
    }
}
