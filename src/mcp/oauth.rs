//! OAuth 2.0 flow support for MCPs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tiny_http::{Response, Server};
use tokio::sync::oneshot;
use uuid::Uuid;

/// OAuth token storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl OAuthToken {
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            now >= expires_at
        } else {
            false // No expiry = never expires
        }
    }
}

/// Smithery OAuth configuration
#[derive(Debug, Clone)]
pub struct SmitheryOAuthConfig {
    pub server_qualified_name: String, // e.g., "@owner/server-name"
    pub redirect_uri: String,          // e.g., "http://localhost:PORT"
}

/// OAuth callback result
#[derive(Debug)]
pub struct OAuthCallbackResult {
    pub token: Option<String>,
    pub error: Option<String>,
}

/// Start a local HTTP server to receive OAuth callback
/// Returns (port, receiver for token)
pub async fn start_oauth_callback_server(
) -> Result<(u16, oneshot::Receiver<OAuthCallbackResult>), String> {
    // Find available port
    let server = Server::http("127.0.0.1:0")
        .map_err(|e| format!("Failed to start callback server: {}", e))?;

    let port = server
        .server_addr()
        .to_ip()
        .ok_or("Failed to get server address")?
        .port();

    let (tx, rx) = oneshot::channel();

    // Spawn task to handle callback
    tokio::task::spawn_blocking(move || {
        handle_oauth_callback(server, tx);
    });

    Ok((port, rx))
}

