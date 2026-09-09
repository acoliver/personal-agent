use personal_agent::mcp::{
    build_command, build_env_for_config, McpAuthType, McpConfig, McpPackage, McpPackageArg,
    McpPackageArgType, McpPackageType, McpSource, McpTransport, SecretsManager,
};
use uuid::Uuid;

fn base_config() -> McpConfig {
    McpConfig {
        id: Uuid::new_v4(),
        name: "Test MCP".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "test".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@test/mcp".to_string(),
            runtime_hint: Some("npx".to_string()),
        },
        transport: McpTransport::Stdio,
        auth_type: McpAuthType::ApiKey,
        env_vars: vec![personal_agent::mcp::EnvVarConfig {
            name: "API_KEY".to_string(),
            required: true,
            is_secret: true,
            value: None,
        }],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

#[test]
fn build_command_includes_named_package_args() {
    let mut config = base_config();
    config.package_args = vec![McpPackageArg {
        arg_type: McpPackageArgType::Named,
        name: "allowed-directories".to_string(),
        description: None,
        required: true,
        default: None,
    }];
    config.config = serde_json::json!({
        "package_args": {
            "allowed-directories": "/tmp,/var/log"
        }
    });

    let (_cmd, args) = build_command(&config);
    assert!(args.contains(&"--allowed-directories".to_string()));
    assert!(args.contains(&"/tmp".to_string()));
    assert!(args.contains(&"/var/log".to_string()));
}

#[test]
fn build_env_for_config_loads_secrets() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let config = base_config();
    secrets
        .store_api_key_named(config.id, "API_KEY", "secret")
        .unwrap();

    let env = build_env_for_config(&config, &secrets).unwrap();
    assert_eq!(env.get("API_KEY"), Some(&"secret".to_string()));
}

#[test]
fn build_env_for_config_errors_when_secret_is_missing() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let config = base_config();
    let result = build_env_for_config(&config, &secrets);
    assert!(
        result.is_err(),
        "ApiKey auth with no stored secret must fail fast instead of launching unauthenticated"
    );
}

#[test]
fn build_env_for_config_takes_non_secret_values_from_config() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.env_vars.push(personal_agent::mcp::EnvVarConfig {
        name: "REGION".to_string(),
        required: false,
        is_secret: false,
        value: Some("eu-west-1".to_string()),
    });
    secrets
        .store_api_key_named(config.id, "API_KEY", "secret")
        .unwrap();

    let env = build_env_for_config(&config, &secrets).unwrap();
    assert_eq!(env.get("API_KEY"), Some(&"secret".to_string()));
    assert_eq!(
        env.get("REGION"),
        Some(&"eu-west-1".to_string()),
        "non-secret vars must come from the config value, not the keychain"
    );
}

#[test]
fn build_env_for_config_keyfile_auth_does_not_touch_keychain() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.auth_type = McpAuthType::Keyfile;
    config.keyfile_path = Some(std::path::PathBuf::from("/tmp/keyfile-auth.json"));
    // The secret var HAS a stored entry; keyfile auth must ignore it.
    secrets
        .store_api_key_named(config.id, "API_KEY", "unused-keychain-value")
        .unwrap();
    config.env_vars.push(personal_agent::mcp::EnvVarConfig {
        name: "WORKSPACE".to_string(),
        required: false,
        is_secret: false,
        value: Some("acme".to_string()),
    });

    let env = build_env_for_config(&config, &secrets).unwrap();
    assert!(
        !env.contains_key("API_KEY"),
        "keyfile auth resolves from keyfile_path and must never keychain-load secret vars"
    );
    assert_eq!(env.get("WORKSPACE"), Some(&"acme".to_string()));
}
