# Phase 06: Integration Stub

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P06`

## Prerequisites

- Required: Phase 05a completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P05a.md`
- Expected files from previous phase:
  - `src/main_gpui.rs`
  - `src/ui_gpui/views/main_panel.rs`
  - `src/presentation/chat_presenter.rs`
  - `src/presentation/settings_presenter.rs`
- Preflight verification: Phase 0.5 MUST be completed before any implementation phase

## Requirements Implemented (Expanded)

### REQ-INT-001: Integrate into existing runtime call paths

**Full Text**: The remediation MUST be wired into the active GPUI runtime call paths (startup, bridge, presenter loops, and main-panel dispatch), not implemented in isolation.

**Behavior**:
- GIVEN: Existing startup and navigation flow in `src/main_gpui.rs` and `src/ui_gpui/views/main_panel.rs`
- WHEN: Integration stub is applied
- THEN: Existing code paths point to the unified event/command architecture, even if behavior remains stubbed for later phases

**Why This Matters**: Avoids the common failure where new logic exists but is never reached by real user flows.

### REQ-INT-001.1: Existing code will use the feature

**Full Text**: Explicit integration touchpoints must be updated so current UI actions traverse unified wiring.

**Behavior**:
- GIVEN: Buttons in Chat/Settings/Model/MCP views
- WHEN: Those views emit `UserEvent`s
- THEN: The active startup wiring and presenter intake path are the ones under remediation

**Why This Matters**: Prevents dead code and false confidence from isolated tests.

## Implementation Tasks

### Files to Modify

- `src/main_gpui.rs`
  - Add/adjust minimal integration stubs so all presenter start paths use the same active transport model
  - Ensure no new parallel startup branch is introduced

- `src/ui_gpui/views/main_panel.rs`
  - Add/adjust integration stubs for command dispatch entrypoint reachability
  - Ensure active render loop remains the single command drain source

### Required Code Markers

Every function/struct/test created in this phase MUST include:

```rust
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P06
/// @requirement REQ-INT-001
/// @pseudocode lines 127-173
```

## Pseudocode References

- `analysis/pseudocode/component-001-event-pipeline.md` lines 127-173
- `analysis/pseudocode/component-002-main-panel-routing.md` lines 148-171
- `analysis/pseudocode/component-006-settings-flow.md` lines 150-166

## Verification Commands

### Automated Checks (Structural)

```bash
# Plan markers
grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P06" src/main_gpui.rs src/ui_gpui/views/main_panel.rs | wc -l
# Expected: >= 2

# Requirement markers
grep -r "@requirement REQ-INT-001" src/main_gpui.rs src/ui_gpui/views/main_panel.rs | wc -l
# Expected: >= 2

# Compile
cargo build --bin personal_agent_gpui || exit 1
```

### Structural Verification Checklist

- [ ] Previous phase marker file exists (`.completed/P05a.md`)
- [ ] No skipped phases
- [ ] Listed files modified in-place (no V2/parallel files)
- [ ] Plan and requirement markers added
- [ ] Build passes

### Deferred Implementation Detection (MANDATORY after impl phases)

Stub phase exception applies; implementation can remain partial here. Still run checks to bound risk.

```bash
grep -rn -E "(// FIXME|// HACK)" src/main_gpui.rs src/ui_gpui/views/main_panel.rs --include="*.rs"
# Expected: No new matches from this phase
```

### Semantic Verification Checklist (MANDATORY)

1. **Does the integration stub connect existing paths?**
   - [ ] Startup path in `main_gpui.rs` references unified model stubs
   - [ ] MainPanel remains active command routing entrypoint

2. **Is this real integration, not isolated scaffolding?**
   - [ ] No detached module/file created for integration-only demo code
   - [ ] Existing runtime files are modified directly

3. **Reachability by users**
   - [ ] View actions continue to hit active wiring path

4. **What’s missing (to be completed in P07/P08)?**
   - [ ] Full integration behavior assertions via integration tests
   - [ ] End-to-end implementation fill-in

## Success Criteria

- Integration stubs compile and are reachable through active runtime code paths
- No parallel wiring architecture introduced

## Failure Recovery

If this phase fails:

1. Rollback commands:
   - `git checkout -- src/main_gpui.rs`
   - `git checkout -- src/ui_gpui/views/main_panel.rs`
2. Files to revert:
   - `src/main_gpui.rs`
   - `src/ui_gpui/views/main_panel.rs`
3. Cannot proceed to Phase 07 until fixed

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P06.md`

Contents:

```markdown
Phase: P06
Completed: YYYY-MM-DD HH:MM
Files Modified:
- src/main_gpui.rs
- src/ui_gpui/views/main_panel.rs
Verification:
- marker checks
- cargo build --bin personal_agent_gpui
Verdict: PASS/FAIL
```
