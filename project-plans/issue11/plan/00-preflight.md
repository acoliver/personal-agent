# Phase 00: Preflight Verification

## Phase ID

`PLAN-20260325-ISSUE11.P00`

## Prerequisites

- Required: none
- Verification: none
- Expected files from previous phase: none

## Requirements Implemented (Expanded)

### REQ-COV-001: Coverage gate reaches at least 80% line coverage

**Full Text**: The project must reach the enforced 80% workspace line coverage gate and do so using meaningful behavioral tests.
**Behavior**:
- GIVEN the current codebase and enforced coverage gate
- WHEN preflight is executed
- THEN the real baseline, enforcement points, and highest-value targets are documented before implementation starts

**Why This Matters**: Preflight prevents fake progress and ensures implementation is driven by the real gap.

### REQ-COV-002: New tests are behavioral per goodtests.md

**Full Text**: Added tests must be behavioral, externally meaningful, and consistent with `dev-docs/goodtests.md`.
**Behavior**:
- GIVEN the existing mixed-quality test suite
- WHEN preflight reviews current tests and target areas
- THEN implementation guidance distinguishes strong behavioral tests from weak structural/mock-theater patterns

**Why This Matters**: The issue explicitly rejects low-value test padding.

### REQ-COV-003: Avoid structural/mock-theater coverage padding

**Full Text**: Coverage work must avoid trivial structural tests and mock-driven theater that does not materially validate behavior.
**Behavior**:
- GIVEN the repo has some low-value coverage-oriented tests already
- WHEN the implementation plan is formed
- THEN it targets behavior-heavy modules and forbids copying weak patterns

**Why This Matters**: Hitting the gate the wrong way would undermine release confidence.

## Implementation Tasks

### Files to Create

- `project-plans/issue11/specification.md` - issue specification and constraints
- `project-plans/issue11/execution-tracker.md` - phase tracking
- `project-plans/issue11/plan/00-overview.md` - overview and sequencing
- `project-plans/issue11/plan/00-preflight.md` - this phase document

### Files to Modify

- none

### Required Evidence

Preflight must record:
- current enforced gate location in `xtask/src/main.rs`
- latest real baseline percentage from CI logs
- local toolchain/coverage execution constraints
- largest likely behavioral hotspots
- examples of strong vs weak existing tests relevant to issue #11

## Verification Commands

```bash
# Gate enforcement evidence
grep -n "LINE_COVERAGE_GATE\|workspace-summary.json" xtask/src/main.rs

# Workflow enforcement evidence
grep -n "cargo coverage\|workspace-summary" .github/workflows/pr-quality-and-e2e.yml

# Baseline evidence from CI
gh run view 23547067341 --job 68551252642 --log | tail -n 80

# Existing test landscape
glob "tests/*chat*.rs"
glob "tests/*presenter*.rs"
glob "tests/*mcp*.rs"
glob "tests/*profile*.rs"
glob "tests/*conversation*.rs"
```

## Structural Verification Checklist

- [ ] Plan/specification/tracker files exist
- [ ] Coverage gate location documented
- [ ] Latest CI baseline documented
- [ ] High-yield targets identified
- [ ] goodtests guidance explicitly incorporated

## Semantic Verification Checklist

- [ ] Preflight proves the actual gap is real and large
- [ ] Preflight distinguishes behavioral targets from low-value padding
- [ ] Preflight gives enough evidence to choose the first implementation batch without guessing

## Success Criteria

- Preflight evidence exists and is specific enough to guide implementation
- No implementation begins before preflight verification passes

## Failure Recovery

If this phase fails:
1. Re-run the missing evidence-gathering commands
2. Update the specification/overview with corrected findings
3. Do not proceed to P01 until P00a records PASS
