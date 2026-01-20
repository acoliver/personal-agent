use personal_agent::mcp::{
    EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageType, McpRuntime, McpSource,
    McpTransport, SecretsManager,
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
async fn runtime_cleanup_idle_removes_connections() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut runtime = McpRuntime::new(secrets);

    let config = base_config();
    let result = runtime.start_mcp(&config).await;
    assert!(result.is_err());
    assert!(!runtime.has_active_mcps());

    runtime.cleanup_idle();
    assert!(!runtime.has_active_mcps());

    let stop_result = runtime.stop_mcp(&config.id);
    assert!(stop_result.is_ok());
}
