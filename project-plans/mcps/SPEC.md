# MCP Integration Specification

## Overview

This document specifies how PersonalAgent will discover, install, configure, authenticate, and connect to MCP (Model Context Protocol) servers, enabling the AI agent to use external tools like file systems, calendars, web search, and more.

## Goals

1. User adds an MCP via search or URL
2. User provides credentials (API key, keyfile path, or OAuth)
3. PersonalAgent stores credentials securely
4. When user asks agent to do something, agent can use MCP tools
5. No environment variable names exposed to user - all handled internally

---

## 1. User Interface

### 1.1 Entry Point: Settings Panel

MCPs are managed in the Settings panel alongside Profiles:

```
+------------------------------------------+
| < Settings                  Refresh Models|
+------------------------------------------+
| Profiles                                  |
| +--------------------------------------+ |
| | > Claude Sonnet 4               [*]  | |
| | > GPT-4o                              | |
| +--------------------------------------+ |
| |  -   +   Edit                        | |
| +--------------------------------------+ |
|                                          |
| MCPs                                     |
| +--------------------------------------+ |
| | [x] GitHub                           | |
| | [ ] Filesystem                       | |
| +--------------------------------------+ |
| |  -   +   Edit                        | |
| +--------------------------------------+ |
|                                          |
| Global Hotkey: [Cmd+Shift+Space    ]    |
+------------------------------------------+
```

- Checkbox = enabled/disabled
- Selected row = can Edit or Delete (-)
- Click + to add new MCP

### 1.2 Add MCP Screen

```
+------------------------------------------+
| < Add MCP                                 |
+------------------------------------------+
|                                          |
| URL: [________________________________]  |
|                                          |
| -- or search registry --                 |
|                                          |
| Registry: [Select...              v]     |
|            - Official                    |
|            - Smithery                    |
|            - Both                        |
|                                          |
| Search:   [________________________]     |
|           (select registry first)        |
|                                          |
| +--------------------------------------+ |
| |                                      | |
| |                                      | |
| |                                      | |
| +--------------------------------------+ |
|                                          |
|                                  [Next]  |
+------------------------------------------+
```

**Behavior:**
- URL field: paste npx command, docker image, or HTTP URL directly
- Registry dropdown: disabled search until selected
- Search field: enabled only after registry selected
- Results table: populated after search, click row to select
- Next button: enabled when URL has content OR a search result is selected

### 1.3 Add MCP Screen (with search results)

```
+------------------------------------------+
| < Add MCP                                 |
+------------------------------------------+
|                                          |
| URL: [________________________________]  |
|                                          |
| -- or search registry --                 |
|                                          |
| Registry: [Both                   v]     |
| Search:   [github_________________]      |
|                                          |
| +--------------------------------------+ |
| | > GitHub                  [Official] | |
| |   Manage repos, issues, PRs...       | |
| +--------------------------------------+ |
| |   GitHub                  [Smithery] | |
| |   GitHub integration for AI...       | |
| +--------------------------------------+ |
| |   GitHub Gist             [Smithery] | |
| |   Create and manage gists...         | |
| +--------------------------------------+ |
|                                          |
|                                  [Next]  |
+------------------------------------------+
```

**Behavior:**
- `>` indicates selected row
- [Official] or [Smithery] badge shows source
- Click Next to proceed to configuration

### 1.4 Configure MCP - API Key/PAT

When the MCP requires an API key or PAT (detected from registry metadata):

