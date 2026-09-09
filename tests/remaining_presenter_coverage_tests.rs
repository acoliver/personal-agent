use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;
use uuid::Uuid;

fn test_rich_mcp_config(id: Uuid, name: &str) -> personal_agent::mcp::McpConfig {
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

/// Returns the `TempDir` guard alongside the path so the directory is
/// deleted when the test ends instead of leaking.
fn temp_config_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.json");
    let default_config = personal_agent::config::Config::default();
    let json = serde_json::to_string_pretty(&default_config).expect("serialize default config");
    std::fs::write(&path, json).expect("write default config");
    (dir, path)
}

use personal_agent::events::{
    bus::EventBus,
    types::{
        AppEvent, McpEvent, McpRegistrySource, ModelProfile as EventModelProfile, ModelProfileAuth,
        ModelProfileParameters, ProfileEvent, SystemEvent, UserEvent,
    },
};
use personal_agent::models::{AuthConfig, ModelParameters, ModelProfile};
use personal_agent::presentation::{
    api_key_manager_presenter::ApiKeyManagerPresenter,
    error_presenter::ErrorPresenter,
    mcp_add_presenter::McpAddPresenter,
    mcp_configure_presenter::McpConfigurePresenter,
    profile_editor_presenter::ProfileEditorPresenter,
    view_command::{ErrorSeverity, ViewCommand, ViewId},
};
use personal_agent::services::{
    secure_store, McpRegistryService, McpServerStatus, McpService, McpTool, ProfileService,
    ServiceError,
};
use serdes_ai_mcp::{McpServerConfig, McpTransportConfig};

struct MockProfileService {
    profiles: tokio::sync::Mutex<Vec<ModelProfile>>,
    create_result: tokio::sync::Mutex<Result<ModelProfile, ServiceError>>,
    update_result: tokio::sync::Mutex<Option<Result<ModelProfile, ServiceError>>>,
    test_connection_result: tokio::sync::Mutex<Result<(), ServiceError>>,
    last_create: tokio::sync::Mutex<Option<(String, String, String)>>,
}

impl MockProfileService {
    fn new(profiles: Vec<ModelProfile>) -> Self {
        let create_profile = profiles.first().cloned().unwrap_or_else(default_profile);
        Self {
            profiles: tokio::sync::Mutex::new(profiles),
            create_result: tokio::sync::Mutex::new(Ok(create_profile)),
            update_result: tokio::sync::Mutex::new(None),
            test_connection_result: tokio::sync::Mutex::new(Ok(())),
            last_create: tokio::sync::Mutex::new(None),
        }
    }

    async fn set_create_result(&self, result: Result<ModelProfile, ServiceError>) {
        *self.create_result.lock().await = result;
    }

    async fn set_update_result(&self, result: Result<ModelProfile, ServiceError>) {
        *self.update_result.lock().await = Some(result);
    }

    async fn set_test_connection_result(&self, result: Result<(), ServiceError>) {
        *self.test_connection_result.lock().await = result;
    }
}

#[async_trait]
impl ProfileService for MockProfileService {
    async fn list(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        Ok(self.profiles.lock().await.clone())
    }

    async fn get(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        self.profiles
            .lock()
            .await
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| ServiceError::NotFound(format!("profile {id} not found")))
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
    ) -> Result<ModelProfile, ServiceError> {
        *self.last_create.lock().await = Some((name, provider, model));
        self.create_result.lock().await.clone()
    }

    #[allow(clippy::significant_drop_tightening)]
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        provider: Option<String>,
        model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _parameters: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> Result<ModelProfile, ServiceError> {
        let update_result = self.update_result.lock().await.clone();
        if let Some(result) = update_result {
            return result;
        }

        let mut profiles = self.profiles.lock().await;
        let profile = profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or_else(|| ServiceError::NotFound("missing profile".to_string()))?;

        if let Some(name) = name {
            profile.name = name;
        }
        if let Some(provider) = provider {
            profile.provider_id = provider;
        }
        if let Some(model) = model {
            profile.model_id = model;
        }

        Ok(profile.clone())
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), ServiceError> {
        self.test_connection_result.lock().await.clone()
    }

    async fn get_default(&self) -> Result<Option<ModelProfile>, ServiceError> {
        Ok(self.profiles.lock().await.first().cloned())
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }
}

struct MockMcpRegistryService {
    search_result:
        tokio::sync::Mutex<Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError>>,
    list_all_result:
        tokio::sync::Mutex<Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError>>,
}

impl MockMcpRegistryService {
    fn new(
        search_result: Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError>,
        list_all_result: Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError>,
    ) -> Self {
        Self {
            search_result: tokio::sync::Mutex::new(search_result),
            list_all_result: tokio::sync::Mutex::new(list_all_result),
        }
    }
}

#[async_trait]
impl McpRegistryService for MockMcpRegistryService {
    async fn search(
        &self,
        _query: &str,
    ) -> Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError> {
        self.search_result.lock().await.clone()
    }

    async fn search_registry(
        &self,
        _query: &str,
        _source: &str,
    ) -> Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError> {
        self.search_result.lock().await.clone()
    }

    async fn get_details(
        &self,
        _name: &str,
    ) -> Result<Option<personal_agent::services::McpRegistryEntry>, ServiceError> {
        Ok(None)
    }

