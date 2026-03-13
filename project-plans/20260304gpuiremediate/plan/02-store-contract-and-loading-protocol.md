# Phase 02: Define Authoritative Store Contract and Loading Protocol

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P02`

## Prerequisites

- Required: Phase 01a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P01a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P01a.md`
- Expected files from previous phase:
  - `analysis/01-state-path-analysis.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-001: Authoritative App Store

**Full Text**: GPUI runtime MUST expose one authoritative store for chat-facing application state.

**Behavior**:
- GIVEN: startup hydration and runtime presenter updates
- WHEN: both mutate state
- THEN: one durable store owns the selected conversation, transcript snapshot, loading state, and related snapshots used by popup views

**Why This Matters**: This is the core convergence point that removes popup-coupled correctness.

### REQ-ARCH-003: Explicit Conversation Loading Protocol

**Full Text**: Selection must be an explicit loading protocol/state, not “clear and hope replay arrives”.

**Behavior**:
- GIVEN: a manual or startup conversation selection
- WHEN: activation happens before transcript replay
- THEN: the store enters explicit loading state and preserves coherent visible state until replacement is ready

**Why This Matters**: Prevents false empty-transcript renders and stale replay races.

## Implementation Tasks

### Files to Create or Validate

- `analysis/02-authoritative-store-contract.md`
- `analysis/pseudocode/01-app-store.md`
- `analysis/pseudocode/02-selection-loading-protocol.md`
- `analysis/pseudocode/03-main-panel-integration.md`

If any of these files already exist from pre-plan drafting, validate them against Phase 00a evidence and tighten them as needed instead of rewriting them mechanically.

### Required Design Content

- minimum authoritative store fields
- store mutation sources and ownership boundary
- generation/request semantics for conversation selection
- explicit `Loading`/`Ready`/`Error` model
- statement that `ConversationActivated` does not clear transcript
- statement that `ConversationMessagesLoaded` remains bulk replacement
- migration note for startup hydration using the same reducer path

## Verification Commands

```bash
test -f project-plans/20260304gpuiremediate/analysis/02-authoritative-store-contract.md
test -f project-plans/20260304gpuiremediate/analysis/pseudocode/01-app-store.md
test -f project-plans/20260304gpuiremediate/analysis/pseudocode/02-selection-loading-protocol.md
test -f project-plans/20260304gpuiremediate/analysis/pseudocode/03-main-panel-integration.md
grep -n "authoritative store\|loading\|ConversationMessagesLoaded\|ConversationActivated\|popup" project-plans/20260304gpuiremediate/analysis/02-authoritative-store-contract.md
grep -n "Loading\|Ready\|ConversationActivated\|ConversationMessagesLoaded" project-plans/20260304gpuiremediate/analysis/pseudocode/01-app-store.md project-plans/20260304gpuiremediate/analysis/pseudocode/02-selection-loading-protocol.md
grep -n "drain_commands\|minting site\|startup transaction\|ConversationCleared\|FinalizeStream" project-plans/20260304gpuiremediate/analysis/02-authoritative-store-contract.md project-plans/20260304gpuiremediate/analysis/pseudocode/01-app-store.md project-plans/20260304gpuiremediate/analysis/pseudocode/03-main-panel-integration.md
```

## Semantic Verification Checklist

- [ ] Store contract is implementation-usable, not aspirational
- [ ] Loading protocol prevents “clear and hope replay arrives” semantics
- [ ] Startup and runtime convergence is explicit in design
- [ ] Popup independence is explicit in design
- [ ] Behavior-preservation constraints are carried forward

## Success Criteria

- Later implementation phases can cite exact store and protocol semantics without guessing
- Pseudocode is focused, numbered, and tied to the targeted seams