```
+------------------------------------------+
| < Configure: GitHub                       |
+------------------------------------------+
|                                          |
| Name: [GitHub_________________________]  |
|                                          |
| This MCP requires authentication.        |
|                                          |
| (*) API Key / PAT:                       |
|     [________________________________]   |
|                                          |
| ( ) Keyfile Path:                        |
|     [________________________________]   |
|     (e.g. ~/.github_token)               |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

**Behavior:**
- Radio buttons: choose one auth method
- API Key field: paste the actual key/token
- Keyfile Path field: paste path to file containing key (we read at runtime)
- Save: validates input, stores config, returns to Settings

### 1.5 Configure MCP - OAuth

When the MCP requires OAuth (detected from registry metadata showing CLIENT_ID/CLIENT_SECRET patterns):

```
+------------------------------------------+
| < Configure: GitHub                       |
+------------------------------------------+
|                                          |
| Name: [GitHub_________________________]  |
|                                          |
| This MCP requires OAuth authentication.  |
|                                          |
|      [  Authorize with GitHub  ]         |
|                                          |
| Status: Not connected                    |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

**After successful OAuth:**

```
+------------------------------------------+
| < Configure: GitHub                       |
+------------------------------------------+
|                                          |
| Name: [GitHub_________________________]  |
|                                          |
| This MCP requires OAuth authentication.  |
|                                          |
|      [  Reauthorize with GitHub  ]       |
|                                          |
| Status: Connected as @acoliver           |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

### 1.6 Configure MCP - No Auth Required

```
+------------------------------------------+
| < Configure: Filesystem                   |
+------------------------------------------+
|                                          |
| Name: [Filesystem____________________]   |
|                                          |
| No authentication required.              |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

### 1.7 Configure MCP - With Custom Config

Some MCPs have configSchema defining additional settings:

```
+------------------------------------------+
| < Configure: Filesystem                   |
+------------------------------------------+
|                                          |
| Name: [Filesystem____________________]   |
|                                          |
| No authentication required.              |
|                                          |
| Allowed Paths:                           |
| +--------------------------------------+ |
| | ~/Documents                      [-] | |
| | ~/Downloads                      [-] | |
| | [+ Add Path]                         | |
| +--------------------------------------+ |
|                                          |
|                          [Cancel] [Save] |
+------------------------------------------+
```

### 1.8 Delete MCP Confirmation

```
+------------------------------------------+
|     Delete "GitHub"?                     |
|                                          |
|  This will remove the MCP and its        |
|  stored credentials.                     |
|                                          |
|              [Cancel]  [Delete]          |
+------------------------------------------+
```

---

## 2. Registry Integration

### 2.1 Supported Registries

| Registry | API Endpoint | Auth Required | Notes |
|----------|--------------|---------------|-------|
| Official MCP | `https://registry.modelcontextprotocol.io/v0.1/servers` | No | Authoritative, has package info |
| Smithery | `https://registry.smithery.ai/servers` | No | Rich metadata, tools list |

### 2.2 Search API

**Official Registry:**
```
GET https://registry.modelcontextprotocol.io/v0.1/servers?search={query}&limit=10
```

Response:
```json
{
  "servers": [{
    "server": {
      "name": "io.github.owner/server-name",
      "description": "...",
      "title": "Friendly Name",
      "version": "1.0.2",
      "packages": [{
        "registryType": "npm",
        "identifier": "package-name",
        "runtimeHint": "npx",
        "transport": { "type": "stdio" },
        "environmentVariables": [
          { "name": "GITHUB_TOKEN", "isSecret": true, "isRequired": true }
        ]
      }]
    }
  }],
  "metadata": { "nextCursor": "...", "count": 3 }
}
```

**Smithery Registry:**
```
GET https://registry.smithery.ai/servers?q={query}&pageSize=10
```

Response:
```json
{
  "servers": [{
    "qualifiedName": "@owner/server-name",
    "displayName": "Friendly Name",
    "description": "...",
    "verified": true,
    "useCount": 11451
  }]
}
```

**Smithery Details:**
```
GET https://registry.smithery.ai/servers/{qualifiedName}
```

Response includes `tools` array and `connections` with `configSchema`.

### 2.3 Search Merge Strategy

1. Fire parallel requests to both registries
2. Collect results, tag each with source
3. Dedupe by similar names (fuzzy match)
4. Sort by: verified first, then useCount/popularity
5. Display merged list with [Official] or [Smithery] badges