    async fn list_all(
        &self,
    ) -> Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError> {
        self.list_all_result.lock().await.clone()
    }

    async fn list_by_tag(
        &self,
        _tag: &str,
    ) -> Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError> {
        Ok(vec![])
    }

    async fn list_trending(
        &self,
    ) -> Result<Vec<personal_agent::services::McpRegistryEntry>, ServiceError> {
        Ok(vec![])
    }

    async fn refresh(&self) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_last_refresh(
        &self,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, ServiceError> {
        Ok(None)
    }

    async fn install(&self, _name: &str, _config_name: Option<String>) -> Result<(), ServiceError> {
        Ok(())
    }
}

struct MockMcpService {
    existing_config: tokio::sync::Mutex<Result<McpServerConfig, ServiceError>>,
    add_result: tokio::sync::Mutex<Result<McpServerConfig, ServiceError>>,
    updated_config: tokio::sync::Mutex<Result<McpServerConfig, ServiceError>>,
    resolved_id: tokio::sync::Mutex<Result<Option<Uuid>, ServiceError>>,
}

impl MockMcpService {
    fn new(config: McpServerConfig) -> Self {
        Self {
            existing_config: tokio::sync::Mutex::new(Ok(config.clone())),
            add_result: tokio::sync::Mutex::new(Ok(config.clone())),
            updated_config: tokio::sync::Mutex::new(Ok(config)),
            resolved_id: tokio::sync::Mutex::new(Ok(None)),
        }
    }

    async fn set_get_result(&self, result: Result<McpServerConfig, ServiceError>) {
        *self.existing_config.lock().await = result;
    }

    async fn set_update_result(&self, result: Result<McpServerConfig, ServiceError>) {
        *self.updated_config.lock().await = result;
    }

    async fn set_resolve_id_result(&self, result: Result<Option<Uuid>, ServiceError>) {
        *self.resolved_id.lock().await = result;
    }
}

#[async_trait]
impl McpService for MockMcpService {
    async fn list(&self) -> Result<Vec<McpServerConfig>, ServiceError> {
        Ok(vec![])
    }

    async fn get(&self, _id: Uuid) -> Result<McpServerConfig, ServiceError> {
        self.existing_config.lock().await.clone()
    }

    async fn get_status(&self, _id: Uuid) -> Result<McpServerStatus, ServiceError> {
        Ok(McpServerStatus::Disconnected)
    }

    async fn set_enabled(&self, _id: Uuid, _enabled: bool) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn get_available_tools(&self, _id: Uuid) -> Result<Vec<McpTool>, ServiceError> {
        Ok(vec![])
    }

    async fn add(
        &self,
        _name: String,
        _command: String,
        _args: Vec<String>,
        _env: Option<Vec<(String, String)>>,
    ) -> Result<McpServerConfig, ServiceError> {
        self.add_result.lock().await.clone()
    }

    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _command: Option<String>,
        _args: Option<Vec<String>>,
        _env: Option<Vec<(String, String)>>,
    ) -> Result<McpServerConfig, ServiceError> {
        self.updated_config.lock().await.clone()
    }

    async fn resolve_id_by_name(&self, _name: &str) -> Result<Option<Uuid>, ServiceError> {
        self.resolved_id.lock().await.clone()
    }

    async fn delete(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn restart(&self, _id: Uuid) -> Result<(), ServiceError> {
        Ok(())
    }

    async fn list_enabled(&self) -> Result<Vec<McpServerConfig>, ServiceError> {
        Ok(vec![])
    }

    async fn get_all_tools(&self) -> Result<Vec<(Uuid, McpTool)>, ServiceError> {
        Ok(vec![])
    }
}

fn default_profile() -> ModelProfile {
    ModelProfile::new(
        "Default".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "test-key".to_string(),
        },
    )
}

fn test_mcp_config(_id: Uuid) -> McpServerConfig {
    McpServerConfig {
        name: "Filesystem".to_string(),
        transport: McpTransportConfig::Stdio {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
        },
    }
}

fn registry_entry() -> personal_agent::services::McpRegistryEntry {
    personal_agent::services::McpRegistryEntry {
        name: "filesystem".to_string(),
        display_name: "Filesystem".to_string(),
        description: "Browse files".to_string(),
        version: "1.0.0".to_string(),
        author: "Test".to_string(),
        license: "MIT".to_string(),
        repository: "https://example.com".to_string(),
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "@test/filesystem".to_string()],
        env: Some(vec![("API_KEY".to_string(), "value".to_string())]),
        tags: vec!["files".to_string()],
        source: "official".to_string(),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    }
}

async fn collect_broadcast_commands(
    view_rx: &mut broadcast::Receiver<ViewCommand>,
) -> Vec<ViewCommand> {
    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
    let mut commands = Vec::new();
    while let Ok(command) = view_rx.try_recv() {
        commands.push(command);
    }
    commands
}

async fn collect_mpsc_commands(
    view_rx: &mut tokio::sync::mpsc::Receiver<ViewCommand>,
) -> Vec<ViewCommand> {
    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
    let mut commands = Vec::new();
    while let Ok(command) = view_rx.try_recv() {
        commands.push(command);
    }
    commands
}

