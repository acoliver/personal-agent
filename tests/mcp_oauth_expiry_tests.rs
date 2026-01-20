use personal_agent::mcp::OAuthToken;

#[test]
fn oauth_token_expiry_detection_handles_missing_and_past() {
    let token = OAuthToken {
        access_token: "token".to_string(),
        token_type: "Bearer".to_string(),
        refresh_token: None,
        expires_at: None,
        scope: None,
    };
    assert!(!token.is_expired());

    let expired = OAuthToken {
        access_token: "token".to_string(),
        token_type: "Bearer".to_string(),
        refresh_token: None,
        expires_at: Some(0),
        scope: None,
    };
    assert!(expired.is_expired());
}
