# Plan: Remediate Refactor - Integration-First

Plan ID: PLAN-20250127-REMEDIATE
Generated: 2025-01-27
Total Phases: 8 (4 implementation + 4 verification)
Requirements: REM-001 through REM-007 from specification.md

## Critical Reminders

Before implementing ANY phase, ensure you have:

1. Completed preflight verification (Phase 01)
2. Previous phase evidence file exists with VERDICT: PASS
3. Run placeholder detection commands BEFORE claiming completion
4. Verified against dev-docs/requirements/** for coherence

## ABSOLUTE RULES - NO EXCEPTIONS

1. **NO CONDITIONAL PASS** - Every verification is PASS or FAIL. Period.
2. **ZERO TOLERANCE FOR PLACEHOLDERS** - In implementation phases:
   - `unimplemented!()` = FAIL
   - `todo!()` = FAIL
   - `// TODO:` = FAIL
   - placeholder strings = FAIL
   - empty implementations where real work expected = FAIL
3. **EVIDENCE REQUIRED** - Every verification must show exact command outputs
4. **PREREQUISITES ENFORCED** - Phase N cannot start until Phase N-1 evidence exists with PASS

## Requirements Summary

### Core Requirements (from specification.md)

| ID | Requirement | Phase |
|----|-------------|-------|
| REM-001 | ChatService.send_message calls SerdesAI Agent | P02 |
| REM-002 | ChatService uses profile from ProfileService | P02 |
| REM-003 | ChatService resolves API key correctly | P02 |
| REM-004 | ChatService attaches MCP tools from McpService | P03 |
| REM-005 | ChatService emits ChatEvent::TextDelta | P02 |
| REM-006 | ChatService emits ChatEvent::StreamCompleted | P02 |
| REM-007 | Tool calls work during streaming | P03 |

### Architecture Coherence (from dev-docs/architecture/**)

| Source | Requirement | How Addressed |
|--------|-------------|---------------|
| ARCHITECTURE_IMPROVEMENTS.md | ChatService orchestrates via SerdesAI Agent | P02 implements Agent integration |
| ARCHITECTURE_IMPROVEMENTS.md | McpService provides toolsets | P03 implements get_toolsets() |
| ARCHITECTURE_IMPROVEMENTS.md | Events flow through EventBus | P02, P03 emit ChatEvent/McpEvent |
| chat-flow.md | ChatService coordinates ConversationService, ProfileService, McpService | P02, P03 wire these together |
| chat-flow.md | HistoryProcessor for context management | P02 adds TruncateByTokens |

### Requirements Coherence (from dev-docs/requirements/**)

| Source | Requirement | How Addressed |
|--------|-------------|---------------|
| services/chat.md | ChatService uses SerdesAI Agent mode | P02 implements AgentBuilder |
| services/chat.md | ChatService emits ChatEvent variants | P02 emits via EventBus |
| services/chat.md | ChatService gets toolsets from McpService | P03 calls get_toolsets() |
| services/mcp.md | McpService provides AbstractToolset implementations | P03 implements get_toolsets() |
| events.md | ChatEvent::StreamStarted, TextDelta, StreamCompleted | P02 emits all required events |

## Phase Overview

| Phase | Title | Status | Primary Requirements |
|-------|-------|--------|---------------------|
| 01 | Preflight Verification | Pending | Verify dependencies, existing code |
| 01a | Preflight Verification Check | Pending | Verify preflight passed |
| 02 | ChatService Implementation | Pending | REM-001, REM-002, REM-003, REM-005, REM-006 |
| 02a | ChatService Verification | Pending | Verify no placeholders, tests pass |
| 03 | MCP Integration | Pending | REM-004, REM-007 |
| 03a | MCP Integration Verification | Pending | Verify toolsets work |
| 04 | End-to-End Verification | Pending | All requirements |
| 04a | Final Verification | Pending | Complete evidence |

## Architecture Alignment

This plan implements the chat flow from `dev-docs/architecture/chat-flow.md`:

```
ChatView (UI) 
  --> ChatService.send_message()
      --> ConversationService.load() + add_user_message()
      --> ProfileService.get_default() + resolve API key
      --> McpService.get_toolsets()
      --> Build SerdesAI Agent with toolsets
      --> AgentStream::new() with message history
      --> Map AgentEvent to ChatEvent
      --> EventBus.emit(ChatEvent::*)
  <-- ChatPresenter handles events
      <-- Updates ChatView
```

## Verification Strategy

Each verification phase (01a, 02a, 03a, 04a) MUST:

1. **Run placeholder detection** (FIRST, BLOCKING):
   ```bash
   grep -rn "unimplemented!" src/services/chat_impl.rs src/services/mcp_impl.rs
   grep -rn "todo!" src/services/chat_impl.rs src/services/mcp_impl.rs
   grep -rn "placeholder" src/services/chat_impl.rs src/services/mcp_impl.rs
   grep -rn "not yet implemented" src/services/chat_impl.rs src/services/mcp_impl.rs
   ```
   If ANY returns matches: FAIL. Do not proceed.

2. **Run build check**:
   ```bash
   cargo build --all-targets
   ```
   Must pass with 0 errors.

3. **Run test check**:
   ```bash
   cargo test services::chat
   cargo test services::mcp
   ```
   Must pass with 0 failures.

4. **Semantic verification** - Read the code and verify it does what it claims.

5. **Create evidence file** at `project-plans/remediate-refactor/plan/.completed/P[NN]A.md`

## Execution Tracker

See `project-plans/remediate-refactor/execution-tracker.md` for detailed phase status.

## Success Criteria

Plan is complete when:

- [ ] All phases have PASS verdict (not conditional)
- [ ] All evidence files exist in `.completed/`
- [ ] `grep -rn "unimplemented!\|todo!\|placeholder" src/services/chat_impl.rs` returns NOTHING
- [ ] `grep -rn "unimplemented!\|todo!\|placeholder" src/services/mcp_impl.rs` returns NOTHING
- [ ] `cargo build --all-targets` passes
- [ ] `cargo test` passes
- [ ] Manual test: send message, receive LLM response

## Anti-Patterns to Reject

Per dev-docs/COORDINATING.md:

- "Conditional pass" = FAIL
- "Pass with warnings" = FAIL
- "Mostly complete" = FAIL
- "Expected failures" (in impl phases) = FAIL
- Hollow implementations (returns defaults without doing work) = FAIL
- Skipping prerequisite checks = FAIL

## References

- `project-plans/remediate-refactor/specification.md` - Full specification
- `dev-docs/PLAN.md` - Planning guidelines
- `dev-docs/PLAN-TEMPLATE.md` - Phase structure
- `dev-docs/COORDINATING.md` - Execution rules
- `dev-docs/architecture/chat-flow.md` - Target architecture
- `dev-docs/requirements/services/chat.md` - Chat service requirements
