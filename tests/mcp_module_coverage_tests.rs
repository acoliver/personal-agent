use personal_agent::mcp::registry::{
    McpRegistryEnvVar, McpRegistryPackage, McpRegistryPackageArgument, McpRegistryRepository,
    McpRegistryTransport,
};
use personal_agent::mcp::{
    generate_smithery_oauth_url, McpAuthType, McpConfig, McpPackage, McpPackageArg,
    McpPackageArgType, McpPackageType, McpRegistry, McpRegistryRemote, McpRegistryServer,
    McpRegistryServerWrapper, McpRegistrySource, McpRuntime, McpSource, McpStatus, McpTransport,
    OAuthConfig, OAuthFlowState, OAuthManager, OAuthToken, RegistryEnvVar, SecretsManager,
    SmitheryOAuthConfig,
};
use serde_json::json;
use uuid::Uuid;

fn stdio_config(id: Uuid) -> McpConfig {
    McpConfig {
        id,
        name: "Filesystem".to_string(),
        enabled: true,
        source: McpSource::Official {
            name: "filesystem".to_string(),
            version: "1.0.0".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@modelcontextprotocol/server-filesystem".to_string(),
            runtime_hint: Some("npx".to_string()),
        },
        transport: McpTransport::Stdio,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: json!({}),
        oauth_token: None,
    }
}

fn wrapper_with_package(
    registry_type: &str,
    transport_type: &str,
    env_names: Vec<(&str, bool, bool)>,
    package_args: Vec<(&str, &str, bool, Option<&str>)>,
) -> McpRegistryServerWrapper {
    McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "filesystem".to_string(),
            description: "Browse files".to_string(),
            repository: McpRegistryRepository {
                url: Some("https://example.com/filesystem".to_string()),
                source: Some("github".to_string()),
            },
            version: "1.2.3".to_string(),
            packages: vec![McpRegistryPackage {
                registry_type: registry_type.to_string(),
                identifier: "@modelcontextprotocol/server-filesystem".to_string(),
                version: Some("1.2.3".to_string()),
                transport: McpRegistryTransport {
                    transport_type: transport_type.to_string(),
                },
                environment_variables: env_names
                    .into_iter()
                    .map(|(name, is_secret, is_required)| McpRegistryEnvVar {
                        name: name.to_string(),
                        description: Some(format!("{name} description")),
                        is_secret,
                        is_required,
                    })
                    .collect(),
                package_arguments: package_args
                    .into_iter()
                    .map(
                        |(argument_type, name, is_required, default)| McpRegistryPackageArgument {
                            argument_type: argument_type.to_string(),
                            name: name.to_string(),
                            description: Some(format!("{name} argument")),
                            is_required,
                            default: default.map(ToString::to_string),
                        },
                    )
                    .collect(),
            }],
            remotes: vec![],
        },
        meta: json!({"source": "official"}),
    }
}

fn wrapper_with_remote(remote_type: &str, url: &str) -> McpRegistryServerWrapper {
    McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "remote-server".to_string(),
            description: "Hosted server".to_string(),
            repository: McpRegistryRepository::default(),
            version: "latest".to_string(),
            packages: vec![],
            remotes: vec![McpRegistryRemote {
                remote_type: remote_type.to_string(),
                url: url.to_string(),
            }],
        },
        meta: json!({"source": "remote"}),
    }
}

#[test]
fn mcp_runtime_new_exposes_empty_running_state_and_status_manager() {
    personal_agent::services::secure_store::use_mock_backend();
    let runtime = McpRuntime::new(SecretsManager::new());
    let status_manager = runtime.status_manager();
    let random_id = Uuid::new_v4();

    assert!(!runtime.has_active_mcps());
    assert_eq!(runtime.active_count(), 0);
    assert!(runtime.get_all_tools().is_empty());
    assert_eq!(runtime.find_tool_provider("missing-tool"), None);
    assert_eq!(status_manager.get_status(&random_id), McpStatus::Stopped);
}

#[test]
fn mcp_runtime_start_mcp_rejects_disabled_configs_and_updates_status() {
    personal_agent::services::secure_store::use_mock_backend();
    let mut runtime = McpRuntime::new(SecretsManager::new());
    let status_manager = runtime.status_manager();
    let mut config = stdio_config(Uuid::new_v4());
    config.enabled = false;

    let error = futures::executor::block_on(runtime.start_mcp(&config))
        .expect_err("disabled MCP should fail");

    assert_eq!(error, "MCP is disabled");
    assert_eq!(status_manager.get_status(&config.id), McpStatus::Stopped);
    assert!(!runtime.has_active_mcps());
}

