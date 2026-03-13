# Phase 03: Write Failing Convergence Tests First

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P03`

## Prerequisites

- Required: Phase 02a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P02a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P02a.md`
- Preflight verification: Phase 00a completed

## Requirements Implemented (Expanded)

### REQ-INT-001: Test-First Recovery

**Full Text**: Failing tests for startup/manual-selection convergence MUST be authored before implementation.

**Behavior**:
- GIVEN: the target authoritative-store architecture is not yet implemented
- WHEN: this phase adds regression/integration tests
- THEN: they fail for the right semantic reasons until the new architecture exists

**Why This Matters**: Prevents placebo implementation and confirms the recovery architecture solves the real bug.

### REQ-ARCH-002: Startup/Runtime Convergence

**Full Text**: Startup hydration and manual selection MUST converge on the same transcript replacement semantics.

**Behavior**:
- GIVEN: startup and manual selection events
- WHEN: tests exercise both paths
- THEN: both are expected to produce the same selected conversation snapshot and rendered transcript snapshot

**Why This Matters**: This is the exact behavior gap being recovered.

## Implementation Tasks

### Files to Create or Modify

- Add conversation-switch convergence tests to `tests/chat_view_conversation_switch_regression_tests.rs`
- Add startup/manual-equivalence and no-flash startup assertions to `tests/chat_startup_scrollback_layout_regression_tests.rs`
- Add presenter-side selection/settings protocol assertions to `tests/presenter_selection_and_settings_tests.rs`
- Define any `SelectionTracker` / `assert_load_failure_protocol(...)` test-local fixture inside the specific test file that uses it unless two files truly need the same helper; avoid creating a new shared test utility module unless duplication becomes material

### Required Test Authoring Constraints

- Tests must target observable behavior through the existing presenter/view/main-panel harnesses already present in the repository.
- Tests may introduce test-local helpers or lightweight protocol fixtures local to the test file when needed.
- Tests must **not** import, instantiate, or assume a concrete authoritative store module from `src/ui_gpui/app_store.rs` yet, because that module is created in Phase 04.
- Base Phase 03 on the harnesses the repo demonstrably already has today:
  - `tests/presenter_selection_and_settings_tests.rs` can observe emitted `ViewCommand` traffic from `ChatPresenter`
  - `tests/chat_view_conversation_switch_regression_tests.rs` and `tests/chat_startup_scrollback_layout_regression_tests.rs` already prove behavior largely by source/ordering/readback assertions
  - `tests/gpui_wiring_command_routing_tests.rs` is **not** a safe pre-implementation dependency for this phase because it intentionally assumes a routing function that does not exist yet
- Therefore, Phase 03 must distinguish two acceptable pre-implementation test shapes:
  1. real runnable behavior tests against existing presenter harnesses that fail semantically
  2. source/readback guardrail tests that prove high-risk invariants already visible in code
- Source/readback guardrail tests are auxiliary only. They may support the phase, but they may not be the primary failing proof for startup/manual convergence or popup-independence architecture failure.
- Do **not** require popup-remount/store-subscription behavior to be fully runnable in Phase 03 if the current harnesses cannot exercise it yet; instead, place the strongest currently-runnable assertions here and defer fully store-backed remount behavior to Phases 05-07.
- Use a test-local protocol fixture such as `SelectionTracker` to consume existing presenter harness outputs and centralize assertions over the emitted command sequence.
- In Phase 03, `SelectionTracker` should only assert things that the current contract can actually observe today: selected conversation id, ordering between `ConversationActivated` and `ConversationMessagesLoaded`, absence of `ConversationCleared`, and transcript replay payload properties.
- Freshness-token and explicit-load-failure assertions may be represented in Phase 03 by an intentionally failing helper entry point that makes the missing protocol explicit, but only for the portions of behavior that are impossible to assert before Phase 05 because the variants/fields do not exist yet.
- Expected pre-implementation failures must be assertion/semantic failures or explicit protocol-mismatch failures in test-local helpers; compile failures caused by referencing a not-yet-created store are not acceptable.
- These protocol-gap sentinels are secondary guardrails, not substitutes for real behavior tests.
- At least one pre-implementation failing test in this phase must be explicitly behavior-led/proof-led for the core convergence defect: it must fail because the current design cannot deliver startup/manual-selection equivalence without authoritative store ownership, not merely because a future protocol field/variant is absent.
- minimum acceptable core behavior-led failure in Phase 03: a presenter-driven runnable test that proves the current emitted command/replay behavior cannot establish startup/manual-selection equivalence under the existing architecture without authoritative store ownership; a pure source/readback guardrail is not sufficient for this minimum.

