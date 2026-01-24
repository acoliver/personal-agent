# MCP Service Requirements

The MCP Service manages running MCP server instances - spawning, stopping, health monitoring, and providing tool access to the ChatService/Agent. This is **separate from McpRegistryService** which handles discovery/search.

---

## Responsibilities

- Spawn MCP server processes (stdio transport)
- Manage MCP server lifecycle (start, stop, restart)
- Track MCP health status
- Provide tool definitions to Agent
- Execute tool calls via MCP protocol
- Persist MCP configurations

---

## Service Interface

```rust
pub trait McpService: Send + Sync {
    // --- Configuration Management ---
    
    /// Add a new MCP configuration (doesn't start it)
    fn add(&self, config: &McpConfig) -> Result<Uuid>;
    
    /// Get MCP configuration by ID
    fn get(&self, id: Uuid) -> Result<Option<McpConfig>>;
    
    /// List all configured MCPs
    fn list(&self) -> Result<Vec<McpConfig>>;
    
    /// Update MCP configuration
    fn update(&self, id: Uuid, config: &McpConfig) -> Result<()>;
    
    /// Delete MCP configuration (stops it first if running)
    fn delete(&self, id: Uuid) -> Result<()>;
    
    // --- Lifecycle Management ---
    
    /// Start an MCP server
    async fn start(&self, id: Uuid) -> Result<()>;
    
    /// Stop an MCP server
    async fn stop(&self, id: Uuid) -> Result<()>;
    
    /// Restart an MCP server
    async fn restart(&self, id: Uuid) -> Result<()>;
    
    /// Get status of an MCP server
    fn status(&self, id: Uuid) -> Result<McpStatus>;
    
    /// Get status of all configured MCPs
    fn all_status(&self) -> Result<Vec<(Uuid, McpStatus)>>;
    
    // --- Tool Access ---
    
    /// Get all available tools from running MCPs
    fn available_tools(&self) -> Vec<McpTool>;
    
    /// Get toolsets for SerdesAI Agent integration
    /// Returns Vec of toolsets that can be attached to an Agent
    fn get_toolsets(&self) -> Vec<Arc<dyn AbstractToolset>>;
    
    /// Call a tool on an MCP server
    async fn call_tool(
        &self, 
        mcp_id: Uuid, 
        tool_name: &str, 
        arguments: &serde_json::Value
    ) -> Result<serde_json::Value>;
    
    /// Cancel an in-progress tool call
    /// Sends MCP `notifications/cancelled` to the server
    /// Success depends on MCP server implementation
    async fn cancel_tool_call(
        &self,
        mcp_id: Uuid,
        request_id: &str,
    ) -> Result<()>;
    
    /// Find which MCP provides a tool
    fn find_tool_provider(&self, tool_name: &str) -> Option<Uuid>;
}
```

---

## Data Model

### MCP Configuration

```rust
pub struct McpConfig {
    /// Unique identifier
    pub id: Uuid,
    
    /// User-friendly display name
    pub name: String,
    
    /// Optional description
    pub description: Option<String>,
    
    /// How to run this MCP
    pub transport: McpTransport,
    
    /// Environment variables to pass
    pub env_vars: Vec<EnvVar>,
    
    /// Auto-start on application launch
    pub auto_start: bool,
    
    /// Enabled (disabled MCPs won't provide tools)
    pub enabled: bool,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,
    
    /// Source registry (for updates)
    pub source: Option<McpRegistrySource>,
}

pub enum McpTransport {
    /// Stdio transport (spawn process)
    Stdio {
        /// Command to run (e.g., "npx")
        command: String,
        /// Arguments (e.g., ["-y", "@modelcontextprotocol/server-github"])
        args: Vec<String>,
        /// Working directory (optional)
        working_dir: Option<PathBuf>,
    },
    /// HTTP/SSE transport (connect to running server)
    Http {
        /// Server URL
        url: String,
    },
}

pub struct EnvVar {
    /// Variable name
    pub name: String,
    
    /// Value source
    pub value_source: EnvVarSource,
}

pub enum EnvVarSource {
    /// Plain text value (not recommended for secrets)
    Plain(String),
    
    /// Read from SecretsService
    Secret,
    
    /// Read from keyfile at runtime
    Keyfile(PathBuf),
    
    /// Read from environment
    Env(String),
}
```

### MCP Status

