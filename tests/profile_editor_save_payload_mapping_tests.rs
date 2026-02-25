//! Profile Editor SaveProfile payload mapping tests
//!
//! Ensures SaveProfile payload fields are mapped into ProfileService update/create
//! semantics so field edits persist through presenter boundaries.

use std::sync::{Arc, Mutex};

use personal_agent::events::types::{ModelProfile as EventModelProfile, ModelProfileAuth, ModelProfileParameters};
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::services::{ProfileService, ServiceError, ServiceResult};

#[derive(Clone, Default)]
struct RecordingProfileService {
    create_calls: Arc<Mutex<Vec<CreateCall>>>,
    update_calls: Arc<Mutex<Vec<UpdateCall>>>,
    update_result: Arc<Mutex<Option<ServiceResult<ModelProfile>>>>,
}

#[derive(Clone, Debug)]
struct CreateCall {
    name: String,
    provider: String,
    model: String,
    auth: AuthConfig,
    parameters: ModelParameters,
}

#[derive(Clone, Debug)]
struct UpdateCall {
    id: uuid::Uuid,
    name: Option<String>,
    model: Option<String>,
    auth: Option<AuthConfig>,
    parameters: Option<ModelParameters>,
}

impl RecordingProfileService {
    fn with_update_result(result: ServiceResult<ModelProfile>) -> Self {
        Self {
            update_result: Arc::new(Mutex::new(Some(result))),
            ..Default::default()
        }
    }

    fn update_calls(&self) -> Vec<UpdateCall> {
        self.update_calls
            .lock()
            .expect("update calls lock poisoned")
            .clone()
    }

    fn create_calls(&self) -> Vec<CreateCall> {
        self.create_calls
            .lock()
            .expect("create calls lock poisoned")
            .clone()
    }
}

