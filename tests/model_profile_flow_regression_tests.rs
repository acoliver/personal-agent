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
fn bug_model_selected_should_use_shared_provider_base_url_defaults() {
    let source = include_str!("../src/ui_gpui/views/profile_editor_view.rs");

    assert!(
        source.contains("default_api_base_url_for_provider(&provider_id)"),
        "ModelSelected prefill should delegate provider base URL fallback to shared defaults"
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

#[test]
fn bug_api_key_storage_uses_keychain_not_inline_paste() {
    // API keys are now stored in the OS keychain via secure_store, not pasted inline.
    // Verify the profile editor no longer has inline API key entry fields.
    let source = include_str!("../src/ui_gpui/views/profile_editor_view.rs");

    assert!(
        !source.contains(".id(\"btn-paste-api-key\")"),
        "Inline API key paste button should be removed — keys are stored via keychain"
    );
    assert!(
        source.contains("key_label") || source.contains("api_key_label"),
        "Profile editor should reference keychain labels instead of inline API keys"
    );
}

#[test]
fn bug_browse_model_should_preserve_or_refresh_available_api_keys_for_new_profile_flow() {
    let source = include_str!("../src/ui_gpui/views/profile_editor_view.rs");

    assert!(
        source.contains("let available_keys = this.state.data.available_keys.clone();")
            && source.contains("this.state.data.available_keys = available_keys;")
            && source.contains("this.request_api_key_refresh();"),
        "Browse -> ModelSelector should not strand the new-profile API key dropdown with an empty available_keys list"
    );
}

#[test]
fn bug_chat_service_should_fallback_to_profile_system_prompt_when_conversation_has_none() {
    let source = include_str!("../src/services/chat_impl.rs");
    let start = source
        .find("let system_prompt = conversation")
        .or_else(|| source.find("fn system_prompt_for_conversation"))
        .expect("system_prompt extraction should exist");
    let end = (start + 520).min(source.len());
    let window = &source[start..end];

    let has_profile_fallback = window.contains("profile.system_prompt")
        || window.contains("unwrap_or(&profile.system_prompt)")
        || window.contains("unwrap_or(profile.system_prompt")
        || window.contains("if system_prompt.is_empty()")
        || window.contains("if system_prompt.trim().is_empty()");

    assert!(
        has_profile_fallback,
        "ChatService should fallback to profile.system_prompt when no system message exists in conversation"
    );
}
