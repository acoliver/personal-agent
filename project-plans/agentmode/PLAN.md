# Agent Mode Migration Plan

## Overview

Migrate PersonalAgent's MCP integration from our custom runtime management to SerdesAI's Agent mode with `McpToolset` support. This will:
1. Eliminate the tokio runtime lifecycle issues (MCP clients dying when temporary runtimes shut down)
2. Reduce our MCP code by ~1000 lines
3. Leverage SerdesAI's streaming + tool execution loop

## Current Architecture

```
PersonalAgent
├── src/mcp/
│   ├── manager.rs      (628 lines) - env building, command construction
│   ├── runtime.rs      (487 lines) - connection lifecycle, tool routing  
│   ├── service.rs      (212 lines) - global singleton, tool registry
│   ├── status.rs       (313 lines) - thread-safe status tracking
│   ├── secrets.rs      (264 lines) - keychain/keyfile storage
│   ├── registry.rs     (613 lines) - official + Smithery search
│   ├── types.rs        (326 lines) - McpConfig, McpSource, etc.
│   ├── oauth.rs        (573 lines) - OAuth infrastructure
│   └── mod.rs          (18 lines)
└── src/ui/chat_view.rs - Manual tool execution loop
```

**Current Problem**: We spawn MCP clients from temporary tokio runtimes. When those runtimes shut down, the clients' reader tasks die, causing "Tokio 1.x context was found, but it is being shutdown" errors.

## Target Architecture

```
PersonalAgent
├── src/mcp/
│   ├── secrets.rs      (keep) - keychain/keyfile storage
│   ├── registry.rs     (keep) - official + Smithery search  
│   ├── types.rs        (keep) - McpConfig, McpSource, etc.
│   ├── oauth.rs        (keep) - OAuth infrastructure
│   ├── toolset.rs      (NEW)  - Bridge to SerdesAI McpToolset
│   ├── status.rs       (REFACTOR) - Simplified status based on toolset state
│   └── mod.rs          (update)
├── src/agent/
│   ├── mod.rs          (NEW)  - PersonalAgent wrapper
│   └── runtime.rs      (NEW)  - Global runtime management
├── src/llm/
│   └── client_agent.rs (UPDATE) - Remove McpService usage, use PersonalAgent
└── src/ui/
    ├── chat_view.rs     (UPDATE) - Use Agent::run_stream() instead of manual loop
    ├── settings_view.rs (UPDATE) - Remove McpService usage
    └── mcp_add_view.rs  (UPDATE) - Remove McpService usage
```

**Files to DELETE**:
- `src/mcp/manager.rs` - env building moves to toolset.rs
- `src/mcp/runtime.rs` - McpToolset handles connections
- `src/mcp/service.rs` - Agent handles tool routing

**Files using McpService that must be updated**:
- `src/ui/chat_view.rs` - Primary user, manual tool loop
- `src/ui/settings_view.rs` - MCP status display, reload triggers
- `src/ui/mcp_add_view.rs` - Adding new MCPs
- `src/llm/client_agent.rs` - Tool execution bridge
- `src/mcp/mod.rs` - Re-exports

## Prerequisites

