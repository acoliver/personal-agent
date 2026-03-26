#[test]
fn api_key_manager_registers_entity_input_handler_for_text_entry() {
    let source = include_str!("../src/ui_gpui/views/api_key_manager_view/render.rs");

    assert!(
        source.contains("ElementInputHandler::new(bounds, entity)"),
        "API Key Manager must register an ElementInputHandler so the label and value fields are actually editable"
    );
    assert!(
        source.contains("window.handle_input(")
            && source.contains("ElementInputHandler::new(bounds, entity)"),
        "API Key Manager must wire window.handle_input to the focused entity"
    );
}

#[test]
fn api_key_manager_supports_cmd_v_paste_for_active_field() {
    let source = include_str!("../src/ui_gpui/views/api_key_manager_view/render.rs");

    assert!(
        source.contains("modifiers.platform && key == \"v\"")
            && source.contains("cx.read_from_clipboard()"),
        "API Key Manager should support Cmd+V paste from the system clipboard"
    );
}

#[test]
fn api_key_manager_exposes_mask_toggle_for_secret_value() {
    let source = include_str!("../src/ui_gpui/views/api_key_manager_view/render.rs");

    assert!(
        source.contains("checkbox-mask-key") && source.contains("Mask"),
        "API Key Manager should expose a mask toggle for the key value field"
    );
    assert!(
        source.contains("this.state.mask_value = !this.state.mask_value"),
        "Mask toggle should actually flip the masked/unmasked state"
    );
}

#[test]
fn api_key_manager_keeps_label_non_editable_during_edit_mode_tabbing() {
    let source = include_str!("../src/ui_gpui/views/api_key_manager_view/render.rs");

    assert!(
        source.contains("(EditMode::Editing { .. }, Some(ActiveField::Value | ActiveField::Label))")
            && source.contains("self.state.active_field = Some(ActiveField::Value);"),
        "API Key Manager should keep focus on the value field while editing an existing key so the fixed label cannot become editable via Tab"
    );
}

#[test]
fn profile_editor_requests_key_refresh_when_bridge_is_attached() {
    let source = include_str!("../src/ui_gpui/views/profile_editor_view/mod.rs");

    assert!(
        source.contains("fn request_api_key_refresh(&self)")
            && source.contains("self.emit(&UserEvent::RefreshApiKeys);")
            && source.contains("self.request_api_key_refresh();"),
        "Profile editor should refresh key labels when its bridge is attached so the dropdown is populated"
    );
}

#[test]
fn main_panel_runtime_snapshot_requests_include_api_keys() {
    // Narrowed: request_runtime_snapshots is in main_panel/startup.rs after extraction
    let source = include_str!("../src/ui_gpui/views/main_panel/startup.rs");

    assert!(
        source.contains("bridge.emit(UserEvent::RefreshApiKeys)"),
        "MainPanel runtime snapshot requests should include RefreshApiKeys so profile editor key labels repopulate after navigation/reset flows"
    );
}

#[test]
fn main_panel_profile_editor_navigation_requests_api_keys() {
    // Narrowed: navigation handling is in main_panel/render.rs after extraction
    let source = include_str!("../src/ui_gpui/views/main_panel/render.rs");

    assert!(
        source.contains("if view_id == ViewId::ProfileEditor")
            && source.contains("gpui_bridge.emit(UserEvent::RefreshApiKeys)"),
        "Navigating into ProfileEditor should actively request API keys so the new-profile dropdown does not stay stranded empty"
    );
}

#[test]
fn profile_editor_empty_key_dropdown_requests_refresh_instead_of_silent_no_op() {
    let source = include_str!("../src/ui_gpui/views/profile_editor_view/render.rs");

    assert!(
        source.contains("if this.state.data.available_keys.is_empty()")
            && source.contains("this.request_api_key_refresh();")
            && !source.contains("if keys.is_empty() {\n                                        return;\n                                    }"),
        "An empty Select API Key dropdown should request a refresh instead of appearing permanently disabled"
    );
}

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
