# Phase 03a: Final Verification and PR Audit

## Phase ID

`PLAN-20260325-ISSUE11.P03a`

## Prerequisites

- Required: `P03` completion evidence exists
- Verification: `project-plans/issue11/plan/.completed/P03.md` must exist
- Expected files from previous phase: `P03.md`

## Requirements Implemented (Expanded)

### REQ-COV-004: Full verification passes before PR submission

**Full Text**: Full project verification must pass before the PR is submitted.
**Behavior**:
- GIVEN the branch claims readiness
- WHEN verification audits the local checks and changed tests
- THEN PASS is granted only if the branch is genuinely PR-ready

**Why This Matters**: This is the gate before opening the PR.

## Verification Commands

```bash
cargo test --lib --tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo coverage

git status --short
```

## Success Criteria

- All local verification commands pass
- changed tests remain good tests under `dev-docs/goodtests.md`
- branch is ready for commit/push/PR creation
- if any verification command fails, verdict is FAIL and PR creation is blocked
