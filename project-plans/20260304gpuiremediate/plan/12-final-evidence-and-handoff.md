# Phase 12: Final Evidence and Handoff

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P12`

## Prerequisites

- Required: Phase 11a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P11a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P11a.md`

## Requirements Implemented (Expanded)

### REQ-INT-002: Verification and Quality Gates

**Full Text**: The final phase must consolidate evidence that the recovery architecture is complete, verified, and implementation-usable.

**Behavior**:
- GIVEN: all prior phases passed
- WHEN: final evidence is assembled
- THEN: execution can proceed without architectural ambiguity and with clear completion criteria

**Why This Matters**: The recovery plan is meant to be executable by future implementation work, not merely descriptive.

## Required Deliverables

Create these concrete handoff artifacts:

1. `project-plans/20260304gpuiremediate/plan/.completed/P12.md`
2. `project-plans/20260304gpuiremediate/plan/.completed/final-handoff.md`

### `plan/.completed/P12.md` Required Sections

```markdown
# Phase 12 Completion

## Summary
## Completed Evidence Index
## Final Architecture Invariants
## Remaining Compatibility Shims or Follow-Ups
## Verification Recap
## Verdict
```

### `plan/.completed/final-handoff.md` Required Sections

```markdown
# GPUI Remediation Final Handoff

## Scope Guardrails
## Implemented Architecture Summary
## Phase-by-Phase PASS Evidence
## Key File/Module Targets
## Required Runtime Invariants
## Architecture Risk Matrix and Named Proofs
## Verification Commands and Results Summary
## Known Remaining Bounded Debt
## Ready-for-Execution Statement
```

The handoff must include:
- final evidence summary linked to all completed phases
- concise list of resulting architecture invariants
- explicit note of any intentionally retained compatibility shim or debt still in scope for follow-up
- exact target store module path (`src/ui_gpui/app_store.rs` unless formally superseded during preflight)
- the always-live GPUI runtime reduction ingress owner/path (`spawn_runtime_bridge_pump(...)` in `src/main_gpui.rs` unless Phase 00a formally supersedes it)
- the exact single ordinary-runtime `selection_generation` minting site (`begin_selection(conversation_id)` in the authoritative store boundary)
- the exact production selection intent handler/path (`handle_select_conversation_intent(...)` rooted in `src/main_gpui.rs` unless Phase 00a formally supersedes it)
- explicit confirmation that no other ordinary-runtime production path directly calls `begin_selection(...)`, emits enriched selection transport, or dispatches transcript load work for selected-conversation changes
- if implementation uses repo-idiomatic equivalent names for any plan-standardized shorthand seam, the handoff must include an explicit one-to-one mapping back to the `Normative Shorthand -> Repo-Mapped Equivalent Rule` in `plan/00-overview.md` and prove the responsibility was not split

- the chosen startup hydration form, explicitly naming whether the implementation/evidence ended in Startup Mode A or Startup Mode B and why
- if Startup Mode B was used, the exact repo-grounded seam class (`StartupServiceSeamUnavailable`, `AsyncOnlySourceBeforeMount`, or `StartupCompositionDoesNotProvideTranscriptOutcome`) plus source file/function origin
- if Startup Mode B was used, the handoff must explicitly state why the two non-chosen seam classes do not apply, using exact source file/function evidence rather than prose preference

- the chosen `FinalizeStream` durable model (direct-finalize durable append)
- the exact same-id reselection semantics
- the remaining popup-local state that is still intentionally ephemeral-only
- explicit bounded handling for `ConversationCleared`
- explicit final status line for GPUI `ShowToolCall` / `UpdateToolCall` behavior
- final checklist proving:
  - authoritative store owns chat state
  - startup/manual convergence exists
  - popup independence exists
  - MainPanel is reduced
  - preserved behaviors remain intact
  - full verification suite passed and the quality helper is either green or explicitly shown to have no regression beyond the Phase 00a baseline exception scope
  - every row in the final architecture risk matrix maps to a named passing proof artifact

### Required Proof Format For Every Row In `Architecture Risk Matrix and Named Proofs`

Every proof entry must include these fields:

- **Invariant**
- **Proof type** (`test`, `observer`, or `readback + behavior`)
- **Exact artifact name**
- **Command**
- **Observed result**
- **Why this proves the invariant**
- **Residual caveat** (or `none`)

The final matrix must include named rows for at least:
- always-live GPUI runtime ingress
- single ordinary-runtime minting site
- startup atomic publication / no-flash behavior
- chosen startup hydration form
- popup-absent mutation then reopen via production-path ingress
- anti-mirror / single-authority proof
- `FinalizeStream` direct-finalize durable transcript materialization
- exact same-id reselection semantics
- bounded `ConversationCleared` behavior
- final GPUI status of `ShowToolCall` / `UpdateToolCall`

## Verification Commands

```bash
ls project-plans/20260304gpuiremediate/plan/.completed/
grep -R "Verdict: PASS\|## Verdict: PASS" project-plans/20260304gpuiremediate/plan/.completed
grep -n "^## " project-plans/20260304gpuiremediate/plan/.completed/P12.md
grep -n "^## " project-plans/20260304gpuiremediate/plan/.completed/final-handoff.md
```

## Semantic Verification Checklist

- [ ] evidence is consolidated and traceable
- [ ] final invariants are short, concrete, and implementation-relevant
- [ ] no architectural ambiguity remains for execution
- [ ] final handoff artifacts exist at the exact required paths with the expected section structure
- [ ] final handoff names the always-live ingress, single minting site, selection intent handler, chosen startup hydration form, chosen `FinalizeStream` model, same-id reselection semantics, remaining local ephemeral state, bounded `ConversationCleared`, and the full architecture risk matrix with named proofs
- [ ] every proof row in the handoff follows the required proof format rather than a prose-only summary

## Success Criteria

- The plan can be executed phase-by-phase without guessing intent, deliverables, or verification scope
