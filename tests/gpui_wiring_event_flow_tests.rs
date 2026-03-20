//! GPUI Wiring - Event Flow Integrity Tests
//!
//! Tests that user events from GPUI views reach the responsible presenter
//! through the global `EventBus`, with special focus on mismatch hotspot variants
//! that currently block profile/MCP flows end-to-end.
//!
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
//! @requirement REQ-WIRE-001

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use personal_agent::events::types::UserEvent;
use personal_agent::events::{AppEvent, EventBus};
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::presentation::view_command::McpRegistryResult;
use personal_agent::presentation::ViewCommand;
use personal_agent::services::{
    McpRegistryEntry, McpServerStatus, McpTool, ServiceError, ServiceResult,
};

use personal_agent::ui_gpui::bridge::{spawn_user_event_forwarder, GpuiBridge};

/// Create a temporary config file with a valid default Config for test isolation.
fn temp_config_path() -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("create temp dir");
    let dir_path = dir.keep();
    let path = dir_path.join("config.json");
    let default_config = personal_agent::config::Config::default();
    let json = serde_json::to_string_pretty(&default_config).expect("serialize default config");
    std::fs::write(&path, json).expect("write default config");
    path
}

/// Create a temporary config file whose path is unwritable (for failure tests).
fn broken_config_path() -> std::path::PathBuf {
    std::path::PathBuf::from("/nonexistent/dir/config.json")
}

/// Build a rich `McpConfig` for test payloads (replaces old lossy placeholder).
fn test_rich_mcp_config(id: uuid::Uuid, name: &str) -> personal_agent::mcp::McpConfig {
    personal_agent::mcp::McpConfig {
        id,
        name: name.to_string(),
        enabled: true,
        source: personal_agent::mcp::McpSource::Manual { url: String::new() },
        package: personal_agent::mcp::McpPackage {
            package_type: personal_agent::mcp::McpPackageType::Npm,
            identifier: String::new(),
            runtime_hint: None,
        },
        transport: personal_agent::mcp::McpTransport::Stdio,
        auth_type: personal_agent::mcp::McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::Value::Null,
        oauth_token: None,
    }
}

// ============================================================
// No-op service stubs for presenter construction
// ============================================================

struct NoopProfileService;

#[async_trait::async_trait]
impl personal_agent::services::ProfileService for NoopProfileService {
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        Ok(vec![])
    }
    async fn get(&self, _id: uuid::Uuid) -> ServiceResult<ModelProfile> {
        Err(ServiceError::NotFound("noop".into()))
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
        Err(ServiceError::NotFound("noop".into()))
    }
    async fn update(
        &self,
        _id: uuid::Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _parameters: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        Err(ServiceError::NotFound("noop".into()))
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

struct NoopModelsRegistryService;

#[async_trait::async_trait]
impl personal_agent::services::ModelsRegistryService for NoopModelsRegistryService {
    async fn refresh(&self) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_model(
        &self,
        _provider: &str,
        _model: &str,
    ) -> ServiceResult<Option<personal_agent::registry::ModelInfo>> {
        Ok(None)
    }

    async fn get_provider(
        &self,
        _provider: &str,
    ) -> ServiceResult<Vec<personal_agent::registry::ModelInfo>> {
        Ok(vec![])
    }

    async fn get_provider_api_url(&self, _provider: &str) -> ServiceResult<Option<String>> {
        Ok(None)
    }

    async fn list_providers(&self) -> ServiceResult<Vec<String>> {
        Ok(vec![])
    }

    async fn list_all(&self) -> ServiceResult<Vec<personal_agent::registry::ModelInfo>> {
        Ok(vec![])
    }

    async fn search(
        &self,
        _query: &str,
    ) -> ServiceResult<Vec<personal_agent::registry::ModelInfo>> {
        Ok(vec![])
    }

    async fn check_update(&self) -> ServiceResult<bool> {
        Ok(false)
    }

    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(None)
    }
}

struct NoopMcpRegistryService;

#[derive(Clone, Default)]
struct RecordingProfileService {
    created: Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
    #[allow(clippy::type_complexity)]
    updated: Arc<std::sync::Mutex<Vec<(uuid::Uuid, Option<String>, Option<String>)>>>,
}

impl RecordingProfileService {
    fn created_calls(&self) -> Vec<(String, String, String)> {
        self.created
            .lock()
            .expect("recording lock poisoned")
            .clone()
    }

    fn updated_calls(&self) -> Vec<(uuid::Uuid, Option<String>, Option<String>)> {
        self.updated
            .lock()
            .expect("recording lock poisoned")
            .clone()
    }
}

#[async_trait::async_trait]
impl personal_agent::services::ProfileService for RecordingProfileService {
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        Ok(vec![])
    }

    async fn get(&self, _id: uuid::Uuid) -> ServiceResult<ModelProfile> {
        Err(ServiceError::NotFound("noop".into()))
    }

    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        _base_url: Option<String>,
        _auth: AuthConfig,
        _parameters: ModelParameters,
        _system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        self.created.lock().expect("recording lock poisoned").push((
            name.clone(),
            provider.clone(),
            model.clone(),
        ));

        Ok(ModelProfile {
            id: uuid::Uuid::new_v4(),
            name,
            provider_id: provider,
            model_id: model,
            base_url: "https://api.openai.com/v1".to_string(),
            auth: AuthConfig::Keychain {
                label: String::new(),
            },
            parameters: ModelParameters::default(),
            system_prompt: personal_agent::models::profile::DEFAULT_SYSTEM_PROMPT.to_string(),
        })
    }

    async fn update(
        &self,
        id: uuid::Uuid,
        name: Option<String>,
        _provider: Option<String>,
        model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _parameters: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        self.updated
            .lock()
            .expect("recording lock poisoned")
            .push((id, name, model));

        Err(ServiceError::NotFound("noop".into()))
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

#[async_trait::async_trait]
impl personal_agent::services::McpRegistryService for NoopMcpRegistryService {
    async fn search(&self, _query: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }
    async fn search_registry(
        &self,
        _query: &str,
        _source: &str,
    ) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }
    async fn get_details(&self, _name: &str) -> ServiceResult<Option<McpRegistryEntry>> {
        Ok(None)
    }
    async fn list_all(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }
    async fn list_by_tag(&self, _tag: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }
    async fn list_trending(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }
    async fn refresh(&self) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(None)
    }
    async fn install(&self, _name: &str, _config_name: Option<String>) -> ServiceResult<()> {
        Ok(())
    }
}

