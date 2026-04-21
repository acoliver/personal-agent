# Phase 05a: Cancel Pipeline Impl Verification

Plan ID: `PLAN-20260416-ISSUE173.P05a`

## Checks

1. `cargo build --all-targets 2>&1 | tail -5` — 0 errors.
2. `cargo test --lib --tests 2>&1 | tail -20` — 0 failures.
3. `cargo test chat_presenter 2>&1 | grep -E "test result|^test"` — P04 tests pass.
4. `grep -n "StopStreaming\\b" src/events/types.rs` — struct variant, has `conversation_id: Uuid`.
5. `grep -rn "UserEvent::StopStreaming\b" src/ | grep -v "conversation_id"` — must be empty (no unit-variant usages left).
6. Placeholder detection on all modified files — no matches.
7. `grep -c "@plan PLAN-20260416-ISSUE173.P05" src/` (recursive).

## Code inspection

- Read the new emission sites in render.rs. Confirm they read the selected
  conversation id from view state and skip emission if `None`.
- Read `handle_stop_streaming` in chat_presenter.rs. Confirm it calls
  `chat_service.cancel(conversation_id)` with the id from the match arm.

## Verdict

PASS / FAIL in `project-plans/issue173/plan/.completed/P05A.md`.
