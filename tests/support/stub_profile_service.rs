//! A read-only [`ProfileService`] over a fixed list, for presenter tests that
//! only need `list()` to answer.

use personal_agent::models::profile::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::services::{ProfileService, ServiceError};
use uuid::Uuid;

pub struct StubProfileService {
    profiles: Vec<ModelProfile>,
}

impl StubProfileService {
    #[must_use]
    pub const fn new(profiles: Vec<ModelProfile>) -> Self {
        Self { profiles }
    }
}

#[async_trait::async_trait]
impl ProfileService for StubProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(self.profiles.clone())
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("profile {id}")))
    }

    async fn create(
        &self,
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: AuthConfig,
        _parameters: ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::Internal("stub".to_string()))
    }

    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _parameters: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        Err(ServiceError::Internal("stub".to_string()))
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        Ok(self.profiles.first().cloned())
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}
