use personal_agent::mcp::{OAuthConfig, OAuthManager};
use uuid::Uuid;

#[test]
fn oauth_manager_handles_missing_config() {
    let mut manager = OAuthManager::new();
    let id = Uuid::new_v4();

    let result = manager.generate_auth_url(id);
    assert!(result.is_err());
}

#[test]
fn oauth_manager_encodes_scope_and_state() {
    let mut manager = OAuthManager::new();
    let id = Uuid::new_v4();

    manager.register_config(
        id,
        OAuthConfig {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            redirect_uri: "http://localhost:1234/callback".to_string(),
            scopes: vec!["scope:a".to_string(), "scope:b".to_string()],
        },
    );

    let url = manager.generate_auth_url(id).unwrap();
    assert!(url.contains("response_type=code"));
    assert!(url.contains("scope=scope%3Aa%20scope%3Ab"));

    let state = url
        .split("state=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .unwrap();

    assert_eq!(manager.get_mcp_for_state(state), Some(id));
}
