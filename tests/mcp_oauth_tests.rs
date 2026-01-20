use personal_agent::mcp::{OAuthConfig, OAuthManager, OAuthToken};
use uuid::Uuid;

#[test]
fn oauth_manager_tracks_state_and_tokens() {
    let mut manager = OAuthManager::new();
    let mcp_id = Uuid::new_v4();

    let config = OAuthConfig {
        client_id: "client".to_string(),
        client_secret: "secret".to_string(),
        auth_url: "https://auth.example.com/authorize".to_string(),
        token_url: "https://auth.example.com/token".to_string(),
        redirect_uri: "http://localhost:1234".to_string(),
        scopes: vec!["profile".to_string(), "email".to_string()],
    };

    manager.register_config(mcp_id, config);
    let url = manager.generate_auth_url(mcp_id).unwrap();
    assert!(url.contains("response_type=code"));
    assert!(url.contains("client_id=client"));
    assert!(url.contains("scope=profile%20email"));

    let state = url
        .split("state=")
        .nth(1)
        .and_then(|value| value.split('&').next())
        .unwrap_or_default()
        .to_string();

    assert_eq!(manager.get_mcp_for_state(&state), Some(mcp_id));

    let token = OAuthToken {
        access_token: "access".to_string(),
        token_type: "Bearer".to_string(),
        refresh_token: None,
        expires_at: Some(i64::MAX),
        scope: None,
    };
    manager.store_token(mcp_id, token);
    assert!(manager.has_valid_token(&mcp_id));

    manager.clear_pending_flow(&state);
    assert!(manager.get_mcp_for_state(&state).is_none());

    manager.delete_mcp(&mcp_id);
    assert!(manager.get_token(&mcp_id).is_none());
}
