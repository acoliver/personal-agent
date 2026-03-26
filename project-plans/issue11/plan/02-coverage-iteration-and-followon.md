# Phase 02: Coverage Iteration and Follow-on Batch

## Phase ID

`PLAN-20260325-ISSUE11.P02`

## Prerequisites

- Required: `P01a` completed with PASS
- Verification: `grep "^## Verdict: PASS" project-plans/issue11/plan/.completed/P01A.md`
- Expected files from previous phase: `P01.md`, `P01A.md`

## Requirements Implemented (Expanded)

### REQ-COV-001: Coverage gate reaches at least 80% line coverage

**Full Text**: The project must reach the enforced 80% workspace line coverage gate and do so using meaningful behavioral tests.
**Behavior**:
- GIVEN Batch A has landed locally
- WHEN coverage is run again
- THEN the remaining gap is measured and a follow-on behavioral batch closes it or a follow-on plan is created

**Why This Matters**: The issue is complete only when the actual gate passes.

### REQ-COV-005: If still below 80%, create follow-on plans and continue looping

**Full Text**: If the initial implementation does not hit the gate, create additional follow-on plans and keep iterating until coverage passes.
**Behavior**:
- GIVEN coverage may still be below 80 after Batch A
- WHEN the remaining gap is known
- THEN create `project-plans/issue11/followups/PLAN-20260325-ISSUE11-followup-N.md` and continue with another behavioral batch

**Why This Matters**: We need an explicit loop instead of silently stopping short.

## Implementation Tasks

1. Run `cargo coverage` locally.
2. If coverage passes, record the passing result and skip follow-on plan creation.
3. If coverage fails, inspect the remaining hotspots.
4. Add the next behavioral batch in the strongest remaining area, likely among:
   - `src/services/mcp_impl.rs`
   - `src/presentation/settings_presenter.rs`
   - `src/services/profile_migration.rs`
   - other large uncovered behavior-heavy modules evidenced by the new report
5. If the remaining work is substantial, write a numbered follow-on plan under `project-plans/issue11/followups/` before implementing that batch.

## Required Outputs

- either a passing local `cargo coverage` result
- or a new follow-on plan document plus the next behavioral coverage batch

## Verification Commands

```bash
cargo coverage
cargo test --lib --tests
```

## Success Criteria

- Coverage passes locally OR a follow-on plan is created and implemented in the same loop
- Any additional tests added remain compliant with `goodtests.md`
- Evidence records the before/after coverage numbers