struct NoopMcpService;

#[async_trait::async_trait]
impl personal_agent::services::McpService for NoopMcpService {
    async fn list(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
        Ok(vec![])
    }
    async fn get(&self, _id: uuid::Uuid) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Ok(serdes_ai_mcp::McpServerConfig {
            name: "noop".to_string(),
            transport: serdes_ai_mcp::McpTransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![],
            },
        })
    }
    async fn get_status(&self, _id: uuid::Uuid) -> ServiceResult<McpServerStatus> {
        Ok(McpServerStatus::Disconnected)
    }
    async fn set_enabled(&self, _id: uuid::Uuid, _enabled: bool) -> ServiceResult<()> {
        Ok(())
    }
    async fn get_available_tools(&self, _id: uuid::Uuid) -> ServiceResult<Vec<McpTool>> {
        Ok(vec![])
    }
    async fn add(
        &self,
        name: String,
        command: String,
        args: Vec<String>,
        _env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Ok(serdes_ai_mcp::McpServerConfig {
            name,
            transport: serdes_ai_mcp::McpTransportConfig::Stdio { command, args },
        })
    }
    async fn update(
        &self,
        _id: uuid::Uuid,
        _name: Option<String>,
        _command: Option<String>,
        _args: Option<Vec<String>>,
        _env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Err(ServiceError::NotFound("noop".into()))
    }
    async fn delete(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }
    async fn restart(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }
    async fn list_enabled(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
        Ok(vec![])
    }
    async fn get_all_tools(&self) -> ServiceResult<Vec<(uuid::Uuid, McpTool)>> {
        Ok(vec![])
    }
}

// ============================================================
// Tests: Forwarder transport layer

#[derive(Clone, Default)]
struct RecordingMcpRegistryService {
    entries: Arc<std::sync::Mutex<Vec<McpRegistryEntry>>>,
}

impl RecordingMcpRegistryService {
    fn with_entries(entries: Vec<McpRegistryEntry>) -> Self {
        Self {
            entries: Arc::new(std::sync::Mutex::new(entries)),
        }
    }
}

#[async_trait::async_trait]
impl personal_agent::services::McpRegistryService for RecordingMcpRegistryService {
    async fn search(&self, _query: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(self.entries.lock().expect("registry lock poisoned").clone())
    }

    async fn search_registry(&self, _query: &str, _source: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(self.entries.lock().expect("registry lock poisoned").clone())
    }

    async fn get_details(&self, _name: &str) -> ServiceResult<Option<McpRegistryEntry>> {
        Ok(None)
    }

    async fn list_all(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(self.entries.lock().expect("registry lock poisoned").clone())
    }

    async fn list_by_tag(&self, _tag: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }

    async fn list_trending(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        Ok(vec![])
    }

    async fn refresh(&self) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(None)
    }

    async fn install(&self, _name: &str, _config_name: Option<String>) -> ServiceResult<()> {
        Ok(())
    }
}

// ============================================================

/// REQ-WIRE-001: `spawn_user_event_forwarder` delivers `OpenModelSelector` to `EventBus`
///
/// GIVEN: active runtime with forwarder and `EventBus` subscriber
/// WHEN:  `OpenModelSelector` is emitted through the flume channel
/// THEN:  `EventBus` subscriber receives `AppEvent::User(OpenModelSelector)`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_forwarder_delivers_open_model_selector_to_event_bus() {
    let event_bus = Arc::new(EventBus::new(32));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let mut bus_rx = event_bus.subscribe();

    let _fwd = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    user_tx.send(UserEvent::OpenModelSelector).unwrap();

    let received = tokio::time::timeout(Duration::from_millis(200), bus_rx.recv())
        .await
        .expect("timed out waiting for EventBus delivery")
        .expect("EventBus channel closed");

    assert!(
        matches!(received, AppEvent::User(UserEvent::OpenModelSelector)),
        "EventBus must receive the forwarded OpenModelSelector event, got {received:?}"
    );
}

// ============================================================
// Tests: Mismatch Hotspot – SaveProfileEditor
// ============================================================