---

## 3. Storage

### 3.1 Directory Structure

```
~/Library/Application Support/PersonalAgent/
+-- config.json              # Main config (profiles, mcp list, settings)
+-- cache/
|   +-- models.json          # models.dev cache
+-- secrets/
    +-- mcp_{uuid}.key       # API key (plain text, chmod 600)
    +-- mcp_{uuid}.oauth     # OAuth tokens (JSON)
```

### 3.2 Main Config (config.json)

```json
{
  "version": "1.0",
  "profiles": [...],
  "mcps": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "GitHub",
      "enabled": true,
      "source": {
        "type": "official",
        "name": "github/github-mcp-server",
        "version": "0.28.1"
      },
      "package": {
        "type": "npm",
        "identifier": "@github/mcp-server",
        "runtime_hint": "npx"
      },
      "transport": "stdio",
      "auth_type": "api_key",
      "keyfile_path": null,
      "config": {}
    }
  ],
  "global_hotkey": "Cmd+Shift+Space"
}
```

**Fields:**
- `id`: UUID for this MCP instance
- `name`: User-friendly display name
- `enabled`: Whether to load this MCP
- `source`: Where it came from (official/smithery/manual)
- `package`: How to install/run it
- `transport`: "stdio" or "http"
- `auth_type`: "none", "api_key", "keyfile", or "oauth"
- `keyfile_path`: Only if auth_type == "keyfile"
- `config`: MCP-specific settings from configSchema

### 3.3 API Key Storage (secrets/mcp_{uuid}.key)

Plain text file containing just the key:
```
ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

File permissions: `chmod 600` (owner read/write only)

### 3.4 OAuth Token Storage (secrets/mcp_{uuid}.oauth)

```json
{
  "provider": "github",
  "access_token": "gho_xxxxxxxxxxxxxxxxxxxx",
  "refresh_token": "ghr_xxxxxxxxxxxxxxxxxxxx",
  "expires_at": "2026-01-15T14:00:00Z",
  "scope": "repo,read:org"
}
```

File permissions: `chmod 600`

---

## 4. Authentication

### 4.1 Auth Type Detection

When user selects an MCP, we inspect the registry metadata:

1. **Check `environmentVariables` from Official registry:**
   - If has `*_TOKEN`, `*_PAT`, `*_API_KEY` with `isSecret: true` -> `api_key` type
   - If has `*_CLIENT_ID` AND `*_CLIENT_SECRET` -> `oauth` type
   - If no secret env vars -> `none` type

2. **Check `configSchema` from Smithery:**
   - Look for fields marked as secrets
   - Infer auth type from field names

3. **Fallback:** Show api_key option by default

### 4.2 API Key Flow

1. User pastes key OR keyfile path
2. If key: write to `secrets/mcp_{uuid}.key`
3. If keyfile: store path in config, read file at runtime
4. At spawn time: set env var with key value

**Multiple credentials:** Some MCPs require multiple env vars (e.g., AWS needs `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY`). The `environmentVariables` array from registry metadata tells us which vars are needed. Store each as `mcp_{uuid}_{var_name}.key`.

### 4.3 OAuth Flow

**Step 1: Initiate**
1. User clicks "Authorize with {Provider}"
2. We register a custom URL scheme: `personalagent://oauth/callback`
3. Open browser to provider's OAuth URL:
   ```
   https://github.com/login/oauth/authorize
     ?client_id={our_client_id}
     &redirect_uri=personalagent://oauth/callback
     &scope=repo,read:org
     &state={random_state}
   ```

**Step 2: Callback**
1. User authorizes in browser
2. Browser redirects to `personalagent://oauth/callback?code={auth_code}&state={state}`
3. macOS launches our app with the URL (custom URL scheme handler)
4. We extract auth code

**Step 3: Token Exchange**
1. POST to provider's token endpoint:
   ```
   POST https://github.com/login/oauth/access_token
   Content-Type: application/x-www-form-urlencoded
   
   client_id={id}&client_secret={secret}&code={auth_code}
   ```
