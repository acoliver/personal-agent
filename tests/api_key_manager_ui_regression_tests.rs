use async_trait::async_trait;
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::presentation::{
    api_key_manager_presenter::ApiKeyManagerPresenter, ViewCommand,
};
use personal_agent::services::{secure_store, ProfileService, ServiceError, ServiceResult};
use tokio::sync::broadcast;
use uuid::Uuid;

struct ApiKeyManagerTestProfileService {
    profiles: Vec<ModelProfile>,
}

#[async_trait]
impl ProfileService for ApiKeyManagerTestProfileService {
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        Ok(self.profiles.clone())
    }

    async fn get(&self, id: Uuid) -> ServiceResult<ModelProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound("profile not found".to_string()))
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
    ) -> ServiceResult<ModelProfile> {
        Err(ServiceError::Internal("not used in test".to_string()))
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
    ) -> ServiceResult<ModelProfile> {
        Err(ServiceError::Internal("not used in test".to_string()))
    }

    async fn delete(&self, _id: Uuid) -> ServiceResult<()> {
        Err(ServiceError::Internal("not used in test".to_string()))
    }

    async fn test_connection(&self, _id: Uuid) -> ServiceResult<()> {
        Err(ServiceError::Internal("not used in test".to_string()))
    }

    async fn get_default(&self) -> ServiceResult<Option<ModelProfile>> {
        Ok(self.profiles.first().cloned())
    }

    async fn set_default(&self, _id: Uuid) -> ServiceResult<()> {
        Err(ServiceError::Internal("not used in test".to_string()))
    }
}

#[tokio::test]
async fn api_key_manager_presenter_start_emits_initial_key_list() {
    use std::sync::Arc;

    use personal_agent::events::AppEvent;

    secure_store::use_mock_backend();
    let _ = secure_store::api_keys::delete("alpha");
    let _ = secure_store::api_keys::delete("beta");
    secure_store::api_keys::store("alpha", "sk-alpha-1234").expect("store alpha in mock backend");
    secure_store::api_keys::store("beta", "sk-beta-9876").expect("store beta in mock backend");

    let profile_service = Arc::new(ApiKeyManagerTestProfileService {
        profiles: vec![ModelProfile {
            id: Uuid::new_v4(),
            name: "Profile Alpha".to_string(),
            provider_id: "openai".to_string(),
            model_id: "gpt-4o-mini".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            auth: AuthConfig::Keychain {
                label: "alpha".to_string(),
            },
            parameters: ModelParameters::default(),
            system_prompt: "test prompt".to_string(),
        }],
    }) as Arc<dyn ProfileService>;

    let event_tx = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel(32);
    let mut presenter = ApiKeyManagerPresenter::new(profile_service, &event_tx, view_tx);

    presenter.start().await.expect("start presenter");

    let command = view_rx.recv().await.expect("initial command");
    let ViewCommand::ApiKeysListed { keys } = command else {
        panic!("expected ApiKeysListed on startup");
    };

    let alpha = keys
        .iter()
        .find(|key| key.label == "alpha")
        .expect("alpha listed");
    assert_eq!(alpha.used_by, vec!["Profile Alpha".to_string()]);
    assert_eq!(alpha.masked_value, "••••••••");

    let beta = keys
        .iter()
        .find(|key| key.label == "beta")
        .expect("beta listed");
    assert!(
        beta.used_by.is_empty(),
        "unused keys should still be listed"
    );
}