/// REQ-WIRE-001: `ProfileEditorPresenter` handles `SaveProfileEditor` via `EventBus`
///
/// GIVEN: `ProfileEditorPresenter` subscribed to `EventBus` with a `view_tx` receiver
/// WHEN:  `AppEvent::User(SaveProfileEditor)` is published to `EventBus`
/// THEN:  `ProfileEditorPresenter` emits at least one `ViewCommand` in response
///        (indicating it reacted to the event rather than silently dropping it)
///
/// This test exposes the mismatch: `ProfileEditorPresenter` currently handles
/// `SaveProfile` but NOT `SaveProfileEditor`, so it silently drops the event.
/// The test MUST FAIL pre-implementation.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_profile_editor_presenter_handles_save_profile_editor() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let profile_service: Arc<dyn personal_agent::services::ProfileService> =
        Arc::new(NoopProfileService);

    let mut presenter = personal_agent::presentation::ProfileEditorPresenter::new(
        profile_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");

    // Give the event loop a moment to start
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Publish the mismatch hotspot event directly to the sender the presenter subscribed to
    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveProfileEditor))
        .ok();

    // Allow presenter to process
    tokio::time::sleep(Duration::from_millis(150)).await;

    // ASSERTION: presenter must have emitted at least one ViewCommand in reaction.
    // Pre-implementation this assertion fails because the event is dropped silently.
    let cmd = view_rx.try_recv().ok();
    assert!(
        cmd.is_some(),
        "ProfileEditorPresenter must emit a ViewCommand when it receives \
         UserEvent::SaveProfileEditor; currently it silently drops the event"
    );
}

// ============================================================
// Tests: Mismatch Hotspot – McpAddNext
// ============================================================

/// REQ-WIRE-001: `McpAddPresenter` handles `McpAddNext` via `EventBus`
///
/// GIVEN: `McpAddPresenter` subscribed to `EventBus` with a `view_tx` receiver
/// WHEN:  `AppEvent::User(McpAddNext)` is published to `EventBus`
/// THEN:  `McpAddPresenter` emits at least one `ViewCommand` in response
///
/// This test exposes the mismatch: `McpAddPresenter` currently handles
/// `SearchMcpRegistry` and `SelectMcpFromRegistry` but NOT `McpAddNext`.
/// The test MUST FAIL pre-implementation.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_add_presenter_handles_mcp_add_next() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_registry_service: Arc<dyn personal_agent::services::McpRegistryService> =
        Arc::new(NoopMcpRegistryService);

    let mut presenter = personal_agent::presentation::McpAddPresenter::new(
        mcp_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::McpAddNext {
            manual_entry: Some("npx -y @modelcontextprotocol/server-fetch".to_string()),
        }))
        .ok();

    tokio::time::sleep(Duration::from_millis(150)).await;

    let cmd = view_rx.try_recv().ok();
    assert!(
        cmd.is_some(),
        "McpAddPresenter must emit a ViewCommand when it receives \
         UserEvent::McpAddNext; currently it silently drops the event"
    );
}

// ============================================================
// Tests: End-to-end forwarder path for all three mismatch events
// ============================================================

/// REQ-WIRE-001: Full path – GPUI bridge emits `SaveProfileEditor` → `EventBus` receives it
///
/// GIVEN: active forwarder bridging flume → `EventBus`
/// WHEN:  `GpuiBridge` emits `UserEvent::SaveProfileEditor`
/// THEN:  `EventBus` subscriber receives `AppEvent::User(SaveProfileEditor)`
///
/// This verifies the transport layer is intact regardless of presenter handling.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_full_path_save_profile_editor_reaches_event_bus() {
    let event_bus = Arc::new(EventBus::new(32));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (_view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let mut bus_rx = event_bus.subscribe();

    let bridge = GpuiBridge::new(user_tx, view_rx);
    let _fwd = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    bridge.emit(UserEvent::SaveProfileEditor);

    let received = tokio::time::timeout(Duration::from_millis(200), bus_rx.recv())
        .await
        .expect("timed out")
        .expect("bus closed");

    assert!(
        matches!(received, AppEvent::User(UserEvent::SaveProfileEditor)),
        "Expected SaveProfileEditor on EventBus, got {received:?}"
    );
}

/// REQ-WIRE-001: Full path – GPUI bridge emits `McpAddNext` → `EventBus` receives it
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_full_path_mcp_add_next_reaches_event_bus() {
    let event_bus = Arc::new(EventBus::new(32));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (_view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let mut bus_rx = event_bus.subscribe();

    let bridge = GpuiBridge::new(user_tx, view_rx);
    let _fwd = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    bridge.emit(UserEvent::McpAddNext {
        manual_entry: Some("npx -y @modelcontextprotocol/server-fetch".to_string()),
    });

    let received = tokio::time::timeout(Duration::from_millis(200), bus_rx.recv())
        .await
        .expect("timed out")
        .expect("bus closed");

    assert!(
        matches!(
            received,
            AppEvent::User(UserEvent::McpAddNext {
                manual_entry: Some(ref entry)
            }) if entry == "npx -y @modelcontextprotocol/server-fetch"
        ),
        "Expected McpAddNext on EventBus, got {received:?}"
    );
}

/// REQ-WIRE-001: `SaveProfileEditor` persists selected model from prior `SelectModel` event
///
/// GIVEN: `ProfileEditorPresenter` subscribed to `EventBus` with a recording profile service
/// WHEN:  `SelectModel` is published, followed by `SaveProfileEditor`
/// THEN:  `ProfileService::create` is called with provider/model from `SelectModel`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_profile_editor_save_uses_selected_model_context() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, _view_rx) = broadcast::channel::<ViewCommand>(32);

    let recording = RecordingProfileService::default();
    let profile_service: Arc<dyn personal_agent::services::ProfileService> =
        Arc::new(recording.clone());

    let mut presenter = personal_agent::presentation::ProfileEditorPresenter::new(
        profile_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SelectModel {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-sonnet".to_string(),
        }))
        .ok();

    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveProfileEditor))
        .ok();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let calls = recording.created_calls();
    assert_eq!(
        calls.len(),
        1,
        "SaveProfileEditor should persist exactly one profile"
    );

    let (name, provider, model) = &calls[0];
    assert_eq!(
        provider, "anthropic",
        "provider should come from SelectModel"
    );
    assert_eq!(
        model, "claude-3-5-sonnet",
        "model should come from SelectModel"
    );
    assert_eq!(
        name, model,
        "lightweight save currently uses model id as profile name"
    );
}

