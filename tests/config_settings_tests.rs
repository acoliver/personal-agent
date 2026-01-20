use personal_agent::config::Config;
use personal_agent::mcp::{
    McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport,
};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn config_get_enabled_mcps_filters_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut config = Config::load(config_path.clone()).unwrap();

    let mut mcp = McpConfig {
        id: Uuid::new_v4(),
        name: "MCP".to_string(),
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

    config.mcps.push(mcp.clone());
    mcp.enabled = false;
    mcp.id = Uuid::new_v4();
    config.mcps.push(mcp);

    let enabled = config.get_enabled_mcps();
    assert_eq!(enabled.len(), 1);
    assert!(enabled.iter().all(|entry| entry.enabled));
}
