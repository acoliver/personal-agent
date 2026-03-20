use personal_agent::config::{
    default_api_base_url_for_provider, provider_api_url, provider_api_url_map, Config,
};
use personal_agent::events::global::init_event_bus;
use personal_agent::events::types::SystemEvent;
use personal_agent::events::{self, AppEvent};
use personal_agent::llm::error::{LlmError, LlmResult};
use personal_agent::mcp::registry::{
    McpRegistryEnvVar, McpRegistryPackage, McpRegistryPackageArgument, McpRegistryRemote,
    McpRegistryRepository, McpRegistryServer, McpRegistryServerWrapper, McpRegistrySource,
    McpRegistryTransport,
};
use personal_agent::mcp::{
    EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageArg, McpPackageArgType,
    McpPackageType, McpRuntime, McpService, McpSource, McpStatus, McpTransport, SecretsManager,
};
use personal_agent::registry::{ModelRegistry, Provider, RegistryCache, RegistryManager};
use personal_agent::services::app_settings::AppSettingsService;
use personal_agent::services::app_settings_impl::AppSettingsServiceImpl;
use personal_agent::services::ServiceError;
use std::collections::HashMap;
use tempfile::TempDir;
use uuid::Uuid;

fn temp_registry_cache_path(temp_dir: &TempDir) -> std::path::PathBuf {
    temp_dir.path().join("registry-cache.json")
}

fn sample_registry(provider_id: &str, api: Option<&str>) -> ModelRegistry {
    let mut providers = HashMap::new();
    providers.insert(
        provider_id.to_string(),
        Provider {
            id: provider_id.to_string(),
            name: provider_id.to_string(),
            env: vec!["PROVIDER_KEY".to_string()],
            npm: None,
            api: api.map(ToOwned::to_owned),
            doc: None,
            models: HashMap::new(),
        },
    );

    ModelRegistry { providers }
}

fn write_registry_cache_with_default_path(registry: &ModelRegistry) {
    let cache_path = RegistryCache::default_path().unwrap();
    let cache = RegistryCache::new(cache_path.clone(), 24);
    cache.save(registry).unwrap();
    assert!(cache_path.exists());
}

