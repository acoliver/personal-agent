# Phase 04: GPUI Wiring TDD

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P04`

## Prerequisites

- Required: Phase 03a completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P03a.md`
- Expected files from previous phase:
  - `src/main_gpui.rs`
  - `src/presentation/*_presenter.rs`
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

### REQ-WIRE-001: Event Flow Integrity

**Full Text**: User events from every GPUI view must reach the responsible presenter through global EventBus.

**Behavior**:
- GIVEN: View emits `UserEvent::OpenModelSelector`, `UserEvent::SaveProfileEditor`, `UserEvent::McpAddNext`
- WHEN: Runtime forwarder and presenter subscriptions are active
- THEN: matching presenter handlers are executed

**Why This Matters**: These are currently mismatch hotspots that block profile/MCP flows end-to-end.

### REQ-WIRE-002: ViewCommand Delivery

**Full Text**: MainPanel must route all relevant ViewCommand variants to concrete view handlers.

**Behavior**:
- GIVEN: Presenter emits `ViewCommand` for chat/history/settings/model/profile/mcp
- WHEN: command drain loop runs
- THEN: target view state updates reflect command intent

**Why This Matters**: Today almost all commands are dropped at MainPanel.

## Implementation Tasks (Tests Only)

### Files to Create

- `tests/gpui_wiring_event_flow_tests.rs`
  - MUST include markers:
    - `/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04`
    - `/// @requirement REQ-WIRE-001`
  - Tests:
    - EventBus receives forwarded view events
    - Presenter handlers invoked for model/profile/mcp mismatch variants

- `tests/gpui_wiring_command_routing_tests.rs`
  - MUST include markers:
    - `/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04`
    - `/// @requirement REQ-WIRE-002`
  - Tests:
    - MainPanel routes representative ViewCommand variants to each view

### Files to Modify

- `tests/mod.rs` or integration test registration entrypoint as needed
  - Add module declarations for new tests

## Test Constraints

- NO reverse testing (`#[should_panic]` forbidden)
- Tests must fail naturally pre-implementation
- Tests must assert real behavior, not mere invocation

## Verification Commands

### Structural Checks

```bash
# Ensure tests exist
ls tests/gpui_wiring_event_flow_tests.rs
ls tests/gpui_wiring_command_routing_tests.rs

# No reverse tests
grep -r "should_panic" tests/gpui_wiring_* && exit 1 || true

# Expect failing tests before impl
cargo test gpui_wiring -- --nocapture
```

### Structural Checklist

- [ ] Test files created
- [ ] Plan/requirement markers present
- [ ] No reverse testing patterns
- [ ] Tests fail for behavioral reasons pre-impl

### Semantic Checklist

- [ ] Tests assert end-to-end event flow from emit -> presenter reaction
- [ ] Tests assert command routing, not just command existence
- [ ] Tests cover mismatch events (`SaveProfileEditor`, `SaveMcp`, `McpAddNext`)

## Success Criteria

- Test suite in place and failing for the right reasons
- Ready for implementation phase without rewriting tests

## Failure Recovery

- Remove weak/non-behavioral tests
- Rewrite tests to validate observable outputs

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P04.md`

```markdown
Phase: P04
Completed: YYYY-MM-DD HH:MM
Tests Added:
- tests/gpui_wiring_event_flow_tests.rs
- tests/gpui_wiring_command_routing_tests.rs
Test Status Pre-Impl: FAIL (expected)
Verdict: PASS/FAIL
```
