# Phase 10a: Sidebar TDD Verification

Plan ID: `PLAN-20260416-ISSUE173.P10a`

## Checks

1. Test functions present with correct names.
2. `grep -c "@plan PLAN-20260416-ISSUE173.P10" src/ui_gpui/` ≥ 4.
3. `cargo build --all-targets 2>&1 | tail -20` — expected failures referencing
   `streaming_conversation_ids` or `conversation_has_streaming_indicator`.
4. No production code changed (only test files).

PASS / FAIL into `project-plans/issue173/plan/.completed/P10A.md`.