#[tokio::test]
async fn api_key_manager_lists_keys_and_handles_store_delete_errors() {
    secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let _ = secure_store::api_keys::delete("used-label");
    let _ = secure_store::api_keys::delete("");
    secure_store::api_keys::store("used-label", "secret-1234").expect("store used label for test");

    let profile = ModelProfile::new(
        "Uses Key".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "used-label".to_string(),
        },
    );
    let profile_service = Arc::new(MockProfileService::new(vec![profile]));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(64);

    let mut presenter =
        ApiKeyManagerPresenter::new_with_event_bus(profile_service, &event_bus, view_tx);
    presenter.start().await.expect("start presenter");

    let startup_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(startup_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ApiKeysListed { keys }
            if keys.iter().any(|key| key.label == "used-label" && key.used_by == vec!["Uses Key".to_string()])
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::StoreApiKey {
            label: "new-label".to_string(),
            value: "stored-value".to_string(),
        }))
        .expect("publish store api key");
    let store_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(store_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ApiKeyStored { label } if label == "new-label"
    )));
    assert!(store_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ApiKeysListed { keys }
            if keys.iter().any(|key| key.label == "new-label")
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::DeleteApiKey {
            label: "new-label".to_string(),
        }))
        .expect("publish delete api key");
    let delete_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(delete_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ApiKeyDeleted { label } if label == "new-label"
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::DeleteApiKey {
            label: "missing-label".to_string(),
        }))
        .expect("publish delete missing api key");
    let delete_missing_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(delete_missing_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ApiKeyDeleted { label } if label == "missing-label"
    )));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn profile_editor_handles_save_paths_and_connection_feedback() {
    let mut existing = default_profile();
    existing.name = "Existing".to_string();
    let created_profile = {
        let mut profile = default_profile();
        profile.name = "claude-3-7-sonnet".to_string();
        profile
    };

    let profile_service = Arc::new(MockProfileService::new(vec![existing.clone()]));
    profile_service
        .set_create_result(Ok(created_profile.clone()))
        .await;
    let event_bus = Arc::new(EventBus::new(64));
    let mut event_rx = event_bus.subscribe();
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter =
        ProfileEditorPresenter::new_with_event_bus(profile_service.clone(), &event_bus, view_tx);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    let save_profile = EventModelProfile {
        id: existing.id,
        name: "Updated Existing".to_string(),
        provider_id: Some("anthropic".to_string()),
        model_id: Some("claude-3-7-sonnet".to_string()),
        base_url: Some("https://api.anthropic.com".to_string()),
        auth: Some(ModelProfileAuth::Keychain {
            label: "anthropic-key".to_string(),
        }),
        parameters: Some(ModelProfileParameters {
            temperature: Some(0.2),
            max_tokens: Some(2048),
            max_tokens_field_name: Some("max_tokens".to_string()),
            extra_request_fields: Some(serde_json::json!({})),

            show_thinking: Some(true),
            enable_thinking: Some(true),
            thinking_budget: Some(512),
            reasoning_effort: None,
            context_window_size: None,
        }),
        system_prompt: Some("Be helpful".to_string()),
    };

    event_bus
        .publish(AppEvent::User(UserEvent::SaveProfile {
            profile: Box::new(save_profile),
        }))
        .expect("publish save profile");
    let save_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(save_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ProfileUpdated { id, name }
            if *id == existing.id && name == "Updated Existing"
    )));
    assert!(save_commands.iter().any(|command| matches!(
        command,
        ViewCommand::NavigateTo {
            view: ViewId::Settings
        }
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SelectModel {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-7-sonnet".to_string(),
        }))
        .expect("publish select model");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SaveProfileEditor))
        .expect("publish save profile editor");
    let create_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(create_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ProfileCreated { id, name }
            if *id == created_profile.id && name == "claude-3-7-sonnet"
    )));
    assert!(create_commands.iter().any(|command| matches!(
        command,
        ViewCommand::DefaultProfileChanged { profile_id: Some(id) } if *id == created_profile.id
    )));

    let mut saw_created_event = false;
    let mut saw_default_changed_event = false;
    while let Ok(event) = event_rx.try_recv() {
        match event {
            AppEvent::Profile(ProfileEvent::Created { id, .. }) if id == created_profile.id => {
                saw_created_event = true;
            }
            AppEvent::Profile(ProfileEvent::DefaultChanged {
                profile_id: Some(id),
            }) if id == created_profile.id => {
                saw_default_changed_event = true;
            }
            _ => {}
        }
    }
    assert!(saw_created_event);
    assert!(saw_default_changed_event);

    let profile_id = existing.id;
    event_bus
        .publish(AppEvent::User(UserEvent::TestProfileConnection {
            id: profile_id,
        }))
        .expect("publish test connection success");
    let test_success = collect_broadcast_commands(&mut view_rx).await;
    assert!(test_success.iter().any(|command| matches!(
        command,
        ViewCommand::ProfileTestStarted { id } if *id == profile_id
    )));
    assert!(test_success.iter().any(|command| matches!(
        command,
        ViewCommand::ProfileTestCompleted {
            id,
            success: true,
            response_time_ms: None,
            error: None,
        } if *id == profile_id
    )));

    profile_service
        .set_test_connection_result(Err(ServiceError::Internal("network down".to_string())))
        .await;
    event_bus
        .publish(AppEvent::User(UserEvent::TestProfileConnection {
            id: profile_id,
        }))
        .expect("publish test connection failure");
    let test_failure = collect_broadcast_commands(&mut view_rx).await;
    assert!(test_failure.iter().any(|command| matches!(
        command,
        ViewCommand::ProfileTestCompleted {
            id,
            success: false,
            error: Some(error),
            ..
        } if *id == profile_id && error.contains("network down")
    )));

    event_bus
        .publish(AppEvent::Profile(ProfileEvent::TestCompleted {
            id: profile_id,
            success: false,
            response_time_ms: None,
            error: None,
        }))
        .expect("publish profile domain failure");
    let domain_failure = collect_broadcast_commands(&mut view_rx).await;
    assert!(domain_failure.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Connection Failed" && message == "Unknown error"
    )));
}