```rust
pub struct McpStatus {
    /// Current state
    pub state: McpState,
    
    /// Process ID (if running)
    pub pid: Option<u32>,
    
    /// Last error message
    pub error: Option<String>,
    
    /// When state last changed
    pub state_changed_at: DateTime<Utc>,
    
    /// Number of tools available
    pub tool_count: usize,
    
    /// Last health check result
    pub last_health_check: Option<DateTime<Utc>>,
}

pub enum McpState {
    /// Not started
    Stopped,
    /// Starting up (initializing)
    Starting,
    /// Running and healthy
    Running,
    /// Running but unhealthy
    Unhealthy,
    /// Process crashed
    Crashed,
    /// Stopping
    Stopping,
}
```

### MCP Tool

```rust
pub struct McpTool {
    /// Which MCP provides this tool
    pub mcp_id: Uuid,
    
    /// MCP name (for display)
    pub mcp_name: String,
    
    /// Tool name
    pub name: String,
    
    /// Tool description
    pub description: String,
    
    /// Input schema (JSON Schema)
    pub input_schema: serde_json::Value,
}
```

---

## Storage Format

### File Location

```
~/Library/Application Support/PersonalAgent/mcps/
├── a1b2c3d4-e5f6-....json
├── b2c3d4e5-f6a7-....json
└── ...
```

### MCP JSON Format

```json
{
  "id": "a1b2c3d4-e5f6-...",
  "name": "GitHub",
  "description": "GitHub repository access",
  "transport": {
    "type": "stdio",
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "working_dir": null
  },
  "env_vars": [
    {
      "name": "GITHUB_TOKEN",
      "value_source": { "type": "secret" }
    }
  ],
  "auto_start": true,
  "enabled": true,
  "created_at": "2025-01-21T19:30:00Z",
  "updated_at": "2025-01-21T20:45:00Z",
  "source": {
    "type": "official",
    "name": "server-github",
    "version": null
  }
}
```

---

## Lifecycle Operations

### Start MCP

| Step | Action |
|------|--------|
| 1 | Load config |
| 2 | Resolve environment variables |
| 3 | Spawn process or connect to HTTP endpoint |
| 4 | Initialize MCP protocol |
| 5 | Fetch tool list |
| 6 | Update status to Running |

```rust
async fn start(&self, id: Uuid) -> Result<()> {
    let config = self.get(id)?
        .ok_or(Error::NotFound)?;
    
    // Already running?
    if matches!(self.status(id)?.state, McpState::Running) {
        return Ok(());
    }
    
    self.set_status(id, McpState::Starting)?;
    
    // Resolve environment variables
    let env = self.resolve_env_vars(&config)?;
    
    // Start based on transport type
    let client = match &config.transport {
        McpTransport::Stdio { command, args, working_dir } => {
            self.start_stdio_mcp(command, args, working_dir.as_ref(), &env).await?
        }
        McpTransport::Http { url } => {
            self.connect_http_mcp(url).await?
        }
    };
    
    // Initialize and fetch tools
    let tools = client.list_tools().await?;
    
    // Store client and tools
    self.running_mcps.insert(id, RunningMcp {
        client,
        tools,
        started_at: Utc::now(),
    });
    
    self.set_status(id, McpState::Running)?;
    
    Ok(())
}
```

### Stop MCP

| Step | Action |
|------|--------|
| 1 | Set status to Stopping |
| 2 | Send shutdown to MCP client |
| 3 | Wait for graceful shutdown (timeout) |
| 4 | Force kill if needed |
| 5 | Remove from running MCPs |
| 6 | Set status to Stopped |

```rust
async fn stop(&self, id: Uuid) -> Result<()> {
    let running = self.running_mcps.remove(&id);
    
    let Some(running) = running else {
        return Ok(()); // Already stopped
    };
    
    self.set_status(id, McpState::Stopping)?;
    
    // Graceful shutdown
    if let Err(e) = tokio::time::timeout(
        Duration::from_secs(5),
        running.client.shutdown(),
    ).await {
        log::warn!("MCP {} didn't shutdown gracefully: {}", id, e);
        running.client.force_kill()?;
    }
    
    self.set_status(id, McpState::Stopped)?;
    
    Ok(())
}
```

### Restart MCP

```rust
async fn restart(&self, id: Uuid) -> Result<()> {
    self.stop(id).await?;
    self.start(id).await?;
    Ok(())
}
```

---

## Environment Variable Resolution

```rust
fn resolve_env_vars(&self, config: &McpConfig) -> Result<HashMap<String, String>> {
    let mut env = HashMap::new();
    
    for var in &config.env_vars {
        let value = match &var.value_source {
            EnvVarSource::Plain(v) => v.clone(),
            EnvVarSource::Secret => {
                self.secrets_service
                    .get(&SecretKey::mcp_env_var(config.id, &var.name))?
                    .ok_or(Error::SecretNotFound(var.name.clone()))?
            }
            EnvVarSource::Keyfile(path) => {
                self.secrets_service.read_keyfile(path)?
            }
            EnvVarSource::Env(name) => {
                std::env::var(name)
                    .map_err(|_| Error::EnvVarNotFound(name.clone()))?
            }
        };
        
        env.insert(var.name.clone(), value);
    }
    
    Ok(env)
}
```

