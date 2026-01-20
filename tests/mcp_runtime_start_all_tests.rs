use personal_agent::mcp::{
    McpAuthType, McpConfig, McpPackage, McpPackageType, McpRuntime, McpSource, McpTransport,
    SecretsManager,
};
use personal_agent::Config;
use tempfile::TempDir;
use uuid::Uuid;

fn disabled_config(name: &str) -> McpConfig {
    McpConfig {
        id: Uuid::new_v4(),
        name: name.to_string(),
        enabled: false,
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
    }
}

#[tokio::test]
async fn start_all_returns_error_per_disabled_mcp() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut runtime = McpRuntime::new(secrets);

    let mut config = Config::default();
    let first = disabled_config("one");
    let second = disabled_config("two");
    config.mcps = vec![first.clone(), second.clone()];

    let results = runtime.start_all(&config).await;
    assert!(results.is_empty());
}