1. [OK] SerdesAI PR #4 merged (transport enhancements - spawn_with_env, with_headers)
2.  SerdesAI PR for `.toolset()` merged (issue #5)
3. Update Cargo.toml to use latest serdes-ai with toolset support

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| SerdesAI API changes | Medium | High | Pin version, add adapter layer |
| Runtime still dies | Medium | Critical | Add integration test before merge |
| Performance regression | Low | Medium | Add benchmarks before/after |
| User config breaks | Low | High | Test config loading |
| MCP reconnection fails | Medium | High | Add retry logic |

### Feature Flag Strategy

Use compile-time feature flag for gradual rollout:
```rust
// In src/lib.rs or src/main.rs
#[cfg(feature = "agent_mode")]
use crate::agent::PersonalAgent;

#[cfg(not(feature = "agent_mode"))]
use crate::mcp::McpService;

// In Cargo.toml
[features]
default = []
agent_mode = []  # Enable new agent-based architecture
```

**Example usage in chat_view.rs:**
```rust
fn send_message(&self, prompt: String) {
    #[cfg(feature = "agent_mode")]
    {
        // New path: Use PersonalAgent with global runtime
        use crate::agent::runtime::{run_in_agent_runtime, spawn_in_agent_runtime};
        use crate::agent::global_agent;
        
        // Clone data needed for async block
        let prompt = prompt.clone();
        let streaming_response = Arc::clone(&self.streaming_response);
        
        // Spawn in global runtime (non-blocking)
        spawn_in_agent_runtime(async move {
            // Access global agent with read lock
            let lock = global_agent().read().await;
            let agent = match lock.as_ref() {
                Some(a) => a,
                None => {
                    eprintln!("Agent not initialized");
                    return;
                }
            };
            
            // Run stream
            match agent.run_stream(&prompt).await {
                Ok(mut stream) => {
                    while let Some(event) = stream.next().await {
                        // Handle events (see Phase 5 for full implementation)
                    }
                }
                Err(e) => {
                    eprintln!("Agent stream error: {}", e);
                }
            }
        });
    }
    
    #[cfg(not(feature = "agent_mode"))]
    {
        // Old path: Use McpService with manual tool loop
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                // Old implementation...
            });
        });
    }
}
```

### Rollback Plan

1. Disable `agent_mode` feature flag
2. Rebuild with old code path
3. If structural changes made, revert to pre-migration commit
4. Document issues for future attempt

## Implementation Phases

### Phase Dependencies

```
Phase 0 (Prerequisites)
    |
    v
Phase 1 (Global Runtime) ──────────────────────┐
    |                                           |
    v                                           |
Phase 2 (Toolset Bridge) ← depends on Phase 1  |
    |                                           |
    v                                           |
Phase 3 (Agent Wrapper) ← depends on Phase 2   |
    |                                           |
    v                                           |
Phase 4 (Status Tracking) ← depends on Phase 2 |
    |                                           |
    v                                           |
Phase 5 (Chat View Update) ← depends on Phase 1, 3
    |
    v
Phase 5.5 (Integration Test)
    |
    v
Phase 6 (Cleanup) ← depends on all above
```

### Phase 0: Prerequisites (1-2 days)

Verify SerdesAI has all needed functionality.

**Verification Checklist**:
- [ ] `StdioTransport::spawn_with_env(cmd, args, env)` exists
- [ ] `HttpTransport::with_headers(url, headers)` exists  
- [ ] `McpToolset::new(client).with_id(name)` works
- [ ] `AgentBuilder::toolset(ts).build_async()` works
- [ ] Verify `AgentStreamEvent` variants match our usage:
  - `TextDelta { text: String }`
  - `ThinkingDelta { text: String }`
  - `ToolCallStart { tool_name, tool_call_id }`
  - `ToolExecuted { tool_name, tool_call_id, success, error }`
  - `RunComplete { run_id }`
  - `Error { message }`
- [ ] Create `MockMcpClient` or `MemoryTransport` for testing

**Tests**:
```rust
#[tokio::test]
async fn test_serdesai_spawn_with_env_exists() {
    let env = HashMap::from([("TEST".to_string(), "value".to_string())]);
    // Verify method exists and compiles
    let result = StdioTransport::spawn_with_env("echo", &["test"], env).await;
    // May fail due to process, but method exists
}

#[tokio::test]
async fn test_serdesai_http_with_headers_exists() {
    let headers = HashMap::from([("Authorization".to_string(), "Bearer test".to_string())]);
    let transport = HttpTransport::with_headers("http://example.com", headers);
    // Verify creation works
}

#[test]
fn test_agent_stream_event_variants_exist() {
    // Verify the variants we use exist in SerdesAI
    use serdes_ai::prelude::AgentStreamEvent;
    
    let _ = AgentStreamEvent::TextDelta { text: "test".to_string() };
    let _ = AgentStreamEvent::ThinkingDelta { text: "test".to_string() };
    let _ = AgentStreamEvent::ToolCallStart { 
        tool_name: "test".to_string(),
        tool_call_id: Some("id".to_string()),
    };
    let _ = AgentStreamEvent::ToolExecuted {
        tool_name: "test".to_string(),
        tool_call_id: Some("id".to_string()),
        success: true,
        error: None,
    };
    let _ = AgentStreamEvent::RunComplete { run_id: "test".to_string() };
    let _ = AgentStreamEvent::Error { message: "test".to_string() };
}
```

---

### Phase 1: Global Runtime Management (2-3 days)

**THE KEY FIX**: All MCP/Agent operations must happen in a single, long-lived runtime.

**Tests First** (`src/agent/runtime_tests.rs`):
```rust
#[test]
fn test_global_runtime_exists() {
    let runtime = agent_runtime();
    // Runtime exists and can spawn tasks
    let handle = runtime.spawn(async { 42 });
    let result = runtime.block_on(handle).unwrap();
    assert_eq!(result, 42);
}

#[test]
fn test_runtime_survives_multiple_calls() {
    // First spawn
    let result1 = run_in_agent_runtime(async { 1 });
    assert_eq!(result1, 1);
    
    // Second spawn - same runtime, still works
    let result2 = run_in_agent_runtime(async { 2 });
    assert_eq!(result2, 2);
}

#[test]
fn test_spawn_in_global_runtime() {
    let result = run_in_agent_runtime(async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        42
    });
    assert_eq!(result, 42);
}

#[test]
fn test_concurrent_operations() {
    let handles: Vec<_> = (0..10).map(|i| {
        std::thread::spawn(move || {
            run_in_agent_runtime(async move { i * 2 })
        })
    }).collect();
    
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.len(), 10);
}
```

**Implementation** (`src/agent/runtime.rs`):
```rust
use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

static AGENT_RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("agent-runtime")
        .build()
        .expect("Failed to create agent runtime")
});

/// Get the global agent runtime
pub fn agent_runtime() -> &'static Runtime {
    &AGENT_RUNTIME
}

/// Run a future in the agent runtime (blocking)
pub fn run_in_agent_runtime<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    AGENT_RUNTIME.block_on(future)
}

/// Spawn a task in the agent runtime (non-blocking)
pub fn spawn_in_agent_runtime<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    AGENT_RUNTIME.spawn(future)
}

/// Integration test to verify the tokio fix works
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_runtime_survives_after_spawn_completes() {
        // Create something in global runtime
        let handle = spawn_in_agent_runtime(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            "created"
        });
        
        // Wait for spawn to complete
        let result = handle.await.unwrap();
        assert_eq!(result, "created");
        
        // Wait a bit more
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Runtime should still be alive - this would fail with old approach
        let handle2 = spawn_in_agent_runtime(async {
            "still alive"
        });
        
        let result2 = handle2.await.unwrap();
        assert_eq!(result2, "still alive");
    }
}
```

---

### Phase 2: Toolset Bridge (2-3 days)

**Tests First** (`src/mcp/toolset_tests.rs`):
```rust
#[tokio::test]
async fn test_build_env_from_config() {
    let secrets = SecretsManager::new_temp();
    secrets.store("test_mcp", "API_KEY", "secret123").unwrap();
    
    let config = McpConfig {
        name: "test_mcp".to_string(),
        env_vars: vec![EnvVarConfig { name: "API_KEY".to_string(), required: true }],
        ..Default::default()
    };
    
    let env = build_env_for_config(&config, &secrets).unwrap();
    assert_eq!(env.get("API_KEY"), Some(&"secret123".to_string()));
}

#[tokio::test]
async fn test_build_env_missing_required_secret() {
    let secrets = SecretsManager::new_temp();
    // Don't store the secret
    
    let config = McpConfig {
        env_vars: vec![EnvVarConfig { name: "API_KEY".to_string(), required: true }],
        ..Default::default()
    };
    
    let result = build_env_for_config(&config, &secrets);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_build_headers_with_oauth_token() {
    let config = McpConfig {
        oauth_token: Some("oauth_token_123".to_string()),
        ..Default::default()
    };
    
    let headers = build_headers_for_config(&config);
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer oauth_token_123".to_string())
    );
}

#[tokio::test]
async fn test_build_headers_from_keyfile() {
    let temp_keyfile = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_keyfile.path(), "keyfile_token_456").unwrap();
    
    let config = McpConfig {
        keyfile_path: Some(temp_keyfile.path().to_string_lossy().to_string()),
        ..Default::default()
    };
    
    let headers = build_headers_for_config(&config);
    assert_eq!(
        headers.get("Authorization"),
        Some(&"Bearer keyfile_token_456".to_string())
    );
}

#[tokio::test]
async fn test_build_command_npm() {
    let config = McpConfig {
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@mcp/server-filesystem".to_string(),
            runtime_hint: Some("npx".to_string()),
        },
        ..Default::default()
    };
    
    let (cmd, args) = build_command(&config);
    assert_eq!(cmd, "npx");
    assert!(args.contains(&"-y".to_string()));
    assert!(args.contains(&"@mcp/server-filesystem".to_string()));
}

#[tokio::test]
async fn test_create_stdio_toolset_command_builds() {
    // Use a mock or simple command that exists
    let config = McpConfig {
        transport: McpTransport::Stdio,
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@echo/test".to_string(),
            runtime_hint: Some("echo".to_string()), // Use echo as test
        },
        ..Default::default()
    };
    
    // This tests the command building, not actual MCP connection
    let (cmd, args) = build_command(&config);
    assert!(!cmd.is_empty());
}

// Integration test with timeout
#[tokio::test]
async fn test_http_toolset_creation_timeout() {
    let config = McpConfig {
        transport: McpTransport::Http,
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "http://localhost:99999/nonexistent".to_string(),
            runtime_hint: None,
        },
        ..Default::default()
    };
    
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        create_toolset_from_config(&config)
    ).await;
    
    // Should timeout or error, not hang forever
    assert!(result.is_err() || result.unwrap().is_err());
}
```

**Implementation** (`src/mcp/toolset.rs`):
```rust
use serdes_ai_mcp::{McpClient, McpToolset, StdioTransport, HttpTransport};
use std::collections::HashMap;
use std::time::Duration;

const MCP_INIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Create a toolset from MCP configuration
pub async fn create_toolset_from_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<McpToolset, McpError> {
    let result = tokio::time::timeout(
        MCP_INIT_TIMEOUT,
        create_toolset_internal(config, secrets)
    ).await;
    
    match result {
        Ok(inner) => inner,
        Err(_) => Err(McpError::Timeout(format!(
            "MCP {} failed to initialize within {}s",
            config.name,
            MCP_INIT_TIMEOUT.as_secs()
        ))),
    }
}

async fn create_toolset_internal(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<McpToolset, McpError> {
    match config.transport {
        McpTransport::Stdio => create_stdio_toolset(config, secrets).await,
        McpTransport::Http => create_http_toolset(config).await,
    }
}

async fn create_stdio_toolset(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<McpToolset, McpError> {
    let (cmd, args) = build_command(config);
    let env = build_env_for_config(config, secrets)?;
    
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let transport = StdioTransport::spawn_with_env(&cmd, &args_refs, env).await?;
    let client = McpClient::new(transport);
    client.initialize().await?;
    
    Ok(McpToolset::new(client).with_id(&config.name))
}

async fn create_http_toolset(config: &McpConfig) -> Result<McpToolset, McpError> {
    let headers = build_headers_for_config(config);
    let transport = if headers.is_empty() {
        HttpTransport::new(&config.package.identifier)
    } else {
        HttpTransport::with_headers(&config.package.identifier, headers)
    };
    
    let client = McpClient::new(transport);
    client.initialize().await?;
    
    Ok(McpToolset::new(client).with_id(&config.name))
}

pub fn build_command(config: &McpConfig) -> (String, Vec<String>) {
    match config.package.package_type {
        McpPackageType::Npm => {
            let runtime = config.package.runtime_hint.as_deref().unwrap_or("npx");
            (
                runtime.to_string(),
                vec!["-y".to_string(), config.package.identifier.clone()],
            )
        }
        McpPackageType::Docker => {
            ("docker".to_string(), vec![
                "run".to_string(),
                "-i".to_string(),
                "--rm".to_string(),
                config.package.identifier.clone(),
            ])
        }
        McpPackageType::Http => {
            // HTTP doesn't need a command
            (String::new(), vec![])
        }
    }
}

pub fn build_env_for_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<HashMap<String, String>, McpError> {
    let mut env = HashMap::new();
    
    for var in &config.env_vars {
        if let Some(value) = secrets.load(&config.name, &var.name)? {
            env.insert(var.name.clone(), value);
        } else if var.required {
            return Err(McpError::MissingSecret(format!(
                "Required secret {} not found for MCP {}",
                var.name, config.name
            )));
        }
    }
    
    Ok(env)
}

pub fn build_headers_for_config(config: &McpConfig) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    
    // Priority: oauth_token > keyfile > env vars with token-like names
    if let Some(ref token) = config.oauth_token {
        headers.insert("Authorization".to_string(), format!("Bearer {}", token));
    } else if let Some(ref keyfile) = config.keyfile_path {
        if let Ok(token) = std::fs::read_to_string(keyfile) {
            headers.insert("Authorization".to_string(), format!("Bearer {}", token.trim()));
        }
    }
    
    headers
}
```

---

### Phase 3: Agent Wrapper (2-3 days)

**Tests First** (`src/agent/tests.rs`):
```rust
use crate::agent::{PersonalAgent, AgentError};

#[tokio::test]
async fn test_agent_creation_no_mcps() {
    let profile = Profile::test_default();
    let agent = PersonalAgent::new(&profile, &[]).await.unwrap();
    assert!(agent.tool_count() == 0);
}

#[tokio::test]
async fn test_agent_creation_with_disabled_mcp() {
    let profile = Profile::test_default();
    let config = McpConfig {
        enabled: false,
        ..McpConfig::test_default()
    };
    
    let agent = PersonalAgent::new(&profile, &[config]).await.unwrap();
    // Disabled MCPs should not create toolsets
    assert!(agent.tool_count() == 0);
}

#[tokio::test]
async fn test_agent_rebuild_on_profile_change() {
    let profile1 = Profile::test_with_prompt("You are assistant 1");
    let agent1 = PersonalAgent::new(&profile1, &[]).await.unwrap();
    
    let profile2 = Profile::test_with_prompt("You are assistant 2");
    let agent2 = PersonalAgent::new(&profile2, &[]).await.unwrap();
    
    // Different profiles should create different agents
    // (Can't easily test internal state, but verify no errors)
}

#[tokio::test]
async fn test_agent_tool_name_collision_errors() {
    // If two MCPs provide same tool name, should error
    let config1 = McpConfig {
        name: "mcp1".to_string(),
        // ... would need mock that provides "search" tool
    };
    let config2 = McpConfig {
        name: "mcp2".to_string(),
        // ... would need mock that provides "search" tool
    };
    
    // This test requires MockMcpClient - skip for now
}

#[tokio::test]
async fn test_agent_survives_across_calls() {
    let profile = Profile::test_default();
    let agent = PersonalAgent::new(&profile, &[]).await.unwrap();
    
    // First run
    let stream1 = agent.run_stream("Hello").await;
    assert!(stream1.is_ok());
    
    // Second run - should still work
    let stream2 = agent.run_stream("World").await;
    assert!(stream2.is_ok());
}

#[test]
fn test_agent_can_be_shared_across_threads() {
    use std::sync::Arc;
    
    let agent = Arc::new(run_in_agent_runtime(async {
        let profile = Profile::test_default();
        PersonalAgent::new(&profile, &[]).await.unwrap()
    }));
    
    let handles: Vec<_> = (0..5).map(|_| {
        let agent = Arc::clone(&agent);
        std::thread::spawn(move || {
            run_in_agent_runtime(async move {
                agent.run_stream("test").await.is_ok()
            })
        })
    }).collect();
    
    for handle in handles {
        assert!(handle.join().unwrap());
    }
}
```

**Implementation** (`src/agent/mod.rs`):
```rust
pub mod runtime;
#[cfg(test)]
mod tests;

use serdes_ai::prelude::*;
use serdes_ai_mcp::McpToolset;
use std::sync::Arc;

use crate::config::Profile;
use crate::mcp::{McpConfig, SecretsManager, create_toolset_from_config};

#[derive(Debug)]
pub enum AgentError {
    ModelError(String),
    ToolsetError(String),
    BuildError(String),
}

pub struct PersonalAgent {
    agent: Agent<(), String>,
    #[allow(dead_code)]
    toolsets: Vec<Arc<McpToolset>>,
}

/// Global agent singleton - created once, lives for program lifetime
/// Use get_or_init_agent() to access
static GLOBAL_AGENT: once_cell::sync::OnceCell<tokio::sync::RwLock<Option<PersonalAgent>>> = 
    once_cell::sync::OnceCell::new();

/// Get the global agent lock
pub fn global_agent() -> &'static tokio::sync::RwLock<Option<PersonalAgent>> {
    GLOBAL_AGENT.get_or_init(|| tokio::sync::RwLock::new(None))
}

/// Initialize or reinitialize the global agent
/// Call this when profile or MCP config changes
pub async fn init_global_agent(
    profile: &Profile,
    mcp_configs: &[McpConfig],
) -> Result<(), AgentError> {
    let agent = PersonalAgent::new(profile, mcp_configs).await?;
    let mut lock = global_agent().write().await;
    *lock = Some(agent);
    Ok(())
}

/// Get the current agent for running prompts
/// Returns None if not initialized
pub async fn with_global_agent<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&PersonalAgent) -> T,
{
    let lock = global_agent().read().await;
    lock.as_ref().map(f)
}

impl PersonalAgent {
    pub async fn new(
        profile: &Profile,
        mcp_configs: &[McpConfig],
    ) -> Result<Self, AgentError> {
        let secrets = SecretsManager::default();
        Self::new_with_secrets(profile, mcp_configs, &secrets).await
    }
    
    pub async fn new_with_secrets(
        profile: &Profile,
        mcp_configs: &[McpConfig],
        secrets: &SecretsManager,
    ) -> Result<Self, AgentError> {
        // Build model from profile
        let model = Self::build_model(profile)?;
        
        // Start building the agent
        let mut builder = agent(model);
        
        if let Some(ref prompt) = profile.system_prompt {
            builder = builder.system_prompt(prompt.clone());
        }
        
        // Temperature, etc.
        if let Some(temp) = profile.temperature {
            builder = builder.temperature(temp);
        }
        
        // Create toolsets for enabled MCPs
        let mut toolsets = Vec::new();
        for config in mcp_configs.iter().filter(|c| c.enabled) {
            match create_toolset_from_config(config, secrets).await {
                Ok(toolset) => {
                    let arc = Arc::new(toolset);
                    toolsets.push(arc.clone());
                    // Note: SerdesAI's .toolset() may need Arc support
                    // or we adapt here
                    builder = builder.toolset(arc.as_ref().clone());
                }
                Err(e) => {
                    tracing::error!(
                        mcp_name = %config.name,
                        error = %e,
                        "Failed to create MCP toolset, skipping"
                    );
                    // Continue with other MCPs
                }
            }
        }
        
        // Build the agent
        let agent = builder.build_async().await
            .map_err(|e| AgentError::BuildError(e.to_string()))?;
        
        Ok(Self { agent, toolsets })
    }
    
    fn build_model(profile: &Profile) -> Result<Arc<dyn Model>, AgentError> {
        // Use SerdesAI's model building
        let model_spec = format!("{}:{}", profile.provider, profile.model);
        
        let config = ModelConfig::new(&model_spec)
            .with_api_key(&profile.api_key);
        
        config.build_model()
            .map_err(|e| AgentError::ModelError(e.to_string()))
    }
    
    pub async fn run_stream(
        &self,
        prompt: &str,
    ) -> Result<AgentStream, AgentError> {
        self.agent.run_stream(prompt, ()).await
            .map_err(|e| AgentError::BuildError(e.to_string()))
    }
    
    pub fn tool_count(&self) -> usize {
        self.agent.tool_definitions().len()
    }
}
```

---

### Phase 4: Status Tracking (1-2 days)

**Tests First** (`src/mcp/status_tests.rs`):
```rust
#[tokio::test]
async fn test_status_from_toolset_connected() {
    // Create a toolset that successfully connects
    let toolset = create_mock_toolset_connected();
    let status = get_toolset_status(&toolset).await;
    assert_eq!(status, McpStatus::Running);
}

#[tokio::test]
async fn test_status_from_toolset_disconnected() {
    // Create a toolset that fails to connect
    let toolset = create_mock_toolset_failed("Connection refused");
    let status = get_toolset_status(&toolset).await;
    assert!(matches!(status, McpStatus::Error(_)));
}

#[tokio::test]
async fn test_status_for_disabled_mcp() {
    let config = McpConfig { enabled: false, ..Default::default() };
    let status = get_config_status(&config);
    assert_eq!(status, McpStatus::Disabled);
}

#[tokio::test]
async fn test_aggregate_status_all_running() {
    let statuses = vec![McpStatus::Running, McpStatus::Running];
    let aggregate = aggregate_mcp_status(&statuses);
    assert_eq!(aggregate, AggregateStatus::AllHealthy);
}

#[tokio::test]
async fn test_aggregate_status_some_errors() {
    let statuses = vec![McpStatus::Running, McpStatus::Error("test".to_string())];
    let aggregate = aggregate_mcp_status(&statuses);
    assert_eq!(aggregate, AggregateStatus::PartialFailure);
}
```

**Implementation** (`src/mcp/status.rs` - refactored):
```rust
use serdes_ai_mcp::McpToolset;

#[derive(Debug, Clone, PartialEq)]
pub enum McpStatus {
    Disabled,
    Starting,
    Running,
    Error(String),
    Stopped,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateStatus {
    AllHealthy,
    PartialFailure,
    AllFailed,
    NoMcps,
}

/// Get status for a single toolset
pub async fn get_toolset_status(toolset: &McpToolset) -> McpStatus {
    // Check if the toolset's underlying client is connected
    // McpToolset tracks this via its client's is_initialized state
    if toolset.cached_tools().is_some() {
        McpStatus::Running
    } else {
        // Try to refresh - if it fails, we're in error state
        match toolset.refresh().await {
            Ok(_) => McpStatus::Running,
            Err(e) => McpStatus::Error(e.to_string()),
        }
    }
}

/// Get status for a config (before toolset creation)
pub fn get_config_status(config: &McpConfig) -> McpStatus {
    if !config.enabled {
        McpStatus::Disabled
    } else {
        McpStatus::Starting
    }
}

/// Aggregate status from multiple MCPs
pub fn aggregate_mcp_status(statuses: &[McpStatus]) -> AggregateStatus {
    if statuses.is_empty() {
        return AggregateStatus::NoMcps;
    }
    
    let running = statuses.iter().filter(|s| matches!(s, McpStatus::Running)).count();
    let errors = statuses.iter().filter(|s| matches!(s, McpStatus::Error(_))).count();
    
    if errors == statuses.len() {
        AggregateStatus::AllFailed
    } else if errors > 0 {
        AggregateStatus::PartialFailure
    } else if running == statuses.len() {
        AggregateStatus::AllHealthy
    } else {
        AggregateStatus::PartialFailure
    }
}
```

---

### Phase 5: Chat View Update (2-3 days)

**Tests First** (`src/ui/chat_view_tests.rs`):
```rust
#[test]
fn test_text_delta_appends_to_buffer() {
    let event = AgentStreamEvent::TextDelta { text: "Hello".to_string() };
    let mut buffer = String::new();
    handle_stream_event(&event, &mut buffer, &mut String::new());
    assert_eq!(buffer, "Hello");
}

#[test]
fn test_thinking_delta_appends_to_thinking_buffer() {
    let event = AgentStreamEvent::ThinkingDelta { text: "Thinking...".to_string() };
    let mut text_buffer = String::new();
    let mut thinking_buffer = String::new();
    handle_stream_event(&event, &mut text_buffer, &mut thinking_buffer);
    assert_eq!(thinking_buffer, "Thinking...");
    assert!(text_buffer.is_empty());
}

#[test]
fn test_tool_executed_logs_correctly() {
    let event = AgentStreamEvent::ToolExecuted {
        tool_name: "search".to_string(),
        tool_call_id: Some("123".to_string()),
        success: true,
        error: None,
    };
    
    let log = format_tool_event(&event);
    assert!(log.contains("search"));
    assert!(log.contains("success") || log.contains("[OK]"));
}

#[test]
fn test_run_complete_finalizes_message() {
    let event = AgentStreamEvent::RunComplete { run_id: "test".to_string() };
    let finalized = should_finalize(&event);
    assert!(finalized);
}

#[test]
fn test_error_event_shows_message() {
    let event = AgentStreamEvent::Error { message: "Connection lost".to_string() };
    let error_msg = extract_error(&event);
    assert_eq!(error_msg, Some("Connection lost".to_string()));
}
```

**Implementation Changes** (`src/ui/chat_view.rs`):

Key change - use global runtime and Agent:
```rust
// OLD (problematic):
thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        // Agent created HERE - in temporary runtime - BAD
    });
});

// NEW (correct):
use crate::agent::{runtime::spawn_in_agent_runtime, global_agent};

// In send_message():
spawn_in_agent_runtime(async move {
    // Get agent from global singleton (must be initialized at app startup)
    let lock = global_agent().read().await;
    let agent = match lock.as_ref() {
        Some(a) => a,
        None => {
            tracing::error!("Agent not initialized");
            return;
        }
    };
    
    let mut stream = match agent.run_stream(&prompt).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to start agent stream");
            return;
        }
    };
    
    while let Some(event) = stream.next().await {
        match event? {
            AgentStreamEvent::TextDelta { text } => {
                if let Ok(mut buf) = streaming_response.lock() {
                    buf.push_str(&text);
                }
            }
            AgentStreamEvent::ThinkingDelta { text } => {
                if let Ok(mut buf) = streaming_thinking.lock() {
                    buf.push_str(&text);
                }
            }
            AgentStreamEvent::ToolCallStart { tool_name, .. } => {
                tracing::info!(tool = %tool_name, "Tool call started");
            }
            AgentStreamEvent::ToolExecuted { tool_name, success, error, .. } => {
                if success {
                    tracing::info!(tool = %tool_name, "Tool executed successfully");
                } else {
                    tracing::warn!(tool = %tool_name, error = ?error, "Tool execution failed");
                }
            }
            AgentStreamEvent::RunComplete { .. } => {
                // Signal completion
                if let Ok(mut buf) = streaming_response.lock() {
                    buf.push('␄'); // EOT marker
                }
            }
            AgentStreamEvent::Error { message } => {
                tracing::error!(error = %message, "Agent stream error");
                if let Ok(mut buf) = streaming_response.lock() {
                    buf.push_str(&format!("\n\n[Error: {}]", message));
                    buf.push('␄');
                }
            }
            _ => {}
        }
    }
});
```

---

### Phase 5.5: Integration Test (1 day)

**Before Phase 6, verify the full path works:**

```rust
// tests/integration/agent_stream_test.rs
use personal_agent::agent::{PersonalAgent, runtime::run_in_agent_runtime};
use personal_agent::config::Profile;
use futures::StreamExt;

#[test]
fn test_full_agent_stream_path() {
    run_in_agent_runtime(async {
        // Create a test profile (no MCPs for this basic test)
        let profile = Profile::test_default();
        
        // Create agent in global runtime
        let agent = PersonalAgent::new(&profile, &[]).await.unwrap();
        
        // Run a simple stream
        let mut stream = agent.run_stream("Say hello").await.unwrap();
        
        let mut got_text = false;
        let mut got_complete = false;
        
        while let Some(event) = stream.next().await {
            match event.unwrap() {
                AgentStreamEvent::TextDelta { text } => {
                    assert!(!text.is_empty());
                    got_text = true;
                }
                AgentStreamEvent::RunComplete { .. } => {
                    got_complete = true;
                }
                _ => {}
            }
        }
        
        assert!(got_text, "Should receive text deltas");
        assert!(got_complete, "Should receive run complete");
    });
}

#[test]
fn test_agent_with_mock_mcp_tool_execution() {
    // NOTE: Full MCP tool execution integration testing is deferred to Phase 0
    // when MockMcpClient or MemoryTransport is created.
    //
    // For Phase 5.5, we rely on:
    // 1. Unit tests for each component (toolset creation, agent creation, event handling)
    // 2. The basic stream test above (model without tools)
    // 3. Manual testing with real MCP servers
    //
    // Full mock test will verify:
    // - Agent receives tool_use from model
    // - Agent calls tool via McpToolset  
    // - Agent receives tool result
    // - Agent continues to final response
    //
    // This requires either:
    // - SerdesAI's MemoryTransport (if available)
    // - A custom MockMcpClient that returns predefined tool responses
    // - A test MCP server binary that can be spawned
}
```

---

### Phase 6: Cleanup & Polish (1-2 days)

**Prerequisites**: Phases 1-5 complete and integration tests passing.

**Checklist**:
- [ ] Delete `src/mcp/manager.rs`
- [ ] Delete `src/mcp/runtime.rs` (our old one, not agent/runtime.rs)
- [ ] Delete `src/mcp/service.rs`
- [ ] Update `src/mcp/mod.rs` exports
- [ ] Update `src/llm/client_agent.rs` - remove McpService references
- [ ] Update `src/ui/settings_view.rs` - remove McpService references
- [ ] Update `src/ui/mcp_add_view.rs` - remove McpService references
- [ ] Remove unused dependencies from Cargo.toml
- [ ] Run `cargo clippy` and fix warnings
- [ ] Run `cargo fmt`
- [ ] Full test suite: `cargo test`
- [ ] Coverage check: `cargo tarpaulin`
- [ ] Manual testing of all features

**Note on Rollback**: After this phase, rollback requires git revert. Consider:
1. Keep deletions in a separate commit for easy revert
2. Run in production with feature flag for 1-2 weeks before deleting old code

---

## Code Coverage Requirements

| Module | Current | Target | Notes |
|--------|---------|--------|-------|
| src/agent/mod.rs | N/A | 80%+ | New module |
| src/agent/runtime.rs | N/A | 90%+ | Critical, simple code |
| src/mcp/toolset.rs | N/A | 85%+ | Critical path |
| src/mcp/secrets.rs | ~70% | 80%+ | Keep existing |
| src/mcp/registry.rs | ~65% | 70%+ | Keep existing |
| src/mcp/types.rs | ~60% | 70%+ | Keep existing |

Run coverage: `cargo tarpaulin --out Html --output-dir coverage/`

## Integration Points Map

```
┌─────────────────┐     ┌──────────────┐
│ mcp_add_view.rs │────▶│ McpRegistry  │ (keep)
└─────────────────┘     └──────────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────┐
│ mcp_configure   │────▶│ secrets.rs   │ (keep)
│ _view.rs        │     └──────────────┘
└─────────────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────┐     ┌──────────────┐
│ settings_view   │────▶│ status.rs    │◀────│ McpToolset   │
└─────────────────┘     │ (refactored) │     │ (SerdesAI)   │
                        └──────────────┘     └──────────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────┐     ┌──────────────┐
│ chat_view.rs    │────▶│PersonalAgent │────▶│ Agent        │
└─────────────────┘     │ (new)        │     │ (SerdesAI)   │
         │              └──────────────┘     └──────────────┘
         ▼                     │
┌─────────────────┐            ▼
│ (streaming UI)  │◀───┌──────────────┐
└─────────────────┘    │ AgentStream  │
                       │ (SerdesAI)   │
                       └──────────────┘
```

## Edge Cases to Handle

| Edge Case | Handling |
|-----------|----------|
| MCP server timeout | 30s timeout in `create_toolset_from_config` |
| MCP server crash mid-execution | SerdesAI returns ToolError, we surface to user |
| MCP process dies between calls | Detect on next call, mark status as Error, allow retry |
| Large tool output | SerdesAI handles streaming |
| Unicode in tool names | Pass through, UI handles |
| Rapid MCP toggle | Rebuild agent on config change |
| OAuth token expiry | Return error, user must re-auth |
| MCP returns invalid JSON | SerdesAI error handling |
| System sleep/wake | Reconnect on next request |
| Tool name collision | Error at build time |
| User cancels mid-stream | Set cancel flag, stop polling stream |
| Concurrent chat requests | Queue or reject, agent is single-use |
| MCP loses connectivity after init | Detect via ToolError, update status, allow reinit |

### MCP Crash Recovery

When an MCP process crashes or connection is lost:

```rust
// In create_toolset_from_config, add retry logic:
pub async fn create_toolset_with_retry(
    config: &McpConfig,
    secrets: &SecretsManager,
    max_retries: u32,
) -> Result<McpToolset, McpError> {
    let mut last_error = None;
    
    for attempt in 0..max_retries {
        match create_toolset_from_config(config, secrets).await {
            Ok(toolset) => return Ok(toolset),
            Err(e) => {
                tracing::warn!(
                    mcp = %config.name,
                    attempt = attempt + 1,
                    error = %e,
                    "MCP connection failed, retrying..."
                );
                last_error = Some(e);
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await; // Exponential backoff
            }
        }
    }
    
    Err(last_error.unwrap_or(McpError::Unknown("Max retries exceeded".to_string())))
}
```

## Success Criteria

1. [OK] All existing MCP functionality works (search, add, configure, delete, tool execution)
2. [OK] No "Tokio shutting down" errors
3. [OK] Streaming works with thinking, text, and tool events
4. [OK] Code reduced by ~1000 lines
5. [OK] Test coverage maintained or improved
6. [OK] Manual testing passes all scenarios
7. [OK] No performance regression (informal benchmark)
