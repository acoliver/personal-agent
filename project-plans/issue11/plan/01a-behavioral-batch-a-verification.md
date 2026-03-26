# Phase 01a: Behavioral Coverage Batch A Verification

## Phase ID

`PLAN-20260325-ISSUE11.P01a`

## Prerequisites

- Required: `P01` completion evidence exists
- Verification: `grep "^## Verdict: PASS" project-plans/issue11/plan/.completed/P01.md` is NOT sufficient by itself; audit the code and tests
- Expected files from previous phase: `project-plans/issue11/plan/.completed/P01.md`

## Requirements Implemented (Expanded)

### REQ-COV-002: New tests are behavioral per goodtests.md

**Full Text**: Added tests must be behavioral, externally meaningful, and consistent with `dev-docs/goodtests.md`.
**Behavior**:
- GIVEN Batch A has added or changed tests
- WHEN verification audits each changed test
- THEN only strong behavioral tests count as PASS evidence

**Why This Matters**: This phase specifically prevents mock theater or structural padding from slipping through.

### REQ-COV-003: Avoid structural/mock-theater coverage padding

**Full Text**: Coverage work must avoid trivial structural tests and mock-driven theater that does not materially validate behavior.
**Behavior**:
- GIVEN changed tests may include some mixed patterns
- WHEN verification reviews them against `goodtests.md`
- THEN weak tests are called out and must be remediated before proceeding

**Why This Matters**: A coverage increase achieved by weak tests is not success.

## Verification Commands

```bash
git diff --name-only -- tests/ src/

cargo test --lib --tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

grep -rn "@plan PLAN-20260325-ISSUE11.P01" tests src || true
```

## Mandatory Semantic Audit

For every changed test file, verify and record:
1. the behavior or contract under test
2. the observable outcome asserted
3. why the test would fail for a real regression
4. why it is not merely proving its own setup or internal wiring

If any changed test is mainly structural or mock theater, verdict is FAIL until remediated.

## Success Criteria

- All changed tests in Batch A satisfy `dev-docs/goodtests.md`
- verification commands pass
- evidence file names any test removed/reworked for quality reasons