#[test]
fn mcp_runtime_start_mcp_validates_required_package_arguments() {
    personal_agent::services::secure_store::use_mock_backend();
    let mut runtime = McpRuntime::new(SecretsManager::new());
    let status_manager = runtime.status_manager();
    let mut config = stdio_config(Uuid::new_v4());
    config.package_args = vec![McpPackageArg {
        arg_type: McpPackageArgType::Named,
        name: "workspace".to_string(),
        description: Some("required workspace path".to_string()),
        required: true,
        default: None,
    }];
    config.config = json!({"package_args": {"workspace": "   "}});

    let error = futures::executor::block_on(runtime.start_mcp(&config))
        .expect_err("missing package arg should fail");

    assert_eq!(error, "Missing required package argument: workspace");
    assert_eq!(
        status_manager.get_status(&config.id),
        McpStatus::Error("Missing required package argument: workspace".to_string())
    );
}

#[test]
fn mcp_runtime_start_mcp_fails_for_empty_stdio_command() {
    personal_agent::services::secure_store::use_mock_backend();
    let mut runtime = McpRuntime::new(SecretsManager::new());
    let status_manager = runtime.status_manager();
    let mut config = stdio_config(Uuid::new_v4());
    config.package.identifier.clear();
    config.package.runtime_hint = Some(String::new());

    let error = futures::executor::block_on(runtime.start_mcp(&config))
        .expect_err("empty stdio command should fail");

    assert_eq!(error, "Empty command for stdio transport");
    assert_eq!(
        status_manager.get_status(&config.id),
        McpStatus::Error("Empty command".to_string())
    );
}

#[test]
fn mcp_runtime_stop_and_cleanup_idle_preserve_empty_state_invariants() {
    personal_agent::services::secure_store::use_mock_backend();
    let mut runtime = McpRuntime::new(SecretsManager::new());
    let id = Uuid::new_v4();

    runtime.cleanup_idle();
    assert!(!runtime.has_active_mcps());
    assert_eq!(runtime.active_count(), 0);

    runtime
        .stop_mcp(&id)
        .expect("stopping unknown MCP should still succeed");
    assert_eq!(runtime.status_manager().get_status(&id), McpStatus::Stopped);
    assert!(!runtime.has_active_mcps());
}

#[test]
fn mcp_registry_entry_to_config_maps_package_entries_for_npm_and_docker() {
    let npm_wrapper = wrapper_with_package(
        "npm",
        "stdio",
        vec![
            ("API_KEY", true, true),
            ("LOG_LEVEL", false, false),
            ("CLIENT_ID", false, false),
            ("CLIENT_SECRET", true, false),
        ],
        vec![
            ("named", "workspace", true, Some(".")),
            ("positional", "path", false, None),
        ],
    );
    let docker_wrapper = wrapper_with_package("oci", "streamable-http", vec![], vec![]);

    let npm_config = McpRegistry::entry_to_config(&npm_wrapper).expect("npm entry should map");
    let docker_config =
        McpRegistry::entry_to_config(&docker_wrapper).expect("docker entry should map");

    assert_eq!(npm_config.name, "filesystem");
    assert!(npm_config.enabled);
    assert_eq!(npm_config.transport, McpTransport::Stdio);
    assert_eq!(npm_config.package.package_type, McpPackageType::Npm);
    assert_eq!(npm_config.package.runtime_hint.as_deref(), Some("npx"));
    assert_eq!(npm_config.env_vars.len(), 4);
    assert_eq!(npm_config.env_vars[0].name, "API_KEY");
    assert!(npm_config.env_vars[0].required);
    assert_eq!(npm_config.auth_type, McpAuthType::OAuth);
    assert_eq!(npm_config.package_args.len(), 2);
    assert_eq!(
        npm_config.package_args[0].arg_type,
        McpPackageArgType::Named
    );
    assert_eq!(npm_config.package_args[0].name, "workspace");
    assert!(npm_config.package_args[0].required);
    assert_eq!(npm_config.package_args[0].default.as_deref(), Some("."));
    assert_eq!(
        npm_config.package_args[1].arg_type,
        McpPackageArgType::Positional
    );
    assert_eq!(npm_config.config, json!({}));
    assert_eq!(npm_config.oauth_token, None);

    assert_eq!(docker_config.transport, McpTransport::Http);
    assert_eq!(docker_config.package.package_type, McpPackageType::Docker);
    assert_eq!(
        docker_config.package.runtime_hint.as_deref(),
        Some("docker")
    );
    assert_eq!(docker_config.auth_type, McpAuthType::None);
}