#[tokio::test]
async fn profile_editor_falls_back_to_create_and_surfaces_save_errors() {
    let existing = default_profile();
    let created_profile = {
        let mut profile = default_profile();
        profile.name = "Created Via Fallback".to_string();
        profile
    };

    let profile_service = Arc::new(MockProfileService::new(vec![existing.clone()]));
    profile_service
        .set_update_result(Err(ServiceError::NotFound("missing".to_string())))
        .await;
    profile_service
        .set_create_result(Ok(created_profile.clone()))
        .await;
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter =
        ProfileEditorPresenter::new_with_event_bus(profile_service.clone(), &event_bus, view_tx);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SaveProfile {
            profile: Box::new(EventModelProfile {
                id: existing.id,
                name: "Created Via Fallback".to_string(),
                provider_id: None,
                model_id: None,
                base_url: None,
                auth: None,
                parameters: None,
                system_prompt: None,
            }),
        }))
        .expect("publish save profile fallback");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ProfileUpdated { id, name }
            if *id == created_profile.id && name == "Created Via Fallback"
    )));

    profile_service
        .set_update_result(Err(ServiceError::Internal("write failed".to_string())))
        .await;
    event_bus
        .publish(AppEvent::User(UserEvent::SaveProfile {
            profile: Box::new(EventModelProfile {
                id: existing.id,
                name: "Broken".to_string(),
                provider_id: Some("openai".to_string()),
                model_id: Some("gpt-4o".to_string()),
                base_url: None,
                auth: None,
                parameters: None,
                system_prompt: None,
            }),
        }))
        .expect("publish save profile error");

    let error_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(error_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Save Failed" && message.contains("write failed")
    )));
}

#[tokio::test]
async fn error_presenter_surfaces_system_chat_and_mcp_errors() {
    let (event_tx, _) = broadcast::channel(64);
    let (view_tx, mut view_rx) = tokio::sync::mpsc::channel(64);
    let mut presenter = ErrorPresenter::new(&event_tx, view_tx);
    presenter.start().await.expect("start presenter");

    event_tx
        .send(AppEvent::System(SystemEvent::Error {
            source: "Config".to_string(),
            error: "broken".to_string(),
            context: Some("while loading".to_string()),
        }))
        .expect("send system error");
    event_tx
        .send(AppEvent::Chat(
            personal_agent::events::types::ChatEvent::StreamError {
                conversation_id: Uuid::new_v4(),
                error: "recoverable issue".to_string(),
                recoverable: true,
                diagnostics: None,
            },
        ))
        .expect("send chat error");
    event_tx
        .send(AppEvent::Mcp(McpEvent::StartFailed {
            id: Uuid::new_v4(),
            name: "Filesystem".to_string(),
            error: "spawn failed".to_string(),
        }))
        .expect("send mcp start failed");
    event_tx
        .send(AppEvent::Mcp(McpEvent::Unhealthy {
            id: Uuid::new_v4(),
            name: "Filesystem".to_string(),
            error: "heartbeat failed".to_string(),
        }))
        .expect("send mcp unhealthy");

    let commands = collect_mpsc_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Critical,
        } if title == "Config Error" && message.contains("while loading")
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Chat Error" && message == "recoverable issue"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "MCP Server Error" && message.contains("spawn failed")
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "MCP Server Unhealthy" && message.contains("heartbeat failed")
    )));
}

#[tokio::test]
async fn mcp_add_presenter_handles_search_selection_and_errors() {
    let registry_entry = registry_entry();
    let mcp_registry_service = Arc::new(MockMcpRegistryService::new(
        Ok(vec![registry_entry.clone()]),
        Ok(vec![registry_entry.clone()]),
    ));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter =
        McpAddPresenter::new_with_event_bus(mcp_registry_service, &event_bus, view_tx);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SearchMcpRegistry {
            query: "file".to_string(),
            source: McpRegistrySource {
                name: "official".to_string(),
            },
        }))
        .expect("publish search registry");
    let search_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(search_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpRegistrySearchResults { results }
            if results.iter().any(|result| result.id == "filesystem" && result.source == "official")
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SelectMcpFromRegistry {
            source: McpRegistrySource {
                name: "community::filesystem".to_string(),
            },
        }))
        .expect("publish select from registry");
    let select_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(select_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigureDraftLoaded {
            id,
            name,
            package,
            env_var_name,
            command,
            ..
        } if id == "community::filesystem"
            && name == "Filesystem"
            && package == "filesystem"
            && env_var_name == "API_KEY"
            && command == "npx"
    )));
    assert!(select_commands.iter().any(|command| matches!(
        command,
        ViewCommand::NavigateTo {
            view: ViewId::McpConfigure
        }
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::McpAddNext {
            manual_entry: Some("npx -y @test/filesystem".to_string()),
        }))
        .expect("publish mcp add next");
    let next_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(next_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigureDraftLoaded {
            package,
            package_type,
            command,
            ..
        } if package == "@test/filesystem"
            && *package_type == personal_agent::mcp::McpPackageType::Npm
            && command == "npx"
    )));
    assert!(next_commands.iter().any(|command| matches!(
        command,
        ViewCommand::NavigateTo {
            view: ViewId::McpConfigure
        }
    )));
}