/// Handle OAuth callback request
fn handle_oauth_callback(server: Server, tx: oneshot::Sender<OAuthCallbackResult>) {
    // Wait for single request - recv() returns Result<Request, IoError>
    if let Ok(request) = server.recv() {
        let url = request.url();

        // Parse query parameters
        let mut token = None;
        let mut error = None;

        if let Some(query_start) = url.find('?') {
            let query = &url[query_start + 1..];
            for pair in query.split('&') {
                if let Some(eq_idx) = pair.find('=') {
                    let key = &pair[..eq_idx];
                    let value = &pair[eq_idx + 1..];

                    match key {
                        "access_token" | "token" => {
                            token =
                                Some(urlencoding::decode(value).unwrap_or_default().to_string());
                        }
                        "error" => {
                            error =
                                Some(urlencoding::decode(value).unwrap_or_default().to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        // Send success page
        let response_html = if token.is_some() {
            "<html><body><h1>Authentication Successful</h1><p>You can close this window.</p></body></html>"
        } else {
            "<html><body><h1>Authentication Failed</h1><p>You can close this window.</p></body></html>"
        };

        let response = Response::from_string(response_html).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
        );
        let _ = request.respond(response);

        // Send result
        let _ = tx.send(OAuthCallbackResult { token, error });
    }
}

/// Generate Smithery OAuth URL
pub fn generate_smithery_oauth_url(config: &SmitheryOAuthConfig) -> String {
    // Smithery OAuth URL format: https://smithery.ai/server/{qualified_name}/authorize?redirect_uri={uri}
    format!(
        "https://smithery.ai/server/{}/authorize?redirect_uri={}",
        urlencoding::encode(&config.server_qualified_name),
        urlencoding::encode(&config.redirect_uri)
    )
}

/// OAuth configuration for an MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// OAuth flow state
#[derive(Debug, Clone)]
pub enum OAuthFlowState {
    NotStarted,
    AwaitingCallback {
        state: String,
        pkce_verifier: Option<String>,
    },
    TokenReceived {
        token: OAuthToken,
    },
    Error {
        message: String,
    },
}

/// OAuth Manager handles OAuth flows for MCPs
pub struct OAuthManager {
    configs: HashMap<Uuid, OAuthConfig>,
    tokens: HashMap<Uuid, OAuthToken>,
    pending_flows: Arc<Mutex<HashMap<String, Uuid>>>, // state -> mcp_id
}

impl OAuthManager {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            tokens: HashMap::new(),
            pending_flows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register OAuth config for an MCP
    pub fn register_config(&mut self, mcp_id: Uuid, config: OAuthConfig) {
        self.configs.insert(mcp_id, config);
    }

    /// Get OAuth config for an MCP
    pub fn get_config(&self, mcp_id: &Uuid) -> Option<&OAuthConfig> {
        self.configs.get(mcp_id)
    }

    /// Store a token for an MCP
    pub fn store_token(&mut self, mcp_id: Uuid, token: OAuthToken) {
        self.tokens.insert(mcp_id, token);
    }

    /// Get token for an MCP
    pub fn get_token(&self, mcp_id: &Uuid) -> Option<&OAuthToken> {
        self.tokens.get(mcp_id)
    }

    /// Check if MCP has valid (non-expired) token
    pub fn has_valid_token(&self, mcp_id: &Uuid) -> bool {
        self.tokens
            .get(mcp_id)
            .map(|t| !t.is_expired())
            .unwrap_or(false)
    }

    /// Generate authorization URL for OAuth flow
    pub fn generate_auth_url(&mut self, mcp_id: Uuid) -> Result<String, String> {
        let config = self
            .configs
            .get(&mcp_id)
            .ok_or_else(|| "No OAuth config registered for MCP".to_string())?;

        // Generate state parameter for CSRF protection
        let state = uuid::Uuid::new_v4().to_string();

        // Store pending flow
        if let Ok(mut flows) = self.pending_flows.lock() {
            flows.insert(state.clone(), mcp_id);
        }

        // Build authorization URL
        let mut url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&state={}",
            config.auth_url,
            urlencoding::encode(&config.client_id),
            urlencoding::encode(&config.redirect_uri),
            urlencoding::encode(&state)
        );

        if !config.scopes.is_empty() {
            url.push_str(&format!(
                "&scope={}",
                urlencoding::encode(&config.scopes.join(" "))
            ));
        }

        Ok(url)
    }

    /// Get MCP ID from OAuth state parameter
    pub fn get_mcp_for_state(&self, state: &str) -> Option<Uuid> {
        self.pending_flows
            .lock()
            .ok()
            .and_then(|flows| flows.get(state).copied())
    }

    /// Clear pending flow
    pub fn clear_pending_flow(&mut self, state: &str) {
        if let Ok(mut flows) = self.pending_flows.lock() {
            flows.remove(state);
        }
    }

    /// Delete token and config for an MCP
    pub fn delete_mcp(&mut self, mcp_id: &Uuid) {
        self.configs.remove(mcp_id);
        self.tokens.remove(mcp_id);
    }
}

impl Default for OAuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "test_client".to_string(),
            client_secret: "test_secret".to_string(),
            auth_url: "https://auth.example.com/authorize".to_string(),
            token_url: "https://auth.example.com/token".to_string(),
            redirect_uri: "http://localhost:8080/callback".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
        }
    }

    fn create_test_token() -> OAuthToken {
        OAuthToken {
            access_token: "test_access_token".to_string(),
            token_type: "Bearer".to_string(),
            refresh_token: Some("test_refresh_token".to_string()),
            expires_at: None,
            scope: Some("read write".to_string()),
        }
    }

    #[test]
    fn test_token_is_not_expired_without_expiry() {
        let token = create_test_token();
        assert!(!token.is_expired());
    }

    #[test]
    fn test_token_is_expired_when_past_expiry() {
        let mut token = create_test_token();
        // Set expiry to 1 second ago
        token.expires_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - 1,
        );
        assert!(token.is_expired());
    }

    #[test]
    fn test_token_is_not_expired_when_before_expiry() {
        let mut token = create_test_token();
        // Set expiry to 3600 seconds in the future
        token.expires_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                + 3600,
        );
        assert!(!token.is_expired());
    }

    #[test]
    fn test_register_and_get_config() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let config = create_test_config();

        manager.register_config(mcp_id, config.clone());

        let retrieved = manager.get_config(&mcp_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().client_id, config.client_id);
    }

    #[test]
    fn test_get_nonexistent_config() {
        let manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();

        assert!(manager.get_config(&mcp_id).is_none());
    }

    #[test]
    fn test_store_and_get_token() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let token = create_test_token();

        manager.store_token(mcp_id, token.clone());

        let retrieved = manager.get_token(&mcp_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().access_token, token.access_token);
    }

    #[test]
    fn test_has_valid_token_no_token() {
        let manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();

        assert!(!manager.has_valid_token(&mcp_id));
    }

    #[test]
    fn test_has_valid_token_valid() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let token = create_test_token();

        manager.store_token(mcp_id, token);

        assert!(manager.has_valid_token(&mcp_id));
    }

    #[test]
    fn test_has_valid_token_expired() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let mut token = create_test_token();
        token.expires_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
                - 1,
        );

        manager.store_token(mcp_id, token);

        assert!(!manager.has_valid_token(&mcp_id));
    }

    #[test]
    fn test_generate_auth_url_no_config() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();

        let result = manager.generate_auth_url(mcp_id);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No OAuth config registered for MCP");
    }

    #[test]
    fn test_generate_auth_url_with_config() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let config = create_test_config();

        manager.register_config(mcp_id, config.clone());
        let result = manager.generate_auth_url(mcp_id);

        assert!(result.is_ok());
        let url = result.unwrap();

        // Check URL components
        assert!(url.starts_with(&config.auth_url));
        assert!(url.contains("response_type=code"));
        assert!(url.contains(&format!(
            "client_id={}",
            urlencoding::encode(&config.client_id)
        )));
        assert!(url.contains(&format!(
            "redirect_uri={}",
            urlencoding::encode(&config.redirect_uri)
        )));
        assert!(url.contains("state="));
        assert!(url.contains("scope=read%20write"));
    }

    #[test]
    fn test_generate_auth_url_no_scopes() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let mut config = create_test_config();
        config.scopes.clear();

        manager.register_config(mcp_id, config.clone());
        let result = manager.generate_auth_url(mcp_id);

        assert!(result.is_ok());
        let url = result.unwrap();

        assert!(!url.contains("scope="));
    }

    #[test]
    fn test_get_mcp_for_state() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let config = create_test_config();

        manager.register_config(mcp_id, config);
        let url = manager.generate_auth_url(mcp_id).unwrap();

        // Extract state from URL
        let state = url
            .split("state=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .unwrap();
        let decoded_state = urlencoding::decode(state).unwrap();

        let retrieved_mcp_id = manager.get_mcp_for_state(&decoded_state);
        assert_eq!(retrieved_mcp_id, Some(mcp_id));
    }

    #[test]
    fn test_get_mcp_for_invalid_state() {
        let manager = OAuthManager::new();
        let result = manager.get_mcp_for_state("invalid_state");
        assert!(result.is_none());
    }

    #[test]
    fn test_clear_pending_flow() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let config = create_test_config();

        manager.register_config(mcp_id, config);
        let url = manager.generate_auth_url(mcp_id).unwrap();

        // Extract state from URL
        let state = url
            .split("state=")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .unwrap();
        let decoded_state = urlencoding::decode(state).unwrap();

        assert!(manager.get_mcp_for_state(&decoded_state).is_some());

        manager.clear_pending_flow(&decoded_state);

        assert!(manager.get_mcp_for_state(&decoded_state).is_none());
    }

    #[test]
    fn test_delete_mcp() {
        let mut manager = OAuthManager::new();
        let mcp_id = Uuid::new_v4();
        let config = create_test_config();
        let token = create_test_token();

        manager.register_config(mcp_id, config);
        manager.store_token(mcp_id, token);

        assert!(manager.get_config(&mcp_id).is_some());
        assert!(manager.get_token(&mcp_id).is_some());

        manager.delete_mcp(&mcp_id);

        assert!(manager.get_config(&mcp_id).is_none());
        assert!(manager.get_token(&mcp_id).is_none());
    }

    #[test]
    fn test_oauth_token_serialization() {
        let token = create_test_token();
        let json = serde_json::to_string(&token).unwrap();
        let deserialized: OAuthToken = serde_json::from_str(&json).unwrap();

        assert_eq!(token.access_token, deserialized.access_token);
        assert_eq!(token.token_type, deserialized.token_type);
        assert_eq!(token.refresh_token, deserialized.refresh_token);
        assert_eq!(token.expires_at, deserialized.expires_at);
        assert_eq!(token.scope, deserialized.scope);
    }

    #[test]
    fn test_oauth_config_serialization() {
        let config = create_test_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: OAuthConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.client_id, deserialized.client_id);
        assert_eq!(config.client_secret, deserialized.client_secret);
        assert_eq!(config.auth_url, deserialized.auth_url);
        assert_eq!(config.token_url, deserialized.token_url);
        assert_eq!(config.redirect_uri, deserialized.redirect_uri);
        assert_eq!(config.scopes, deserialized.scopes);
    }

    #[test]
    fn test_oauth_config_default_scopes() {
        let json = r#"{
            "client_id": "test",
            "client_secret": "secret",
            "auth_url": "https://auth.example.com",
            "token_url": "https://token.example.com",
            "redirect_uri": "http://localhost:8080"
        }"#;

        let config: OAuthConfig = serde_json::from_str(json).unwrap();
        assert!(config.scopes.is_empty());
    }

    #[test]
    fn test_oauth_token_skip_none_fields() {
        let token = OAuthToken {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            refresh_token: None,
            expires_at: None,
            scope: None,
        };

        let json = serde_json::to_string(&token).unwrap();

        assert!(!json.contains("refresh_token"));
        assert!(!json.contains("expires_at"));
        assert!(!json.contains("scope"));
    }

    #[test]
    fn test_oauth_manager_default() {
        let manager = OAuthManager::default();
        let mcp_id = Uuid::new_v4();

        assert!(manager.get_config(&mcp_id).is_none());
        assert!(manager.get_token(&mcp_id).is_none());
        assert!(!manager.has_valid_token(&mcp_id));
    }

    #[test]
    fn test_multiple_mcps() {
        let mut manager = OAuthManager::new();
        let mcp_id1 = Uuid::new_v4();
        let mcp_id2 = Uuid::new_v4();

        let mut config1 = create_test_config();
        config1.client_id = "client1".to_string();
        let mut config2 = create_test_config();
        config2.client_id = "client2".to_string();

        manager.register_config(mcp_id1, config1);
        manager.register_config(mcp_id2, config2);

        let url1 = manager.generate_auth_url(mcp_id1).unwrap();
        let url2 = manager.generate_auth_url(mcp_id2).unwrap();

        assert!(url1.contains("client1"));
        assert!(url2.contains("client2"));
        assert_ne!(url1, url2);
    }
}
