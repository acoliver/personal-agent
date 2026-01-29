# Phase 03: MCP Integration

## Phase ID

`PLAN-20250127-REMEDIATE.P03`

## Prerequisites

- Required: Phase 02a completed with PASS verdict
- Verification: `grep "^## Verdict: PASS" project-plans/remediate-refactor/plan/.completed/P02A.md`
- Evidence file exists: `project-plans/remediate-refactor/plan/.completed/P02A.md`

## Requirements Implemented (Expanded)

### REM-004: ChatService attaches MCP tools from McpService

**Full Text**: ChatService must get available MCP toolsets from McpService and attach them to the SerdesAI Agent.

**Behavior**:
- GIVEN: MCPs are configured and enabled
- WHEN: ChatService builds the Agent
- THEN: MCP toolsets are attached via `builder.toolset()`

**Why This Matters**: Tool use is a core feature - agents need MCP tools to be useful.

### REM-007: Tool calls work during streaming

**Full Text**: When the Agent makes tool calls, they must execute via MCP and results returned to the Agent.

**Behavior**:
- GIVEN: Agent requests a tool call
- WHEN: Tool execution is triggered
- THEN: MCP executes the tool and returns result to Agent

**Why This Matters**: Tool calls are the whole point of MCP integration.

## Implementation Tasks

### Files to Modify

#### 1. Assess MCP Toolset Integration Options

**Current State:**

The existing `src/mcp/service.rs` has a working `McpService` singleton that:
- Manages MCP lifecycle via `McpRuntime`
- Provides `get_tools()` returning `Vec<ToolDefinition>` 
- Provides `get_llm_tools()` returning `Vec<crate::llm::Tool>`
- Provides `call_tool()` for tool execution

**The Problem:**

SerdesAI Agent expects `AbstractToolset` implementations, but:
- `serdes_ai_mcp::McpToolset` exists but may need MCP client directly
- Current `McpRuntime` wraps MCP clients internally
- We need to either expose clients or create a bridge

**Option A: Use LLM Tool Definitions (Simpler)**

The existing `get_llm_tools()` returns tool definitions. ChatService can:
1. Get tool definitions from `McpService::global().lock().await.get_llm_tools()`
2. Pass these to SerdesAI Agent as available tools
3. When Agent requests tool call, use `McpService::global().lock().await.call_tool()`

This is similar to how `src/llm/client_agent.rs` handles tool calls.

**Option B: Create McpToolset Bridge (More Complex)**

Create a wrapper that implements `AbstractToolset` and delegates to `McpService`:

```rust
/// @plan PLAN-20250127-REMEDIATE.P03
/// @requirement REM-004
struct McpToolsetBridge {
    mcp_service: Arc<Mutex<crate::mcp::McpService>>,
}

#[async_trait]
impl AbstractToolset for McpToolsetBridge {
    fn tools(&self) -> Vec<Tool> {
        // Convert from McpService tools
    }
    
    async fn call(&self, name: &str, args: Value) -> Result<Value, ToolError> {
        let mut service = self.mcp_service.lock().await;
        service.call_tool(name, args).await.map_err(|e| ToolError::Execution(e))
    }
}
```

**Decision**: Start with Option A (simpler). The existing code in `src/llm/client_agent.rs` already handles tool calls this way.

#### 2. Reference: Existing Tool Handling in client_agent.rs

See `src/llm/client_agent.rs` for the existing pattern:

```rust
// Line ~83: add_mcp_toolsets_to_builder
// Line ~97: attach_tool_callbacks
```

This code already:
- Gets tools from MCP service
- Attaches them to Agent builder
- Sets up callbacks for tool execution

**ChatService can reuse or adapt this existing implementation.**

#### 3. Update ChatService to Include Tools

Update `send_message()` to:
1. Get tools from `McpService::global()`
2. Pass tools to Agent (either via builder or callbacks)
3. Handle tool call events in the stream

**Code Markers Required**:

```rust
/// @plan PLAN-20250127-REMEDIATE.P03
/// @requirement REM-004, REM-007
```

### Alternative: Delegate to Existing LlmClient

The existing `src/llm/client.rs` and `src/llm/client_agent.rs` already handle MCP tool integration. ChatServiceImpl could:

1. Create an `LlmClient` from the profile
2. Use `LlmClient`'s existing agent/tool handling
3. Map events through EventBus

This avoids duplicating the MCP integration logic.

## CRITICAL: Anti-Placeholder Rules

**THIS IS AN IMPLEMENTATION PHASE. The following are COMPLETE FAILURE:**

- `unimplemented!()` anywhere = FAIL
- `todo!()` anywhere = FAIL
- `// TODO:` comments = FAIL
- Returning `Vec::new()` without checking MCPs = FAIL (hollow implementation)
- Not actually wiring to MCP_SERVICE = FAIL

**The code MUST actually get toolsets from running MCPs.**

## Verification Commands (Run Before Claiming Done)

```bash
# 1. Placeholder detection for mcp_impl.rs (MUST ALL RETURN EMPTY)
grep -rn "unimplemented!" src/services/mcp_impl.rs
grep -rn "todo!" src/services/mcp_impl.rs
grep -rn "placeholder" src/services/mcp_impl.rs
grep -rn "// TODO\|// FIXME" src/services/mcp_impl.rs

# 2. Placeholder detection for chat_impl.rs (MUST STILL BE EMPTY)
grep -rn "unimplemented!" src/services/chat_impl.rs
grep -rn "todo!" src/services/chat_impl.rs
grep -rn "placeholder" src/services/chat_impl.rs

# 3. Verify get_toolsets() exists and is implemented
grep -A20 "fn get_toolsets" src/services/mcp_impl.rs

# 4. Verify ChatService uses toolsets
grep -n "get_toolsets\|toolset" src/services/chat_impl.rs

# 5. Build check
cargo build --all-targets

# 6. Test check
cargo test services::mcp
cargo test services::chat
```

## Success Criteria

- [ ] All placeholder detection commands return EMPTY for mcp_impl.rs
- [ ] All placeholder detection commands still return EMPTY for chat_impl.rs
- [ ] `get_toolsets()` is implemented and returns actual toolsets
- [ ] ChatService calls `get_toolsets()` and attaches to Agent
- [ ] `cargo build --all-targets` passes
- [ ] `cargo test services::mcp` passes
- [ ] `cargo test services::chat` passes

## Deliverables

1. Modified `src/services/mcp_impl.rs` with get_toolsets() implementation
2. Modified `src/services/chat_impl.rs` to use toolsets
3. Evidence file at `project-plans/remediate-refactor/plan/.completed/P03.md`

## Phase Completion Marker

Create: `project-plans/remediate-refactor/plan/.completed/P03.md`

Contents MUST include:
- All grep outputs for mcp_impl.rs (must be empty)
- grep showing get_toolsets() implementation
- grep showing ChatService uses toolsets
- Build output (must pass)
- Test output (must pass)
- Verdict: PASS or FAIL (no conditional)

## Failure Recovery

If this phase fails:
1. `git checkout -- src/services/mcp_impl.rs src/services/chat_impl.rs`
2. Review error messages
3. Fix issues and re-run verification
4. Do NOT proceed to Phase 04 until this passes