2. Receive access_token, refresh_token, expires_in
3. Store in `secrets/mcp_{uuid}.oauth`

**Step 4: Token Refresh**
1. Before using MCP, check if `expires_at` is past
2. If expired, POST to refresh endpoint:
   ```
   POST https://github.com/login/oauth/access_token
   grant_type=refresh_token&refresh_token={token}&client_id={id}
   ```
3. Update stored tokens

**Note:** We need to register OAuth apps with providers (GitHub, Google, etc.) to get client_id/client_secret. For initial implementation, we may only support PAT/API key auth.

**OAuth client credentials (Phase 3):**
- Option A: Embed in binary (extractable but acceptable for open-source)
- Option B: User provides own OAuth app credentials (power user)
- Option C: Backend service holds credentials (requires server infrastructure)
- Decision deferred to Phase 3 implementation.

### 4.4 Custom URL Scheme Registration

**Option A: Info.plist (for bundled .app)**

If PersonalAgent is bundled as a `.app`, add to `Info.plist`:
```xml
<key>CFBundleURLTypes</key>
<array>
  <dict>
    <key>CFBundleURLName</key>
    <string>PersonalAgent OAuth</string>
    <key>CFBundleTypeRole</key>
    <string>Viewer</string>
    <key>CFBundleURLSchemes</key>
    <array>
      <string>personalagent</string>
    </array>
  </dict>
</array>
```

**Option B: Programmatic Registration**

For unbundled development, use `LSSetDefaultHandlerForURLScheme`:
```rust
// Register at startup
extern "C" {
    fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundleID: CFStringRef) -> OSStatus;
}
```

**Handling the URL callback:**

In macOS, URL scheme callbacks come via Apple Events. Using objc2:

```rust
use objc2_app_kit::NSAppleEventManager;
use objc2_foundation::{NSAppleEventDescriptor, ns_string};

// In applicationDidFinishLaunching or init:
unsafe {
    let event_manager = NSAppleEventManager::sharedAppleEventManager();
    event_manager.setEventHandler_andSelector_forEventClass_andEventID(
        &*delegate,                           // handler object
        sel!(handleGetURLEvent:withReplyEvent:),  // selector
        kInternetEventClass,                  // 0x4755524C = 'GURL'  
        kAEGetURL,                            // 0x4755524C = 'GURL'
    );
}

// Handler method on delegate:
#[method(handleGetURLEvent:withReplyEvent:)]
fn handle_url_event(
    &self,
    event: &NSAppleEventDescriptor,
    _reply: &NSAppleEventDescriptor
) {
    // Extract URL string from event
    let url_descriptor = unsafe { 
        event.paramDescriptorForKeyword(keyDirectObject) 
    };
    if let Some(desc) = url_descriptor {
        if let Some(url_string) = desc.stringValue() {
            // Parse: personalagent://oauth/callback?code=xxx&state=yyy
            self.handle_oauth_callback(&url_string);
        }
    }
}
```

**Constants:**
```rust
const kInternetEventClass: u32 = 0x4755524C; // 'GURL'
const kAEGetURL: u32 = 0x4755524C;           // 'GURL'
const keyDirectObject: u32 = 0x2D2D2D2D;     // '----'
```

**Alternative: Use `open` crate with localhost**

If URL scheme handling proves complex, fall back to localhost:
```rust
// Spawn temporary HTTP server
let listener = TcpListener::bind("127.0.0.1:0")?;
let port = listener.local_addr()?.port();
let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", port);

// Open browser
open::that(&auth_url)?;

// Wait for callback (with timeout)
let (stream, _) = listener.accept()?;
// Parse HTTP request, extract code parameter
```

Both approaches work - URL scheme is cleaner, localhost is simpler to implement.

---

## 5. MCP Installation & Execution

### 5.1 Package Types

