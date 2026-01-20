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
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expires_at| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                .unwrap_or(0);
            now >= expires_at
        })
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
///
/// # Errors
///
/// Returns an error if the server fails to start.
pub async fn start_oauth_callback_server(
) -> Result<(u16, oneshot::Receiver<OAuthCallbackResult>), String> {
    // Find available port
    let server =
        Server::http("127.0.0.1:0").map_err(|e| format!("Failed to start callback server: {e}"))?;

    let port = server
        .server_addr()
        .to_ip()
        .ok_or("Failed to get server address")?
        .port();

    let (tx, rx) = oneshot::channel();

    // Spawn task to handle callback
    tokio::task::spawn_blocking(move || {
        handle_oauth_callback(&server, tx);
    });

    Ok((port, rx))
}

/// Handle OAuth callback request
fn handle_oauth_callback(server: &Server, tx: oneshot::Sender<OAuthCallbackResult>) {
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
#[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn get_config(&self, mcp_id: &Uuid) -> Option<&OAuthConfig> {
        self.configs.get(mcp_id)
    }

    /// Store a token for an MCP
    pub fn store_token(&mut self, mcp_id: Uuid, token: OAuthToken) {
        self.tokens.insert(mcp_id, token);
    }

    /// Get token for an MCP
    #[must_use]
    pub fn get_token(&self, mcp_id: &Uuid) -> Option<&OAuthToken> {
        self.tokens.get(mcp_id)
    }

    /// Check if MCP has valid (non-expired) token
    #[must_use]
    pub fn has_valid_token(&self, mcp_id: &Uuid) -> bool {
        self.tokens.get(mcp_id).is_some_and(|t| !t.is_expired())
    }

    /// Generate authorization URL for OAuth flow
    ///
    /// # Errors
    ///
    /// Returns an error if no OAuth config is registered for the MCP.
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
            use std::fmt::Write;

            let _ = write!(
                url,
                "&scope={}",
                urlencoding::encode(&config.scopes.join(" "))
            );
        }

        Ok(url)
    }

    /// Get MCP ID from OAuth state parameter
    #[must_use]
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
