# Phase 10: Quality and Coverage Gate

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P10`

## Prerequisites

- Required: Phase 09a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P09a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P09a.md`

## Requirements Implemented (Expanded)

### REQ-INT-002: Verification and Quality Gates

**Full Text**: Each implementation phase and final integration must define structural checks, semantic verification, reachability checks, placeholder-detection commands, and coverage expectations.

**Behavior**:
- GIVEN: implementation is functionally complete
- WHEN: this quality phase runs
- THEN: formatting, typecheck, linting, placeholder detection, and quality helper evidence prove the code is real and maintainable

**Why This Matters**: Architectural recovery is not done if it passes only narrow tests while carrying placeholders or quality debt.

## Implementation Tasks

### Required Quality Gates

- formatting gate
- typecheck gate
- clippy zero-warning gate
- anti-placeholder grep gates
- quality helper execution
- coverage expectation write-up for affected seams

### Quality Helper Summary

`bash scripts/check-quality.sh` is the repository quality helper used to catch project-standard issues beyond a single targeted test suite. This phase requires it because the recovery crosses shared presenter/view/runtime seams and must satisfy the same repository-wide quality bar as other integrated changes.

### Coverage Expectations

Coverage evidence must be concrete about these seams:
- startup hydration -> authoritative store -> first render
- manual selection -> loading protocol -> transcript replacement
- popup reopen -> latest snapshot render
- stale replay rejection
- explicit load-failure handling via `ConversationLoadFailed` or the implemented equivalent
- preserved provider/layout regressions

Required architecture risk matrix for this phase's evidence:

| Risk / Invariant | Required named proof |
|------------------|----------------------|
| startup known transcript no-flash / atomic publication | named combined subscriber-visible snapshot/revision observer + first-subscriber readback proof only |
| manual selection convergence | named deterministic test only |
| stale success ignored | named deterministic test |
| stale failure ignored | named deterministic test |
| popup-absent mutation then reopen | named deterministic test only |
| single ordinary-runtime `selection_generation` minting site | named deterministic behavior proof required; code/readback may support but not replace it |
| same-id reselection semantics (`Loading`/`Ready` no-op, `Error` retry) | named deterministic behavior proof required; code/readback may support but not replace it |
| streaming/thinking preservation across selection changes | named deterministic test |
| `FinalizeStream` durable transcript materialization | named deterministic test plus streamed-interaction duplicate-check proof |
| bounded `ConversationCleared` exception behavior | named deterministic same-turn test only |
| anti-mirror / single-authority behavior | named deterministic test only |
| ignored/no-op/stale inputs do not publish | named deterministic observer proof only |
| always-live GPUI ingress is authoritative | named deterministic behavior proof required; code/readback may support but not replace it |

Coverage is acceptable only if tests would fail when the authoritative store reduction or loading protocol is removed.

## Verification Commands

```bash
cargo fmt --all
cargo check -q
cargo clippy --all-targets -- -D warnings
bash scripts/check-quality.sh
grep -rn "todo!\|unimplemented!" src/ui_gpui src/presentation src/main_gpui.rs tests/chat_startup_scrollback_layout_regression_tests.rs tests/chat_view_conversation_switch_regression_tests.rs tests/presenter_selection_and_settings_tests.rs tests/seven_bugs_regression_tests.rs tests/llm_client_helpers_tests.rs tests/kimi_provider_quirks_integration_tests.rs --include="*.rs"
grep -rn -E '(assert!\(true\)|todo!\(|unimplemented!\(|panic!\(\s*".*Phase [0-9]+ prerequisite|// TODO: recovery|// FIXME: recovery|// HACK: recovery)' src/ui_gpui src/presentation src/main_gpui.rs tests/chat_startup_scrollback_layout_regression_tests.rs tests/chat_view_conversation_switch_regression_tests.rs tests/presenter_selection_and_settings_tests.rs tests/seven_bugs_regression_tests.rs tests/llm_client_helpers_tests.rs tests/kimi_provider_quirks_integration_tests.rs --include="*.rs"
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P0[3-9]\|@plan[: ]PLAN-20260304-GPUIREMEDIATE.P10" src tests --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-\|@requirement[: ]REQ-INT-" src tests --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/" src tests --include="*.rs"
```

## Semantic Verification Checklist

- [ ] no placeholders in implementation/test code for recovered paths
- [ ] quality helper passes, or a Phase 00a baseline exception rule is honored with explicit no-regression evidence for recovery-touched files
- [ ] coverage narrative proves major recovery seams are tested
- [ ] tests are anti-placeholder: they would fail if store-based behavior were removed
- [ ] required architecture risk matrix is complete and ties every critical invariant to a named test/proof
- [ ] anti-mirror / single-authority coverage is proven by behavior evidence, not only grep/readback evidence
- [ ] publication-discipline coverage proves ignored/no-op/stale inputs do not create extra visible publications where the architecture says they must not

## Success Criteria

- Quality gates and placeholder detection are green with explicit evidence
 proven by behavior evidence, not only grep/readback evidence
- [ ] publication-discipline coverage proves ignored/no-op/stale inputs do not create extra visible publications where the architecture says they must not

## Success Criteria

- Quality gates and placeholder detection are green with explicit evidence
