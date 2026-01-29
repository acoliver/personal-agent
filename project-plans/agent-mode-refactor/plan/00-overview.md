# Plan: Agent Mode Implementation

**Plan ID:** PLAN-20250128-AGENT
**Generated:** 2025-01-28
**Total Phases:** 6 (3 implementation + 3 verification)

---

## Why This Plan Exists

The previous "remediation" plan (PLAN-20250127-REMEDIATE) was FRAUDULENT:

1. It claimed ChatService used SerdesAI but actually used raw `model.request_stream()`
2. It collected tool_use events but NEVER EXECUTED THEM
3. It passed "verification" because verification only checked structural things

This plan fixes it FOR REAL by implementing Agent mode as specified in:
- `dev-docs/requirements/services/chat.md`
- `dev-docs/architecture/chat-flow.md`

---

## Architecture Summary

See `diagrams.md` for visual representations.

**BEFORE (Wrong):**
```
ChatService → LlmClient → model.request_stream() → tools collected but NOT executed
```

**AFTER (Correct):**
```
ChatService → AgentBuilder → agent.run_stream() → agent EXECUTES tools internally
```

---

## Requirements Traceability

| Req ID | Requirement | Phase | Verification |
|--------|-------------|-------|--------------|
| AGENT-001 | Use AgentBuilder | P01 | grep "AgentBuilder" |
| AGENT-002 | ModelConfig for model setup | P01 | grep "ModelConfig" |
| AGENT-003 | Attach toolsets via .toolset() | P02 | grep "\.toolset(" |
| AGENT-004 | HistoryProcessor for context | P01 | grep "history_processor" |
| AGENT-005 | Agent handles tool execution | P01+P02 | Code review |
| AGENT-006 | Map AgentStreamEvent to ChatEvent | P01 | grep "AgentStreamEvent" |
| E2E-001 | Real LLM call works | P03 | E2E test passes |
| E2E-002 | MCP catalog fetch works | P03 | E2E test passes |

---

## Phase Summary

| Phase | Name | Description | Key Deliverable |
|-------|------|-------------|-----------------|
| P01 | Wire ChatService to Agent | Use existing `create_agent()` + `run_agent_stream()` | No more `request_stream_with_tools()` |
| P02 | Expose Tool Events | Add ToolCallStarted/Completed to StreamEvent, ChatEvent | Tests can verify tool execution |
| P03 | MCP Registry Install | Implement `registry.install()` | Service can install MCPs |
| P04 | E2E Test | Full flow test | Proves everything works |

## What Already Exists (We Reuse)

- `src/llm/client_agent.rs` - `AgentClientExt` trait with `create_agent()`, `run_agent_stream()`
- `src/llm/client_agent.rs` - `McpToolExecutor` that calls `McpService.call_tool()`
- `src/mcp/service.rs` - `McpService::global()` singleton

## What We Change

| File | Change |
|------|--------|
| `src/services/chat_impl.rs` | Call `create_agent()` + `run_agent_stream()` instead of `request_stream_with_tools()` |
| `src/llm/events.rs` | Add `ToolCallStarted`, `ToolCallCompleted` variants |
| `src/events/chat.rs` | Add tool event variants |
| `src/llm/client_agent.rs` | Emit tool events (not just eprintln) |
| `src/services/mcp_registry_impl.rs` | Implement `install()` (currently stub) |
| `tests/e2e_agent_tool_execution.rs` | NEW: E2E test proving full flow |

---

## Anti-Fakery Rules

### BLOCKING Verification Commands

These commands MUST return expected results or the phase FAILS:

**For P01 (Agent Mode):**
```bash
# MUST have matches:
grep -n "AgentBuilder" src/services/chat_impl.rs

# MUST be empty (no raw model usage):
grep -n "model.request_stream\|request_stream_with_tools" src/services/chat_impl.rs

# MUST have matches:
grep -n "AgentStreamEvent" src/services/chat_impl.rs
```

**For P02 (Toolsets):**
```bash
# MUST have matches:
grep -n "\.toolset(" src/services/chat_impl.rs
```

**For P03 (E2E):**
```bash
# MUST pass:
cargo test --test e2e_agent_mode -- --ignored --nocapture
```

### NO Conditional Pass

There is NO "conditional pass". Either:
- All verification commands return expected results → PASS
- Any verification fails → FAIL

"Pass with minor issues" = FAIL
"Mostly complete" = FAIL
"Works except for X" = FAIL

---

## Success Criteria

The plan is COMPLETE when ALL of the following are true:

1. `grep -n "AgentBuilder" src/services/chat_impl.rs` returns matches
2. `grep -n "model.request_stream" src/services/chat_impl.rs` returns NOTHING
3. `grep -n "\.toolset(" src/services/chat_impl.rs` returns matches
4. `grep -n "AgentStreamEvent" src/services/chat_impl.rs` returns matches
5. `cargo build --all-targets` passes
6. `cargo test --lib services::chat` passes
7. `cargo test --test e2e_agent_mode -- --ignored` passes with real API call
8. Manual code review confirms agent handles tool execution (no manual loop)

---

## Evidence Files

All evidence goes in `project-plans/agent-mode-refactor/plan/.completed/`:

- P01.md - Agent mode implementation evidence
- P01A.md - Agent mode verification evidence
- P02.md - Toolset integration evidence
- P02A.md - Toolset verification evidence
- P03.md - E2E test evidence
- P03A.md - Final verification evidence

---

## References

- `project-plans/agent-mode-refactor/specification.md` - Full specification
- `project-plans/agent-mode-refactor/diagrams.md` - Architecture diagrams
- `dev-docs/requirements/services/chat.md` - Requirements
- `dev-docs/architecture/chat-flow.md` - Architecture
- `src/llm/stream.rs` - Existing agent code reference
- `src/llm/client_agent.rs` - Existing agent+MCP code reference
