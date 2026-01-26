# MCP Service Requirements

The MCP Service manages Model Context Protocol servers - spawning, communication, and tool execution.

---

## Canonical Terminology

- McpConfig, McpSource, and EnvVarSpec are defined in `dev-docs/requirements/data-models.md`.
- McpService owns McpConfig persistence and runtime lifecycle.


## Responsibilities

- Spawn MCP server processes
- Manage server lifecycle (start, stop, restart)
- Execute tool calls
- Track server status
- Handle credentials/environment

---

## Service Interface

```rust
pub trait McpService: Send + Sync {
    /// Get all configured MCPs
    fn list(&self) -> Vec<McpConfig>;
    
    /// Get MCP by ID
    fn get(&self, id: Uuid) -> Option<McpConfig>;
    
    /// Get status of an MCP
    fn status(&self, id: Uuid) -> McpStatus;
    
    /// Start an MCP server
    async fn start(&self, id: Uuid) -> Result<()>;
    
    /// Stop an MCP server
    async fn stop(&self, id: Uuid) -> Result<()>;
    
    /// Get available tools from all running MCPs
    fn available_tools(&self) -> Vec<Tool>;
    
    /// Find which MCP provides a tool
    fn find_tool_provider(&self, tool_name: &str) -> Option<Uuid>;
    
    /// Execute a tool call
    async fn call_tool(
        &self,
        mcp_id: Uuid,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String>;
    
    /// Create MCP config
    fn create(&self, config: McpConfig) -> Result<()>;
    
    /// Update MCP config
    fn update(&self, config: McpConfig) -> Result<()>;
    
    /// Delete MCP config
    fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Shutdown all running MCPs
    async fn shutdown_all(&self) -> Result<()>;
}

pub enum McpStatus {
    Stopped,
    Starting,
    Running { tools: Vec<String> },
    Error { message: String },
    Disabled,
}
```

---

## Data Model

### McpConfig

```rust
pub struct McpConfig {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub source: McpSource,
    pub package: McpPackage,
    pub transport: McpTransport,
    pub env_vars: HashMap<String, EnvVarConfig>,
    pub config: serde_json::Value,
}

pub enum McpSource {
    Official { name: String, version: String },
    Smithery { qualified_name: String },
    Manual,
}

pub struct McpPackage {
    pub package_type: McpPackageType,
    pub identifier: String,
    pub runtime_hint: Option<String>,
}

pub enum McpPackageType {
    Npm,      // npx @scope/package
    Docker,   // docker run image
    Binary,   // Direct executable
    Http,     // HTTP endpoint
}

pub enum McpTransport {
    Stdio,
    Http { url: String },
}

pub struct EnvVarConfig {
    pub name: String,
    pub value: Option<String>,         // Direct value
    pub secret_ref: Option<String>,    // Reference to secrets store
    pub is_secret: bool,
}
```

---

## Server Lifecycle

### Startup Sequence

| Step | Action |
|------|--------|
| 1 | Check MCP is enabled |
| 2 | Resolve credentials from secrets |
| 3 | Build environment variables |
| 4 | Spawn process based on package type |
| 5 | Initialize MCP protocol handshake |
| 6 | Request tool list |
| 7 | Update status to Running |

### Spawn by Package Type

**NPM (npx)**
```bash
npx @scope/package [args]
```

**Docker**
```bash
docker run --rm -i image:tag [args]
```

**Binary**
```bash
/path/to/binary [args]
```

**HTTP**
- No process to spawn
- Connect to existing endpoint

### Shutdown Sequence

| Step | Action |
|------|--------|
| 1 | Send shutdown signal |
| 2 | Wait for graceful exit (timeout 5s) |
| 3 | Force kill if needed |
| 4 | Update status to Stopped |

---

## Tool Execution

### Call Flow

```
1. ChatService requests tool call
2. McpService finds provider MCP
3. McpService sends request via transport
4. MCP server executes tool
5. McpService receives result
6. Result returned to ChatService
```