| Type | Example | How to Run |
|------|---------|------------|
| npm | `@github/mcp-server` | `npx -y @github/mcp-server` |
| docker | `ghcr.io/github/github-mcp-server` | `docker run -i --rm {image}` |
| http | `https://server.smithery.ai/github/mcp` | HTTP transport |

### 5.2 Spawning stdio MCP

Using SerdesAI's `McpClient`:

```rust
use serdes_ai_mcp::McpClient;
use std::collections::HashMap;
use tokio::process::Command;

async fn spawn_mcp(config: &McpConfig) -> Result<McpClient, Error> {
    // Build environment with credentials
    let mut env: HashMap<String, String> = std::env::vars().collect();
    
    // Inject credentials based on auth_type
    match config.auth_type {
        AuthType::ApiKey => {
            let key = read_key_file(&secrets_path(&config.id))?;
            // Map to expected env var name from registry metadata
            env.insert("GITHUB_TOKEN".to_string(), key);
        }
        AuthType::Keyfile => {
            let key = std::fs::read_to_string(&config.keyfile_path.unwrap())?;
            env.insert("GITHUB_TOKEN".to_string(), key.trim().to_string());
        }
        AuthType::OAuth => {
            let tokens = read_oauth_tokens(&secrets_path(&config.id))?;
            // Refresh if needed
            let access_token = ensure_fresh_token(tokens).await?;
            env.insert("GITHUB_TOKEN".to_string(), access_token);
        }
        AuthType::None => {}
    }
    
    // Spawn with environment
    let (command, args) = match &config.package.type {
        PackageType::Npm { runtime_hint } => {
            (runtime_hint.clone(), vec!["-y".to_string(), config.package.identifier.clone()])
        }
        PackageType::Docker { image } => {
            ("docker".to_string(), vec!["run".to_string(), "-i".to_string(), "--rm".to_string(), image.clone()])
        }
    };
    
    // Note: SerdesAI's StdioTransport needs modification to accept env vars
    // Current implementation: Command::new(command).args(args).spawn()
    // We need: Command::new(command).args(args).envs(env).spawn()
    
    let client = McpClient::stdio(&command, &args_refs).await?;
    client.initialize().await?;
    
    Ok(client)
}
```

### 5.3 SerdesAI Modification Required

**Status:** Issue #3 filed upstream: https://github.com/janfeddersen-wq/serdesAI/issues/3

The current `StdioTransport::spawn` doesn't accept environment variables. Options:

1. **Wait for upstream PR** - Best long-term but blocks Phase 1

2. **Fork SerdesAI locally** - Add `spawn_with_env`:
   ```rust
   pub async fn spawn_with_env(
       command: &str, 
       args: &[&str],
       env: HashMap<String, String>
   ) -> McpResult<Self> {
       let mut cmd = Command::new(command);
       cmd.args(args)
          .envs(env)  // Add this
          .stdin(...)
          .stdout(...)
          .stderr(...)
          .spawn()
   }
   ```

3. **Wrap with our own spawn** (recommended for Phase 1):
   ```rust
   // In personal-agent crate, don't use McpClient::stdio directly
   pub async fn spawn_mcp_with_env(
       command: &str,
       args: &[&str],
       env: HashMap<String, String>,
   ) -> Result<McpClient> {
       // Spawn process ourselves with env vars
       let mut child = tokio::process::Command::new(command)
           .args(args)
           .envs(env)
           .stdin(std::process::Stdio::piped())
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped())
           .spawn()?;
       
       // Wrap child's stdin/stdout in a transport
       // Use SerdesAI's protocol layer on top
       // ...
   }
   ```

**Phase 1 plan:** Use option 3 (wrap) to unblock development. Switch to upstream once PR merged.

### 5.4 HTTP Transport

For MCPs with HTTP transport (like Smithery hosted servers):

```rust
let client = McpClient::http(&config.deployment_url)?;

// Add auth header if needed
// Note: SerdesAI's HttpTransport may need modification to add headers
```

