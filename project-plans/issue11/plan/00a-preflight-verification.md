# Phase 00a: Preflight Verification Audit

## Phase ID

`PLAN-20260325-ISSUE11.P00a`

## Prerequisites

- Required: `PLAN-20260325-ISSUE11.P00`
- Verification: `project-plans/issue11/plan/.completed/P00.md` must exist
- Expected files from previous phase: `P00.md`

## Requirements Implemented (Expanded)

### REQ-COV-001: Coverage gate reaches at least 80% line coverage

**Full Text**: The project must reach the enforced 80% workspace line coverage gate and do so using meaningful behavioral tests.
**Behavior**:
- GIVEN preflight evidence has been collected
- WHEN verification audits it
- THEN the real baseline and enforcement points are confirmed before implementation proceeds

**Why This Matters**: Verification prevents a speculative or fraudulent implementation start.

### REQ-COV-002: New tests are behavioral per goodtests.md

**Full Text**: Added tests must be behavioral, externally meaningful, and consistent with `dev-docs/goodtests.md`.
**Behavior**:
- GIVEN candidate target modules and existing tests
- WHEN verification reviews the preflight findings
- THEN the selected strategy is confirmed to favor behavioral tests over structural padding

**Why This Matters**: The audit must keep the plan aligned with the issue’s explicit quality bar.

## Verification Commands

```bash
# Prerequisite evidence
ls project-plans/issue11/plan/.completed/P00.md

# Coverage gate evidence
grep -n "LINE_COVERAGE_GATE\|workspace-summary.json" xtask/src/main.rs

# Workflow evidence
grep -n "cargo coverage\|workspace-summary" .github/workflows/pr-quality-and-e2e.yml

# Baseline evidence from CI
gh run view 23547067341 --job 68551252642 --log | tail -n 80
```

## Structural Verification Checklist

- [ ] P00 evidence file exists
- [ ] Coverage gate evidence is cited
- [ ] Workflow enforcement evidence is cited
- [ ] Baseline percentage is cited
- [ ] Target list is present

## Semantic Verification Checklist

- [ ] Preflight findings are sufficient to guide P01
- [ ] No evidence suggests reliance on structural/mock-theater tests
- [ ] Remaining ambiguity is small enough for implementation to proceed safely

## Success Criteria

- `## Verdict: PASS` only if all preflight evidence is present and coherent
- Any missing or contradictory evidence results in FAIL and remediation
