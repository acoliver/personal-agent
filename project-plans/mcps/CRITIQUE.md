# MCP Specification Critique

Reviewed by: `reviewer` subagent (opus-thinking)

---

## Executive Summary

The spec provides a solid foundation but has significant gaps that must be addressed before implementation:

| Priority | Issue | Status |
|----------|-------|--------|
| **P0** | OAuth flow won't work with localhost - need URL scheme | WARNING: Spec updated |
| **P0** | SerdesAI doesn't support env var injection | WARNING: Needs upstream PR |
| **P0** | Credential storage not specified | WARNING: Spec updated |
| **P1** | MCP lifecycle management missing | WARNING: Added to spec |
| **P1** | Tool namespace collisions not handled |  TODO |
| **P1** | Error recovery flows undefined |  TODO |
| **P2** | Chat view tool display not wireframed |  TODO |
| **P2** | Data model incomplete | WARNING: Spec updated |

---

## Critical Issues & Resolutions

### 1. OAuth Flow - URL Scheme Required

**Problem:** The spec mentioned localhost callback server, which is problematic:
- macOS sandboxed apps cannot easily listen on ports
- Port conflicts with multiple instances
- Firewall may block

**Resolution:** Use custom URL scheme `personalagent://oauth/callback`

**Already addressed in SPEC.md Section 4.4**

### 2. SerdesAI Environment Variable Injection

**Problem:** SerdesAI's `StdioTransport::spawn()` doesn't accept environment variables:
```rust
Command::new(command)
    .args(args)
    .stdin(std::process::Stdio::piped())
    // No .envs() call!
```

**Resolution Options:**
1. **Fork SerdesAI** and add `envs` parameter (preferred - submit PR upstream)
2. **Wrap spawn** in our own function that sets env before calling
3. **Set process-wide env vars** before spawn (hacky, race condition risk)

**Recommendation:** Option 1 - submit PR to SerdesAI adding:
```rust
pub async fn spawn_with_env(
    command: &str,
    args: &[&str],
    env: HashMap<String, String>
) -> McpResult<Self>
```

### 3. Credential Storage - Keychain

**Problem:** Spec said "consider macOS Keychain" - not specific enough.

**Resolution:** Use `security-framework` crate for macOS Keychain:
```rust
const SERVICE: &str = "ai.personalagent.mcp";

fn store_credential(mcp_id: Uuid, key: &str, value: &str) -> Result<()> {
    let account = format!("{}.{}", mcp_id, key);
    set_generic_password(SERVICE, &account, value.as_bytes())?;
    Ok(())
}
```

**File-based fallback** for OAuth tokens (JSON with chmod 600):
- API keys → Keychain
- OAuth tokens → `secrets/mcp_{uuid}.oauth` (needs refresh logic)

### 4. MCP Lifecycle Management

**Problem:** No specification for:
- Tracking running MCP servers
- Graceful shutdown on app quit
- Crash recovery
- Timeout handling

**Resolution:** Add `McpManager` component:
```rust
struct McpManager {
    active: HashMap<Uuid, McpClient>,
    health_interval: Duration,
}

impl McpManager {
    async fn start_mcp(&mut self, config: &McpConfig) -> Result<()>;
    async fn stop_mcp(&mut self, id: Uuid) -> Result<()>;
    async fn shutdown_all(&mut self) -> Result<()>;
    async fn health_check(&self, id: Uuid) -> HealthStatus;
}
```

**Lifecycle rules:**
- Start MCPs lazily on first tool call (not at app start)
- Shutdown after 30min idle (configurable)
- Auto-restart crashed servers (max 3 attempts)
- Graceful shutdown on app quit (SIGTERM, wait 5s, SIGKILL)

### 5. Tool Namespace Collisions

**Problem:** Multiple MCPs might have same-named tools (e.g., `read_file`).

**Resolution:** Prefix tool names with MCP name:
- `github.search_repositories`
- `filesystem.read_file`

Or let user configure per-MCP prefix in settings.

---

## Missing Sections to Add

### Error Handling UX

| Scenario | User Feedback |
|----------|---------------|
| OAuth fails | "Authorization failed. Please try again." + Retry button |
| MCP crashes | Toast: "GitHub MCP disconnected. Reconnecting..." |
| npx download fails | "Failed to install MCP. Check network connection." |
| Tool timeout | "Tool call timed out after 30s" in chat |
| Invalid credentials | "Authentication failed. Check your API key." |

### Chat View Tool Display

```
┌────────────────────────────────────────────────────────┐
│   Using github.search_repositories...                │
│  ┌──────────────────────────────────────────────────┐  │
│  │ query: "rust MCP libraries"                      │  │
│  └──────────────────────────────────────────────────┘  │
│                                                        │
│  Found 15 repositories:                                │
│  1. anthropics/mcp-server - Official MCP servers      │
│  2. serdes-ai/mcp - Rust MCP implementation           │
│  ...                                                   │
└────────────────────────────────────────────────────────┘
```

### Testing Strategy

1. **Unit tests:** Mock `McpClient` for tool routing logic
2. **Integration tests:** Use `MemoryTransport` from SerdesAI
3. **E2E tests:** Spin up real MCP server (filesystem) in CI

### Performance Requirements

| Metric | Target |
|--------|--------|
| MCP cold start | < 5s |
| Tool call timeout | 30s default |
| Max concurrent MCPs | 10 |
| Memory per MCP | < 100MB |

---

## Recommendations

### Phase 1 (MVP)
- API key auth only (no OAuth)
- stdio transport only
- Manual MCP addition (no registry search)
- Single MCP at a time

### Phase 2
- Registry search (Smithery + Official)
- Multiple MCPs
- HTTP transport
- OAuth flow

### Phase 3
- Tool filtering per MCP
- Custom MCP trust levels
- Metrics/telemetry

---

## Action Items Before Implementation

1. [ ] Submit PR to SerdesAI for env var injection
2. [ ] Add `McpManager` to data model in SPEC.md
3. [ ] Wireframe tool display in chat view
4. [ ] Define all error states and UX
5. [ ] Decide: Keychain vs file storage for credentials
6. [ ] Add testing section to spec
