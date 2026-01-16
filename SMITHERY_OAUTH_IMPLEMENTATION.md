# Smithery OAuth Implementation

## Research Findings

Based on research of Smithery documentation and the MCP SDK:

### Smithery OAuth Flow
1. **Authorization URL**: `https://smithery.ai/server/{qualified_name}/authorize?redirect_uri={callback_url}`
2. **User Authentication**: User logs into Smithery, authorizes the app, and configures server connections
3. **Token Exchange**: Smithery redirects back with OAuth tokens
4. **Authenticated Requests**: Tokens are sent via `Authorization: Bearer {token}` header

### Key Points
- Smithery uses OAuth for **remote hosted servers** only (server.smithery.ai/*)
- The MCP SDK handles the OAuth flow via an `OAuthProvider` interface
- Tokens must be stored and included in HTTP headers for all requests
- No client registration needed - uses Client ID Metadata Documents (CIMD)

## Implementation

### 1. OAuth Infrastructure (`src/mcp/oauth.rs`)

Added:
- `SmitheryOAuthConfig` - Configuration for Smithery OAuth flow
- `OAuthCallbackResult` - Result of OAuth callback with token or error
- `start_oauth_callback_server()` - Starts local HTTP server to receive OAuth callback
- `handle_oauth_callback()` - Handles the callback request and extracts token
- `generate_smithery_oauth_url()` - Generates the Smithery OAuth URL

The local callback server:
- Binds to `127.0.0.1:0` (random available port)
- Waits for a single OAuth callback request
- Parses `access_token` or `error` from query parameters
- Returns HTML page to user (success/failure)
- Sends result via oneshot channel

### 2. Token Storage (`src/mcp/types.rs`)

Added `oauth_token: Option<String>` field to `McpConfig`:
```rust
pub struct McpConfig {
    // ... existing fields ...
    /// OAuth token for Smithery or other OAuth-based MCPs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_token: Option<String>,
}
```

Updated all struct initializers across:
- `src/mcp/registry.rs` (2 locations)
- `src/mcp/types.rs` (2 test functions)
- `src/mcp/runtime.rs` (1 test function)
- `src/mcp/manager.rs` (1 test function)
- `src/ui/mcp_configure_view.rs` (1 location)

### 3. Runtime OAuth Support (`src/mcp/runtime.rs`)

Updated HTTP transport initialization to use OAuth tokens:
```rust
// Check for OAuth token first (highest priority for Smithery servers)
if let Some(ref oauth_token) = config.oauth_token {
    headers.insert("Authorization".to_string(), format!("Bearer {}", oauth_token));
} else {
    // Fall back to env var auth
    // ...
}
```

### 4. Dependencies (`Cargo.toml`)

Added `tiny_http = "0.12"` for the local OAuth callback server.

## Still TODO

### UI Integration (Not Yet Implemented)

The OAuth flow UI needs to be added to `src/ui/mcp_configure_view.rs`:

1. **Detect OAuth MCPs**: When `auth_type == OAuth` and source is Smithery
2. **Show OAuth Button**: Replace env var fields with "Connect with Smithery" button
3. **Handle OAuth Flow**:
   ```rust
   async fn connect_smithery_clicked(&self) {
       // Extract qualified name from config
       let qualified_name = extract_qualified_name(&self.config);
       
       // Start callback server
       let (port, receiver) = start_oauth_callback_server().await?;
       
       // Generate OAuth URL
       let oauth_config = SmitheryOAuthConfig {
           server_qualified_name: qualified_name,
           redirect_uri: format!("http://localhost:{}", port),
       };
       let oauth_url = generate_smithery_oauth_url(&oauth_config);
       
       // Open browser
       open_url(&oauth_url);
       
       // Wait for callback (with timeout)
       let result = tokio::time::timeout(
           Duration::from_secs(300), 
           receiver
       ).await??;
       
       // Store token if successful
       if let Some(token) = result.token {
           self.config.oauth_token = Some(token);
           // Save to config
       } else if let Some(error) = result.error {
           self.show_error("OAuth Error", &error);
       }
   }
   ```

4. **Update UI State**: Show "Connected [OK]" when token exists

### Registry Integration

The registry search already marks Smithery servers with `auth_type: OAuth`:
```rust
"smithery-oauth" => (McpTransport::Http, McpAuthType::OAuth)
```

This means when a user selects a Smithery server from the registry, it will already have:
- `transport: McpTransport::Http`
- `auth_type: McpAuthType::OAuth`
- `package.identifier: "https://server.smithery.ai/{qualified_name}"`

### Testing Plan

1. **Unit Tests**: OAuth callback server and URL generation
2. **Integration Test**: Full OAuth flow with mock Smithery server
3. **Manual Test**: Connect to real Smithery MCP (e.g., Google Drive)

## Architecture Notes

### Why Local Callback Server?

Smithery OAuth requires a redirect URI. Options:
1. **Custom URL scheme** (`personal-agent://callback`) - Requires macOS app registration
2. **Local HTTP server** (`http://localhost:PORT`) - Simple, works immediately

We chose option 2 for simplicity and cross-platform compatibility.

### Token Security

Currently stores tokens in:
- Config file (JSON on disk)
- Memory (McpConfig struct)

**Future improvement**: Use macOS Keychain via `security` command or keychain-services crate.

### Refresh Tokens

Smithery may provide refresh tokens. The `OAuthToken` struct supports this:
```rust
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    // ...
}
```

But the current implementation doesn't use this yet. Need to:
1. Parse full OAuth response (not just access_token)
2. Store OAuthToken in config instead of String
3. Implement token refresh before expiry

## Files Modified

1. `src/mcp/oauth.rs` - Added OAuth callback server and utilities
2. `src/mcp/types.rs` - Added oauth_token field to McpConfig
3. `src/mcp/registry.rs` - Added oauth_token: None to initializers
4. `src/mcp/runtime.rs` - Use oauth_token in HTTP headers
5. `src/mcp/manager.rs` - Added oauth_token: None to test config
6. `src/ui/mcp_configure_view.rs` - Added oauth_token: None to config creation
7. `Cargo.toml` - Added tiny_http dependency

## Build Status

[OK] Compiles successfully with `cargo build --bin personal_agent_menubar`

## Next Steps

1. **Implement OAuth UI flow** in mcp_configure_view.rs
2. **Add URL opening** functionality (use `open` command on macOS)
3. **Add timeout handling** for OAuth callback
4. **Test with real Smithery server** (e.g., @modelcontextprotocol/server-google-drive)
5. **Improve token security** with macOS Keychain
6. **Implement refresh token** handling for long-lived sessions