### Request Format (JSON-RPC)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "search_web",
    "arguments": {
      "query": "rust async"
    }
  }
}
```

### Response Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Search results..."
      }
    ]
  }
}
```

---

## Error Handling

All errors use the standard error contract:

```json
{ "code": "string", "message": "string", "field": "string" }
```

### Startup Errors

| Error | Status | Notes |
|-------|--------|-------|
| Package not found | Error | code=NOT_FOUND, "Package not found" |
| Docker not running | Error | code=SERVICE_UNAVAILABLE, "Docker not running" |
| Auth missing | Error | code=VALIDATION_ERROR, "Missing required credentials" |
| Handshake timeout | Error | code=NETWORK_ERROR, "Handshake timed out" |

### Runtime Errors

| Error | Handling |
|-------|----------|
| Process crashed | Set status=Error, allow restart (code=SERVICE_UNAVAILABLE) |
| Tool call timeout | Return error result (code=NETWORK_ERROR) |
| Tool call failed | Return error in result (code=SERVICE_UNAVAILABLE) |
| Connection lost | Set status=Error (code=NETWORK_ERROR) |

### Graceful Degradation

- Individual MCP failure does NOT block others
- Tool list excludes failed MCPs
- Error status shown in UI
- Retry/restart available

---

## Credential Management

### Environment Variables

MCPs receive credentials via environment:

## Validation Rules

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| name | Non-empty after trim | VALIDATION_ERROR | Name is required |
| package.identifier | Non-empty | VALIDATION_ERROR | Package identifier required |
| transport | Required | VALIDATION_ERROR | Transport required |
| env_vars | Required secrets present | VALIDATION_ERROR | Missing required credentials |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| MC-NT1 | start disabled MCP | CONFLICT, status unchanged |
| MC-NT2 | call_tool with unknown mcp_id | NOT_FOUND, tool not executed |
| MC-NT3 | create with missing name | VALIDATION_ERROR, config not saved |
| MC-NT4 | registry env var required but not provided | VALIDATION_ERROR, save blocked |

## End-to-End Flow (MCP Toggle)

1. UI toggles enabled state.
2. McpService.update persists enabled flag.
3. If enabled: McpService.start, update status.
4. If start fails: revert enabled flag, return error.


```rust
fn build_environment(mcp: &McpConfig, secrets: &SecretsManager) -> HashMap<String, String> {
    let mut env = HashMap::new();
    
    for (name, config) in &mcp.env_vars {
        let value = if let Some(secret_ref) = &config.secret_ref {
            secrets.get(secret_ref).unwrap_or_default()
        } else {
            config.value.clone().unwrap_or_default()
        };
        env.insert(name.clone(), value);
    }
    
    env
}
```

### Secrets Storage

```
~/Library/Application Support/PersonalAgent/secrets/
├── mcp_{uuid}.key          # API keys
├── mcp_{uuid}.oauth        # OAuth tokens
└── ...
```

---

## Transport: Stdio

### Communication

- stdin: Send JSON-RPC requests
- stdout: Receive JSON-RPC responses
- stderr: Log/debug output (ignored)

### Protocol

1. Write request as single line JSON
2. Read response as single line JSON
3. Handle streaming for long operations

---

## Transport: HTTP

### Communication

- POST requests to endpoint
- JSON-RPC in request body
- JSON-RPC in response body

### Authentication

- Bearer token in Authorization header
- Or custom header per MCP config

---

## Test Requirements

| ID | Test |
|----|------|
| MC-T1 | Start spawns process correctly |
| MC-T2 | Stop terminates process |
| MC-T3 | Tool list populated after start |
| MC-T4 | Tool call returns result |
| MC-T5 | Failed MCP doesn't block others |
| MC-T6 | Environment variables set correctly |
| MC-T7 | Secrets resolved from store |
| MC-T8 | Shutdown all stops everything |
| MC-T9 | Status reflects actual state |
