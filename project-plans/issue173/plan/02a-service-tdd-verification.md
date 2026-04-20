# Phase 02a: Service Layer TDD Verification

Plan ID: `PLAN-20260416-ISSUE173.P02a`

## Role

Skeptical auditor. Verify the five mandatory tests exist, are behavioral (not
reverse), and fail for the intended reason.

## Checks

1. `ls src/services/chat_impl/tests/concurrent_streams.rs` exists.
2. `grep -c "@plan PLAN-20260416-ISSUE173.P02" src/services/chat_impl/tests/concurrent_streams.rs` ≥ 5.
3. Each of the five test names from P02 is present.
4. `grep -n "should_panic" src/services/chat_impl/tests/concurrent_streams.rs` returns nothing.
5. `cargo build --all-targets 2>&1 | tail -40` — document output. Expected:
   compile errors specifically about `is_streaming_for`,
   `cancel(conversation_id)`, or the test-only `begin_stream_for_test` helper
   not yet being wired up — errors must be consistent with the spec.
6. No changes to files outside `src/services/chat_impl/tests.rs`,
   `src/services/chat_impl/tests/concurrent_streams.rs`, or any `#[cfg(test)]`
   shim in `chat_impl.rs` — verify via `git status`.

## Verdict

- PASS: all 6 checks pass.
- FAIL: any check fails. List each failing check with evidence.

Write `project-plans/issue173/plan/.completed/P02A.md` with full command output.
