// @plan PLAN-20250125-REFACTOR.P07
//! Secrets service for secure storage of sensitive data
//!
//! Provides secure storage for API keys, tokens, and other sensitive configuration.

use async_trait::async_trait;

use super::ServiceResult;

/// Secrets service trait
///
/// Implementation: [`super::secrets_impl::SecretsServiceImpl`]
#[async_trait]
pub trait SecretsService: Send + Sync {
    /// Store a secret value
    ///
    /// # Arguments
    /// * `key` - The key to identify the secret
    /// * `value` - The secret value to store
    async fn store(&self, key: String, value: String) -> ServiceResult<()>;

    /// Retrieve a secret value
    ///
    /// Returns None if the key doesn't exist
    async fn get(&self, key: &str) -> ServiceResult<Option<String>>;

    /// Delete a secret
    async fn delete(&self, key: &str) -> ServiceResult<()>;

    /// List all secret keys
    async fn list_keys(&self) -> ServiceResult<Vec<String>>;

    /// Check if a key exists
    async fn exists(&self, key: &str) -> ServiceResult<bool>;

    /// Store an API key for a specific provider
    ///
    /// # Arguments
    /// * `provider` - The provider name (e.g., "openai", "anthropic")
    /// * `api_key` - The API key to store
    async fn store_api_key(&self, provider: String, api_key: String) -> ServiceResult<()>;

    /// Get an API key for a specific provider
    async fn get_api_key(&self, provider: &str) -> ServiceResult<Option<String>>;

    /// Delete an API key for a specific provider
    async fn delete_api_key(&self, provider: &str) -> ServiceResult<()>;
}
