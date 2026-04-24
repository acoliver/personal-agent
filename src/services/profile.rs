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
#[allow(clippy::too_many_arguments)]
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
    /// * `base_url` - Optional base URL override (uses provider default when None/empty)
    /// * `auth` - Authentication configuration
    /// * `parameters` - Model parameters (temperature, `max_tokens`, etc.)
    /// * `system_prompt` - Optional system prompt override
    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        base_url: Option<String>,
        auth: AuthConfig,
        parameters: ModelParameters,
        system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile>;

    /// Update an existing profile
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        provider: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
        auth: Option<AuthConfig>,
        parameters: Option<ModelParameters>,
        system_prompt: Option<String>,
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

    /// Update the profile's `context_window_size` (the editor field labeled
    /// "CONTEXT LIMIT").
    ///
    /// This lives outside `update`'s `parameters` blob because it is stored
    /// at the profile level on disk, not inside `ModelParameters`. The
    /// default implementation is a no-op so test doubles don't have to
    /// implement it; the real [`super::profile_impl::ProfileServiceImpl`]
    /// overrides it to persist the change. Issue #182.
    async fn set_context_window_size(&self, _id: Uuid, _size: usize) -> ServiceResult<()> {
        Ok(())
    }
}