#[test]
fn mcp_registry_entry_to_config_maps_remote_entries_and_rejects_invalid_inputs() {
    let http_wrapper = wrapper_with_remote("http", "https://example.com/mcp");
    let oauth_wrapper =
        wrapper_with_remote("smithery-oauth", "https://server.smithery.ai/@owner/server");
    let unsupported_remote = wrapper_with_remote("websocket", "wss://example.com");
    let unsupported_package = wrapper_with_package("pip", "stdio", vec![], vec![]);
    let unsupported_transport = wrapper_with_package("npm", "sse", vec![], vec![]);
    let empty_wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "empty".to_string(),
            description: "none".to_string(),
            repository: McpRegistryRepository::default(),
            version: "0.1.0".to_string(),
            packages: vec![],
            remotes: vec![],
        },
        meta: json!({}),
    };

    let http_config = McpRegistry::entry_to_config(&http_wrapper).expect("http remote should map");
    let oauth_config =
        McpRegistry::entry_to_config(&oauth_wrapper).expect("oauth remote should map");

    assert_eq!(http_config.transport, McpTransport::Http);
    assert_eq!(http_config.auth_type, McpAuthType::None);
    assert_eq!(http_config.package.package_type, McpPackageType::Http);
    assert_eq!(http_config.package.identifier, "https://example.com/mcp");
    assert_eq!(http_config.package.runtime_hint, None);
    assert!(
        matches!(http_config.source, McpSource::Manual { ref url } if url == "https://example.com/mcp")
    );

    assert_eq!(oauth_config.transport, McpTransport::Http);
    assert_eq!(oauth_config.auth_type, McpAuthType::OAuth);

    assert_eq!(
        McpRegistry::entry_to_config(&unsupported_remote)
            .expect_err("unsupported remote should fail"),
        "Unsupported remote type: websocket"
    );
    assert_eq!(
        McpRegistry::entry_to_config(&unsupported_package)
            .expect_err("unsupported package should fail"),
        "Unsupported registry type: pip"
    );
    assert_eq!(
        McpRegistry::entry_to_config(&unsupported_transport)
            .expect_err("unsupported transport should fail"),
        "Unsupported transport type: sse"
    );
    assert_eq!(
        McpRegistry::entry_to_config(&empty_wrapper).expect_err("empty entry should fail"),
        "Server has neither packages nor remotes"
    );
}

