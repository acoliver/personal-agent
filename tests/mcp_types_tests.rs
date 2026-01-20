use personal_agent::mcp::{detect_auth_type, McpAuthType, RegistryEnvVar};

#[test]
fn detect_auth_type_prefers_oauth_when_client_secrets_present() {
    let vars = vec![
        RegistryEnvVar {
            name: "CLIENT_ID".to_string(),
            is_secret: false,
            is_required: true,
        },
        RegistryEnvVar {
            name: "CLIENT_SECRET".to_string(),
            is_secret: true,
            is_required: true,
        },
    ];

    assert_eq!(detect_auth_type(&vars), McpAuthType::OAuth);
}

#[test]
fn detect_auth_type_defaults_to_api_key_when_secret_present() {
    let vars = vec![RegistryEnvVar {
        name: "SERVICE_API_KEY".to_string(),
        is_secret: true,
        is_required: true,
    }];

    assert_eq!(detect_auth_type(&vars), McpAuthType::ApiKey);
}
