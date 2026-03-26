# Plan: Reach 80% Meaningful Coverage

Plan ID: PLAN-20260325-ISSUE11
Generated: 2026-03-25
Total Phases: 8
Requirements: REQ-COV-001, REQ-COV-002, REQ-COV-003, REQ-COV-004, REQ-COV-005

## Critical Reminders

Before implementing ANY phase, ensure you have:

1. Completed preflight verification (P00 and P00a)
2. Grounded the work in the real enforced coverage gate, not assumptions
3. Written/expanded behavioral tests BEFORE any enabling production refactor
4. Verified all touched tests satisfy `dev-docs/goodtests.md`
5. Avoided structural padding and mock-theater coverage chasing

## Current Baseline

The latest merged-code CI coverage gate reports:
- lines: `65.06% (14608/22453, missed 7845)`
- regions: `62.92%`
- functions: `67.18%`

Evidence source:
- `gh run view 23547067341 --job 68551252642 --log`

## Scope

In scope:
- Behavioral coverage additions in existing tests or new tests under `tests/`
- Small enabling refactors in production code if required to make behavioral tests possible
- Coverage iteration until `cargo coverage` passes
- Follow-on plans under `project-plans/issue11/followups/` if needed
- PR creation only after local verification is green

Out of scope:
- Structural-only tests
- Mock-heavy wiring tests that primarily prove harness behavior
- Coverage padding via enum/display/constructor tours
- Redesigns unrelated to behavioral coverage goals

## Highest-Value Initial Targets

Priority batch A:
- `src/services/chat_impl.rs`
- `src/presentation/chat_presenter.rs`
- `src/mcp/runtime.rs`
- `src/services/conversation_impl.rs`
- `src/services/profile_impl.rs`

Priority batch B if still needed after batch A:
- `src/services/mcp_impl.rs`
- `src/presentation/settings_presenter.rs`
- `src/services/profile_migration.rs`

## Behavioral Test Policy

Every new or modified test must satisfy these standards from `dev-docs/goodtests.md`:
- proves externally meaningful behavior or contract
- uses mocks/fakes only as boundary controls, not as the main evidence
- would fail on a real regression
- is resilient to harmless refactors
- does not merely re-state inputs or current internal wiring

## Mandatory Sequence

`P00 -> P00a -> P01 -> P01a -> P02 -> P02a -> P03 -> P03a`

No skipped steps.

## Phase Map

- **P00/P00a**: preflight verification of coverage gate, local toolchain behavior, existing test landscape, and target selection
- **P01/P01a**: implement and verify first high-yield behavioral coverage batch
- **P02/P02a**: run coverage, inspect remaining gap, implement and verify follow-on behavioral batch or follow-on plan if still needed
- **P03/P03a**: full verification, final coverage pass, PR readiness, and handoff to submit PR

## Follow-on Loop Rule

If P02 verification shows coverage is still below 80%, create `project-plans/issue11/followups/PLAN-20260325-ISSUE11-followup-N.md` and continue the same pattern until the gate passes.

## Success Criteria

- `cargo coverage` passes locally
- workspace line coverage is at least `80.00%`
- all added/changed tests satisfy `dev-docs/goodtests.md`
- `cargo test --lib --tests` passes
- `cargo fmt --all -- --check` passes
- `cargo clippy --all-targets -- -D warnings` passes
- PR opened only after local verification indicates CI should pass