/// REQ-WIRE-001: `SaveProfile` payload attempts update first, then falls back to create
///
/// GIVEN: `ProfileEditorPresenter` with `RecordingProfileService` where update returns `NotFound`
/// WHEN:  `SaveProfile` user event is published with explicit profile id/name
/// THEN:  presenter calls update once with payload id/name, then creates one profile fallback
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_profile_editor_save_profile_attempts_update_then_falls_back_to_create() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, _view_rx) = broadcast::channel::<ViewCommand>(32);

    let recording = RecordingProfileService::default();
    let profile_service: Arc<dyn personal_agent::services::ProfileService> =
        Arc::new(recording.clone());

    let mut presenter = personal_agent::presentation::ProfileEditorPresenter::new(
        profile_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let profile_id = uuid::Uuid::new_v4();
    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveProfile {
            profile: personal_agent::events::types::ModelProfile {
                id: profile_id,
                name: "Edited Profile".to_string(),
                provider_id: None,
                model_id: None,
                base_url: None,
                auth: None,
                parameters: None,
                system_prompt: None,
            },
        }))
        .ok();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let updates = recording.updated_calls();
    assert_eq!(
        updates.len(),
        1,
        "SaveProfile should attempt one update first"
    );
    assert_eq!(updates[0].0, profile_id, "update should target payload id");
    assert_eq!(
        updates[0].1.as_deref(),
        Some("Edited Profile"),
        "update should carry payload name"
    );

    let creates = recording.created_calls();
    assert_eq!(
        creates.len(),
        1,
        "SaveProfile should fallback to one create on NotFound"
    );
    let (created_name, created_provider, created_model) = &creates[0];
    assert_eq!(
        created_name, "Edited Profile",
        "fallback create should preserve payload name"
    );
    assert_eq!(
        created_provider, "openai",
        "fallback create uses current default provider"
    );
    assert_eq!(
        created_model, "gpt-4o",
        "fallback create uses current default model"
    );
}

/// REQ-WIRE-001: `ModelSelectorPresenter` emits `ModelSelected` and NavigateTo(ProfileEditor)
///
/// GIVEN: `ModelSelectorPresenter` subscribed to `EventBus` with view command receiver
/// WHEN:  `SelectModel` user event is published to `EventBus`
/// THEN:  presenter emits `ModelSelected` followed by NavigateTo(ProfileEditor)
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_model_selector_presenter_emits_prefill_and_navigation_on_select_model() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let models_registry_service: Arc<dyn personal_agent::services::ModelsRegistryService> =
        Arc::new(NoopModelsRegistryService);

    let mut presenter = personal_agent::presentation::ModelSelectorPresenter::new(
        models_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SelectModel {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-5-sonnet".to_string(),
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(200), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(200), view_rx.recv())
        .await
        .expect("timed out waiting for second command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::ModelSelected {
                ref provider_id,
                ref model_id,
                provider_api_url: _,
                context_length: _
            } if provider_id == "anthropic" && model_id == "claude-3-5-sonnet"
        ),
        "first command should be ModelSelected with selected provider/model, got {first:?}"
    );

    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo {
                view: personal_agent::presentation::view_command::ViewId::ProfileEditor
            }
        ),
        "second command should navigate to ProfileEditor, got {second:?}"
    );
}

/// REQ-WIRE-001: `ModelSelectorPresenter` emits `ModelSearchResults` for `SearchModels` input
///
/// GIVEN: `ModelSelectorPresenter` subscribed to `EventBus` with view command receiver
/// WHEN:  `SearchModels` user event is published
/// THEN:  presenter emits `ModelSearchResults` (possibly empty list)
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_model_selector_presenter_handles_search_models() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let models_registry_service: Arc<dyn personal_agent::services::ModelsRegistryService> =
        Arc::new(NoopModelsRegistryService);

    let mut presenter = personal_agent::presentation::ModelSelectorPresenter::new(
        models_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SearchModels {
            query: "claude".to_string(),
        }))
        .ok();

    let cmd = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for search command")
        .expect("view channel closed");

    assert!(
        matches!(cmd, ViewCommand::ModelSearchResults { .. }),
        "SearchModels should emit ModelSearchResults, got {cmd:?}"
    );
}

/// REQ-WIRE-001: `ModelSelectorPresenter` emits `ModelSearchResults` for provider filter input
///
/// GIVEN: `ModelSelectorPresenter` subscribed to `EventBus` with view command receiver
/// WHEN:  `FilterModelsByProvider` user event is published
/// THEN:  presenter emits `ModelSearchResults` (possibly empty list)
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_model_selector_presenter_handles_filter_models_by_provider() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let models_registry_service: Arc<dyn personal_agent::services::ModelsRegistryService> =
        Arc::new(NoopModelsRegistryService);

    let mut presenter = personal_agent::presentation::ModelSelectorPresenter::new(
        models_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::FilterModelsByProvider {
            provider_id: Some("anthropic".to_string()),
        }))
        .ok();

    let cmd = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for filter command")
        .expect("view channel closed");

    assert!(
        matches!(cmd, ViewCommand::ModelSearchResults { .. }),
        "FilterModelsByProvider should emit ModelSearchResults, got {cmd:?}"
    );
}

#[derive(Clone, Default)]
struct RecordingMcpService;

