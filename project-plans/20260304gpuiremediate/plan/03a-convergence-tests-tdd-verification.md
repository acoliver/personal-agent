# Phase 03a: Convergence Tests TDD Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P03a`

## Prerequisites

- Required: Phase 03 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P03.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P03.md`

## Verification Commands

```bash
cargo test --test presenter_selection_and_settings_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P03" tests src --include="*.rs"
grep -R -n "@requirement[: ]REQ-INT-001.2\|@requirement[: ]REQ-ARCH-002.4\|@requirement[: ]REQ-ARCH-003.2\|@requirement[: ]REQ-ARCH-003.4\|@requirement[: ]REQ-ARCH-004.3" tests src --include="*.rs"
grep -R -n "SelectionTracker\|selection_generation\|ConversationLoadFailed" tests --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" tests src --include="*.rs"
```
## Phase Verdict Semantics

Phase 03a verifies an expected-red test set.

- `P03a` records `Verdict: PASS` when the tests compile and fail in the expected semantic ways documented in evidence, with at least one behavior-led failure for the core convergence defect.
- `P03a` records `Verdict: FAIL` if the failures are caused by harness breakage, missing modules, fake-green placeholders, or absent behavior-led proof coverage.
- This is the only verification phase where expected failing tests may still produce PASS.
- Any intentional protocol-gap sentinel introduced in Phase 03 must be removed or converted into real assertions by the end of Phase 05; any survival past `P05a` is FAIL.



## Structural Verification Checklist

- [ ] New or updated tests exist in project-standard locations
- [ ] Tests compile
- [ ] Required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched tests/helpers
- [ ] Test-local protocol fixture pattern (for example `SelectionTracker`) exists where freshness/loading semantics need future protocol coverage
- [ ] Tests fail before implementation for semantic reasons

## Semantic Verification Checklist

- [ ] Tests cover startup/manual convergence using current runnable harnesses or honest readback guardrails
- [ ] Tests cover no-clear-on-activation behavior
- [ ] Tests cover bulk replacement behavior under the current contract
- [ ] Phase evidence classifies each failing test as `core behavior-led convergence failure`, `auxiliary source/readback guardrail`, or `protocol-gap sentinel`
- [ ] At least one named core behavior-led convergence failure exists independent of any protocol-gap sentinel helper
- [ ] Any freshness-token / explicit-load-failure assertions that cannot yet run are represented as explicit protocol-gap failures, not fake green coverage
- [ ] Popup reopen/store-subscription proof is not faked in this phase beyond what existing harnesses can actually observe
- [ ] Tests use existing harnesses or test-local helpers rather than referencing the not-yet-created concrete store module
- [ ] Failures would disappear only when the authoritative-store architecture is implemented correctly

## Success Criteria

- Verification evidence shows useful failing tests, not broken scaffolding
