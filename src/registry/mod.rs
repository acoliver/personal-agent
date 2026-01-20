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
