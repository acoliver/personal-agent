# Architect Specification: Agent Mode Implementation

**Plan ID:** PLAN-20250128-AGENT
**Author:** Software Architect
**Date:** 2025-01-28
**Status:** Draft

---

## 1. The Problem

The previous "remediation" was a FRAUD. It claimed to fix ChatService but:

1. **DID NOT use SerdesAI Agent mode** - Used raw `model.request_stream()` instead of `AgentBuilder`
2. **DID NOT implement tool execution** - Tool calls collected but NEVER EXECUTED
3. **DID NOT test MCP catalog** - No test that fetches real catalog or sets up tools
4. **DID NOT test real tool usage** - No E2E test with actual MCP tools (like Exa search)

The requirements at `dev-docs/requirements/services/chat.md` explicitly state:
> "It uses SerdesAI Agent mode for the core LLM interaction loop"

The architecture at `dev-docs/architecture/chat-flow.md` shows:
> Agent handles tool calls with retry logic

**NONE OF THIS WAS IMPLEMENTED.**

---

## 2. What Actually Exists

### Working Code (to reuse)

| File | What it has | Use it? |
|------|-------------|---------|
| `src/llm/stream.rs` | AgentBuilder, AgentStreamEvent, ModelConfig | YES - reference |
| `src/llm/client_agent.rs` | AgentBuilder with MCP tools, ToolExecutor | YES - reference |
| `src/mcp/service.rs` | McpService singleton with get_llm_tools(), call_tool() | YES - for tools |
| `src/services/mcp_registry_impl.rs` | Real HTTP fetch from MCP catalog | YES - 0 unimplemented |

### Broken Code (to fix or replace)

| File | Problem |
|------|---------|
| `src/services/chat_impl.rs` | Uses LlmClient (raw model), not Agent |
| `src/services/mcp.rs` | 8 unimplemented stubs (trait defaults) |
| `src/services/mcp_registry.rs` | 8 unimplemented stubs (trait defaults) |
| `src/llm/client.rs` | No agent mode, just model.request_stream() |

---

## 3. The Solution

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           USER INTERFACE                                 │
│                    (ChatView / ChatPresenter)                            │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ send_message(conversation_id, text)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           CHAT SERVICE                                   │
│                                                                          │
│  1. Load conversation from ConversationService                           │
│  2. Get profile from ProfileService                                      │
│  3. Get API key (resolve from profile)                                   │
│  4. Get MCP toolsets from McpService                                     │
│  5. Build SerdesAI Agent with AgentBuilder          <── THIS IS KEY     │
│  6. Run agent stream with message history                                │
│  7. Map AgentStreamEvent to ChatEvent                                    │
│  8. Emit events via EventBus                                             │
│  9. Save response to ConversationService                                 │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
          ┌─────────────────────────┼─────────────────────────┐
          │                         │                         │
          ▼                         ▼                         ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│ ProfileService  │      │ConversationSvc  │      │   McpService    │
│                 │      │                 │      │                 │
│ get_profile()   │      │ load()          │      │ get_toolsets()  │
│ (includes key)  │      │ save_message()  │      │ (MCP tools)     │
└─────────────────┘      └─────────────────┘      └─────────────────┘
                                                          │
                                                          │ Toolsets from
                                                          ▼ MCP servers
┌─────────────────────────────────────────────────────────────────────────┐
│                        SERDES-AI AGENT                                   │
│                                                                          │
│  AgentBuilder::from_config(model_config)                                 │
│      .system_prompt(...)                                                 │
│      .toolset(mcp_toolset_1)    <── MCP tools attached here             │
│      .toolset(mcp_toolset_2)                                             │
│      .history_processor(TruncateByTokens::new(context_limit))            │
│      .build()                                                            │
│                                                                          │
│  Agent internally:                                                       │
│  - Sends to LLM                                                          │
│  - Receives tool_use request                                             │
│  - EXECUTES TOOL via toolset      <── THIS ACTUALLY RUNS THE TOOL       │
│  - Sends tool result back to LLM                                         │
│  - Continues until done                                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Tool Execution Flow (What Was Missing)

