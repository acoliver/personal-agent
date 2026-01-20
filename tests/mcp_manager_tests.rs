use personal_agent::mcp::{
    McpAuthType, McpConfig, McpManager, McpPackage, McpPackageType, McpSource, McpTransport,
    SecretsManager,
};
use std::time::Duration;
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
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

#[test]
fn manager_tracks_activity_and_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::with_idle_timeout(secrets, Duration::from_secs(0));

    let config = base_config();
    manager.register_active(&config);
    assert!(manager.is_active(&config.id));

    manager.cleanup_idle();
    assert!(!manager.is_active(&config.id));
}

#[test]
fn manager_handle_config_change_stops_disabled_mcp() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);

    let mut config = base_config();
    manager.register_active(&config);
    assert!(manager.is_active(&config.id));

    config.enabled = false;
    manager.handle_config_change(&config).unwrap();
    assert!(!manager.is_active(&config.id));
}