- For seams that current harnesses cannot honestly exercise yet (for example fully store-backed popup-remount proof), explicit protocol-gap or source/readback guardrails are acceptable in this phase so long as later phases promote them into runnable behavior proofs rather than leaving them rhetorical.
- For the not-yet-existing failure variant specifically, use a test-local helper such as `assert_load_failure_protocol(...)` that intentionally fails with an explicit message like `ConversationLoadFailed variant not yet in ViewCommand — Phase 05 prerequisite` until the real variant exists. Do **not** introduce a parallel test-only enum mirroring `ViewCommand`, and do **not** replace this with placeholder `assert!(true)`/`todo!()` behavior.
- Concrete scaffold pattern expected in Phase 03:

```rust
fn assert_load_failure_protocol(commands: &[ViewCommand]) {
    panic!("ConversationLoadFailed variant not yet in ViewCommand — Phase 05 prerequisite; observed {} commands", commands.len());
}
```

- Once Phase 05 lands, replace that intentional panic with real matching/assertions over the now-real `ConversationLoadFailed { conversation_id, selection_generation, message }` variant in the same helper rather than inventing a second helper path.

### Required Test Scenarios

Phase 03 evidence must classify every intentionally failing test into exactly one of these buckets: `core behavior-led convergence failure`, `auxiliary source/readback guardrail`, or `protocol-gap sentinel`. Protocol-gap sentinels and readback guardrails do not count toward the minimum core behavior-led failure requirement.

Phase 03 must cover the strongest behavior that current harnesses can honestly exercise now, plus explicit protocol-gap guardrails for the behavior that cannot yet be fully exercised until later phases.

Runnable-now scenarios:
1. startup hydration and manual selection produce equivalent transcript replay semantics through emitted presenter commands / existing readback seams
2. `ConversationActivated` alone does not clear the transcript
3. `ConversationMessagesLoaded` performs bulk replacement for the selected conversation under the current contract
4. selection does not emit `ConversationCleared`
5. replay payload preserves transcript fidelity needed by current UI reconstruction

Intentional protocol-gap guardrails allowed in this phase:
6. freshness/generation handling is called out through explicit protocol-mismatch assertions that become real tests in Phase 05
7. explicit load failure via `ConversationLoadFailed { conversation_id, selection_generation, message }` is represented by the intentional helper failure until the variant exists
8. popup reopen / remount correctness is documented with the strongest current test seam available here, but fully store-backed remount proof is owned by Phases 05-07 rather than faked in Phase 03

### Required Code Markers

Every created or materially updated test/helper in this phase must include markers matching project conventions:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P03
/// @requirement[: ]REQ-INT-001.2
/// @pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:001-063
```

Use additional `@requirement` markers for the exact requirement each test covers. When a test spans startup/remount behavior, cite `analysis/pseudocode/03-main-panel-integration.md` as appropriate.

## Pseudocode References

- `analysis/pseudocode/02-selection-loading-protocol.md` lines 001-063
- `analysis/pseudocode/03-main-panel-integration.md` lines 009-013, 045-068

## Verification Commands

```bash
cargo test --test presenter_selection_and_settings_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P03" tests src --include="*.rs"
grep -R -n "@requirement[: ]REQ-INT-001.2\|@requirement[: ]REQ-ARCH-002.4\|@requirement[: ]REQ-ARCH-003.2\|@requirement[: ]REQ-ARCH-003.4\|@requirement[: ]REQ-ARCH-004.3" tests src --include="*.rs"
grep -R -n "SelectionTracker\|selection_generation\|ConversationLoadFailed" tests --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" tests src --include="*.rs"
```

## Phase Verdict Semantics

Phase 03 is an explicit expected-red test-authoring phase.

- `P03` records `Verdict: PASS` when the new/updated tests compile, the intended behavior-led coverage exists, and the observed failures are the expected semantic/protocol-gap failures documented in the phase evidence.
- `P03` records `Verdict: FAIL` if tests do not compile, fail for harness breakage/missing modules, rely on fake-green placeholders, or do not include at least one behavior-led failing proof for the core convergence defect.
- This expected-red PASS rule is unique to Phase 03 / 03a and does not relax PASS requirements for later implementation or verification phases.
- Any intentional protocol-gap sentinel introduced in Phase 03 must be removed or converted into real assertions by the end of Phase 05; any survival past `P05a` is FAIL.


## Deferred / Placeholder Detection Expectations

Test phase rules:
- failing tests are expected
- compile failures are not acceptable
- no placeholder assertions like `assert!(true)`
- no reverse tests that assert panic/todo behavior
- no tests that hard-code dependency on a future concrete store type before Phase 04

## Semantic Verification Checklist

- [ ] Tests fail naturally before implementation
- [ ] Failures correspond to missing convergence/store behavior, not harness breakage or missing modules
- [ ] Tests would catch regressions in startup/manual equivalence
- [ ] Tests verify behavior, not only command presence
- [ ] Tests stay implementation-usable before the store module exists
- [ ] Freshness/loading assertions use a test-local protocol fixture pattern rather than a future store import

## Success Criteria

- The new tests compile
- The new tests fail for architectural reasons prior to implementation
- The test set directly exercises the diagnosed state-delivery seams