---

## 6. Agent Integration

### 6.1 Loading MCPs at Startup

```rust
async fn load_enabled_mcps(config: &Config) -> Vec<McpToolset> {
    let mut toolsets = Vec::new();
    
    for mcp_config in config.mcps.iter().filter(|m| m.enabled) {
        match spawn_mcp(mcp_config).await {
            Ok(client) => {
                let toolset = McpToolset::new(client);
                toolsets.push(toolset);
            }
            Err(e) => {
                log::error!("Failed to load MCP {}: {}", mcp_config.name, e);
            }
        }
    }
    
    toolsets
}
```

### 6.2 Using MCPs in Agent

```rust
use serdes_ai::prelude::*;
use serdes_ai::mcp::McpToolset;

async fn create_agent_with_mcps(
    model: &str,
    system_prompt: &str,
    mcp_toolsets: Vec<McpToolset>
) -> Result<Agent, Error> {
    let mut builder = Agent::builder()
        .model(model)
        .system_prompt(system_prompt);
    
    for toolset in mcp_toolsets {
        builder = builder.toolset(toolset);
    }
    
    builder.build()
}
```

### 6.3 Tool Invocation Flow

1. User sends message: "Search GitHub for rust MCP libraries"
2. Agent sees available tools from GitHub MCP (e.g., `search_code`, `search_repositories`)
3. Agent decides to call `search_repositories` with query "rust MCP"
4. SerdesAI routes tool call to McpToolset
5. McpToolset calls `client.call_tool("search_repositories", args)`
6. MCP server executes, returns results
7. Agent incorporates results into response

---