#[tokio::test]
async fn mcp_add_presenter_surfaces_registry_failures() {
    let mcp_registry_service = Arc::new(MockMcpRegistryService::new(
        Err(ServiceError::Internal("search failed".to_string())),
        Err(ServiceError::Internal("load failed".to_string())),
    ));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter =
        McpAddPresenter::new_with_event_bus(mcp_registry_service, &event_bus, view_tx);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SearchMcpRegistry {
            query: "file".to_string(),
            source: McpRegistrySource {
                name: "official".to_string(),
            },
        }))
        .expect("publish search failure");
    let search_error = collect_broadcast_commands(&mut view_rx).await;
    assert!(search_error.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Search Failed" && message.contains("search failed")
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SelectMcpFromRegistry {
            source: McpRegistrySource {
                name: "community::filesystem".to_string(),
            },
        }))
        .expect("publish load failure");
    let load_error = collect_broadcast_commands(&mut view_rx).await;
    assert!(load_error.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Warning,
        } if title == "Load Failed" && message.contains("load failed")
    )));
}

#[tokio::test]
async fn mcp_configure_presenter_handles_load_save_oauth_and_domain_events() {
    let config_id = Uuid::new_v4();
    let resolved_id = Uuid::new_v4();
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(config_id)));
    mcp_service
        .set_resolve_id_result(Ok(Some(resolved_id)))
        .await;
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let (_config_dir, tmp_config) = temp_config_path();
    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::ConfigureMcp { id: config_id }))
        .expect("publish configure mcp");
    let configure_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(configure_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigureDraftLoaded {
            id,
            name,
            command,
            args,
            ..
        } if id == &config_id.to_string()
            && name == "Filesystem"
            && command == "npx"
            && args == &vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()]
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: Uuid::nil(),
            config: Box::new(test_rich_mcp_config(Uuid::nil(), "Filesystem")),
            secrets: vec![],
        }))
        .expect("publish save new mcp config");
    let create_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(create_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigSaved {
            id,
            name: Some(name),
        } if !id.is_nil() && name == "Filesystem"
    )));
    assert!(create_commands.iter().any(|command| matches!(
        command,
        ViewCommand::NavigateTo {
            view: ViewId::Settings
        }
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: config_id,
            config: Box::new(test_rich_mcp_config(config_id, "Filesystem Updated")),
            secrets: vec![],
        }))
        .expect("publish update mcp config");
    let update_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(update_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigSaved {
            id,
            name: Some(name),
        } if *id == config_id && name == "Filesystem Updated"
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::StartMcpOAuth {
            id: config_id,
            provider: "github".to_string(),
        }))
        .expect("publish oauth event");
    let oauth_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(oauth_commands.iter().any(|command| matches!(
        command,
        ViewCommand::ShowNotification { message } if message == "Starting OAuth for github"
    )));

    event_bus
        .publish(AppEvent::Mcp(McpEvent::ConfigSaved { id: config_id }))
        .expect("publish mcp domain event");
    let domain_commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(domain_commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigSaved { id, name: None } if *id == config_id
    )));
}

#[tokio::test]
async fn mcp_configure_presenter_surfaces_load_and_save_errors() {
    let config_id = Uuid::new_v4();
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(config_id)));
    mcp_service
        .set_get_result(Err(ServiceError::Internal(
            "cannot load config".to_string(),
        )))
        .await;
    mcp_service
        .set_update_result(Err(ServiceError::Internal(
            "cannot save config".to_string(),
        )))
        .await;
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter = McpConfigurePresenter::new_with_event_bus(mcp_service, &event_bus, view_tx)
        .with_config_path({
            // Use a platform-appropriate non-existent path
            #[cfg(not(windows))]
            {
                std::path::PathBuf::from("/nonexistent/dir/config.json")
            }
            #[cfg(windows)]
            {
                std::path::PathBuf::from(r"invalid:path\config.json")
            }
        });
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::ConfigureMcp { id: config_id }))
        .expect("publish configure failure");
    let load_error = collect_broadcast_commands(&mut view_rx).await;
    assert!(load_error.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Load Failed" && message.contains("cannot load config")
    )));

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: config_id,
            config: Box::new(test_rich_mcp_config(config_id, "Broken")),
            secrets: vec![],
        }))
        .expect("publish save failure");
    let save_error = collect_broadcast_commands(&mut view_rx).await;
    assert!(save_error.iter().any(|command| matches!(
        command,
        ViewCommand::ShowError {
            title,
            message,
            severity: ErrorSeverity::Error,
        } if title == "Save Failed"
    )));
}