#[async_trait::async_trait]
impl ProfileService for RecordingProfileService {
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        Ok(vec![])
    }

    async fn get(&self, _id: uuid::Uuid) -> ServiceResult<ModelProfile> {
        Err(ServiceError::NotFound("unused".to_string()))
    }

    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        auth: AuthConfig,
        parameters: ModelParameters,
    ) -> ServiceResult<ModelProfile> {
        self.create_calls
            .lock()
            .expect("create calls lock poisoned")
            .push(CreateCall {
                name: name.clone(),
                provider: provider.clone(),
                model: model.clone(),
                auth: auth.clone(),
                parameters: parameters.clone(),
            });

        Ok(ModelProfile {
            id: uuid::Uuid::new_v4(),
            name,
            provider_id: provider,
            model_id: model,
            base_url: "https://api.openai.com/v1".to_string(),
            auth,
            parameters,
            system_prompt: "You are a helpful assistant, be direct and to the point. Respond in English.".to_string(),
        })
    }

    async fn update(
        &self,
        id: uuid::Uuid,
        name: Option<String>,
        model: Option<String>,
        auth: Option<AuthConfig>,
        parameters: Option<ModelParameters>,
    ) -> ServiceResult<ModelProfile> {
        self.update_calls
            .lock()
            .expect("update calls lock poisoned")
            .push(UpdateCall {
                id,
                name: name.clone(),
                model: model.clone(),
                auth: auth.clone(),
                parameters: parameters.clone(),
            });

        if let Some(result) = self
            .update_result
            .lock()
            .expect("update result lock poisoned")
            .take()
        {
            return result;
        }

        Ok(ModelProfile {
            id,
            name: name.unwrap_or_else(|| "Updated Profile".to_string()),
            provider_id: "openai".to_string(),
            model_id: model.unwrap_or_else(|| "gpt-4o".to_string()),
            base_url: "https://api.openai.com/v1".to_string(),
            auth: auth.unwrap_or(AuthConfig::Key {
                value: "".to_string(),
            }),
            parameters: parameters.unwrap_or_default(),
            system_prompt: "You are a helpful assistant, be direct and to the point. Respond in English.".to_string(),
        })
    }

    async fn delete(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn test_connection(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_default(&self) -> ServiceResult<Option<ModelProfile>> {
        Ok(None)
    }

    async fn set_default(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }
}

fn payload() -> EventModelProfile {
    EventModelProfile {
        id: uuid::Uuid::new_v4(),
        name: "Editor Name".to_string(),
        provider_id: Some("anthropic".to_string()),
        model_id: Some("claude-3-5-sonnet".to_string()),
        base_url: Some("https://api.anthropic.com/v1".to_string()),
        auth: Some(ModelProfileAuth::Keyfile {
            path: "/tmp/keyfile.txt".to_string(),
        }),
        parameters: Some(ModelProfileParameters {
            temperature: Some(0.2),
            max_tokens: Some(2048),
            show_thinking: Some(true),
            enable_thinking: Some(true),
            thinking_budget: Some(12000),
        }),
        system_prompt: Some("Be concise".to_string()),
    }
}

#[tokio::test]
async fn test_save_profile_payload_maps_fields_to_update_call() {
    let event_bus_sender: tokio::sync::broadcast::Sender<personal_agent::events::AppEvent> =
        tokio::sync::broadcast::channel::<personal_agent::events::AppEvent>(32).0;
    let (view_tx, mut view_rx) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(32);

    let recording = RecordingProfileService::default();
    let profile_service: Arc<dyn ProfileService> = Arc::new(recording.clone());

    let mut presenter = personal_agent::presentation::ProfileEditorPresenter::new(
        profile_service,
        &event_bus_sender,
        view_tx,
    );

    presenter.start().await.expect("presenter start must succeed");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let save_payload = payload();
    event_bus_sender
        .send(personal_agent::events::AppEvent::User(
            personal_agent::events::types::UserEvent::SaveProfile {
                profile: save_payload.clone(),
            },
        ))
        .ok();

    let cmd = tokio::time::timeout(std::time::Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for ProfileUpdated")
        .expect("view channel closed");

    assert!(
        matches!(cmd, personal_agent::presentation::ViewCommand::ProfileUpdated { .. }),
        "first command should be ProfileUpdated, got {:?}",
        cmd
    );

    let update_calls = recording.update_calls();
    assert_eq!(update_calls.len(), 1, "SaveProfile should issue one update call");

    let call = &update_calls[0];
    assert_eq!(call.id, save_payload.id);
    assert_eq!(call.name.as_deref(), Some("Editor Name"));
    assert_eq!(call.model.as_deref(), Some("claude-3-5-sonnet"));
    assert!(matches!(
        call.auth,
        Some(AuthConfig::Keyfile { .. })
    ));

    let params = call.parameters.clone().expect("parameters should be passed to update");
    assert!((params.temperature - 0.2).abs() < f64::EPSILON);
    assert_eq!(params.max_tokens, 2048);
    assert!(params.show_thinking);
    assert!(params.enable_thinking);
    assert_eq!(params.thinking_budget, Some(12000));
}

#[tokio::test]
async fn test_save_profile_payload_fallback_create_uses_payload_provider_and_model() {
    let event_bus_sender: tokio::sync::broadcast::Sender<personal_agent::events::AppEvent> =
        tokio::sync::broadcast::channel::<personal_agent::events::AppEvent>(32).0;
    let (view_tx, mut view_rx) =
        tokio::sync::broadcast::channel::<personal_agent::presentation::ViewCommand>(32);

    let recording = RecordingProfileService::with_update_result(Err(ServiceError::NotFound(
        "missing".to_string(),
    )));
    let profile_service: Arc<dyn ProfileService> = Arc::new(recording.clone());

    let mut presenter = personal_agent::presentation::ProfileEditorPresenter::new(
        profile_service,
        &event_bus_sender,
        view_tx,
    );

    presenter.start().await.expect("presenter start must succeed");
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    event_bus_sender
        .send(personal_agent::events::AppEvent::User(
            personal_agent::events::types::UserEvent::SaveProfile { profile: payload() },
        ))
        .ok();

    let first = tokio::time::timeout(std::time::Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");
    let second = tokio::time::timeout(std::time::Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for second command")
        .expect("view channel closed");

    assert!(
        matches!(first, personal_agent::presentation::ViewCommand::ProfileUpdated { .. })
    );
    assert!(matches!(second, personal_agent::presentation::ViewCommand::NavigateBack));

    let create_calls = recording.create_calls();
    assert_eq!(create_calls.len(), 1, "NotFound fallback should create exactly one profile");
    let create = &create_calls[0];
    assert_eq!(create.provider, "anthropic");
    assert_eq!(create.model, "claude-3-5-sonnet");
    assert_eq!(create.name, "Editor Name");
    assert!(matches!(create.auth, AuthConfig::Keyfile { .. }));
    assert!((create.parameters.temperature - 0.2).abs() < f64::EPSILON);
    assert_eq!(create.parameters.max_tokens, 2048);
    assert!(create.parameters.show_thinking);
    assert!(create.parameters.enable_thinking);
    assert_eq!(create.parameters.thinking_budget, Some(12000));
}

