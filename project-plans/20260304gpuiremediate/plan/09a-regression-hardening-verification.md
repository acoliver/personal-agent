# Phase 09a: Regression Hardening Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P09a`

## Prerequisites

- Required: Phase 09 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P09.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P09.md`

## Verification Commands

```bash
cargo test --test presenter_selection_and_settings_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests --test llm_client_helpers_tests --test kimi_provider_quirks_integration_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P09" tests src --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-006.1\|@requirement[: ]REQ-ARCH-006.2\|@requirement[: ]REQ-ARCH-006.3\|@requirement[: ]REQ-ARCH-006.4\|@requirement[: ]REQ-ARCH-006.5\|@requirement[: ]REQ-ARCH-006.6\|@requirement[: ]REQ-ARCH-006.7" tests src --include="*.rs"

grep -R -n "@pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" tests src --include="*.rs"
```

## Structural Verification Checklist

- [ ] relevant regression suites were run
- [ ] any newly added regression coverage is in project-standard locations
- [ ] required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched tests/helpers
- [ ] `llm_client_helpers_tests` is included when helper behavior is part of preserved transcript/failure semantics

## Semantic Verification Checklist

- [ ] all preserved behaviors have explicit evidence
- [ ] no preserved behavior is left as an implied assumption
- [ ] freshness-token, failure-path, and no-flash startup coverage are explicitly present where applicable
- [ ] active GPUI streaming/thinking preservation is evidenced where migration touched that state surface
- [ ] named deterministic finalize proof shows one accepted active stream lifecycle yields at most one durable assistant append, duplicate finalize after cleared lifecycle is a no-op, stale/off-target finalize is a no-op, and `Uuid::nil()` resolves before acceptance
- [ ] named same-turn render readback proof shows bounded `ConversationCleared` restores mounted render state from authoritative `current_snapshot()` without revision change
- [ ] any unchanged GPUI treatment of `ShowToolCall` / `UpdateToolCall` is stated explicitly rather than implied
- [ ] helper-layer transcript/failure behavior touched by the migration is re-verified where applicable

## Success Criteria

- Verification clearly ties green regressions back to the preserved behavior list