```
LLM Response: "I'll search for that using Exa"
         │
         ▼
┌─────────────────┐
│  tool_use:      │
│  name: exa_search
│  args: {query}  │
└─────────────────┘
         │
         │  Agent receives tool_use
         ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     AGENT TOOL EXECUTION LOOP                            │
│                                                                          │
│  1. Find toolset that has "exa_search"                                   │
│  2. Call toolset.execute("exa_search", args)                             │
│  3. Toolset calls MCP server via JSON-RPC                                │
│  4. MCP server runs the tool, returns result                             │
│  5. Agent adds tool_result to conversation                               │
│  6. Agent sends updated conversation to LLM                              │
│  7. LLM continues with tool result                                       │
└─────────────────────────────────────────────────────────────────────────┘
         │
         ▼
LLM Response: "Based on my search, here are the results..."
```

---

## 4. Implementation Strategy

### DO NOT revert. Modify existing files.

The changes in `src/services/chat_impl.rs` are structurally correct (EventBus integration, etc). We need to:

1. Replace `LlmClient` usage with `AgentBuilder`
2. Replace `model.request_stream()` with `agent.stream()`
3. Get toolsets from McpService and attach to agent
4. Map `AgentStreamEvent` to our `ChatEvent`

### Files to Change

| File | Change |
|------|--------|
| `src/services/chat_impl.rs` | Replace LlmClient with AgentBuilder |
| `src/services/mcp.rs` | Wire trait methods to McpServiceImpl |
| `src/services/mod.rs` | Maybe add toolset conversion |

### Files to Add

| File | Purpose |
|------|---------|
| `tests/e2e_agent_mode.rs` | E2E test: Agent mode works |
| `tests/e2e_mcp_catalog.rs` | E2E test: Fetch real MCP catalog |
| `tests/e2e_mcp_tools.rs` | E2E test: Use real MCP tool (if available) |

---

## 5. Requirements (from dev-docs)

### From chat.md - MANDATORY

| ID | Requirement | How to Verify |
|----|-------------|---------------|
| AGENT-001 | ChatService uses SerdesAI Agent mode | grep "AgentBuilder" src/services/chat_impl.rs |
| AGENT-002 | Agent built with ModelConfig | grep "ModelConfig" src/services/chat_impl.rs |
| AGENT-003 | MCP toolsets attached via .toolset() | grep "\.toolset\(" src/services/chat_impl.rs |
| AGENT-004 | History processor for context management | grep "history_processor\|HistoryProcessor" |
| AGENT-005 | Agent handles tool execution internally | Code review - no manual tool loop |
| AGENT-006 | AgentStreamEvent mapped to ChatEvent | Code review of event mapping |

### From chat-flow.md - MANDATORY

| ID | Requirement | How to Verify |
|----|-------------|---------------|
| FLOW-001 | ProfileService provides model config | grep "profile_service\|ProfileService" |
| FLOW-002 | McpService provides toolsets | grep "mcp_service.get_toolsets\|get_toolsets" |
| FLOW-003 | ConversationService for persistence | grep "conversation_service" |
| FLOW-004 | Events emitted via EventBus | grep "emit\|EventBus" |

### E2E Verification - MANDATORY

| ID | Test | Acceptance |
|----|------|------------|
| E2E-001 | Real LLM call with Agent mode | Response received |
| E2E-002 | MCP catalog fetch | Can list MCP servers from catalog |
| E2E-003 | Tool attachment | Agent has tools (count > 0 if MCPs configured) |

---

## 6. Anti-Fakery Measures

### Code Verification Commands

Run ALL of these before claiming any phase complete:

```bash
# 1. Must use AgentBuilder, not raw model
grep -n "AgentBuilder" src/services/chat_impl.rs
# MUST return matches

# 2. Must NOT use LlmClient for streaming
grep -n "LlmClient" src/services/chat_impl.rs
# SHOULD return 0 matches (or only for type conversion)

# 3. Must NOT use model.request_stream directly
grep -n "model.request_stream\|request_stream_with_tools" src/services/chat_impl.rs
# MUST return 0 matches

# 4. Must have toolset attachment
grep -n "\.toolset(" src/services/chat_impl.rs
# MUST return matches

# 5. Must have AgentStreamEvent handling
grep -n "AgentStreamEvent" src/services/chat_impl.rs
# MUST return matches
```

### Semantic Verification

After code changes, run:

```bash
# E2E test with real API
cargo test --test e2e_agent_mode -- --ignored --nocapture

# Must see output like:
# "Agent built with X tools"
# "AgentStreamEvent::TextDelta received"
# "Response: ..."
```

---

## 7. Phases

### Phase 1: Replace LlmClient with Agent in ChatService

**Files:** `src/services/chat_impl.rs`

**Changes:**
1. Remove `use crate::llm::LlmClient`
2. Add `use serdes_ai::agent::{AgentBuilder, AgentStreamEvent, ModelConfig}`
3. Replace `LlmClient::from_profile()` with `AgentBuilder::from_config()`
4. Replace `client.request_stream_with_tools()` with agent streaming
5. Map `AgentStreamEvent` to our events

**Verification:**
```bash
grep -n "AgentBuilder" src/services/chat_impl.rs  # MUST have matches
grep -n "LlmClient" src/services/chat_impl.rs     # MUST be 0 or minimal
cargo build --all-targets                          # MUST pass
cargo test --lib services::chat                    # MUST pass
```

### Phase 2: Wire MCP Toolsets

**Files:** `src/services/chat_impl.rs`, possibly `src/services/mcp.rs`

**Changes:**
1. Get toolsets from McpService (or convert get_llm_tools to toolsets)
2. Attach to agent via `.toolset()`

**Verification:**
```bash
grep -n "\.toolset(" src/services/chat_impl.rs  # MUST have matches
grep -n "get_toolsets\|get_llm_tools" src/services/chat_impl.rs  # MUST have matches
cargo build --all-targets
```

### Phase 3: E2E Tests

**Files:** New test files

**Tests:**
1. `e2e_agent_mode.rs` - Agent streams real response
2. `e2e_mcp_catalog.rs` - Fetch MCP catalog
3. (Optional) `e2e_mcp_tool.rs` - If MCP server available

**Verification:**
```bash
cargo test --test e2e_agent_mode -- --ignored --nocapture
# MUST show agent events, not raw model events
```

### Phase 4: Final Verification

**Check everything:**
1. All grep commands from Section 6
2. All E2E tests pass
3. Code review confirms Agent mode

---

## 8. What NOT To Do

1. **DO NOT** create another layer of abstraction
2. **DO NOT** wrap agent in another wrapper
3. **DO NOT** add "will implement later" stubs
4. **DO NOT** claim success without E2E proof
5. **DO NOT** use `unimplemented!()` anywhere
6. **DO NOT** skip the grep verification commands

---

## 9. Success Criteria

The plan is COMPLETE when:

1. `grep -n "AgentBuilder" src/services/chat_impl.rs` returns matches
2. `grep -n "model.request_stream" src/services/chat_impl.rs` returns NOTHING
3. `grep -n "\.toolset(" src/services/chat_impl.rs` returns matches
4. `cargo test --test e2e_agent_mode -- --ignored` PASSES with real API
5. Code inspection confirms Agent handles tool execution (no manual loop)

---

## 10. References

- `dev-docs/requirements/services/chat.md` - Chat service requirements
- `dev-docs/architecture/chat-flow.md` - Architecture flow
- `src/llm/stream.rs` - Existing Agent code to reference
- `src/llm/client_agent.rs` - Existing Agent+MCP code to reference