fn http_config() -> McpConfig {
    McpConfig {
        id: Uuid::new_v4(),
        name: "HTTP MCP".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "https://example.com/mcp".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "https://example.com/mcp".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Http,
        auth_type: McpAuthType::None,
        env_vars: vec![EnvVarConfig {
            name: "API_KEY".to_string(),
            required: false,
        }],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

#[test]
fn llm_error_display_and_classification_cover_variants() {
    let serdes = LlmError::SerdesAi("stream backend failed".to_string());
    let invalid = LlmError::InvalidConfig("missing base url".to_string());
    let auth = LlmError::Auth("bad token".to_string());
    let unsupported = LlmError::UnsupportedModel("legacy-model".to_string());
    let stream = LlmError::Stream("socket closed".to_string());
    let message_conversion = LlmError::MessageConversion("bad role".to_string());
    let io = LlmError::Io(std::io::Error::other("disk error"));
    let json = LlmError::Json(serde_json::from_str::<serde_json::Value>("not json").unwrap_err());
    let keyfile = LlmError::KeyfileRead {
        path: "/tmp/keyfile".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
    };
    let no_key = LlmError::NoApiKey;

    assert_eq!(serdes.to_string(), "SerdesAI error: stream backend failed");
    assert_eq!(
        invalid.to_string(),
        "Invalid model configuration: missing base url"
    );
    assert_eq!(auth.to_string(), "Authentication error: bad token");
    assert_eq!(unsupported.to_string(), "Model not supported: legacy-model");
    assert_eq!(stream.to_string(), "Streaming error: socket closed");
    assert_eq!(
        message_conversion.to_string(),
        "Message conversion error: bad role"
    );
    assert!(io.to_string().contains("IO error: disk error"));
    assert!(json.to_string().starts_with("JSON error:"));
    assert!(keyfile
        .to_string()
        .contains("Failed to read keyfile /tmp/keyfile: missing"));
    assert_eq!(no_key.to_string(), "No API key configured for profile");

    assert!(serdes.is_recoverable());
    assert!(stream.is_recoverable());
    assert!(matches!(io, LlmError::Io(_)) && io.is_recoverable());
    assert!(invalid.is_config_error());
    assert!(auth.is_config_error());
    assert!(unsupported.is_config_error());
    assert!(!message_conversion.is_recoverable());
    assert!(!no_key.is_config_error());

    let result: LlmResult<()> = Err(no_key);
    assert!(result.is_err());
}

#[test]
fn global_event_bus_init_subscribe_emit_and_clone_share_messages() {
    init_event_bus().unwrap();
    init_event_bus().unwrap();

    let mut receiver = events::subscribe();
    let cloned_bus = personal_agent::events::global::get_event_bus_clone();
    let mut clone_receiver = cloned_bus.subscribe();
    let event = AppEvent::System(SystemEvent::HotkeyPressed);

    events::emit(event.clone()).unwrap();

    assert_eq!(receiver.blocking_recv().unwrap(), event);
    assert_eq!(clone_receiver.blocking_recv().unwrap(), event);
}

#[test]
fn provider_defaults_use_builtins_trim_input_and_collect_map() {
    assert_eq!(
        provider_api_url(" openai ").as_deref(),
        Some("https://api.openai.com/v1")
    );
    assert_eq!(
        provider_api_url("anthropic").as_deref(),
        Some("https://api.anthropic.com/v1")
    );
    assert_eq!(
        provider_api_url("synthetic").as_deref(),
        Some("https://api.synthetic.new/v1")
    );
    assert_eq!(
        provider_api_url("moonshotai-cn").as_deref(),
        Some("https://api.moonshot.cn/v1")
    );
    assert_eq!(
        provider_api_url("kimi-for-coding").as_deref(),
        Some("https://api.kimi.com/coding/v1")
    );
    assert_eq!(provider_api_url("   "), None);
    assert_eq!(provider_api_url("unknown-provider"), None);
    assert_eq!(
        default_api_base_url_for_provider("unknown-provider"),
        "https://api.openai.com/v1"
    );

    let url_map = provider_api_url_map(vec![
        "openai".to_string(),
        "unknown-provider".to_string(),
        "anthropic".to_string(),
    ]);
    assert_eq!(url_map.len(), 2);
    assert_eq!(
        url_map.get("openai").map(String::as_str),
        Some("https://api.openai.com/v1")
    );
    assert_eq!(
        url_map.get("anthropic").map(String::as_str),
        Some("https://api.anthropic.com/v1")
    );
}

#[test]
fn registry_manager_uses_cache_and_can_clear_it() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_registry_cache_path(&temp_dir);
    let registry = sample_registry("cached-provider", Some("https://cached.example/v1"));
    let manager = RegistryManager::with_cache(cache_path.clone(), 24);

    RegistryCache::new(cache_path.clone(), 24)
        .save(&registry)
        .unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let loaded = runtime.block_on(manager.get_registry()).unwrap();
    assert_eq!(loaded.providers.len(), 1);
    assert_eq!(
        loaded
            .providers
            .get("cached-provider")
            .and_then(|provider| provider.api.as_deref()),
        Some("https://cached.example/v1")
    );

    let metadata = manager.cache_metadata().unwrap().unwrap();
    assert!(!metadata.is_expired);
    assert!(metadata.size_bytes > 0);
    assert!(metadata.cached_at <= chrono::Utc::now());

    manager.clear_cache().unwrap();
    assert!(!cache_path.exists());
    assert!(manager.cache_metadata().unwrap().is_none());
}

#[test]
fn registry_manager_new_and_default_construct_successfully() {
    let manager = RegistryManager::new().unwrap();
    assert!(manager.cache_metadata().is_ok());

    let default_manager = RegistryManager::default();
    assert!(default_manager.cache_metadata().is_ok());
}

#[tokio::test]
async fn app_settings_round_trip_and_reset_defaults() {
    let temp_dir = TempDir::new().unwrap();
    let settings_path = temp_dir.path().join("nested").join("settings.json");
    let service = AppSettingsServiceImpl::new(settings_path.clone()).unwrap();
    let profile_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();

    assert_eq!(service.get_default_profile_id().await.unwrap(), None);
    assert_eq!(service.get_current_conversation_id().await.unwrap(), None);
    assert_eq!(service.get_hotkey().await.unwrap(), None);
    assert_eq!(service.get_theme().await.unwrap(), None);
    assert_eq!(service.get_setting("missing").await.unwrap(), None);

    service.set_default_profile_id(profile_id).await.unwrap();
    service
        .set_current_conversation_id(conversation_id)
        .await
        .unwrap();
    service.set_hotkey("Cmd+Shift+J".to_string()).await.unwrap();
    service.set_theme("light".to_string()).await.unwrap();
    service
        .set_setting("language", "en-US".to_string())
        .await
        .unwrap();

    assert_eq!(
        service.get_default_profile_id().await.unwrap(),
        Some(profile_id)
    );
    assert_eq!(
        service.get_current_conversation_id().await.unwrap(),
        Some(conversation_id)
    );
    assert_eq!(
        service.get_hotkey().await.unwrap().as_deref(),
        Some("Cmd+Shift+J")
    );
    assert_eq!(service.get_theme().await.unwrap().as_deref(), Some("light"));
    assert_eq!(
        service.get_setting("language").await.unwrap().as_deref(),
        Some("en-US")
    );
    assert!(settings_path.exists());

    service.reset_to_defaults().await.unwrap();
    assert_eq!(service.get_default_profile_id().await.unwrap(), None);
    assert_eq!(service.get_current_conversation_id().await.unwrap(), None);
    assert_eq!(service.get_hotkey().await.unwrap(), None);
    assert_eq!(service.get_theme().await.unwrap(), None);
    assert_eq!(service.get_setting("language").await.unwrap(), None);
}

#[tokio::test]
async fn app_settings_reports_invalid_existing_json_and_invalid_uuids() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_json_path = temp_dir.path().join("invalid.json");
    std::fs::write(&invalid_json_path, "not-json").unwrap();
    let invalid_json_result = AppSettingsServiceImpl::new(invalid_json_path);
    assert!(matches!(
        invalid_json_result,
        Err(ServiceError::Validation(message)) if message.contains("Failed to parse settings JSON")
    ));

    let invalid_uuid_path = temp_dir.path().join("invalid-uuid.json");
    std::fs::write(
        &invalid_uuid_path,
        serde_json::json!({
            "default_profile_id": "not-a-uuid",
            "current_conversation_id": "also-not-a-uuid"
        })
        .to_string(),
    )
    .unwrap();
    let service = AppSettingsServiceImpl::new(invalid_uuid_path).unwrap();

    let default_error = service.get_default_profile_id().await.unwrap_err();
    assert!(matches!(
        default_error,
        ServiceError::Validation(message) if message == "Invalid profile ID UUID"
    ));

    let conversation_error = service.get_current_conversation_id().await.unwrap_err();
    assert!(matches!(
        conversation_error,
        ServiceError::Validation(message) if message == "Invalid conversation ID UUID"
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn mcp_registry_entry_to_config_covers_remote_and_error_branches() {
    let remote_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "remote-server".to_string(),
            description: "Remote MCP".to_string(),
            repository: McpRegistryRepository::default(),
            version: "1.0.0".to_string(),
            packages: vec![],
            remotes: vec![McpRegistryRemote {
                remote_type: "smithery-oauth".to_string(),
                url: "https://remote.example/mcp".to_string(),
            }],
        },
        meta: serde_json::json!({"source": "smithery"}),
    };

    let remote_config = personal_agent::mcp::McpRegistry::entry_to_config(&remote_wrapper).unwrap();
    assert_eq!(remote_config.transport, McpTransport::Http);
    assert_eq!(remote_config.auth_type, McpAuthType::OAuth);
    assert_eq!(remote_config.package.package_type, McpPackageType::Http);
    assert_eq!(
        remote_config.source,
        McpSource::Manual {
            url: "https://remote.example/mcp".to_string()
        }
    );

    let unsupported_package_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "bad-package".to_string(),
            description: "Bad package".to_string(),
            repository: McpRegistryRepository::default(),
            version: "1.0.0".to_string(),
            packages: vec![McpRegistryPackage {
                registry_type: "rubygems".to_string(),
                identifier: "bad/package".to_string(),
                version: None,
                transport: McpRegistryTransport {
                    transport_type: "stdio".to_string(),
                },
                environment_variables: vec![],
                package_arguments: vec![],
            }],
            remotes: vec![],
        },
        meta: serde_json::json!({}),
    };
    assert!(
        personal_agent::mcp::McpRegistry::entry_to_config(&unsupported_package_wrapper)
            .unwrap_err()
            .contains("Unsupported registry type")
    );

    let unsupported_transport_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "bad-transport".to_string(),
            description: "Bad transport".to_string(),
            repository: McpRegistryRepository::default(),
            version: "1.0.0".to_string(),
            packages: vec![McpRegistryPackage {
                registry_type: "npm".to_string(),
                identifier: "good/package".to_string(),
                version: None,
                transport: McpRegistryTransport {
                    transport_type: "websocket".to_string(),
                },
                environment_variables: vec![McpRegistryEnvVar {
                    name: "AUTH_TOKEN".to_string(),
                    description: None,
                    is_secret: true,
                    is_required: true,
                }],
                package_arguments: vec![McpRegistryPackageArgument {
                    argument_type: "positional".to_string(),
                    name: "workspace".to_string(),
                    description: Some("Workspace path".to_string()),
                    is_required: false,
                    default: Some(".".to_string()),
                }],
            }],
            remotes: vec![],
        },
        meta: serde_json::json!({}),
    };
    assert!(
        personal_agent::mcp::McpRegistry::entry_to_config(&unsupported_transport_wrapper)
            .unwrap_err()
            .contains("Unsupported transport type")
    );

    let unsupported_remote_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "bad-remote".to_string(),
            description: "Bad remote".to_string(),
            repository: McpRegistryRepository::default(),
            version: "1.0.0".to_string(),
            packages: vec![],
            remotes: vec![McpRegistryRemote {
                remote_type: "websocket".to_string(),
                url: "wss://example.com".to_string(),
            }],
        },
        meta: serde_json::json!({}),
    };
    assert!(
        personal_agent::mcp::McpRegistry::entry_to_config(&unsupported_remote_wrapper)
            .unwrap_err()
            .contains("Unsupported remote type")
    );

    let empty_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "empty".to_string(),
            description: "No package or remote".to_string(),
            repository: McpRegistryRepository::default(),
            version: "1.0.0".to_string(),
            packages: vec![],
            remotes: vec![],
        },
        meta: serde_json::json!({}),
    };
    assert_eq!(
        personal_agent::mcp::McpRegistry::entry_to_config(&empty_wrapper).unwrap_err(),
        "Server has neither packages nor remotes"
    );
}

