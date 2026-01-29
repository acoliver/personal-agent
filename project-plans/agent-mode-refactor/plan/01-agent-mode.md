# Phase 01: Wire ChatService to Existing Agent Code

**Plan:** PLAN-20250128-AGENT
**Phase:** P01
**Prerequisites:** None
**Subagent:** rustexpert

---

## Objective

ChatService currently bypasses existing Agent code. Wire it up:

**EXISTING AGENT CODE (already works):**
- `src/llm/client_agent.rs` - `AgentClientExt` trait with `create_agent()` and `run_agent_stream()`
- `src/llm/client_agent.rs` - `McpToolExecutor` that calls `McpService.call_tool()`

**WHAT'S BROKEN:**
- `src/services/chat_impl.rs` line 105: `LlmClient::from_profile()` [OK] (fine)
- `src/services/chat_impl.rs` line 156: `client.request_stream_with_tools()`  (bypasses Agent)

**THE FIX:**
Replace `request_stream_with_tools()` with `create_agent()` + `run_agent_stream()`

---

## Implementation Steps

### Step 1: Add Import

**File:** `src/services/chat_impl.rs`

```rust
// Add this import
use crate::llm::client_agent::AgentClientExt;
```

### Step 2: Replace the Streaming Call

**Current code (around line 150-186):**
```rust
// This calls raw model, not agent
let result = client.request_stream_with_tools(&messages, &mcp_tools, |event| {
    match event {
        LlmStreamEvent::ToolUse(tool_use) => {
            pending_tool_calls.push(tool_use);  // NEVER EXECUTED
        }
        ...
    }
});
```

**Replace with:**
```rust
// Get system prompt from conversation
let system_prompt = conversation.messages.iter()
    .find(|m| m.role == MessageRole::System)
    .map(|m| m.content.as_str())
    .unwrap_or("");

// Create Agent with MCP tools (uses existing AgentClientExt)
// @requirement AGENT-001, AGENT-003
let agent = client.create_agent(mcp_tools, system_prompt).await
    .map_err(|e| ServiceError::Internal(format!("Failed to create agent: {}", e)))?;

// Run Agent stream (Agent executes tools internally)
// @requirement AGENT-005, AGENT-006
let result = client.run_agent_stream(&agent, &messages, |event| {
    match event {
        LlmStreamEvent::TextDelta(text) => {
            emit(AppEvent::Chat(ChatEvent::TextDelta { text: text.clone() }));
            let _ = tx.send(ChatStreamEvent::Token(text));
            response_text.push_str(&text);
        }
        LlmStreamEvent::ThinkingDelta(text) => {
            emit(AppEvent::Chat(ChatEvent::ThinkingDelta { text: text.clone() }));
            thinking_text.push_str(&text);
        }
        LlmStreamEvent::Complete => {
            let _ = tx.send(ChatStreamEvent::Complete);
        }
        LlmStreamEvent::Error(err) => {
            emit(AppEvent::Chat(ChatEvent::StreamError {
                conversation_id: event_conversation_id,
                error: err.clone(),
                recoverable: false,
            }));
            let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err)));
        }
        // Tool events are handled INSIDE run_agent_stream
        // Agent automatically executes tools and continues
        _ => {}
    }
}).await;
```

### Step 3: Remove Dead Code

Remove the `pending_tool_calls` vector - it's no longer needed because Agent handles tools internally.

```diff
- let mut pending_tool_calls: Vec<crate::llm::tools::ToolUse> = Vec::new();
```

---

## What This Changes

| Before | After |
|--------|-------|
| `client.request_stream_with_tools()` | `client.create_agent()` + `client.run_agent_stream()` |
| Tools collected in vector, never executed | Agent executes tools automatically via `McpToolExecutor` |
| Raw model streaming | Agent streaming with tool loop |

---

## Verification Commands (BLOCKING)

### Check 1: AgentClientExt is imported
```bash
grep -n "AgentClientExt" src/services/chat_impl.rs
```
**Expected:** At least one match (the import)

### Check 2: create_agent is called
```bash
grep -n "create_agent" src/services/chat_impl.rs
```
**Expected:** At least one match

### Check 3: run_agent_stream is called
```bash
grep -n "run_agent_stream" src/services/chat_impl.rs
```
**Expected:** At least one match

### Check 4: request_stream_with_tools is NOT used
```bash
grep -n "request_stream_with_tools" src/services/chat_impl.rs
```
**Expected:** NO OUTPUT (empty) - this is the old code we're replacing

### Check 5: pending_tool_calls is removed
```bash
grep -n "pending_tool_calls" src/services/chat_impl.rs
```
**Expected:** NO OUTPUT (empty) - dead code removed

### Check 6: Build passes
```bash
cargo build --all-targets 2>&1 | tail -5
```
**Expected:** No errors

### Check 7: Tests pass
```bash
cargo test --lib services::chat 2>&1 | grep -E "^test|passed|failed"
```
**Expected:** All tests pass

---

## Deliverables

1. `src/services/chat_impl.rs` uses `create_agent()` + `run_agent_stream()`
2. No `request_stream_with_tools()` calls remain
3. No `pending_tool_calls` dead code
4. All verification commands pass
5. Evidence file at `plan/.completed/P01.md`

---

## Evidence File Format

```markdown
# Phase 01: Wire ChatService to Agent Evidence

## Date: YYYY-MM-DD

## Verification

### AgentClientExt imported
\`\`\`
$ grep -n "AgentClientExt" src/services/chat_impl.rs
[OUTPUT]
\`\`\`

### create_agent called
\`\`\`
$ grep -n "create_agent" src/services/chat_impl.rs
[OUTPUT]
\`\`\`

### run_agent_stream called
\`\`\`
$ grep -n "run_agent_stream" src/services/chat_impl.rs
[OUTPUT]
\`\`\`

### request_stream_with_tools removed
\`\`\`
$ grep -n "request_stream_with_tools" src/services/chat_impl.rs
[EMPTY - no output]
\`\`\`

### pending_tool_calls removed
\`\`\`
$ grep -n "pending_tool_calls" src/services/chat_impl.rs
[EMPTY - no output]
\`\`\`

### Build
\`\`\`
$ cargo build --all-targets 2>&1 | tail -3
[OUTPUT]
\`\`\`

### Tests
\`\`\`
$ cargo test --lib services::chat 2>&1 | tail -5
[OUTPUT]
\`\`\`

## Verdict

**PASS** or **FAIL**
```
