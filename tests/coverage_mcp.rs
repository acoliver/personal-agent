#![allow(clippy::unwrap_used)]

use personal_agent::mcp::{
    McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn mcp_config_round_trip() {
    let config = McpConfig {
        id: Uuid::new_v4(),
        name: "Test MCP".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "https://example.com".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@example/mcp".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Stdio,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: json!({"package_args": {"allowed-directories": "/tmp"}}),
        oauth_token: None,
    };

    let serialized = serde_json::to_string(&config).unwrap();
    let decoded: McpConfig = serde_json::from_str(&serialized).unwrap();

    assert_eq!(decoded.name, "Test MCP");
    assert_eq!(decoded.package.identifier, "@example/mcp");
    assert!(decoded.enabled);
}
