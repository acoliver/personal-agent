use personal_agent::mcp::{
    EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageArg, McpPackageArgType,
    McpPackageType, McpRuntime, McpSource, McpStatus, McpTransport, SecretsManager,
};
use tempfile::TempDir;
use uuid::Uuid;

fn base_config() -> McpConfig {
    McpConfig {
        id: Uuid::new_v4(),
        name: "Test MCP".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "https://example.com".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "https://example.com".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Http,
        auth_type: McpAuthType::None,
        env_vars: vec![EnvVarConfig {
            name: "API_KEY".to_string(),
            required: true,
        }],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

#[tokio::test]
async fn start_mcp_disabled_sets_stopped_status() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut runtime = McpRuntime::new(secrets);

    let mut config = base_config();
    config.enabled = false;

    let result = runtime.start_mcp(&config).await;
    assert!(result.is_err());

    let status = runtime.status_manager().get_status(&config.id);
    assert_eq!(status, McpStatus::Stopped);
}

#[tokio::test]
async fn start_mcp_missing_package_args_sets_error() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut runtime = McpRuntime::new(secrets);

    let mut config = base_config();
    config.package_args = vec![McpPackageArg {
        arg_type: McpPackageArgType::Named,
        name: "allowed-directories".to_string(),
        description: None,
        required: true,
        default: None,
    }];

    let result = runtime.start_mcp(&config).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Missing required package argument"));

    let status = runtime.status_manager().get_status(&config.id);
    assert!(matches!(status, McpStatus::Error(_)));
}

#[tokio::test]
async fn call_tool_without_provider_returns_error() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut runtime = McpRuntime::new(secrets);

    let result = runtime.call_tool("missing", serde_json::json!({})).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No MCP provides tool"));
}
