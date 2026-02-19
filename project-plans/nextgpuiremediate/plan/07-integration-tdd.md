# Phase 07: Integration TDD

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P07`

## Prerequisites

- Required: Phase 06a completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P06a.md`
- Expected files from previous phase:
  - `src/main_gpui.rs`
  - `src/ui_gpui/views/main_panel.rs`

## Requirements Implemented (Expanded)

### REQ-INT-002: Integration tests verify behavior in-context

**Full Text**: Integration tests MUST validate active GPUI runtime wiring across view actions, EventBus, presenters, and command routing.

**Behavior**:
- GIVEN: the active runtime wiring from prior phases
- WHEN: integration scenarios are executed
- THEN: tests assert real, user-visible outcomes in profile, conversation, MCP, and settings flows

**Why This Matters**: Structural correctness without integration assertions is a known fraud pattern.

### REQ-INT-002.1: Profile flow in-context

**Full Text**: Profile add/edit/default flows must be testable through real event + command paths.

**Behavior**:
- GIVEN: settings/profile views and presenters are wired
- WHEN: create/select/edit actions run
- THEN: command outputs and state transitions match spec

**Why This Matters**: Profile auth/model setup is prerequisite for reliable chat.

### REQ-INT-002.2: Conversation flow in-context

**Full Text**: Conversation load/send/stream/delete must be covered by integration tests.

**Behavior**:
- GIVEN: chat/history views and presenters are wired
- WHEN: user runs conversation lifecycle actions
- THEN: messages and conversation state update through unified flow

**Why This Matters**: Conversation lifecycle is core product behavior.

### REQ-INT-002.3: MCP flow in-context

**Full Text**: MCP add/configure/toggle/delete must be covered by integration tests through active runtime paths.

**Behavior**:
- GIVEN: settings/mcp views and presenters are wired
- WHEN: MCP actions occur
- THEN: status/config commands and state transitions are validated

**Why This Matters**: MCP lifecycle has multiple known mismatch and placeholder defects.

## Implementation Tasks

### Files to Create

- `tests/gpui_integration_profile_flow.rs`
- `tests/gpui_integration_conversation_flow.rs`
- `tests/gpui_integration_mcp_flow.rs`

### Files to Modify

- test module registration file(s) as needed

### Required Code Markers

```rust
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P07
/// @requirement REQ-INT-002
/// @pseudocode lines 011-137
```

## Pseudocode References

- `analysis/pseudocode/component-003-profile-flow.md` lines 011-137
- `analysis/pseudocode/component-004-conversation-flow.md` lines 018-075, 076-246
- `analysis/pseudocode/component-005-mcp-flow.md` lines 035-107, 196-278

## Verification Commands

### Automated Checks (Structural)

```bash
# test files present
ls tests/gpui_integration_profile_flow.rs
ls tests/gpui_integration_conversation_flow.rs
ls tests/gpui_integration_mcp_flow.rs

# marker checks
grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P07" tests/gpui_integration_*
grep -r "@requirement REQ-INT-002" tests/gpui_integration_*

# reverse testing guard
grep -r "should_panic" tests/gpui_integration_* && exit 1 || true

# fail-before-impl expectation
cargo test gpui_integration -- --nocapture
```

### Structural Verification Checklist

- [ ] Previous phase marker present
- [ ] Integration tests created
- [ ] No reverse-testing patterns
- [ ] Tests fail naturally before P08

### Semantic Verification Checklist (MANDATORY)

1. **Does each test validate behavior, not scaffolding?**
   - [ ] Profile flow assertions include persisted/default selection effects
   - [ ] Conversation flow assertions include message/stream transitions
   - [ ] MCP flow assertions include status/config transitions

2. **Would tests catch breakage?**
   - [ ] Removing integration implementation would fail tests

3. **User reachability**
   - [ ] Tests model active runtime pathways (not synthetic isolated components only)

4. **What’s missing before P08?**
   - [ ] Implementation still required to satisfy integration assertions

## Success Criteria

- Integration test suite exists, is behavior-centric, and fails pre-implementation as expected

## Failure Recovery

If this phase fails:

1. Rollback test files with weak assertions
2. Re-author tests to assert observable outputs only
3. Re-run verification

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P07.md`

```markdown
Phase: P07
Completed: YYYY-MM-DD HH:MM
Tests Added:
- tests/gpui_integration_profile_flow.rs
- tests/gpui_integration_conversation_flow.rs
- tests/gpui_integration_mcp_flow.rs
Pre-Impl Test Status: FAIL (expected)
Verdict: PASS/FAIL
```