impl RecordingMcpService {
    fn with_ids(_ids: Vec<uuid::Uuid>) -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl personal_agent::services::McpService for RecordingMcpService {
    async fn list(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
        Ok(vec![])
    }

    async fn get(&self, _id: uuid::Uuid) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Ok(serdes_ai_mcp::McpServerConfig {
            name: "noop".to_string(),
            transport: serdes_ai_mcp::McpTransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![],
            },
        })
    }

    async fn get_status(&self, _id: uuid::Uuid) -> ServiceResult<McpServerStatus> {
        Ok(McpServerStatus::Disconnected)
    }

    async fn set_enabled(&self, _id: uuid::Uuid, _enabled: bool) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_available_tools(&self, _id: uuid::Uuid) -> ServiceResult<Vec<McpTool>> {
        Ok(vec![])
    }

    async fn add(
        &self,
        name: String,
        command: String,
        args: Vec<String>,
        _env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Ok(serdes_ai_mcp::McpServerConfig {
            name,
            transport: serdes_ai_mcp::McpTransportConfig::Stdio { command, args },
        })
    }

    async fn update(
        &self,
        _id: uuid::Uuid,
        _name: Option<String>,
        _command: Option<String>,
        _args: Option<Vec<String>>,
        _env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Ok(serdes_ai_mcp::McpServerConfig {
            name: "updated".to_string(),
            transport: serdes_ai_mcp::McpTransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![],
            },
        })
    }

    async fn resolve_id_by_name(&self, name: &str) -> ServiceResult<Option<uuid::Uuid>> {
        if name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(uuid::Uuid::new_v4()))
        }
    }

    async fn delete(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn restart(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn list_enabled(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
        self.list().await
    }

    async fn get_all_tools(&self) -> ServiceResult<Vec<(uuid::Uuid, McpTool)>> {
        Ok(vec![])
    }
}

#[derive(Clone, Default)]
struct FailingMcpService;

#[async_trait::async_trait]
impl personal_agent::services::McpService for FailingMcpService {
    async fn list(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
        Ok(vec![])
    }

    async fn get(&self, _id: uuid::Uuid) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Err(ServiceError::NotFound("noop".into()))
    }

    async fn get_status(&self, _id: uuid::Uuid) -> ServiceResult<McpServerStatus> {
        Ok(McpServerStatus::Disconnected)
    }

    async fn set_enabled(&self, _id: uuid::Uuid, _enabled: bool) -> ServiceResult<()> {
        Ok(())
    }

    async fn get_available_tools(&self, _id: uuid::Uuid) -> ServiceResult<Vec<McpTool>> {
        Ok(vec![])
    }

    async fn add(
        &self,
        _name: String,
        _command: String,
        _args: Vec<String>,
        _env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Err(ServiceError::Internal("add failed".to_string()))
    }

    async fn update(
        &self,
        _id: uuid::Uuid,
        _name: Option<String>,
        _command: Option<String>,
        _args: Option<Vec<String>>,
        _env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
        Err(ServiceError::Internal("update failed".to_string()))
    }

    async fn resolve_id_by_name(&self, _name: &str) -> ServiceResult<Option<uuid::Uuid>> {
        Err(ServiceError::Internal("resolve failed".to_string()))
    }

    async fn delete(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn restart(&self, _id: uuid::Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn list_enabled(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
        self.list().await
    }

    async fn get_all_tools(&self) -> ServiceResult<Vec<(uuid::Uuid, McpTool)>> {
        Ok(vec![])
    }
}

/// REQ-WIRE-001: `McpAddPresenter` search emits `McpRegistrySearchResults` payload
///
/// GIVEN: `McpAddPresenter` with registry service returning one entry
/// WHEN:  `SearchMcpRegistry` event is published
/// THEN:  presenter emits `ViewCommand::McpRegistrySearchResults` with mapped result
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_add_presenter_search_emits_registry_results() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let entries = vec![McpRegistryEntry {
        name: "fetch".to_string(),
        display_name: "Fetch".to_string(),
        description: "HTTP fetch tools".to_string(),
        version: "1.0.0".to_string(),
        author: "MCP Team".to_string(),
        license: "MIT".to_string(),
        repository: "https://github.com/modelcontextprotocol/servers".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-fetch".to_string(),
        ],
        env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
        tags: vec![],
        source: "official".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }];


    let mcp_registry_service: Arc<dyn personal_agent::services::McpRegistryService> =
        Arc::new(RecordingMcpRegistryService::with_entries(entries));

    let mut presenter = personal_agent::presentation::McpAddPresenter::new(
        mcp_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SearchMcpRegistry {
            query: "fetch".to_string(),
            source: personal_agent::events::types::McpRegistrySource {
                name: "official".to_string(),
            },
        }))
        .ok();

    let cmd = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for search results command")
        .expect("view channel closed");

    let expected = vec![McpRegistryResult {
        id: "fetch".to_string(),
        name: "Fetch".to_string(),
        description: "HTTP fetch tools".to_string(),
        source: "official".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-fetch".to_string(),
        ],
        env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }];

    assert!(
        matches!(cmd, ViewCommand::McpRegistrySearchResults { ref results } if results == &expected),
        "SearchMcpRegistry should emit mapped McpRegistrySearchResults, got {cmd:?}"
    );
}

