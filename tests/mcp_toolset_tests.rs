use personal_agent::mcp::{
    build_command, build_env_for_config, McpAuthType, McpConfig, McpPackage, McpPackageArg,
    McpPackageArgType, McpPackageType, McpSource, McpTransport, SecretsManager,
};
use tempfile::TempDir;
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
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

    let config = base_config();
    secrets.store_api_key(config.id, "secret").unwrap();

    let env = build_env_for_config(&config, &secrets).unwrap();
    assert_eq!(env.get("API_KEY"), Some(&"secret".to_string()));
}