---

## Tool Access

### Available Tools

Returns all tools from all running, enabled MCPs:

```rust
fn available_tools(&self) -> Vec<McpTool> {
    let mut tools = Vec::new();
    
    for (id, running) in &self.running_mcps {
        // Check if still enabled
        if let Ok(Some(config)) = self.get(*id) {
            if config.enabled {
                for tool in &running.tools {
                    tools.push(McpTool {
                        mcp_id: *id,
                        mcp_name: config.name.clone(),
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        input_schema: tool.input_schema.clone(),
                    });
                }
            }
        }
    }
    
    tools
}
```

### Call Tool

```rust
async fn call_tool(
    &self,
    mcp_id: Uuid,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value> {
    let running = self.running_mcps.get(&mcp_id)
        .ok_or(Error::McpNotRunning)?;
    
    let result = running.client
        .call_tool(tool_name, arguments)
        .await?;
    
    Ok(result)
}
```

### Find Tool Provider

```rust
fn find_tool_provider(&self, tool_name: &str) -> Option<Uuid> {
    for (id, running) in &self.running_mcps {
        if running.tools.iter().any(|t| t.name == tool_name) {
            return Some(*id);
        }
    }
    None
}
```

### Cancel Tool Call

Sends an MCP `notifications/cancelled` message to request cancellation of an in-progress tool call.

