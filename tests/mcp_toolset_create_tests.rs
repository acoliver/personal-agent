use personal_agent::mcp::{
    create_toolset_from_config, McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource,
    McpTransport, SecretsManager,
};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn create_toolset_from_config_allows_http_without_command() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

    let config = McpConfig {
        id: Uuid::new_v4(),
        name: "HTTP".to_string(),
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
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    };

    let result = create_toolset_from_config(&config, &secrets).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn create_toolset_from_config_errors_for_stdio_without_command() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

    let config = McpConfig {
        id: Uuid::new_v4(),
        name: "Stdio".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Stdio,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    };

    let result = create_toolset_from_config(&config, &secrets).await;
    assert!(result.is_err());
}
