//! Models registry service implementation

use super::{ModelsRegistryService, ServiceError, ServiceResult};
use crate::registry::{RegistryCache, ModelInfo, ModelRegistry};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const REGISTRY_URL: &str = "https://models.dev.json";
const CACHE_EXPIRY_HOURS: i64 = 24;

/// HTTP cache implementation of ModelsRegistryService
pub struct ModelsRegistryServiceImpl {
    cache: RegistryCache,
    cached_registry: Arc<RwLock<Option<ModelRegistry>>>,
    last_refresh: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
}

impl ModelsRegistryServiceImpl {
    /// Create a new ModelsRegistryServiceImpl with default cache path
    ///
    /// # Errors
    ///
    /// Returns an error if the default cache path cannot be determined.
    pub fn new() -> Result<Self, ServiceError> {
        let cache_path = RegistryCache::default_path().map_err(|e| {
            ServiceError::Io(format!("Failed to determine cache path: {e}"))
        })?;

        Ok(Self::with_cache_path(cache_path))
    }

    /// Create a new ModelsRegistryServiceImpl with a specific cache path
    #[must_use]
    pub fn with_cache_path(cache_path: PathBuf) -> Self {
        Self {
            cache: RegistryCache::new(cache_path, CACHE_EXPIRY_HOURS),
            cached_registry: Arc::new(RwLock::new(None)),
            last_refresh: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetch the registry from the URL
    async fn fetch_from_url(&self) -> Result<ModelRegistry, ServiceError> {
        #[cfg(feature = "reqwest")]
        {
            let client = reqwest::Client::new();
            let response = client
                .get(REGISTRY_URL)
                .send()
                .await
                .map_err(|e| ServiceError::Network(format!("Failed to fetch registry: {e}")))?;

            if !response.status().is_success() {
                return Err(ServiceError::Network(format!(
                    "Registry returned status: {}",
                    response.status()
                )));
            }

            // First try to parse as wrapped cache format
            let text = response
                .text()
                .await
                .map_err(|e| ServiceError::Network(format!("Failed to read response body: {e}")))?;

            // Try to parse as CachedRegistry format (with cached_at and data fields)
            if let Ok(cached) = serde_json::from_str::<crate::registry::cache::CachedRegistry>(&text) {
                return Ok(cached.data);
            }

            // Otherwise try direct ModelRegistry format
            serde_json::from_str(&text)
                .map_err(|e| ServiceError::Serialization(format!("Failed to parse registry: {e}")))
        }

        #[cfg(not(feature = "reqwest"))]
        {
            let _ = REGISTRY_URL; // Suppress unused warning
            Err(ServiceError::Network(
                "HTTP client not available. Build with 'reqwest' feature to enable fetching.".to_string(),
            ))
        }
    }
}

#[async_trait::async_trait]
impl ModelsRegistryService for ModelsRegistryServiceImpl {
    /// Refresh the models registry from remote sources
    async fn refresh(&self) -> ServiceResult<()> {
        // Try to fetch from URL
        match self.fetch_from_url().await {
            Ok(registry) => {
                // Save to cache
                self.cache.save(&registry).map_err(|e| {
                    ServiceError::Io(format!("Failed to save cache: {e}"))
                })?;

                // Update in-memory cache
                *self.cached_registry.write().await = Some(registry);
                *self.last_refresh.write().await = Some(chrono::Utc::now());
                Ok(())
            }
            Err(e) => {
                // If fetch fails, try to load from cache
                match self.cache.load() {
                    Ok(Some(registry)) => {
                        *self.cached_registry.write().await = Some(registry);
                        Err(ServiceError::Network(format!(
                            "Failed to fetch from URL, loaded from cache: {e}"
                        )))
                    }
                    Ok(None) => Err(e),
                    Err(cache_err) => Err(ServiceError::Io(format!(
                        "Failed to fetch from URL and failed to load cache: {e}, {cache_err}"
                    )))
                }
            }
        }
    }

    /// Get information about a specific model
    async fn get_model(&self, provider: &str, model: &str) -> ServiceResult<Option<ModelInfo>> {
        // Ensure we have cached data
        if self.cached_registry.read().await.is_none() {
            if let Ok(Some(registry)) = self.cache.load() {
                *self.cached_registry.write().await = Some(registry);
            }
        }

        let registry = self.cached_registry.read().await;
        let registry = registry.as_ref().ok_or_else(|| {
            ServiceError::Internal("No cached data available. Call refresh() first.".to_string())
        })?;

        Ok(registry.get_model(provider, model).map(|m| m.clone()))
    }

    /// Get all models for a specific provider
    async fn get_provider(&self, provider: &str) -> ServiceResult<Vec<ModelInfo>> {
        // Ensure we have cached data
        if self.cached_registry.read().await.is_none() {
            if let Ok(Some(registry)) = self.cache.load() {
                *self.cached_registry.write().await = Some(registry);
            }
        }

        let registry = self.cached_registry.read().await;
        let registry = registry.as_ref().ok_or_else(|| {
            ServiceError::Internal("No cached data available. Call refresh() first.".to_string())
        })?;

        Ok(registry
            .get_models_for_provider(provider)
            .map(|models| models.into_iter().map(|m| m.clone()).collect())
            .unwrap_or_default())
    }

    /// List all available providers
    async fn list_providers(&self) -> ServiceResult<Vec<String>> {
        // Ensure we have cached data
        if self.cached_registry.read().await.is_none() {
            if let Ok(Some(registry)) = self.cache.load() {
                *self.cached_registry.write().await = Some(registry);
            }
        }

        let registry = self.cached_registry.read().await;
        let registry = registry.as_ref().ok_or_else(|| {
            ServiceError::Internal("No cached data available. Call refresh() first.".to_string())
        })?;

        Ok(registry.get_provider_ids())
    }

    /// List all models from all providers
    async fn list_all(&self) -> ServiceResult<Vec<ModelInfo>> {
        // Ensure we have cached data
        if self.cached_registry.read().await.is_none() {
            if let Ok(Some(registry)) = self.cache.load() {
                *self.cached_registry.write().await = Some(registry);
            }
        }

        let registry = self.cached_registry.read().await;
        let registry = registry.as_ref().ok_or_else(|| {
            ServiceError::Internal("No cached data available. Call refresh() first.".to_string())
        })?;

        let mut all_models = Vec::new();
        for provider in registry.providers.values() {
            for model in provider.models.values() {
                all_models.push(model.clone());
            }
        }

        all_models.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(all_models)
    }

    /// Search for models by name or capabilities
    async fn search(&self, query: &str) -> ServiceResult<Vec<ModelInfo>> {
        // Ensure we have cached data
        if self.cached_registry.read().await.is_none() {
            if let Ok(Some(registry)) = self.cache.load() {
                *self.cached_registry.write().await = Some(registry);
            }
        }

        let registry = self.cached_registry.read().await;
        let registry = registry.as_ref().ok_or_else(|| {
            ServiceError::Internal("No cached data available. Call refresh() first.".to_string())
        })?;

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for provider in registry.providers.values() {
            for model in provider.models.values() {
                if model.id.to_lowercase().contains(&query_lower)
                    || model.name.to_lowercase().contains(&query_lower)
                    || model
                        .family
                        .as_ref()
                        .is_some_and(|f| f.to_lowercase().contains(&query_lower))
                {
                    results.push(model.clone());
                }
            }
        }

        results.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(results)
    }

    /// Check if a registry update is available
    async fn check_update(&self) -> ServiceResult<bool> {
        // For now, just check if cache is expired
        let metadata = self
            .cache
            .metadata()
            .map_err(|e| ServiceError::Io(format!("Failed to read cache metadata: {e}")))?;

        Ok(metadata.map_or(true, |m| m.is_expired))
    }

    /// Get the last refresh timestamp
    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(*self.last_refresh.read().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Modalities, ModelRegistry};
    use tempfile::TempDir;

    fn create_test_registry() -> ModelRegistry {
        let mut providers = std::collections::HashMap::new();

        use crate::registry::Provider;
        let provider = Provider {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            env: vec!["OPENAI_API_KEY".to_string()],
            npm: Some("@ai-sdk/openai".to_string()),
            api: Some("https://api.openai.com/v1".to_string()),
            doc: Some("https://platform.openai.com/docs".to_string()),
            models: {
                let mut models = std::collections::HashMap::new();
                models.insert(
                    "gpt-4".to_string(),
                    ModelInfo {
                        id: "gpt-4".to_string(),
                        name: "GPT-4".to_string(),
                        family: Some("gpt".to_string()),
                        attachment: false,
                        reasoning: false,
                        tool_call: true,
                        structured_output: true,
                        temperature: true,
                        interleaved: false,
                        provider: Some("openai".to_string()),
                        status: Some("active".to_string()),
                        knowledge: Some("2023-12".to_string()),
                        release_date: Some("2023-03-14".to_string()),
                        last_updated: Some("2023-12-11".to_string()),
                        modalities: Some(Modalities {
                            input: vec!["text".to_string()],
                            output: vec!["text".to_string()],
                        }),
                        open_weights: false,
                        cost: Some(crate::registry::Cost {
                            input: 0.03,
                            output: 0.06,
                            cache_read: None,
                        }),
                        limit: Some(crate::registry::Limit {
                            context: 8192,
                            output: 4096,
                        }),
                    },
                );
                models
            },
        };

        providers.insert("openai".to_string(), provider);

        crate::registry::ModelRegistry { providers }
    }

    #[tokio::test]
    async fn test_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test-cache.json");

        let service = ModelsRegistryServiceImpl::with_cache_path(cache_path.clone());

        // Create test registry and save to cache
        let test_registry = create_test_registry();
        service.cache.save(&test_registry).unwrap();

        // Load from cache through the service
        let providers = service.list_providers().await.unwrap();
        assert_eq!(providers, vec!["openai"]);
    }

    #[tokio::test]
    async fn test_search() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test-cache.json");

        let service = ModelsRegistryServiceImpl::with_cache_path(cache_path.clone());

        // Create test registry and save to cache
        let test_registry = create_test_registry();
        service.cache.save(&test_registry).unwrap();

        // Search for models
        let results = service.search("gpt").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "gpt-4");
    }

    #[tokio::test]
    async fn test_get_provider() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test-cache.json");

        let service = ModelsRegistryServiceImpl::with_cache_path(cache_path.clone());

        // Create test registry and save to cache
        let test_registry = create_test_registry();
        service.cache.save(&test_registry).unwrap();

        // Get provider
        let models = service.get_provider("openai").await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gpt-4");
    }

    #[tokio::test]
    async fn test_get_model() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test-cache.json");

        let service = ModelsRegistryServiceImpl::with_cache_path(cache_path.clone());

        // Create test registry and save to cache
        let test_registry = create_test_registry();
        service.cache.save(&test_registry).unwrap();

        // Get model
        let model = service.get_model("openai", "gpt-4").await.unwrap();
        assert!(model.is_some());
        assert_eq!(model.unwrap().id, "gpt-4");
    }

    #[tokio::test]
    async fn test_cache_miss_without_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("test-cache.json");

        let service = ModelsRegistryServiceImpl::with_cache_path(cache_path);

        // Try to list providers without calling refresh first
        let result = service.list_providers().await;
        assert!(result.is_err());
    }
}
