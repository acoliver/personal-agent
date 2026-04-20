# Phase 06a: Approval Gate TDD Verification

Plan ID: `PLAN-20260416-ISSUE173.P06a`

## Checks

1. Tests present: both named functions exist in `src/llm/client_agent/tests.rs`.
2. `grep -c "@plan PLAN-20260416-ISSUE173.P06" src/llm/` ≥ 2.
3. `cargo build --all-targets 2>&1 | tail -10` — consistent failure about
   missing `resolve_all_for_conversation`.
4. `grep -n "should_panic" src/llm/client_agent/tests.rs` — empty.

PASS / FAIL into `project-plans/issue173/plan/.completed/P06A.md`.