/// REQ-WIRE-001: `SearchMcpRegistry` preserves requested registry source in projected results
///
/// GIVEN: `McpAddPresenter` with registry entries
/// WHEN:  `SearchMcpRegistry` is published with source 'smithery'
/// THEN:  presenter emits `McpRegistrySearchResults` where each result.source == 'smithery'
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_add_presenter_search_preserves_requested_source_in_results() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let entries = vec![McpRegistryEntry {
        name: "filesystem".to_string(),
        display_name: "Filesystem".to_string(),
        description: "Filesystem access tools".to_string(),
        version: "1.0.0".to_string(),
        author: "MCP Team".to_string(),
        license: "MIT".to_string(),
        repository: "https://github.com/modelcontextprotocol/servers".to_string(),
        command: "npx -y @modelcontextprotocol/server-filesystem".to_string(),
        args: vec![],
        env: None,
        tags: vec![],
        source: "smithery".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }];

    let mcp_registry_service: Arc<dyn personal_agent::services::McpRegistryService> =
        Arc::new(RecordingMcpRegistryService::with_entries(entries));

    let mut presenter = personal_agent::presentation::McpAddPresenter::new(
        mcp_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SearchMcpRegistry {
            query: "filesystem".to_string(),
            source: personal_agent::events::types::McpRegistrySource {
                name: "smithery".to_string(),
            },
        }))
        .ok();

    let cmd = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for search results command")
        .expect("view channel closed");

    assert!(
        matches!(
            cmd,
            ViewCommand::McpRegistrySearchResults { ref results }
                if results.len() == 1 && results[0].source == "smithery"
        ),
        "SearchMcpRegistry should preserve requested source in projected results, got {cmd:?}"
    );
}

/// REQ-WIRE-001: `SelectMcpFromRegistry` emits configure draft + navigation
///
/// GIVEN: `McpAddPresenter` with registry containing the selected MCP entry
/// WHEN:  `SelectMcpFromRegistry` is published for that entry id
/// THEN:  presenter emits `McpConfigureDraftLoaded` followed by NavigateTo(McpConfigure)
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_add_presenter_select_emits_configure_draft_and_navigate() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let entries = vec![McpRegistryEntry {
        name: "fetch".to_string(),
        display_name: "Fetch".to_string(),
        description: "HTTP fetch tools".to_string(),
        version: "1.0.0".to_string(),
        author: "MCP Team".to_string(),
        license: "MIT".to_string(),
        repository: "https://github.com/modelcontextprotocol/servers".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-fetch".to_string(),
        ],
        env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
        tags: vec![],
        source: "official".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }];

    let mcp_registry_service: Arc<dyn personal_agent::services::McpRegistryService> =
        Arc::new(RecordingMcpRegistryService::with_entries(entries));

    let mut presenter = personal_agent::presentation::McpAddPresenter::new(
        mcp_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SelectMcpFromRegistry {
            source: personal_agent::events::types::McpRegistrySource {
                name: "fetch".to_string(),
            },
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for second command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigureDraftLoaded {
                ref id,
                ref name,
                ref package,
                ref env_var_name,
                ref command,
                ref args,
                ..
            } if id == "official::fetch"
                && name == "Fetch"
                && package == "fetch"
                && env_var_name == "FETCH_API_KEY"
                && command == "npx"
                && args == &vec!["-y".to_string(), "@modelcontextprotocol/server-fetch".to_string()]
        ),
        "first command should be a configure draft for selected MCP, got {first:?}"
    );

    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo { view }
                if view == personal_agent::presentation::view_command::ViewId::McpConfigure
        ),
        "second command should navigate to McpConfigure, got {second:?}"
    );
}

/// REQ-WIRE-001: `SelectMcpFromRegistry` preserves explicit source hint in configure draft id
///
/// GIVEN: `McpAddPresenter` with registry containing selected MCP entry
/// WHEN:  `SelectMcpFromRegistry` is published with source '`smithery::filesystem`'
/// THEN:  presenter emits `McpConfigureDraftLoaded` where id encodes source and package as '`smithery::filesystem`'
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_add_presenter_select_preserves_source_hint_in_configure_draft_id() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let entries = vec![McpRegistryEntry {
        name: "filesystem".to_string(),
        display_name: "Filesystem".to_string(),
        description: "Filesystem tools".to_string(),
        version: "1.0.0".to_string(),
        author: "MCP Team".to_string(),
        license: "MIT".to_string(),
        repository: "https://github.com/modelcontextprotocol/servers".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
        ],
        env: Some(vec![("FILESYSTEM_ROOT".to_string(), "/tmp".to_string())]),
        tags: vec![],
        source: "smithery".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }];

    let mcp_registry_service: Arc<dyn personal_agent::services::McpRegistryService> =
        Arc::new(RecordingMcpRegistryService::with_entries(entries));

    let mut presenter = personal_agent::presentation::McpAddPresenter::new(
        mcp_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SelectMcpFromRegistry {
            source: personal_agent::events::types::McpRegistrySource {
                name: "smithery::filesystem".to_string(),
            },
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigureDraftLoaded {
                ref id,
                ref package,
                ..
            } if id == "smithery::filesystem" && package == "filesystem"
        ),
        "expected source-hinted configure draft id, got {first:?}"
    );
}

/// REQ-WIRE-001: `SelectMcpFromRegistry` missing entry surfaces source name in `ShowError`
///
/// GIVEN: `McpAddPresenter` with registry that does not include selected source
/// WHEN:  `SelectMcpFromRegistry` is published for unknown source name
/// THEN:  presenter emits `ShowError` containing the requested source name
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_add_presenter_select_missing_entry_emits_show_error_with_source_name() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let entries = vec![McpRegistryEntry {
        name: "fetch".to_string(),
        display_name: "Fetch".to_string(),
        description: "HTTP fetch tools".to_string(),
        version: "1.0.0".to_string(),
        author: "MCP Team".to_string(),
        license: "MIT".to_string(),
        repository: "https://github.com/modelcontextprotocol/servers".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-fetch".to_string(),
        ],
        env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
        tags: vec![],
        source: "official".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }];

    let mcp_registry_service: Arc<dyn personal_agent::services::McpRegistryService> =
        Arc::new(RecordingMcpRegistryService::with_entries(entries));

    let mut presenter = personal_agent::presentation::McpAddPresenter::new(
        mcp_registry_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::SelectMcpFromRegistry {
            source: personal_agent::events::types::McpRegistrySource {
                name: "missing-server".to_string(),
            },
        }))
        .ok();

    let cmd = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for ShowError")
        .expect("view channel closed");

    assert!(
        matches!(
            cmd,
            ViewCommand::ShowError { ref message, .. }
                if message.contains("missing-server")
        ),
        "SelectMcpFromRegistry missing source should emit ShowError with source name, got {cmd:?}"
    );
}