## 7. Data Model (Rust)

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub source: McpSource,
    pub package: McpPackage,
    pub transport: McpTransport,
    pub auth_type: McpAuthType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyfile_path: Option<PathBuf>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpSource {
    Official { name: String, version: String },
    Smithery { qualified_name: String },
    Manual { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPackage {
    #[serde(rename = "type")]
    pub package_type: McpPackageType,
    pub identifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpPackageType {
    Npm,
    Docker,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthType {
    None,
    ApiKey,
    Keyfile,
    OAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub provider: String,
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}
```

---

## 8. Security Considerations

1. **Secrets Storage:**
   - All secrets in `~/Library/Application Support/PersonalAgent/secrets/`
   - File permissions: `chmod 600` (owner only)
   - Never log or display secrets

2. **OAuth State:**
   - Generate cryptographically random state parameter
   - Validate state on callback to prevent CSRF

3. **Token Handling:**
   - Refresh tokens before expiry
   - Clear tokens on MCP deletion

4. **Process Isolation:**
   - Each MCP runs as separate subprocess
   - Credentials passed via env vars (not visible to other processes)

---

## 9. MCP Lifecycle Management

### 9.1 McpManager Component

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct McpManager {
    active: HashMap<Uuid, ActiveMcp>,
    idle_timeout: Duration,
    max_restart_attempts: u32,
}

struct ActiveMcp {
    client: McpClient,
    config: McpConfig,
    started_at: Instant,
    last_used: Instant,
    restart_count: u32,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            idle_timeout: Duration::from_secs(30 * 60), // 30 min
            max_restart_attempts: 3,
        }
    }
    
    /// Start an MCP - called lazily on first tool use
    pub async fn start_mcp(&mut self, config: &McpConfig) -> Result<&McpClient> {
        if let Some(active) = self.active.get_mut(&config.id) {
            active.last_used = Instant::now();
            return Ok(&active.client);
        }
        
        let client = spawn_mcp(config).await?;
        self.active.insert(config.id, ActiveMcp {
            client,
            config: config.clone(),
            started_at: Instant::now(),
            last_used: Instant::now(),
            restart_count: 0,
        });
        
        Ok(&self.active.get(&config.id).unwrap().client)
    }
    
    /// Stop a specific MCP
    pub async fn stop_mcp(&mut self, id: Uuid) -> Result<()> {
        if let Some(mut active) = self.active.remove(&id) {
            active.client.close().await?;
        }
        Ok(())
    }
    
    /// Shutdown all MCPs (called on app quit)
    pub async fn shutdown_all(&mut self) -> Result<()> {
        for (_, mut active) in self.active.drain() {
            // Graceful shutdown with timeout
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                active.client.close()
            ).await;
        }
        Ok(())
    }
    
    /// Called periodically to clean up idle MCPs
    pub async fn cleanup_idle(&mut self) {
        let now = Instant::now();
        let idle_ids: Vec<_> = self.active.iter()
            .filter(|(_, a)| now.duration_since(a.last_used) > self.idle_timeout)
            .map(|(id, _)| *id)
            .collect();
        
        for id in idle_ids {
            let _ = self.stop_mcp(id).await;
        }
    }
    
    /// Handle MCP crash - attempt restart
    pub async fn handle_crash(&mut self, id: Uuid) -> Result<()> {
        if let Some(active) = self.active.get_mut(&id) {
            if active.restart_count >= self.max_restart_attempts {
                self.active.remove(&id);
                return Err(Error::MaxRestartsExceeded);
            }
            
            active.restart_count += 1;
            let config = active.config.clone();
            let new_client = spawn_mcp(&config).await?;
            active.client = new_client;
            active.started_at = Instant::now();
        }
        Ok(())
    }
}
```

### 9.2 Lifecycle Rules

| Event | Behavior |
|-------|----------|
| App startup | Load MCP configs but don't spawn (lazy) |
| First tool call | Spawn MCP, cache client |
| Tool call | Update `last_used` timestamp |
| 30min idle | Shutdown MCP (configurable) |
| MCP crash | Auto-restart (max 3 attempts) |
| App quit | SIGTERM all MCPs, wait 5s, SIGKILL |
| MCP disabled | Shutdown if running |
| MCP deleted | Shutdown + delete credentials |

---

## 10. Tool Namespace Handling

### 10.1 Problem

Multiple MCPs may expose tools with same names:
- Filesystem MCP: `read_file`
- GitHub MCP: `read_file` (for repo files)

### 10.2 Solution: Automatic Prefixing

All tools are prefixed with MCP name:
- `filesystem.read_file`
- `github.read_file`

```rust
impl McpToolset {
    pub fn tools_with_prefix(&self) -> Vec<Tool> {
        let prefix = &self.config.name.to_lowercase().replace(" ", "_");
        self.client.list_tools()
            .into_iter()
            .map(|t| Tool {
                name: format!("{}.{}", prefix, t.name),
                ..t
            })
            .collect()
    }
}
```

### 10.3 Agent Prompt Guidance

System prompt includes available tools with full names:
```
Available tools from MCPs:
- github.search_repositories: Search GitHub repos
- github.create_issue: Create a GitHub issue
- filesystem.read_file: Read a local file
- filesystem.write_file: Write to a local file
```

---

## 11. Error Handling & UX

### 11.1 Error States

| Scenario | User Feedback | Recovery |
|----------|---------------|----------|
| OAuth fails | "Authorization failed. Please try again." | Show Retry button |
| OAuth timeout | "Authorization timed out after 2 minutes." | Show Retry button |
| MCP crashes | Toast: "{MCP} disconnected. Reconnecting..." | Auto-restart |
| Restart fails | Toast: "{MCP} failed to restart. Check settings." | Manual retry |
| npx fails | "Failed to install {MCP}. Check network." | Manual retry |
| Invalid API key | "Authentication failed. Check your API key." | Edit settings |
| Tool timeout | "Tool call timed out after 30s" (in chat) | Can retry |
| Tool error | Show error message from MCP in chat | Depends |
| Rate limited | "Rate limited. Try again in X seconds." | Auto-retry with backoff |
| Network offline | "Network unavailable. Check connection." | Manual retry |

### 11.2 Chat View: Tool Call Display

When agent uses an MCP tool, show in chat:

```
┌────────────────────────────────────────────────────────────┐
│  Using github.search_repositories...                     │
│ ┌────────────────────────────────────────────────────────┐ │
│ │ query: "rust MCP client"                               │ │
│ │ sort: "stars"                                          │ │
│ └────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────┐
│ [OK] github.search_repositories completed                     │
│                                                            │
│ Found 12 repositories:                                     │
│                                                            │
│ 1. **anthropics/mcp** [STAR] 2.3k                              │
│    Official MCP protocol implementation                    │
│                                                            │
│ 2. **serdes-ai/mcp** [STAR] 156                                │
│    Rust MCP client and toolset                             │
│                                                            │
│ ...                                                        │
└────────────────────────────────────────────────────────────┘
```

**Tool call states:**
-  Spinner + "Using {tool}..." = in progress
- [OK] Green check + "completed" = success
-  Red X + error message = failure
-  Clock + "timed out" = timeout

### 11.3 Settings: MCP Status Indicators

```
+--------------------------------------+
| [x] GitHub              [Connected]  |
| [ ] Filesystem          [Idle]       |
| [x] Brave Search        [Error]      |
+--------------------------------------+
```

Status badges:
- **Connected** (green): Currently running
- **Idle** (gray): Enabled but not started
- **Error** (red): Failed to start or crashed
- **Disabled** (no badge): Checkbox unchecked

---

## 12. Performance Requirements

| Metric | Target | Notes |
|--------|--------|-------|
| MCP cold start | < 5s | Time to first tool call |
| Tool call latency | < 30s | Default timeout |
| Max concurrent MCPs | 10 | Memory constraint |
| Memory per MCP | < 100MB | Guideline |
| Idle timeout | 30min | Configurable |
| Health check interval | 60s | For running MCPs |
| Rate limit backoff | Exponential | 1s, 2s, 4s, 8s... max 60s |

---

## 13. Testing Strategy

### 13.1 Unit Tests

- Config parsing/serialization
- Auth type detection from registry metadata
- Tool name prefixing
- Token refresh logic

### 13.2 Integration Tests

Use SerdesAI's `MemoryTransport`:
```rust
#[tokio::test]
async fn test_tool_routing() {
    let transport = MemoryTransport::new();
    transport.push_response(/* mock response */).await;
    
    let client = McpClient::with_transport(transport);
    let toolset = McpToolset::new("test", client);
    
    let result = toolset.call("search", json!({"q": "test"})).await;
    assert!(result.is_ok());
}
```

### 13.3 E2E Tests (CI)

- Start real Filesystem MCP
- Call `list_directory` tool
- Verify results

---

## 14. Implementation Phases

### Phase 1: MVP
- [x] Spec complete
- [ ] **BLOCKER: Implement env var wrapper** (fork SerdesAI or wrap `Command::new()`)
- [ ] API key auth only (no OAuth)
- [ ] stdio transport only
- [ ] Manual MCP URL entry (no registry search)
- [ ] Single MCP at a time
- [ ] Basic tool display in chat

### Phase 2: Full
- [ ] Registry search (Official + Smithery)
- [ ] Multiple concurrent MCPs
- [ ] Tool namespace prefixing
- [ ] MCP lifecycle management
- [ ] Error recovery & restart

### Phase 3: Polish
- [ ] OAuth flow
- [ ] HTTP transport
- [ ] Tool filtering per MCP
- [ ] Custom config schemas
- [ ] Metrics/health monitoring

---

## 15. Future Considerations

1. **OAuth App Registration:**
   - Need to register PersonalAgent with GitHub, Google, etc.
   - Manage client_id/client_secret securely

2. **Tool Filtering:**
   - Allow user to enable/disable specific tools per MCP
   - Respect MCP's read-only mode

3. **Skills Standard:**
   - Future support for skills (higher-level abstractions over MCPs)
