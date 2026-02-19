# Phase 03: GPUI Wiring Stub

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P03`

## Prerequisites

- Required: Phase 02a completed
- Verification: `grep -r "PLAN-20260219-NEXTGPUIREMEDIATE.P02a" project-plans/nextgpuiremediate/plan`
- Expected files from previous phase:
  - `analysis/pseudocode/component-001-event-pipeline.md`
  - `analysis/pseudocode/component-002-main-panel-routing.md`
  - `analysis/pseudocode/component-003-profile-flow.md`
  - `analysis/pseudocode/component-004-conversation-flow.md`
  - `analysis/pseudocode/component-005-mcp-flow.md`
  - `analysis/pseudocode/component-006-settings-flow.md`
- Preflight verification: Phase 0.5 MUST be completed before implementation

## Requirements Implemented (Expanded)

### REQ-WIRE-001: Event Flow Integrity

**Full Text**: UserEvents emitted by GPUI views MUST traverse a single, deterministic path (GpuiBridge -> user_event_forwarder -> global EventBus -> presenter subscriptions) with no split intake channels.

**Behavior**:
- GIVEN: A user clicks a UI action in any GPUI view
- WHEN: The view emits a `UserEvent`
- THEN: The event is delivered to presenters via the global EventBus path only

**Why This Matters**: Split event intake causes some presenters to never receive actions, which makes the UI look responsive but functionally broken.

### REQ-WIRE-002: ViewCommand Delivery

**Full Text**: ViewCommands from all presenters MUST flow through one unified command path into GPUI so no view updates are silently dropped.

**Behavior**:
- GIVEN: Any presenter emits a `ViewCommand`
- WHEN: The command forwarding loop runs
- THEN: MainPanel receives and routes the command to the right view

**Why This Matters**: Fragmented command channels produce stale UI and hidden failures.

### REQ-WIRE-006: Presenter Channel Unification

**Full Text**: All presenters MUST use the same command output transport (mpsc -> flume -> bridge) and same event intake source (global EventBus).

**Behavior**:
- GIVEN: Startup in `main_gpui.rs`
- WHEN: presenters are constructed
- THEN: they all subscribe to global EventBus and emit to the same output stream

**Why This Matters**: Mixed constructor contracts are the primary root cause of final-mile wiring failure.

## Implementation Tasks

### Files to Modify

- `src/main_gpui.rs`
  - Add minimal stub scaffolding for unified presenter constructor usage
  - Keep runtime compiling while deferring full behavior to later phases
  - Add markers:
    - `/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03`
    - `/// @requirement REQ-WIRE-001`

- `src/presentation/model_selector_presenter.rs`
  - Introduce stub constructor signature compatible with unified channel plan
  - Add markers:
    - `/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03`
    - `/// @requirement REQ-WIRE-006`

- `src/presentation/settings_presenter.rs`
  - Introduce stub constructor compatibility for unified path
  - Add markers:
    - `/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03`
    - `/// @requirement REQ-WIRE-006`

### Required Code Markers

Every stub function/struct change in this phase MUST include:

```rust
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
/// @requirement REQ-WIRE-001
/// @pseudocode lines 090-136 (component-001-event-pipeline.md)
```

## Pseudocode References

- `analysis/pseudocode/component-001-event-pipeline.md` lines 090-136 (bridge/channel setup)
- `analysis/pseudocode/component-002-main-panel-routing.md` lines 089-147 (MainPanel container roles)

## Verification Commands

### Automated Checks (Structural)

```bash
# Check plan markers
grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P03" src/main_gpui.rs src/presentation | wc -l
# Expected: >= 3

# Check requirements markers
grep -r "@requirement REQ-WIRE-00" src/main_gpui.rs src/presentation | wc -l
# Expected: >= 3

# Compile baseline
cargo build --bin personal_agent_gpui || exit 1
```

### Structural Verification Checklist

- [ ] Previous phase artifacts present
- [ ] No skipped phases
- [ ] Target files modified with stub-safe changes
- [ ] Plan/requirement markers added
- [ ] Build passes

### Deferred Implementation Detection

Stub phase exception applies. `todo!()`/`unimplemented!()` may exist in stub sections only.

## Semantic Verification Checklist (MANDATORY)

- [ ] Stub changes compile and preserve startup path
- [ ] New constructor signatures are reachable from `main_gpui.rs`
- [ ] No duplicate V2 files created
- [ ] No isolated feature branch in code (edits are in existing files)

## Success Criteria

- Build passes
- Stub interfaces are in place for unification and later TDD/impl phases
- No regressions introduced in startup wiring

## Failure Recovery

If this phase fails:

1. Rollback commands:
   - `git checkout -- src/main_gpui.rs`
   - `git checkout -- src/presentation/model_selector_presenter.rs`
   - `git checkout -- src/presentation/settings_presenter.rs`
2. Cannot proceed to Phase 04 until compile baseline is restored

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P03.md`

Contents:

```markdown
Phase: P03
Completed: YYYY-MM-DD HH:MM
Files Modified:
- src/main_gpui.rs
- src/presentation/model_selector_presenter.rs
- src/presentation/settings_presenter.rs
Verification:
- cargo build --bin personal_agent_gpui [PASS/FAIL]
Verdict: PASS/FAIL
```