/// REQ-WIRE-001: `ConfigureMcp` loads persisted MCP and emits configure draft + navigation
///
/// GIVEN: `McpConfigurePresenter` with MCP service returning a stored config
/// WHEN:  `ConfigureMcp` event is published
/// THEN:  presenter emits `McpConfigureDraftLoaded` followed by NavigateTo(McpConfigure)
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_configure_presenter_configure_mcp_loads_draft_and_navigates() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> =
        Arc::new(RecordingMcpService::with_ids(vec![uuid::Uuid::new_v4()]));

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let configured_id = uuid::Uuid::new_v4();
    event_bus_sender
        .send(AppEvent::User(UserEvent::ConfigureMcp {
            id: configured_id,
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for draft command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for navigation command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigureDraftLoaded {
                ref id,
                ref name,
                ref command,
                ..
            } if id == &configured_id.to_string() && name == "noop" && command == "npx"
        ),
        "first command should be MCP configure draft loaded from service, got {first:?}"
    );
    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo { view }
                if view == personal_agent::presentation::view_command::ViewId::McpConfigure
        ),
        "second command should navigate to McpConfigure, got {second:?}"
    );
}

/// REQ-WIRE-001: `ConfigureMcp` load failure emits `ShowError` only
///
/// GIVEN: `McpConfigurePresenter` with MCP service failing get(id)
/// WHEN:  `ConfigureMcp` event is published
/// THEN:  presenter emits `ShowError` and does not navigate
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_configure_presenter_configure_mcp_failure_emits_error_only() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> = Arc::new(FailingMcpService);

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    event_bus_sender
        .send(AppEvent::User(UserEvent::ConfigureMcp {
            id: uuid::Uuid::new_v4(),
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for error command")
        .expect("view channel closed");

    assert!(
        matches!(first, ViewCommand::ShowError { .. }),
        "first command should be ShowError when ConfigureMcp load fails, got {first:?}"
    );

    let no_second = tokio::time::timeout(Duration::from_millis(120), view_rx.recv()).await;
    assert!(
        no_second.is_err(),
        "ConfigureMcp failure should not emit a follow-up navigation command"
    );
}

/// REQ-WIRE-001: `SaveMcpConfig` persists MCP fields and emits `McpConfigSaved` + Settings navigation
///
/// GIVEN: `McpConfigurePresenter` with MCP service update path available
/// WHEN:  `SaveMcpConfig` event is published
/// THEN:  presenter emits `McpConfigSaved` then navigates to `Settings`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_configure_presenter_save_mcp_config_emits_saved_then_navigate_to_settings() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> =
        Arc::new(RecordingMcpService::with_ids(vec![uuid::Uuid::new_v4()]));

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    )
    .with_config_path(temp_config_path());

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let id = uuid::Uuid::new_v4();
    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveMcpConfig {
            id,
            config: Box::new(test_rich_mcp_config(id, "Fetch")),
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for second command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigSaved {
                id: saved_id,
                name: Some(_),
            } if saved_id == id
        ),
        "first command should be McpConfigSaved with name payload, got {first:?}"
    );
    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo {
                view: personal_agent::presentation::view_command::ViewId::Settings
            }
        ),
        "second command should navigate to Settings, got {second:?}"
    );
}

/// REQ-WIRE-001: `SaveMcpConfig` create path (nil id) emits `McpConfigSaved` with name and Settings navigation
///
/// GIVEN: `McpConfigurePresenter` and `SaveMcpConfig` payload with nil id
/// WHEN:  `SaveMcpConfig` event is published
/// THEN:  presenter uses create path and emits McpConfigSaved(name) then navigates to `Settings`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_configure_presenter_save_mcp_config_nil_id_emits_saved_then_navigate_to_settings() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> =
        Arc::new(RecordingMcpService::with_ids(vec![]));

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    )
    .with_config_path(temp_config_path());

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let id = uuid::Uuid::nil();
    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveMcpConfig {
            id,
            config: Box::new(test_rich_mcp_config(id, "Registry Fetch")),
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for second command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigSaved {
                id: saved_id,
                name: Some(ref n),
            } if saved_id != uuid::Uuid::nil() && n == "Registry Fetch"
        ),
        "first command should be McpConfigSaved with non-nil id and saved name, got {first:?}"
    );
    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo {
                view: personal_agent::presentation::view_command::ViewId::Settings
            }
        ),
        "second command should navigate to Settings, got {second:?}"
    );
}

/// REQ-WIRE-001: `SaveMcpConfig` failure emits `ShowError` and does not navigate back
///
/// GIVEN: `McpConfigurePresenter` with update path failing
/// WHEN:  `SaveMcpConfig` event is published for non-nil id
/// THEN:  presenter emits `ShowError` and does not emit `NavigateBack`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_configure_presenter_save_mcp_config_failure_emits_error_only() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> = Arc::new(NoopMcpService);

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    )
    .with_config_path(broken_config_path());

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let id = uuid::Uuid::new_v4();
    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveMcpConfig {
            id,
            config: Box::new(test_rich_mcp_config(id, "Broken MCP")),
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");

    assert!(
        matches!(first, ViewCommand::ShowError { .. }),
        "first command should be ShowError for failed SaveMcpConfig, got {first:?}"
    );

    let maybe_second = tokio::time::timeout(Duration::from_millis(120), view_rx.recv()).await;
    assert!(
        maybe_second.is_err(),
        "SaveMcpConfig failure should not emit a follow-up NavigateBack command"
    );
}

