# Escalation: P05a Failed After 3 Remediation Attempts

Plan: `PLAN-20260402-MARKDOWN`
Phase: `P05a` (Parser Implementation Verification)
Status: **ESCALATED**

## Why this escalation exists

Per `dev-docs/COORDINATING.md`, remediation is capped at 3 attempts. `P05a` remains FAIL after attempt 3, so execution cannot proceed to `P06+` without human direction.

## Evidence artifacts

- Verification evidence: `project-plans/issue62/plan/.completed/P05a.md`
- Tracker: `project-plans/issue62/execution-tracker.md`

## Current failing gates

1. `cargo test --lib -- markdown_content` fails (4 tests)
2. `cargo clippy --all-targets -- -D warnings` fails
3. `cargo fmt --all -- --check` fails

## Verified root causes

### 1) List parsing lifecycle bug
`parse_markdown_blocks()` list-item flow still emits incorrect structure:
- `Event::Start(Tag::Item)` and `Event::End(TagEnd::Item)` both manipulate item buffering in a way that causes duplicate/split top-level blocks.
- Failing tests:
  - `test_parse_unordered_list`
  - `test_parse_ordered_list`
  - `test_parse_task_list_markers`

### 2) Table header capture bug
Table header row is not reliably captured into `header` (observed as `header.len() == 0` in failing test), indicating timing/state issue in `TableHead`/`TableRow` finalization.
- Failing test:
  - `test_parse_table`

### 3) Strict clippy debt in parser module
`cargo clippy --all-targets -- -D warnings` currently reports many errors, including:
- dead code visibility/use in current phase wiring
- style/structure lints (`too_many_lines`, `match_same_arms`, `use_self`, etc.)
- doc/style lints

## Decision options (need approval)

### Option A (recommended): Focused parser fix + scoped lint policy for this phase
- Fix list/table parser semantics first so all parser tests pass.
- Apply narrowly-scoped clippy allowances in `markdown_content.rs` only where justified for this phase (while module integration is incomplete), then continue to next phases.
- Pros: Fastest path to unblock with controlled risk.
- Cons: Introduces temporary local lint allowances that should be removed later.

### Option B: Refactor parser heavily now to satisfy clippy without allowances
- Fully refactor `parse_markdown_blocks()` into smaller helper functions and clean all style lints now.
- Pros: Cleaner module earlier.
- Cons: Larger change set and higher regression risk before integration phases.

### Option C: Re-sequence plan checks
- Keep P05 semantic correctness strict, but defer full `clippy --all-targets -D warnings` gate to a later integration phase where module is fully wired and dead-code noise is naturally reduced.
- Pros: Aligns lint gate with reachability.
- Cons: Requires explicit plan change approval.

## Requested human decision

Please select one:
- **Approve Option A**
- **Approve Option B**
- **Approve Option C**
- **Provide alternate instruction**

Execution is intentionally paused on phase progression until this decision is made.

---

## Human Decision Recorded (2026-04-03)

Decision: **Approve A + C**

Interpretation applied:
1. Keep project clippy configuration unchanged (`cargo clippy --all-targets -- -D warnings`).
2. Do not enforce clippy as pass/fail during stub phases (P03/P03a, P06/P06a, P10/P10a).
3. For parser phases that coexist with later renderer/integration stubs, treat clippy as non-gating in P05/P05a; enforce strict clippy again at P08a and later full gates.
4. Apply this sequencing approach to subsequent phases with intentional stubs, while preserving strict clippy gates for implementation/cleanup/final verification phases.

Files updated to reflect this approval:
- `project-plans/issue62/plan/00-overview.md`
- `project-plans/issue62/plan/03-parser-stub.md`
- `project-plans/issue62/plan/03a-parser-stub-verification.md`
- `project-plans/issue62/plan/05-parser-impl.md`
- `project-plans/issue62/plan/05a-parser-impl-verification.md`
- `project-plans/issue62/plan/06-renderer-stub.md`
- `project-plans/issue62/plan/06a-renderer-stub-verification.md`
- `project-plans/issue62/plan/10-integration-stub.md`
- `project-plans/issue62/plan/10a-integration-stub-verification.md`