#[test]
fn detect_auth_type_distinguishes_oauth_api_key_and_none() {
    let oauth = vec![
        RegistryEnvVar {
            name: "SERVICE_CLIENT_ID".to_string(),
            is_secret: false,
            is_required: true,
        },
        RegistryEnvVar {
            name: "SERVICE_CLIENT_SECRET".to_string(),
            is_secret: true,
            is_required: true,
        },
    ];
    let api_key = vec![RegistryEnvVar {
        name: "GITHUB_PAT".to_string(),
        is_secret: true,
        is_required: true,
    }];
    let none = vec![RegistryEnvVar {
        name: "LOG_LEVEL".to_string(),
        is_secret: false,
        is_required: false,
    }];

    assert_eq!(
        personal_agent::mcp::detect_auth_type(&oauth),
        McpAuthType::OAuth
    );
    assert_eq!(
        personal_agent::mcp::detect_auth_type(&api_key),
        McpAuthType::ApiKey
    );
    assert_eq!(
        personal_agent::mcp::detect_auth_type(&none),
        McpAuthType::None
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn oauth_token_and_manager_cover_storage_state_and_url_generation() {
    let mcp_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let now: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch should be available")
        .as_secs()
        .try_into()
        .expect("unix timestamp should fit in i64");
    let valid_token = OAuthToken {
        access_token: "access-valid".to_string(),
        token_type: "Bearer".to_string(),
        refresh_token: Some("refresh".to_string()),
        expires_at: Some(now + 3600),
        scope: Some("read write".to_string()),
    };
    let expired_token = OAuthToken {
        access_token: "access-expired".to_string(),
        token_type: "Bearer".to_string(),
        refresh_token: None,
        expires_at: Some(now - 1),
        scope: None,
    };
    let no_expiry_token = OAuthToken {
        access_token: "forever".to_string(),
        token_type: "Bearer".to_string(),
        refresh_token: None,
        expires_at: None,
        scope: None,
    };
    let config = OAuthConfig {
        client_id: "client-123".to_string(),
        client_secret: "secret-abc".to_string(),
        auth_url: "https://auth.example.com/authorize".to_string(),
        token_url: "https://auth.example.com/token".to_string(),
        redirect_uri: "http://localhost:7788/callback".to_string(),
        scopes: vec!["read".to_string(), "write".to_string()],
    };
    let mut manager = OAuthManager::new();

    assert!(!valid_token.is_expired());
    assert!(expired_token.is_expired());
    assert!(!no_expiry_token.is_expired());
    assert!(manager.get_config(&mcp_id).is_none());
    assert!(manager.get_token(&mcp_id).is_none());
    assert!(!manager.has_valid_token(&mcp_id));
    assert_eq!(
        manager
            .generate_auth_url(mcp_id)
            .expect_err("missing config should fail"),
        "No OAuth config registered for MCP"
    );

    manager.register_config(mcp_id, config);
    assert_eq!(
        manager
            .get_config(&mcp_id)
            .expect("config stored")
            .client_id,
        "client-123"
    );

    let auth_url = manager
        .generate_auth_url(mcp_id)
        .expect("auth url should be generated");
    assert!(auth_url
        .starts_with("https://auth.example.com/authorize?response_type=code&client_id=client-123"));
    assert!(auth_url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A7788%2Fcallback"));
    assert!(auth_url.contains("scope=read%20write"));
    let state_key = auth_url
        .split("state=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .expect("state should exist")
        .to_string();
    assert_eq!(manager.get_mcp_for_state(&state_key), Some(mcp_id));

    manager.store_token(mcp_id, valid_token.clone());
    manager.store_token(other_id, expired_token);
    assert_eq!(
        manager
            .get_token(&mcp_id)
            .expect("token stored")
            .access_token,
        "access-valid"
    );
    assert!(manager.has_valid_token(&mcp_id));
    assert!(!manager.has_valid_token(&other_id));

    manager.clear_pending_flow(&state_key);
    assert_eq!(manager.get_mcp_for_state(&state_key), None);

    manager.delete_mcp(&mcp_id);
    assert!(manager.get_config(&mcp_id).is_none());
    assert!(manager.get_token(&mcp_id).is_none());
    assert!(!manager.has_valid_token(&mcp_id));

    let awaiting = OAuthFlowState::AwaitingCallback {
        state: "csrf-state".to_string(),
        pkce_verifier: Some("pkce".to_string()),
    };
    let received = OAuthFlowState::TokenReceived { token: valid_token };
    let error = OAuthFlowState::Error {
        message: "authorization failed".to_string(),
    };

    assert!(matches!(
        OAuthFlowState::NotStarted,
        OAuthFlowState::NotStarted
    ));
    assert!(
        matches!(awaiting, OAuthFlowState::AwaitingCallback { state, pkce_verifier } if state == "csrf-state" && pkce_verifier.as_deref() == Some("pkce"))
    );
    assert!(
        matches!(received, OAuthFlowState::TokenReceived { token } if token.access_token == "access-valid")
    );
    assert!(
        matches!(error, OAuthFlowState::Error { message } if message == "authorization failed")
    );

    let smithery_url = generate_smithery_oauth_url(&SmitheryOAuthConfig {
        server_qualified_name: "@owner/server-name".to_string(),
        redirect_uri: "http://localhost:4321/callback?source=test".to_string(),
    });
    assert_eq!(
        smithery_url,
        "https://smithery.ai/server/%40owner%2Fserver-name/authorize?redirect_uri=http%3A%2F%2Flocalhost%3A4321%2Fcallback%3Fsource%3Dtest"
    );
}

#[test]
fn registry_source_variants_are_available() {
    assert!(matches!(
        McpRegistrySource::Official,
        McpRegistrySource::Official
    ));
    assert!(matches!(
        McpRegistrySource::Smithery,
        McpRegistrySource::Smithery
    ));
}
