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
fn build_headers_errors_when_keyfile_is_unreadable() {
    let temp_dir = TempDir::new().unwrap();
    let missing_keyfile = temp_dir.path().join("missing-token.txt");

    let mut config = base_config();
    config.oauth_token = None;
    config.keyfile_path = Some(PathBuf::from(&missing_keyfile));

    let secrets = SecretsManager::new();
    let err = build_headers_for_config(&config, &secrets).expect_err(
        "an unreadable keyfile must fail fast instead of sending an unauthenticated request",
    );
    let message = err.to_string();
    assert!(
        message.contains("cannot read keyfile"),
        "the error must explain the keyfile failure, got: {message}"
    );
    assert!(
        message.contains(missing_keyfile.to_string_lossy().as_ref()),
        "the error must name the unreadable path, got: {message}"
    );
    assert!(
        message.contains(&config.name),
        "the error must name the misconfigured MCP, got: {message}"
    );
}

#[test]
fn build_headers_sends_bearer_authorization_for_http_api_key_auth() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![EnvVarConfig {
        name: "EXA_API_KEY".to_string(),
        required: true,
        is_secret: true,
        value: None,
    }];
    secrets
        .store_api_key_named(config.id, "EXA_API_KEY", "exa-secret")
        .unwrap();

    // The shared header rule turns key-ish env names into a Bearer header.
    let headers = build_headers_for_config(&config, &secrets).unwrap();
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer exa-secret".to_string())
    );
}

#[test]
fn build_headers_errors_when_api_key_secret_is_missing() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![EnvVarConfig {
        name: "EXA_API_KEY".to_string(),
        required: true,
        is_secret: true,
        value: None,
    }];

    let result = build_headers_for_config(&config, &secrets);
    assert!(
        result.is_err(),
        "ApiKey auth over Http must fail fast without a stored secret"
    );
}

#[test]
fn build_headers_omits_all_headers_for_stdio_transport() {
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
        value: None,
    }];
    secrets
        .store_api_key_named(config.id, "EXA_API_KEY", "exa-secret")
        .unwrap();

    let headers = build_headers_for_config(&config, &secrets).unwrap();
    assert!(headers.is_empty());
}

#[test]
fn build_headers_errors_when_http_api_key_has_multiple_secret_vars() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    let mut config = base_config();
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![
        EnvVarConfig {
            name: "PRIMARY_TOKEN".to_string(),
            required: true,
            is_secret: true,
            value: None,
        },
        EnvVarConfig {
            name: "SECONDARY_TOKEN".to_string(),
            required: true,
            is_secret: true,
            value: None,
        },
    ];
    for var in &config.env_vars {
        secrets
            .store_api_key_named(config.id, &var.name, "some-secret")
            .unwrap();
    }

    let err = build_headers_for_config(&config, &secrets)
        .expect_err("zero or multiple secret vars must fail fast for Http + ApiKey");
    assert!(
        err.to_string().contains(&config.name),
        "the error must name the misconfigured MCP, got: {err}"
    );
}

#[test]
fn build_headers_names_bearer_and_custom_headers_by_var_role() {
    personal_agent::services::secure_store::use_mock_backend();
    let secrets = SecretsManager::new();

    // A single secret var whose name carries no key/token hint must reach
    // the wire as an X-{NAME} custom header, not a Bearer credential.
    let mut config = base_config();
    config.auth_type = McpAuthType::ApiKey;
    config.oauth_token = None;
    config.env_vars = vec![EnvVarConfig {
        name: "TENANT".to_string(),
        required: true,
        is_secret: true,
        value: None,
    }];
    secrets
        .store_api_key_named(config.id, "TENANT", "tenant-42")
        .unwrap();

    let headers = build_headers_for_config(&config, &secrets).unwrap();
    assert_eq!(headers.get("X-TENANT"), Some(&"tenant-42".to_string()));
    assert!(!headers.contains_key("Authorization"));
}

#[test]
fn derive_http_auth_header_splits_bearer_and_custom_names() {
    let (header, value) = personal_agent::mcp::derive_http_auth_header("EXA_API_KEY", "sk-1");
    assert_eq!(header, "Authorization");
    assert_eq!(value, "Bearer sk-1");

    let (header, value) = personal_agent::mcp::derive_http_auth_header("workspace", "acme");
    assert_eq!(header, "X-WORKSPACE");
    assert_eq!(value, "acme");
}
