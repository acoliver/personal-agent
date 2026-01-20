use personal_agent::mcp::{OAuthConfig, OAuthManager};
use uuid::Uuid;

#[test]
fn oauth_manager_encodes_scopes_and_redirect() {
    let mut manager = OAuthManager::new();
    let mcp_id = Uuid::new_v4();

    manager.register_config(
        mcp_id,
        OAuthConfig {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            redirect_uri: "http://localhost:1234/callback".to_string(),
            scopes: vec!["scope:a".to_string(), "scope:b".to_string()],
        },
    );

    let url = manager.generate_auth_url(mcp_id).unwrap();
    assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1234%2Fcallback"));
    assert!(url.contains("scope=scope%3Aa%20scope%3Ab"));
}
