use personal_agent::mcp::{
    build_headers_for_config, EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageType,
    McpSource, McpTransport, SecretsManager,
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
    let secrets = SecretsManager::new();
    let headers = build_headers_for_config(&config, &secrets).unwrap();

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

    let secrets = SecretsManager::new();
    let headers = build_headers_for_config(&config, &secrets).unwrap();
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer file-token".to_string())
    );
}

#[test]
fn build_headers_emits_x_api_key_for_http_api_key_auth() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![EnvVarConfig {
        name: "EXA_API_KEY".to_string(),
        required: true,
        is_secret: true,
    }];
    secrets
        .store_api_key_named(config.id, "EXA_API_KEY", "exa-secret")
        .unwrap();

    let headers = build_headers_for_config(&config, &secrets).unwrap();
    assert_eq!(headers.get("x-api-key"), Some(&"exa-secret".to_string()));
}

#[test]
fn build_headers_errors_when_api_key_secret_is_missing() {
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![EnvVarConfig {
        name: "EXA_API_KEY".to_string(),
        required: true,
        is_secret: true,
    }];

    let result = build_headers_for_config(&config, &secrets);
    assert!(
        result.is_err(),
        "ApiKey auth over Http must fail fast without a stored secret"
    );
}

#[test]
fn build_headers_skips_x_api_key_for_stdio_transport() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.transport = McpTransport::Stdio;
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![EnvVarConfig {
        name: "EXA_API_KEY".to_string(),
        required: true,
        is_secret: true,
    }];
    secrets
        .store_api_key_named(config.id, "EXA_API_KEY", "exa-secret")
        .unwrap();

    let headers = build_headers_for_config(&config, &secrets).unwrap();
    assert!(headers.is_empty());
}