/// Returns the `TempDir` guard alongside the path so the directory is
/// deleted when the test ends instead of leaking.
fn temp_config_with_env_mcp(
    id: Uuid,
    name: &str,
    env_var_names: &[&str],
) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.json");
    let mut config = personal_agent::config::Config::default();
    config.mcps.push(personal_agent::mcp::McpConfig {
        id,
        name: name.to_string(),
        enabled: true,
        source: personal_agent::mcp::McpSource::Manual { url: String::new() },
        package: personal_agent::mcp::McpPackage {
            package_type: personal_agent::mcp::McpPackageType::Npm,
            identifier: "@example/exa".to_string(),
            runtime_hint: Some("npx".to_string()),
        },
        transport: personal_agent::mcp::McpTransport::Stdio,
        auth_type: personal_agent::mcp::McpAuthType::ApiKey,
        env_vars: env_var_names
            .iter()
            .map(|env_var_name| personal_agent::mcp::EnvVarConfig {
                name: (*env_var_name).to_string(),
                required: true,
                is_secret: true,
                value: None,
            })
            .collect(),
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::Value::Null,
        oauth_token: None,
    });
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    std::fs::write(&path, json).expect("write config");
    (dir, path)
}

#[tokio::test]
async fn mcp_configure_draft_loads_env_vars_from_app_config() {
    personal_agent::services::secure_store::use_mock_backend();
    let config_id = Uuid::new_v4();
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(config_id)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let (_config_dir, tmp_config) = temp_config_with_env_mcp(config_id, "Exa", &["EXA_API_KEY"]);
    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::ConfigureMcp { id: config_id }))
        .expect("publish configure mcp");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigureDraftLoaded {
            id,
            env,
            env_var_name,
            ..
        } if id == &config_id.to_string()
            && env == &vec![("EXA_API_KEY".to_string(), String::new(), true)]
            && env_var_name == "EXA_API_KEY"
    )));
}

#[tokio::test]
async fn mcp_configure_save_stores_keychain_secrets() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(Uuid::new_v4())));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let (_config_dir, tmp_config) = temp_config_path();
    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config.clone());
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: Uuid::nil(),
            config: Box::new(test_rich_mcp_config(Uuid::nil(), "Exa")),
            secrets: vec![(
                "EXA_API_KEY".to_string(),
                personal_agent::events::types::SecretValue::new("sk-123"),
            )],
        }))
        .expect("publish save mcp config");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    let saved_id = commands
        .iter()
        .find_map(|command| match command {
            ViewCommand::McpConfigSaved { id, name: Some(_) } if !id.is_nil() => Some(*id),
            _ => None,
        })
        .expect("save should emit McpConfigSaved with a generated id");

    let stored =
        personal_agent::services::secure_store::mcp_keys::get_named(saved_id, "EXA_API_KEY")
            .expect("keychain lookup should succeed");
    assert_eq!(stored.as_deref(), Some("sk-123"));

    let config_json = std::fs::read_to_string(&tmp_config).expect("read saved config");
    assert!(
        !config_json.contains("sk-123"),
        "plaintext secret must never be serialized into the config file"
    );
}

/// Serializes tests that touch the process-global mock keychain, including
/// failure injection, so parallel tests cannot observe each other's state.
static KEYCHAIN_WRITE_SERIALIZER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Clears the mock-store failure flag on drop, so an assert failure in the
/// middle of an injection test cannot poison later tests in this binary.
struct MockStoreFailureGuard;

impl Drop for MockStoreFailureGuard {
    fn drop(&mut self) {
        secure_store::set_mock_store_failure(false);
    }
}

#[tokio::test]
async fn mcp_configure_save_persists_realistic_secret_payload() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(Uuid::new_v4())));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let (_config_dir, tmp_config) = temp_config_path();
    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config.clone());
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    // The exact payload shape the configure screen emits for an ApiKey MCP:
    // one secret env var (no value in the config) plus one plain var.
    let mut config = test_rich_mcp_config(Uuid::nil(), "Exa Pro");
    config.auth_type = personal_agent::mcp::McpAuthType::ApiKey;
    config.env_vars = vec![
        personal_agent::mcp::EnvVarConfig {
            name: "EXA_API_KEY".to_string(),
            required: true,
            is_secret: true,
            value: None,
        },
        personal_agent::mcp::EnvVarConfig {
            name: "EXA_WORKSPACE".to_string(),
            required: true,
            is_secret: false,
            value: Some("acme".to_string()),
        },
    ];

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: Uuid::nil(),
            config: Box::new(config),
            secrets: vec![(
                "EXA_API_KEY".to_string(),
                personal_agent::events::types::SecretValue::new("sk-real-4f2a"),
            )],
        }))
        .expect("publish save mcp config");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    let saved_id = commands
        .iter()
        .find_map(|command| match command {
            ViewCommand::McpConfigSaved { id, name: Some(_) } if !id.is_nil() => Some(*id),
            _ => None,
        })
        .expect("save should emit McpConfigSaved");

    let stored = secure_store::mcp_keys::get_named(saved_id, "EXA_API_KEY")
        .expect("keychain lookup should succeed");
    assert_eq!(stored.as_deref(), Some("sk-real-4f2a"));

    let config_json = std::fs::read_to_string(&tmp_config).expect("read saved config");
    let persisted: personal_agent::config::Config =
        serde_json::from_str(&config_json).expect("parse saved config");
    let saved = persisted
        .mcps
        .iter()
        .find(|mcp| mcp.id == saved_id)
        .expect("saved MCP entry");
    let secret_var = saved
        .env_vars
        .iter()
        .find(|var| var.name == "EXA_API_KEY")
        .expect("secret var persisted");
    assert!(secret_var.is_secret);
    assert_eq!(
        secret_var.value, None,
        "a secret var must never carry a serialized value"
    );
    let plain_var = saved
        .env_vars
        .iter()
        .find(|var| var.name == "EXA_WORKSPACE")
        .expect("plain var persisted");
    assert!(!plain_var.is_secret);
    assert_eq!(plain_var.value.as_deref(), Some("acme"));
}

