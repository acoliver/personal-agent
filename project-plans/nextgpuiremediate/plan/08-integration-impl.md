# Phase 08: Integration Implementation

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P08`

## Prerequisites

- Required: Phase 07a completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P07a.md`
- Expected files from previous phase:
  - `tests/gpui_integration_profile_flow.rs`
  - `tests/gpui_integration_conversation_flow.rs`
  - `tests/gpui_integration_mcp_flow.rs`
- Preflight verification: Phase 0.5 MUST be completed before any implementation phase

## Requirements Implemented (Expanded)

### REQ-INT-002: Implement integration contracts end-to-end

**Full Text**: Implement integration points so the tested profile, conversation, MCP, and settings flows pass in active GPUI runtime.

**Behavior**:
- GIVEN: integration tests authored in Phase 07
- WHEN: integration implementation is applied
- THEN: those tests pass and represent reachable user behavior

**Why This Matters**: This phase converts planned wiring into true end-to-end functionality.

### REQ-WIRE-003: UserEvent mismatch remediation in active paths

**Full Text**: Align GPUI-emitted UserEvents and presenter handlers for known mismatch variants (`SaveProfileEditor`, `SaveMcp`, `McpAddNext`) through active flow.

**Behavior**:
- GIVEN: user triggers profile/mcp actions in GPUI views
- WHEN: events are forwarded and handled
- THEN: correct presenter handlers execute and emit the right commands

**Why This Matters**: These mismatch variants are confirmed blockers for final-mile workflows.

## Implementation Tasks

### Files to Modify

- `src/main_gpui.rs`
- `src/ui_gpui/views/main_panel.rs`
- `src/presentation/chat_presenter.rs`
- `src/presentation/history_presenter.rs`
- `src/presentation/settings_presenter.rs`
- `src/presentation/profile_editor_presenter.rs`
- `src/presentation/mcp_add_presenter.rs`
- `src/presentation/mcp_configure_presenter.rs`
- Any additional in-scope files directly required by integration tests

### Required Code Markers

```rust
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P08
/// @requirement REQ-INT-002
/// @pseudocode lines 081-246
```

## Pseudocode References

- `analysis/pseudocode/component-001-event-pipeline.md` lines 146-173
- `analysis/pseudocode/component-002-main-panel-routing.md` lines 113-171
- `analysis/pseudocode/component-003-profile-flow.md` lines 052-137
- `analysis/pseudocode/component-004-conversation-flow.md` lines 081-246
- `analysis/pseudocode/component-005-mcp-flow.md` lines 015-107, 196-278
- `analysis/pseudocode/component-006-settings-flow.md` lines 001-220

## Verification Commands

### Automated Checks (Structural)

```bash
cargo build --bin personal_agent_gpui || exit 1
cargo test gpui_wiring -- --nocapture || exit 1
cargo test gpui_integration -- --nocapture || exit 1
```

### Deferred Implementation Detection (MANDATORY)

```bash
grep -rn "todo!\|unimplemented!" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
grep -rn "fn .* \{\s*\}" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
```

### Semantic Verification Checklist (MANDATORY)

1. **Does code satisfy integration requirements?**
   - [ ] Profile flow passes integration tests
   - [ ] Conversation flow passes integration tests
   - [ ] MCP flow passes integration tests
   - [ ] Settings flow updates are reachable

2. **Is this real implementation?**
   - [ ] Deferred implementation detection passes
   - [ ] No placeholder stubs in implemented paths

3. **Would tests fail if removed?**
   - [ ] Integration tests are strong enough to catch regressions

4. **Reachability**
   - [ ] Active runtime startup and MainPanel route these behaviors

5. **What’s missing after this phase?**
   - [ ] Final verification and evidence capture in P08a

## Success Criteria

- Integration tests pass
- No deferred implementation artifacts in implemented paths
- End-to-end runtime flows are reachable through active GPUI stack

## Failure Recovery

If this phase fails:

1. Roll back failing files selectively
2. Re-implement by pseudocode segment order
3. Re-run integration tests after each segment

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P08.md`

```markdown
Phase: P08
Completed: YYYY-MM-DD HH:MM
Files Modified: [list]
Tests Passing:
- gpui_wiring
- gpui_integration
Deferred Implementation Detection: PASS/FAIL
Verdict: PASS/FAIL
```
