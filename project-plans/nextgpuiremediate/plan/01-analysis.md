# Phase 01: Analysis

## Phase ID
`PLAN-20260219-NEXTGPUIREMEDIATE.P01`

## Prerequisites

- Required: Phase 0.5 completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P0.5.md`
- Expected files from previous phase:
  - `plan/00-overview.md`
  - `plan/00a-preflight-verification.md`

## Requirements Implemented (Expanded)

### REQ-WIRE-001: Event Flow Integrity

**Full Text**: The active GPUI runtime must use a single end-to-end event pipeline from views to presenters through global EventBus.

**Behavior**:
- GIVEN: UI actions emitted by GPUI views
- WHEN: startup wiring initializes bridges/presenters
- THEN: events route consistently through global EventBus path

**Why This Matters**: This is the root defect behind dead controls and inconsistent behavior.

### REQ-INT-001: Integration Analysis Coverage

**Full Text**: Plan must identify exact integration touchpoints, replacement targets, and user access paths in the active GPUI runtime.

**Behavior**:
- GIVEN: current runtime source tree
- WHEN: analysis phase completes
- THEN: domain model documents concrete file/function-level integration map

**Why This Matters**: Prevents isolated “perfect” implementations that do not solve active runtime issues.

## Implementation Tasks

### Files to Create/Update

- `analysis/domain-model.md`
  - Document concrete component relationships
  - Map exact touchpoints in:
    - `src/main_gpui.rs`
    - `src/ui_gpui/views/main_panel.rs`
    - `src/presentation/*`
    - `src/events/types.rs`

### Required Analysis Outputs

- Existing code that will use changes (explicit file/function list)
- Existing code to replace/remove
- User access points (buttons/shortcuts/view actions)
- Scope boundaries (what is explicitly out-of-scope for first working version)

## Verification Commands

```bash
# Ensure analysis artifact exists
ls project-plans/nextgpuiremediate/analysis/domain-model.md

# Ensure integration sections exist
grep -n "Existing Code That Will Use" project-plans/nextgpuiremediate/analysis/domain-model.md
grep -n "Existing Code To Be Replaced" project-plans/nextgpuiremediate/analysis/domain-model.md
grep -n "User Access Points" project-plans/nextgpuiremediate/analysis/domain-model.md
grep -n "Out-of-Scope" project-plans/nextgpuiremediate/analysis/domain-model.md
```

## Success Criteria

- Domain model includes actionable integration map with file-level specificity

## Failure Recovery

- Re-run codebase analysis and expand missing integration sections before Phase 01a

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P01.md`