/// REQ-WIRE-001: `SaveMcpConfig` create path keeps command payload from configure draft
///
/// GIVEN: `McpConfigurePresenter` with nil-id create save request
/// WHEN:  `SaveMcpConfig` is published with command/args payload
/// THEN:  presenter emits `McpConfigSaved` + Settings navigation (create flow accepts payload)
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
async fn test_mcp_configure_presenter_save_mcp_config_nil_id_with_command_payload_navigates_to_settings() {
    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> =
        Arc::new(RecordingMcpService::with_ids(vec![]));

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    )
    .with_config_path(temp_config_path());

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let id = uuid::Uuid::nil();
    event_bus_sender
        .send(AppEvent::User(UserEvent::SaveMcpConfig {
            id,
            config: Box::new(test_rich_mcp_config(id, "Filesystem")),
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for first command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for second command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigSaved {
                id: saved_id,
                name: Some(ref n),
            } if saved_id != uuid::Uuid::nil() && n == "Filesystem"
        ),
        "first command should be McpConfigSaved with non-nil id and preserved name, got {first:?}"
    );
    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo {
                view: personal_agent::presentation::view_command::ViewId::Settings
            }
        ),
        "second command should navigate to Settings, got {second:?}"
    );
}

/// REQ-WIRE-001: `ConfigureMcp` maps SSE transport URL into configure draft command field
///
/// GIVEN: `McpConfigurePresenter` and MCP service returning SSE transport
/// WHEN:  `ConfigureMcp` event is published
/// THEN:  presenter emits draft with command=url and empty args, then `NavigateTo`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-001
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn test_mcp_configure_presenter_configure_mcp_maps_sse_transport_to_command() {
    struct SseMcpService;

    #[async_trait::async_trait]
    impl personal_agent::services::McpService for SseMcpService {
        async fn list(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
            Ok(vec![])
        }

        async fn get(&self, _id: uuid::Uuid) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
            Ok(serdes_ai_mcp::McpServerConfig {
                name: "sse-server".to_string(),
                transport: serdes_ai_mcp::McpTransportConfig::Sse {
                    url: "https://mcp.example.com/sse".to_string(),
                },
            })
        }

        async fn get_status(&self, _id: uuid::Uuid) -> ServiceResult<McpServerStatus> {
            Ok(McpServerStatus::Disconnected)
        }

        async fn set_enabled(&self, _id: uuid::Uuid, _enabled: bool) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_available_tools(&self, _id: uuid::Uuid) -> ServiceResult<Vec<McpTool>> {
            Ok(vec![])
        }

        async fn add(
            &self,
            _name: String,
            _command: String,
            _args: Vec<String>,
            _env: Option<Vec<(String, String)>>,
        ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
            Err(ServiceError::NotFound("noop".into()))
        }

        async fn update(
            &self,
            _id: uuid::Uuid,
            _name: Option<String>,
            _command: Option<String>,
            _args: Option<Vec<String>>,
            _env: Option<Vec<(String, String)>>,
        ) -> ServiceResult<serdes_ai_mcp::McpServerConfig> {
            Err(ServiceError::NotFound("noop".into()))
        }

        async fn delete(&self, _id: uuid::Uuid) -> ServiceResult<()> {
            Ok(())
        }

        async fn restart(&self, _id: uuid::Uuid) -> ServiceResult<()> {
            Ok(())
        }

        async fn list_enabled(&self) -> ServiceResult<Vec<serdes_ai_mcp::McpServerConfig>> {
            Ok(vec![])
        }

        async fn get_all_tools(&self) -> ServiceResult<Vec<(uuid::Uuid, McpTool)>> {
            Ok(vec![])
        }
    }

    let event_bus_sender: broadcast::Sender<AppEvent> = broadcast::channel::<AppEvent>(32).0;
    let (view_tx, mut view_rx) = broadcast::channel::<ViewCommand>(32);

    let mcp_service: Arc<dyn personal_agent::services::McpService> = Arc::new(SseMcpService);

    let mut presenter = personal_agent::presentation::McpConfigurePresenter::new(
        mcp_service,
        &event_bus_sender,
        view_tx,
    );

    presenter
        .start()
        .await
        .expect("presenter start must succeed");
    tokio::time::sleep(Duration::from_millis(20)).await;

    let configured_id = uuid::Uuid::new_v4();
    event_bus_sender
        .send(AppEvent::User(UserEvent::ConfigureMcp {
            id: configured_id,
        }))
        .ok();

    let first = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for draft command")
        .expect("view channel closed");
    let second = tokio::time::timeout(Duration::from_millis(250), view_rx.recv())
        .await
        .expect("timed out waiting for navigation command")
        .expect("view channel closed");

    assert!(
        matches!(
            first,
            ViewCommand::McpConfigureDraftLoaded {
                ref id,
                ref name,
                ref command,
                ref url,
                ..
            } if id == &configured_id.to_string()
                && name == "sse-server"
                && command.is_empty()
                && *url == Some("https://mcp.example.com/sse".to_string())
        ),
        "first command should map SSE transport into configure draft url, got {first:?}"
    );
    assert!(
        matches!(
            second,
            ViewCommand::NavigateTo { view }
                if view == personal_agent::presentation::view_command::ViewId::McpConfigure
        ),
        "second command should navigate to McpConfigure, got {second:?}"
    );
}
