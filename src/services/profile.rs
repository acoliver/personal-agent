// @plan PLAN-20250125-REFACTOR.P07
//! Profile service for managing AI model profiles
//!
//! Handles CRUD operations for model profiles including authentication,
//! parameters, and connection testing.

use async_trait::async_trait;
use uuid::Uuid;

use crate::models::{AuthConfig, ModelParameters, ModelProfile};

use super::ServiceResult;

/// Model profile service trait
///
/// Implementation: [`super::profile_impl::ProfileServiceImpl`]
#[async_trait]
pub trait ProfileService: Send + Sync {
    /// List all model profiles
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>>;

    /// Get a specific profile by ID
    async fn get(&self, id: Uuid) -> ServiceResult<ModelProfile>;

    /// Create a new model profile
    ///
    /// # Arguments
    /// * `name` - Profile name
    /// * `provider` - Model provider (e.g., "openai", "anthropic")
    /// * `model` - Model identifier (e.g., "gpt-4", "claude-3-opus")
    /// * `auth` - Authentication configuration
    /// * `parameters` - Model parameters (temperature, max_tokens, etc.)
    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        auth: AuthConfig,
        parameters: ModelParameters,
    ) -> ServiceResult<ModelProfile>;

    /// Update an existing profile
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        model: Option<String>,
        auth: Option<AuthConfig>,
        parameters: Option<ModelParameters>,
    ) -> ServiceResult<ModelProfile>;

    /// Delete a profile
    async fn delete(&self, id: Uuid) -> ServiceResult<()>;

    /// Test connection to the model API
    ///
    /// Returns Ok(()) if the connection is successful
    async fn test_connection(&self, id: Uuid) -> ServiceResult<()>;

    /// Get the default profile
    async fn get_default(&self) -> ServiceResult<Option<ModelProfile>>;

    /// Set a profile as the default
    async fn set_default(&self, id: Uuid) -> ServiceResult<()>;
}