#[tokio::test]
async fn mcp_configure_save_with_empty_secrets_preserves_stored_key() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let existing_id = Uuid::new_v4();
    secure_store::mcp_keys::store_named(existing_id, "EXA_API_KEY", "stored-key-9f31")
        .expect("pre-store key");

    let (_config_dir, tmp_config) = temp_config_with_env_mcp(existing_id, "Exa", &["EXA_API_KEY"]);
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(existing_id)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    // Editing the MCP without retyping the key emits an empty secrets
    // payload; the keychain entry must survive untouched.
    let mut config = test_rich_mcp_config(existing_id, "Exa Updated");
    config.auth_type = personal_agent::mcp::McpAuthType::ApiKey;
    config.env_vars = vec![personal_agent::mcp::EnvVarConfig {
        name: "EXA_API_KEY".to_string(),
        required: true,
        is_secret: true,
        value: None,
    }];

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: existing_id,
            config: Box::new(config),
            secrets: vec![],
        }))
        .expect("publish save mcp config");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(commands.iter().any(|command| matches!(
        command,
        ViewCommand::McpConfigSaved { id, .. } if *id == existing_id
    )));

    let stored = secure_store::mcp_keys::get_named(existing_id, "EXA_API_KEY")
        .expect("keychain lookup should succeed");
    assert_eq!(
        stored.as_deref(),
        Some("stored-key-9f31"),
        "an edit without a typed key must preserve the stored keychain entry"
    );
}

#[tokio::test]
async fn mcp_configure_save_surfaces_keychain_failure_and_writes_nothing() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let _failure = MockStoreFailureGuard;
    secure_store::set_mock_store_failure(true);
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(Uuid::new_v4())));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let (_config_dir, tmp_config) = temp_config_path();
    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config.clone());
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: Uuid::nil(),
            config: Box::new(test_rich_mcp_config(Uuid::nil(), "Doomed")),
            secrets: vec![(
                "EXA_API_KEY".to_string(),
                personal_agent::events::types::SecretValue::new("sk-never-stored"),
            )],
        }))
        .expect("publish save mcp config");
    let commands = collect_broadcast_commands(&mut view_rx).await;

    assert!(
        commands.iter().any(|command| matches!(
            command,
            ViewCommand::ShowError {
                title,
                severity: ErrorSeverity::Error,
                ..
            } if title == "Save Failed"
        )),
        "a keychain store failure must surface as ShowError"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, ViewCommand::McpConfigSaved { .. })),
        "no McpConfigSaved may follow a failed keychain store"
    );

    let config_json = std::fs::read_to_string(&tmp_config).expect("read untouched config");
    let persisted: personal_agent::config::Config =
        serde_json::from_str(&config_json).expect("parse config");
    assert!(
        persisted.mcps.is_empty(),
        "a failed keychain store must not leave a config entry behind"
    );
}

#[tokio::test]
async fn mcp_configure_draft_never_carries_stored_secret_value() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let config_id = Uuid::new_v4();
    secure_store::mcp_keys::store_named(config_id, "EXA_API_KEY", "canary-only-keychain")
        .expect("pre-store canary");

    let (_config_dir, tmp_config) = temp_config_with_env_mcp(config_id, "Exa", &["EXA_API_KEY"]);
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(config_id)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter =
        McpConfigurePresenter::new_with_event_bus(mcp_service.clone(), &event_bus, view_tx)
            .with_config_path(tmp_config);
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::ConfigureMcp { id: config_id }))
        .expect("publish configure mcp");
    let commands = collect_broadcast_commands(&mut view_rx).await;

    assert!(
        commands.iter().any(|command| matches!(
            command,
            ViewCommand::McpConfigureDraftLoaded {
                id,
                stored_secret_names,
                ..
            } if id == &config_id.to_string()
                && stored_secret_names == &vec!["EXA_API_KEY".to_string()]
        )),
        "the draft must report which secret vars already have keychain entries"
    );
    let rendered = format!("{commands:?}");
    assert!(
        !rendered.contains("canary-only-keychain"),
        "the stored secret value must never appear in any ViewCommand payload, got: {rendered}"
    );
}

/// Unwritable config path that fails the save AFTER the secret store has
/// succeeded, which is exactly when the rollback path runs.
fn unwritable_config_path() -> std::path::PathBuf {
    #[cfg(not(windows))]
    {
        std::path::PathBuf::from("/nonexistent/dir/config.json")
    }
    #[cfg(windows)]
    {
        std::path::PathBuf::from(r"invalid:path\config.json")
    }
}

