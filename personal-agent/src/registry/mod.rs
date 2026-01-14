//! Model registry module for fetching and caching model information from models.dev

mod cache;
mod models_dev;
mod types;

pub use cache::{CacheMetadata, RegistryCache};
pub use models_dev::ModelsDevClient;
pub use types::{Cost, Limit, Modalities, ModelInfo, ModelRegistry, Provider};

use crate::error::Result;

/// Manages the model registry with caching
pub struct RegistryManager {
    client: ModelsDevClient,
    cache: RegistryCache,
}

impl RegistryManager {
    /// Create a new registry manager with default settings
    ///
    /// # Errors
    ///
    /// Returns an error if the default cache path cannot be determined.
    pub fn new() -> Result<Self> {
        let cache_path = RegistryCache::default_path()?;
        Ok(Self {
            client: ModelsDevClient::new(),
            cache: RegistryCache::new(cache_path, 24),
        })
    }

    /// Create a new registry manager with custom cache path and expiry
    #[must_use]
    pub fn with_cache(cache_path: std::path::PathBuf, expiry_hours: i64) -> Self {
        Self {
            client: ModelsDevClient::new(),
            cache: RegistryCache::new(cache_path, expiry_hours),
        }
    }

    /// Create a new registry manager with a custom client (useful for testing)
    #[must_use]
    #[cfg(test)]
    pub fn with_client(client: ModelsDevClient, cache_path: std::path::PathBuf, expiry_hours: i64) -> Self {
        Self {
            client,
            cache: RegistryCache::new(cache_path, expiry_hours),
        }
    }

    /// Get the registry, loading from cache or fetching fresh if needed
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The cache file cannot be read
    /// - The network request to models.dev fails
    /// - The response cannot be parsed
    pub async fn get_registry(&self) -> Result<ModelRegistry> {
        if let Some(cached) = self.cache.load()? {
            return Ok(cached);
        }

        self.refresh().await
    }

    /// Force refresh the registry from the API
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The network request to models.dev fails
    /// - The response cannot be parsed
    /// - The cache file cannot be written
    pub async fn refresh(&self) -> Result<ModelRegistry> {
        let registry = self.client.fetch_registry().await?;
        self.cache.save(&registry)?;
        Ok(registry)
    }

    /// Clear the cache
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be deleted.
    pub fn clear_cache(&self) -> Result<()> {
        self.cache.clear()
    }

    /// Get cache metadata
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be read.
    pub fn cache_metadata(&self) -> Result<Option<CacheMetadata>> {
        self.cache.metadata()
    }
}

impl Default for RegistryManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default RegistryManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_mock_response() -> serde_json::Value {
        serde_json::json!({
            "test-provider": {
                "id": "test-provider",
                "name": "Test Provider",
                "env": ["TEST_API_KEY"],
                "models": {
                    "test-model": {
                        "id": "test-model",
                        "name": "Test Model",
                        "attachment": false,
                        "reasoning": false,
                        "tool_call": true,
                        "structured_output": false,
                        "temperature": true,
                        "interleaved": false,
                        "open_weights": false
                    }
                }
            }
        })
    }

    #[tokio::test]
    async fn test_registry_manager_caching() {
        let mock_server = MockServer::start().await;
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(create_mock_response()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let manager = RegistryManager::with_client(client, cache_path.clone(), 24);

        let registry = manager.get_registry().await;
        assert!(registry.is_ok());

        assert!(cache_path.exists());

        let metadata = manager.cache_metadata().unwrap();
        assert!(metadata.is_some());
        assert!(!metadata.unwrap().is_expired);
    }

    #[tokio::test]
    async fn test_registry_manager_refresh() {
        let mock_server = MockServer::start().await;
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(create_mock_response()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let manager = RegistryManager::with_client(client, cache_path.clone(), 24);

        let registry1 = manager.refresh().await;
        assert!(registry1.is_ok());

        let registry2 = manager.get_registry().await;
        assert!(registry2.is_ok());

        assert_eq!(registry1.unwrap().providers.len(), registry2.unwrap().providers.len());
    }

    #[tokio::test]
    async fn test_registry_manager_clear_cache() {
        let mock_server = MockServer::start().await;
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test_cache.json");

        Mock::given(method("GET"))
            .and(path("/api.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(create_mock_response()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = ModelsDevClient::with_url(format!("{}/api.json", mock_server.uri()));
        let manager = RegistryManager::with_client(client, cache_path.clone(), 24);

        let _registry = manager.get_registry().await.unwrap();
        assert!(cache_path.exists());

        manager.clear_cache().unwrap();
        assert!(!cache_path.exists());
    }
}
