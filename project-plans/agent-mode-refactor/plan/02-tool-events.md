# Phase 02: Expose Tool Events

**Plan:** PLAN-20250128-AGENT
**Phase:** P02
**Prerequisites:** P01 evidence with PASS
**Subagent:** rustexpert

---

## Objective

Currently `run_agent_stream` logs tool events to stderr but doesn't expose them.
We need to emit them as ChatEvents so UI and tests can see tool execution.

---

## Current Code

**File:** `src/llm/client_agent.rs` lines 167-182

```rust
AgentStreamEvent::ToolCallStart { tool_name, .. } => {
    eprintln!("Tool call started: {tool_name}");  // Just logs
}
AgentStreamEvent::ToolExecuted { tool_name, success, error, .. } => {
    if success {
        eprintln!("Tool completed: {tool_name}");  // Just logs
    } else {
        eprintln!("Tool failed: {tool_name} - {error:?}");
    }
}
```

---

## Implementation Steps

### Step 1: Add Tool Event Variants to StreamEvent

**File:** `src/llm/events.rs` (or wherever StreamEvent is defined)

```rust
pub enum StreamEvent {
    TextDelta(String),
    ThinkingDelta(String),
    Complete,
    Error(String),
    // ADD THESE:
    ToolCallStarted { tool_name: String, call_id: String },
    ToolCallCompleted { tool_name: String, call_id: String, success: bool, result: Option<String>, error: Option<String> },
}
```

### Step 2: Update run_agent_stream to Emit Tool Events

**File:** `src/llm/client_agent.rs`

```rust
AgentStreamEvent::ToolCallStart { tool_name, call_id, .. } => {
    on_event(StreamEvent::ToolCallStarted {
        tool_name: tool_name.clone(),
        call_id: call_id.clone(),
    });
}
AgentStreamEvent::ToolExecuted { tool_name, call_id, success, result, error, .. } => {
    on_event(StreamEvent::ToolCallCompleted {
        tool_name,
        call_id,
        success,
        result: result.map(|r| r.to_string()),
        error: error.map(|e| e.to_string()),
    });
}
```

### Step 3: Add Tool Event Variants to ChatEvent

**File:** `src/events/chat.rs` (or wherever ChatEvent is defined)

```rust
pub enum ChatEvent {
    // ... existing variants ...
    
    // ADD:
    ToolCallStarted {
        conversation_id: Uuid,
        tool_call_id: String,
        tool_name: String,
    },
    ToolCallCompleted {
        conversation_id: Uuid,
        tool_call_id: String,
        tool_name: String,
        success: bool,
        result: Option<String>,
        error: Option<String>,
    },
}
```

### Step 4: Map Tool StreamEvents to ChatEvents in ChatService

**File:** `src/services/chat_impl.rs`

In the `run_agent_stream` callback:

```rust
LlmStreamEvent::ToolCallStarted { tool_name, call_id } => {
    emit(AppEvent::Chat(ChatEvent::ToolCallStarted {
        conversation_id: event_conversation_id,
        tool_call_id: call_id,
        tool_name,
    }));
}
LlmStreamEvent::ToolCallCompleted { tool_name, call_id, success, result, error } => {
    emit(AppEvent::Chat(ChatEvent::ToolCallCompleted {
        conversation_id: event_conversation_id,
        tool_call_id: call_id,
        tool_name,
        success,
        result,
        error,
    }));
}
```

### Step 5: Add Tool Variants to ChatStreamEvent

**File:** `src/services/chat.rs` (the trait file)

```rust
pub enum ChatStreamEvent {
    Token(String),
    Complete,
    Error(ServiceError),
    // ADD:
    ToolCallStarted { tool_name: String, call_id: String },
    ToolCallCompleted { tool_name: String, success: bool, result: Option<String> },
}
```

---

## Verification Commands (BLOCKING)

### Check 1: StreamEvent has tool variants
```bash
grep -n "ToolCallStarted\|ToolCallCompleted" src/llm/events.rs
```
**Expected:** At least 2 matches (the enum variants)

### Check 2: ChatEvent has tool variants
```bash
grep -n "ToolCallStarted\|ToolCallCompleted" src/events/chat.rs
```
**Expected:** At least 2 matches

### Check 3: run_agent_stream emits tool events
```bash
grep -n "ToolCallStarted\|ToolCallCompleted" src/llm/client_agent.rs
```
**Expected:** At least 2 matches (the on_event calls)

### Check 4: ChatService maps tool events
```bash
grep -n "ToolCallStarted\|ToolCallCompleted" src/services/chat_impl.rs
```
**Expected:** At least 2 matches

### Check 5: No more eprintln for tool events
```bash
grep -n "eprintln.*Tool" src/llm/client_agent.rs
```
**Expected:** NO OUTPUT (removed the debug prints)

### Check 6: Build passes
```bash
cargo build --all-targets 2>&1 | tail -5
```

### Check 7: Tests pass
```bash
cargo test --lib 2>&1 | grep -E "^test result"
```

---

## Deliverables

1. `StreamEvent` has `ToolCallStarted` and `ToolCallCompleted`
2. `ChatEvent` has tool variants
3. `ChatStreamEvent` has tool variants
4. `run_agent_stream` emits tool events (not just logs)
5. `ChatServiceImpl` maps tool events to ChatEvents
6. Evidence file at `plan/.completed/P02.md`