#[tokio::test]
async fn mcp_configure_failed_save_restores_preexisting_keychain_entry() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let saved_id = Uuid::new_v4();
    secure_store::mcp_keys::store_named(saved_id, "EXA_API_KEY", "prior-working-key")
        .expect("pre-store prior credential");

    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(saved_id)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter = McpConfigurePresenter::new_with_event_bus(mcp_service, &event_bus, view_tx)
        .with_config_path(unwritable_config_path());
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: saved_id,
            config: Box::new(test_rich_mcp_config(saved_id, "Exa Rotated")),
            secrets: vec![(
                "EXA_API_KEY".to_string(),
                personal_agent::events::types::SecretValue::new("rotated-key"),
            )],
        }))
        .expect("publish failing save");
    let commands = collect_broadcast_commands(&mut view_rx).await;

    assert!(
        commands.iter().any(|command| matches!(
            command,
            ViewCommand::ShowError {
                title,
                severity: ErrorSeverity::Error,
                ..
            } if title == "Save Failed"
        )),
        "the save must fail after the store for the rollback to run"
    );

    let stored = secure_store::mcp_keys::get_named(saved_id, "EXA_API_KEY")
        .expect("keychain lookup should succeed");
    assert_eq!(
        stored.as_deref(),
        Some("prior-working-key"),
        "a failed save must restore the previously stored credential, not delete it"
    );
}

#[tokio::test]
async fn mcp_configure_failed_save_deletes_secret_it_created() {
    personal_agent::services::secure_store::use_mock_backend();
    let _serial = KEYCHAIN_WRITE_SERIALIZER.lock().await;
    let saved_id = Uuid::new_v4();

    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(saved_id)));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let mut presenter = McpConfigurePresenter::new_with_event_bus(mcp_service, &event_bus, view_tx)
        .with_config_path(unwritable_config_path());
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: saved_id,
            config: Box::new(test_rich_mcp_config(saved_id, "Exa New")),
            secrets: vec![(
                "EXA_API_KEY".to_string(),
                personal_agent::events::types::SecretValue::new("never-committed-key"),
            )],
        }))
        .expect("publish failing save");
    let commands = collect_broadcast_commands(&mut view_rx).await;

    assert!(
        commands.iter().any(|command| matches!(
            command,
            ViewCommand::ShowError {
                title,
                severity: ErrorSeverity::Error,
                ..
            } if title == "Save Failed"
        )),
        "the save must fail after the store for the rollback to run"
    );

    let stored = secure_store::mcp_keys::get_named(saved_id, "EXA_API_KEY")
        .expect("keychain lookup should succeed");
    assert!(
        stored.is_none(),
        "a failed save must not leave the secret it created behind"
    );
}

#[tokio::test]
async fn mcp_configure_save_rejects_unsafe_env_var_names_before_persisting() {
    let mcp_service = Arc::new(MockMcpService::new(test_mcp_config(Uuid::new_v4())));
    let event_bus = Arc::new(EventBus::new(64));
    let (view_tx, mut view_rx) = broadcast::channel(128);

    let (_config_dir, tmp_config) = temp_config_path();
    let mut presenter = McpConfigurePresenter::new_with_event_bus(mcp_service, &event_bus, view_tx)
        .with_config_path(tmp_config.clone());
    presenter.start().await.expect("start presenter");
    let _ = collect_broadcast_commands(&mut view_rx).await;

    // Invalid name in the declared config env vars.
    let mut config = test_rich_mcp_config(Uuid::nil(), "Exa");
    config.env_vars = vec![personal_agent::mcp::EnvVarConfig {
        name: "BAD NAME".to_string(),
        required: true,
        is_secret: true,
        value: None,
    }];
    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: Uuid::nil(),
            config: Box::new(config),
            secrets: vec![],
        }))
        .expect("publish save with invalid env var name");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(
        commands.iter().any(|command| matches!(
            command,
            ViewCommand::ShowError {
                title,
                message,
                severity: ErrorSeverity::Error,
            } if title == "Save Failed" && message.contains("BAD NAME")
        )),
        "an invalid env var name must surface a descriptive save error"
    );

    // Invalid name in the secrets payload alone.
    event_bus
        .publish(AppEvent::User(UserEvent::SaveMcpConfig {
            id: Uuid::nil(),
            config: Box::new(test_rich_mcp_config(Uuid::nil(), "Exa")),
            secrets: vec![(
                "KEY\r\nINJECTED".to_string(),
                personal_agent::events::types::SecretValue::new("sk-1"),
            )],
        }))
        .expect("publish save with invalid secret name");
    let commands = collect_broadcast_commands(&mut view_rx).await;
    assert!(
        commands.iter().any(|command| matches!(
            command,
            ViewCommand::ShowError {
                title,
                severity: ErrorSeverity::Error,
                ..
            } if title == "Save Failed"
        )),
        "an invalid secret payload name must surface a save error"
    );

    let config_json = std::fs::read_to_string(&tmp_config).expect("read untouched config");
    let persisted: personal_agent::config::Config =
        serde_json::from_str(&config_json).expect("parse config");
    assert!(
        persisted.mcps.is_empty(),
        "rejected saves must not persist a config entry"
    );
}
