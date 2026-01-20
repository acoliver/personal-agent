use personal_agent::mcp::{
    build_headers_for_config, McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource,
    McpTransport,
};
use std::path::PathBuf;
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
        auth_type: McpAuthType::OAuth,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: Some("oauth-token".to_string()),
    }
}

#[test]
fn build_headers_prefers_oauth_token() {
    let config = base_config();
    let headers = build_headers_for_config(&config);

    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer oauth-token".to_string())
    );
}

#[test]
fn build_headers_falls_back_to_keyfile() {
    let temp_dir = TempDir::new().unwrap();
    let keyfile_path = temp_dir.path().join("token.txt");
    std::fs::write(&keyfile_path, "file-token").unwrap();

    let mut config = base_config();
    config.oauth_token = None;
    config.keyfile_path = Some(PathBuf::from(&keyfile_path));

    let headers = build_headers_for_config(&config);
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer file-token".to_string())
    );
}
