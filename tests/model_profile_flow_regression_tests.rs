//! Regression tests for GPUI model-config/profile flow bugs.
//!
//! These tests intentionally encode expected behavior for reported bugs and are
//! expected to fail until implementation fixes land.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio::time::Duration;
use uuid::Uuid;

use personal_agent::events::{types::UserEvent, AppEvent};
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::presentation::{ProfileEditorPresenter, ViewCommand};
use personal_agent::services::{ProfileService, ServiceError, ServiceResult};

#[derive(Clone, Default)]
struct RecordingCreateProfileService {
    created_ids: Arc<Mutex<Vec<Uuid>>>,
}

impl RecordingCreateProfileService {
    fn created_ids(&self) -> Vec<Uuid> {
        self.created_ids
            .lock()
            .expect("created ids lock poisoned")
            .clone()
    }
}

#[async_trait]
impl ProfileService for RecordingCreateProfileService {
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        Ok(vec![])
    }

    async fn get(&self, _id: Uuid) -> ServiceResult<ModelProfile> {
        Err(ServiceError::NotFound("unused".to_string()))
    }

    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        base_url: Option<String>,
        auth: AuthConfig,
        parameters: ModelParameters,
        system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        let mut profile = ModelProfile::new(
            name,
            provider,
            model,
            base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            auth,
        )
        .with_parameters(parameters);

        if let Some(prompt) = system_prompt {
            profile.system_prompt = prompt;
        }

        self.created_ids
            .lock()
            .expect("created ids lock poisoned")
            .push(profile.id);

        Ok(profile)
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
        Err(ServiceError::NotFound("unused".to_string()))
    }

    async fn delete(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_default(&self) -> ServiceResult<Option<ModelProfile>> {
        Ok(None)
    }

    async fn set_default(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn bug_save_profile_editor_should_emit_default_profile_changed() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let recording = RecordingCreateProfileService::default();
    let profile_service: Arc<dyn ProfileService> = Arc::new(recording.clone());

    let mut presenter = ProfileEditorPresenter::new(profile_service, &event_bus_sender, view_tx);
    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SelectModel {
            provider_id: "synthetic".to_string(),
            model_id: "hf:moonshotai/Kimi-K2.5".to_string(),
        }))
        .ok();

    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveProfileEditor))
        .ok();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let created_ids = recording.created_ids();
    assert_eq!(
        created_ids.len(),
        1,
        "SaveProfileEditor should create exactly one profile"
    );

    let mut observed = Vec::new();
    while let Ok(cmd) = view_rx.try_recv() {
        observed.push(cmd);
    }

    assert!(
        observed
            .iter()
            .any(|cmd| matches!(cmd, ViewCommand::DefaultProfileChanged { profile_id: Some(_) })),
        "Saving from profile editor should emit DefaultProfileChanged so chat/settings stay selected after back navigation. Observed commands: {observed:?}"
    );
}

#[test]
fn provider_defaults_should_use_models_dev_metadata_for_kimi_and_builtin_fallbacks() {
    assert_eq!(
        personal_agent::config::provider_api_url("kimi-for-coding").as_deref(),
        Some("https://api.kimi.com/coding/v1")
    );
    assert_eq!(
        personal_agent::config::provider_api_url("moonshotai").as_deref(),
        Some("https://api.moonshot.ai/v1")
    );
    assert_eq!(
        personal_agent::config::provider_api_url("openrouter").as_deref(),
        Some("https://openrouter.ai/api/v1")
    );
    assert_eq!(
        personal_agent::config::default_api_base_url_for_provider("synthetic"),
        "https://api.synthetic.new/v1"
    );
}
