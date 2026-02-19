# Phase 05: GPUI Wiring Implementation

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P05`

## Prerequisites

- Required: Phase 04a completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P04a.md`
- Expected files from previous phase:
  - `tests/gpui_wiring_event_flow_tests.rs`
  - `tests/gpui_wiring_command_routing_tests.rs`
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

### REQ-WIRE-001: Event Flow Integrity

**Full Text**: Standardize presenter intake to global EventBus and remove split event pipelines.

**Behavior**:
- GIVEN: all GPUI UserEvents forwarded into EventBus
- WHEN: any presenter-relevant event arrives
- THEN: corresponding presenter handles it via the same global intake path

**Why This Matters**: Prevents non-deterministic dead paths in profile/settings/mcp flows.

### REQ-WIRE-002: ViewCommand Delivery

**Full Text**: MainPanel must dispatch all supported ViewCommand variants to target views, not only model search results.

**Behavior**:
- GIVEN: commands emitted by any presenter
- WHEN: MainPanel drains command queue
- THEN: command is routed to correct view handler or shared route handler

**Why This Matters**: Without this, behavior exists in services/presenters but never appears in UI.

### REQ-WIRE-006: Presenter Channel Unification

**Full Text**: Replace isolated broadcast-based presenter output paths with unified mpsc path feeding bridge/flume.

**Behavior**:
- GIVEN: runtime startup in `main_gpui.rs`
- WHEN: presenters are created
- THEN: every presenter outputs through one command stream

**Why This Matters**: Eliminates command black holes and simplifies observability.

## Implementation Tasks

### Files to Modify

- `src/main_gpui.rs`
  - Unify presenter construction and output forwarding
  - Remove isolated output channels from active flow
  - Add markers for this phase

- `src/ui_gpui/views/main_panel.rs`
  - Implement full command routing matrix
  - Route chat/history/settings/profile/mcp/model commands

- `src/presentation/model_selector_presenter.rs`
- `src/presentation/settings_presenter.rs`
- `src/presentation/profile_editor_presenter.rs`
- `src/presentation/mcp_add_presenter.rs`
- `src/presentation/mcp_configure_presenter.rs`
  - Align input/output transport contracts with unified model
  - Close mismatch handlers for key UserEvents where in scope

### Required Code Markers

```rust
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
/// @requirement REQ-WIRE-001
/// @pseudocode lines 090-136
```

## Pseudocode References (MANDATORY)

- `component-001-event-pipeline.md` lines 090-136, 137-173
- `component-002-main-panel-routing.md` lines 089-171
- `component-003-profile-flow.md` lines 140-173
- `component-005-mcp-flow.md` lines 015-033, 196-228
- `component-006-settings-flow.md` lines 001-134

## Verification Commands

### Automated Checks

```bash
# Marker checks
grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P05" src/main_gpui.rs src/ui_gpui/views/main_panel.rs src/presentation | wc -l

# Build and targeted tests
cargo build --bin personal_agent_gpui || exit 1
cargo test gpui_wiring -- --nocapture || exit 1
```

### Deferred Implementation Detection (MANDATORY)

```bash
grep -rn "todo!\|unimplemented!" src/main_gpui.rs src/ui_gpui/views/main_panel.rs src/presentation --include="*.rs"
# Expected: No matches in implementation sections

grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/main_gpui.rs src/ui_gpui/views/main_panel.rs src/presentation --include="*.rs"
# Expected: No matches in implementation sections
```

### Semantic Verification Checklist (MANDATORY)

1. **Does behavior exist?**
   - [ ] Events from view actions reach intended presenters
   - [ ] Presenter commands appear in target views

2. **Is it real implementation?**
   - [ ] No placeholder logic in changed paths
   - [ ] Tests verify outputs, not only call counts

3. **Reachability**
   - [ ] Startup path wires all presenters through unified channels
   - [ ] MainPanel routes commands in active render loop

4. **Known mismatch coverage**
   - [ ] `SaveProfileEditor` path aligned
   - [ ] `SaveMcp` path aligned
   - [ ] `McpAddNext` path aligned (if phase scope includes handler adaptation)

## Success Criteria

- `cargo build --bin personal_agent_gpui` passes
- GPUI wiring tests pass
- Event/command paths are unified and observable

## Failure Recovery

1. `git checkout -- src/main_gpui.rs`
2. `git checkout -- src/ui_gpui/views/main_panel.rs`
3. `git checkout -- src/presentation/*_presenter.rs`
4. Re-apply changes by pseudocode block order

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P05.md`

```markdown
Phase: P05
Completed: YYYY-MM-DD HH:MM
Files Modified: [list]
Verification Outputs: [build/tests/deferred checks]
Verdict: PASS/FAIL
```
