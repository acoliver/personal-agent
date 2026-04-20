# Phase 12a: Integration Verification

Plan ID: `PLAN-20260416-ISSUE173.P12a`

## Checks

1. Test module file exists and is wired in.
2. `cargo test --lib three_stream_concurrency 2>&1 | grep -E "^test |test result"` — all pass.
3. `grep -c "@plan PLAN-20260416-ISSUE173.P12" src/services/` ≥ 2.
4. Test reads behaviour, not mocks: inspect the test body and verify it asserts
   on actual `StreamCancelled` events, actual `is_streaming_for` results, and
   actual transcripts — not just on mock call counts.

## Code inspection

- Read the test. Verify it would fail if:
  - `begin_stream` still used a global CAS (concurrent send would be rejected).
  - `cancel` still aborted the global task (all three would die).
  - `resolve_all` still ran unconditionally (B's cancel would resolve A/C's
    approvals).

PASS / FAIL into `project-plans/issue173/plan/.completed/P12A.md`.
