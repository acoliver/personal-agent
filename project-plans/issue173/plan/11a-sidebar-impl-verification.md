# Phase 11a: Sidebar Impl Verification

Plan ID: `PLAN-20260416-ISSUE173.P11a`

## Checks

1. `cargo build --all-targets 2>&1 | tail -5` — 0 errors.
2. `cargo test --lib --tests 2>&1 | tail -20` — 0 failures.
3. `grep -n "streaming_conversation_ids" src/ui_gpui/app_store_types.rs` — field present with plan marker.
4. `grep -n "streaming_conversation_ids" src/ui_gpui/app_store.rs` — populated from `active_streaming_targets`.
5. Helper `conversation_has_streaming_indicator` defined in render_sidebar.rs.
6. Sidebar row render references the helper.
7. Placeholder grep clean.
8. Plan marker count ≥ 5.

## Code inspection

- Read the sidebar row code. Confirm the indicator element is conditionally
  rendered using the helper.
- Confirm the indicator does not alter layout when absent (fixed slot).

PASS / FAIL into `project-plans/issue173/plan/.completed/P11A.md`.
