// @plan PLAN-20250125-REFACTOR.P07
//! Models registry service for discovering available AI models
//!
//! Provides access to the model registry for querying available models
//! and their capabilities.

use async_trait::async_trait;

use crate::registry::ModelInfo;

use super::ServiceResult;

/// Models registry service trait
///
/// Implementation: [`super::models_registry_impl::ModelsRegistryServiceImpl`]
#[async_trait]
pub trait ModelsRegistryService: Send + Sync {
    /// Refresh the models registry from remote sources
    async fn refresh(&self) -> ServiceResult<()>;

    /// Get information about a specific model
    ///
    /// # Arguments
    /// * `provider` - The model provider (e.g., "openai", "anthropic")
    /// * `model` - The model identifier
    async fn get_model(&self, provider: &str, model: &str) -> ServiceResult<Option<ModelInfo>>;

    /// Get all models for a specific provider
    async fn get_provider(&self, provider: &str) -> ServiceResult<Vec<ModelInfo>>;

    /// List all available providers
    async fn list_providers(&self) -> ServiceResult<Vec<String>>;

    /// List all models from all providers
    async fn list_all(&self) -> ServiceResult<Vec<ModelInfo>>;

    /// Search for models by name or capabilities
    ///
    /// # Arguments
    /// * `query` - Search query string
    async fn search(&self, query: &str) -> ServiceResult<Vec<ModelInfo>>;

    /// Check if a registry update is available
    async fn check_update(&self) -> ServiceResult<bool>;

    /// Get the last refresh timestamp
    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>>;
}
