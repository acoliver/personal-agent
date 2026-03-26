# Phase 03: Final Verification and PR Submission Readiness

## Phase ID

`PLAN-20260325-ISSUE11.P03`

## Prerequisites

- Required: `P02a` completed with PASS
- Verification: `grep "^## Verdict: PASS" project-plans/issue11/plan/.completed/P02A.md`
- Expected files from previous phase: `P02.md`, `P02A.md`

## Requirements Implemented (Expanded)

### REQ-COV-004: Full verification passes before PR submission

**Full Text**: Full project verification must pass before the PR is submitted.
**Behavior**:
- GIVEN coverage has reached the gate locally
- WHEN final verification runs
- THEN tests, formatting, linting, and coverage all pass before the PR is created

**Why This Matters**: The user explicitly wants the PR submitted only once it should pass.

### REQ-COV-005: If still below 80%, create follow-on plans and continue looping

**Full Text**: If the initial implementation does not hit the gate, create additional follow-on plans and keep iterating until coverage passes.
**Behavior**:
- GIVEN final verification discovers the gate still failing
- WHEN this phase runs
- THEN PR creation is blocked and the process loops back through follow-on planning

**Why This Matters**: Submission before readiness would waste CI cycles and review effort.

## Implementation Tasks

1. Run the full local verification suite:
   - `cargo test --lib --tests`
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo coverage`
2. Review all changed tests against `dev-docs/goodtests.md` one final time.
3. If everything passes, prepare the branch for commit/push/PR.
4. If anything fails, remediate before proceeding.

## Success Criteria

- Full local verification suite passes
- Coverage remains at or above 80%
- Test review finds no remaining weak tests in the changed set
- Branch is ready for PR submission
