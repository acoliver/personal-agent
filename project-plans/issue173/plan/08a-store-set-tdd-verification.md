# Phase 08a: Store Set TDD Verification

Plan ID: `PLAN-20260416-ISSUE173.P08a`

## Checks

1. Test module created, imports `active_streaming_targets`.
2. `grep -c "@plan PLAN-20260416-ISSUE173.P08" src/ui_gpui/` ≥ 3.
3. `cargo build --all-targets 2>&1 | tail -20` — compile failures consistent
   with field rename.
4. No production code change outside test module wiring.

PASS / FAIL into `project-plans/issue173/plan/.completed/P08A.md`.