**Reference:** [MCP Specification - Cancellation](https://modelcontextprotocol.io/specification/2024-11-05/basic/utilities/cancellation)

```rust
async fn cancel_tool_call(
    &self,
    mcp_id: Uuid,
    request_id: &str,
) -> Result<()> {
    let running = self.running_mcps.get(&mcp_id)
        .ok_or(Error::McpNotRunning)?;
    
    // Send cancellation notification per MCP spec
    running.client.send_notification(
        "notifications/cancelled",
        json!({
            "requestId": request_id,
            "reason": "User requested cancellation"
        }),
    ).await?;
    
    // Note: MCP servers SHOULD stop processing but MAY continue
    // The result will be discarded by the caller regardless
    Ok(())
}
```

**Behavior Notes:**
- Cancellation is **best-effort** - the MCP server may or may not honor it
- Per MCP spec, servers SHOULD stop processing but are not required to
- Even if the server continues, the result will be discarded by SerdesAI
- Some MCPs (e.g., web requests) may not be cancellable mid-flight

### Get Toolsets (SerdesAI Integration)

Returns toolsets that ChatService attaches to the SerdesAI Agent:

```rust
fn get_toolsets(&self) -> Vec<Arc<dyn AbstractToolset>> {
    let mut toolsets = Vec::new();
    
    for (id, running) in &self.running_mcps {
        // Check if still enabled
        if let Ok(Some(config)) = self.get(*id) {
            if config.enabled {
                // Wrap the running MCP client as a toolset
                toolsets.push(Arc::new(McpToolset::new(
                    *id,
                    config.name.clone(),
                    running.client.clone(),
                )) as Arc<dyn AbstractToolset>);
            }
        }
    }
    
    toolsets
}
```

**Note:** `McpToolset` implements SerdesAI's `AbstractToolset` trait, which provides:
- `tools()` → Returns tool definitions
- `call()` → Executes tool calls

This allows the Agent to directly interact with MCPs without ChatService intermediation during streaming.

---

## Health Monitoring

### Health Check Loop

Runs periodically to detect crashed MCPs:

```rust
async fn health_check_loop(&self) {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        
        for (id, running) in &self.running_mcps {
            match running.client.ping().await {
                Ok(_) => {
                    self.set_status(*id, McpState::Running)?;
                }
                Err(e) => {
                    log::warn!("MCP {} health check failed: {}", id, e);
                    self.set_status_with_error(*id, McpState::Unhealthy, &e.to_string())?;
                }
            }
        }
    }
}
```

### Crash Detection

```rust
async fn watch_process(&self, id: Uuid, mut exit_rx: oneshot::Receiver<ExitStatus>) {
    match exit_rx.await {
        Ok(status) if !status.success() => {
            self.running_mcps.remove(&id);
            self.set_status_with_error(
                id,
                McpState::Crashed,
                &format!("Process exited with: {}", status),
            ).ok();
        }
        Ok(_) => {
            // Normal exit (we stopped it)
            self.running_mcps.remove(&id);
        }
        Err(_) => {
            // Channel closed
        }
    }
}
```

---

## Configuration Operations

### Add MCP

```rust
fn add(&self, config: &McpConfig) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let mut config = config.clone();
    config.id = id;
    config.created_at = Utc::now();
    config.updated_at = Utc::now();
    
    self.save_config(&config)?;
    
    Ok(id)
}
```

### Update MCP

```rust
fn update(&self, id: Uuid, config: &McpConfig) -> Result<()> {
    let mut existing = self.get(id)?
        .ok_or(Error::NotFound)?;
    
    // Preserve timestamps
    let mut updated = config.clone();
    updated.id = id;
    updated.created_at = existing.created_at;
    updated.updated_at = Utc::now();
    
    // If running and config changed significantly, restart
    let needs_restart = self.needs_restart(&existing, &updated);
    
    self.save_config(&updated)?;
    
    if needs_restart && matches!(self.status(id)?.state, McpState::Running) {
        self.restart(id).await?;
    }
    
    Ok(())
}

fn needs_restart(&self, old: &McpConfig, new: &McpConfig) -> bool {
    old.transport != new.transport || old.env_vars != new.env_vars
}
```

### Delete MCP

```rust
fn delete(&self, id: Uuid) -> Result<()> {
    // Stop if running
    if matches!(self.status(id)?.state, McpState::Running | McpState::Starting) {
        self.stop(id).await?;
    }
    
    // Delete secrets
    self.secrets_service.delete_all_for_resource(ResourceType::Mcp, id)?;
    
    // Delete config file
    let path = self.config_path(&id);
    std::fs::remove_file(&path)?;
    
    Ok(())
}
```

---

## Auto-Start

On application launch:

```rust
async fn auto_start_mcps(&self) -> Result<()> {
    let configs = self.list()?;
    
    for config in configs {
        if config.auto_start && config.enabled {
            if let Err(e) = self.start(config.id).await {
                log::error!("Failed to auto-start MCP {}: {}", config.name, e);
            }
        }
    }
    
    Ok(())
}
```

---

## UI Integration

### Settings View

| Action | Service Call |
|--------|--------------|
| Show MCPs | `list()` + `all_status()` |
| Toggle enabled | `update(id, ...)` |
| Delete MCP | `delete(id)` |
| Start MCP | `start(id)` |
| Stop MCP | `stop(id)` |
| Restart MCP | `restart(id)` |

### MCP Configure View

| Action | Service Call |
|--------|--------------|
| Save new MCP | `add(config)` |
| Load existing | `get(id)` |
| Update | `update(id, config)` |

### ChatService Integration

| Action | Service Call |
|--------|--------------|
| Get toolsets for Agent | `get_toolsets()` |
| Get tool list for display | `available_tools()` |

**Note:** ChatService calls `get_toolsets()` when building the SerdesAI Agent. The Agent then executes tool calls directly through the toolset interface. `call_tool()` is available for direct invocation if needed but typically the Agent handles this internally.

---

## Error Handling

| Error | Handling |
|-------|----------|
| Process spawn failed | Set Crashed, log error, report to UI |
| Initialization failed | Set Crashed, log error, report to UI |
| Tool call failed | Return error to agent, let agent handle |
| Health check failed | Set Unhealthy, UI shows warning |
| Secret not found | Don't start, report missing secret |

---

## Test Requirements

| ID | Test |
|----|------|
| MC-T1 | add() creates config file |
| MC-T2 | add() generates UUID |
| MC-T3 | get() returns None for non-existent |
| MC-T4 | list() returns all configs |
| MC-T5 | delete() removes config and secrets |
| MC-T6 | start() spawns stdio process |
| MC-T7 | start() connects to HTTP endpoint |
| MC-T8 | start() resolves env vars |
| MC-T9 | stop() terminates process |
| MC-T10 | status() reflects actual state |
| MC-T11 | available_tools() only from running MCPs |
| MC-T12 | available_tools() excludes disabled MCPs |
| MC-T13 | call_tool() routes to correct MCP |
| MC-T14 | call_tool() returns error for stopped MCP |
| MC-T15 | auto_start() starts enabled MCPs |
| MC-T16 | Health check detects unhealthy MCP |
| MC-T17 | Crash detection updates status |
| MC-T18 | get_toolsets() returns AbstractToolset implementations |
| MC-T19 | get_toolsets() excludes disabled MCPs |
| MC-T20 | Toolset executes tool calls correctly |