#[tokio::test]
async fn mcp_runtime_http_paths_and_tool_lookup_behave_consistently() {
    let secrets = SecretsManager::new();
    let mut runtime = McpRuntime::new(secrets);
    let config = http_config();

    assert!(!runtime.has_active_mcps());
    assert_eq!(runtime.active_count(), 0);
    assert!(runtime.get_all_tools().is_empty());
    assert_eq!(runtime.find_tool_provider("missing-tool"), None);

    runtime.stop_mcp(&config.id).unwrap();
    assert_eq!(
        runtime.status_manager().get_status(&config.id),
        McpStatus::Stopped
    );

    let config_state = Config {
        mcps: vec![config.clone()],
        ..Config::default()
    };
    let results = runtime.start_all(&config_state).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, config.id);
    assert!(results[0].1.is_err());
    assert!(!runtime.has_active_mcps());

    runtime.cleanup_idle();
    assert_eq!(runtime.active_count(), 0);
}

#[tokio::test]
#[allow(clippy::significant_drop_tightening)]
async fn mcp_service_empty_runtime_paths_are_stable() {
    let service = McpService::global();

    {
        let guard = service.lock().await;
        assert!(guard.get_tools().is_empty());
        assert!(guard.get_llm_tools().is_empty());
        assert!(!guard.has_active_mcps());
        assert_eq!(guard.active_count(), 0);
        let missing_status = guard.get_status(&Uuid::new_v4());
        assert_eq!(missing_status, None);
    }

    {
        let mut guard = service.lock().await;
        let tool_error = guard.call_tool("missing-tool", serde_json::json!({})).await;
        assert!(tool_error.unwrap_err().contains("No MCP provides tool"));
    }
}

#[test]
fn provider_defaults_prefers_cached_registry_api_when_available() {
    let registry = sample_registry("custom-provider", Some("https://cached.provider/v9"));
    write_registry_cache_with_default_path(&registry);

    assert_eq!(
        provider_api_url("custom-provider").as_deref(),
        Some("https://cached.provider/v9")
    );
}

#[test]
fn mcp_registry_source_variants_are_constructible_for_coverage() {
    assert_eq!(McpRegistrySource::Official, McpRegistrySource::Official);
    assert_eq!(McpRegistrySource::Smithery, McpRegistrySource::Smithery);

    let required_arg = McpPackageArg {
        arg_type: McpPackageArgType::Named,
        name: "workspace".to_string(),
        description: Some("Workspace path".to_string()),
        required: true,
        default: None,
    };
    assert!(required_arg.required);
}
