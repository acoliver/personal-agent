# Phase 11a: Full-Suite Integration Verification Audit

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P11a`

## Prerequisites

- Required: Phase 11 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P11.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P11.md`

## Verification Commands

The first block verifies that Phase 11 lists the full required command set. The second block re-runs that actual verification suite so Phase 11a is an audit, not a rubber stamp.

```bash
grep -n "cargo fmt --all" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
grep -n "cargo check -q" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
grep -n "presenter_selection_and_settings_tests" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
grep -n "llm_client_helpers_tests" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
grep -n "gpui_integration_tests\|gpui_bridge_tests\|gpui_chat_view_tests\|gpui_components_tests\|gpui_wiring_event_flow_tests\|gpui_wiring_command_routing_tests" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
grep -n "kimi_provider_quirks_integration_tests" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
grep -n "scripts/check-quality.sh" project-plans/20260304gpuiremediate/plan/11-full-suite-integration-verification.md
cargo fmt --all
cargo check -q
cargo test --test presenter_selection_and_settings_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests --test llm_client_helpers_tests --test kimi_provider_quirks_integration_tests --test gpui_integration_tests --test gpui_bridge_tests --test gpui_chat_view_tests --test gpui_components_tests --test gpui_wiring_event_flow_tests --test gpui_wiring_command_routing_tests -- --nocapture
cargo clippy --all-targets -- -D warnings
bash scripts/check-quality.sh
test -f project-plans/20260304gpuiremediate/plan/.completed/P11.md && grep -n "Architecture Risk Matrix\|always-live GPUI runtime ingress\|single ordinary-runtime minting site\|startup atomic publication / no-flash behavior\|popup-absent mutation then reopen via production-path ingress\|anti-mirror / single-authority proof\|FinalizeStream direct-finalize durable transcript materialization\|bounded ConversationCleared behavior\|Observed result\|Why this proves the invariant" project-plans/20260304gpuiremediate/plan/.completed/P11.md

```


## Structural Verification Checklist

- [ ] all required final commands are listed exactly
- [ ] project quality helper is included
- [ ] audit evidence includes fresh reruns of the actual final verification commands, not only plan-document greps
- [ ] final architecture risk matrix from Phase 11 is attached to the audit artifact

## Semantic Verification Checklist

- [ ] final suite scope matches the recovery architecture risks
- [ ] no required verification command is omitted
- [ ] rerun final verification evidence still passes at audit time, or any explicit Phase 00a baseline-exception-scoped `scripts/check-quality.sh` failure is re-audited with matching no-regression evidence for recovery-touched files
- [ ] audit confirms the final architecture risk matrix is present and every row cites a named passing test/proof artifact
- [ ] audit confirms anti-mirror / single-authority behavior is proven by the named deterministic unmount/remount harness with same-store-handle identity, not only grep/readback evidence
- [ ] audit confirms always-live ingress and single-minting-site rows in the final architecture risk matrix are backed by deterministic behavior proofs as primary evidence, with code/readback only as support
- [ ] audit confirms bounded `ConversationCleared` evidence includes named same-turn render readback proof and publication-discipline evidence includes named revision-count or subscriber-visible snapshot observer proof

## Success Criteria

- Final verification phase is complete, executable without ambiguity, and revalidated by the audit itself
