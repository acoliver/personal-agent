# Phase 11: Full-Suite Integration Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P11`

## Prerequisites

- Required: Phase 10a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P10a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P10a.md`

## Requirements Implemented (Expanded)

### REQ-INT-002: Final Verification and Quality Gates

**Full Text**: Final verification MUST include the named project suite plus quality checks.

**Behavior**:
- GIVEN: the recovery architecture is implemented and quality gates passed
- WHEN: full-suite verification runs
- THEN: the targeted regression/integration suite confirms the recovery in project context

**Why This Matters**: The change must hold across the project’s known risk areas, not just narrow seam tests.

## Verification Commands

```bash
cargo fmt --all
cargo check -q
cargo test --test presenter_selection_and_settings_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests --test llm_client_helpers_tests --test kimi_provider_quirks_integration_tests --test gpui_integration_tests --test gpui_bridge_tests --test gpui_chat_view_tests --test gpui_components_tests --test gpui_wiring_event_flow_tests --test gpui_wiring_command_routing_tests -- --nocapture
cargo clippy --all-targets -- -D warnings
bash scripts/check-quality.sh
```

## Final Architecture Risk Matrix

| Risk / Invariant | Final proof expectation |
|------------------|-------------------------|
| startup known transcript no-flash / atomic publication | named passing combined subscriber-visible snapshot/revision observer + first-subscriber readback proof |
| manual selection convergence | named passing deterministic test |
| stale success ignored | named passing deterministic test |
| stale failure ignored | named passing deterministic test |
| popup-absent mutation then reopen | named passing deterministic GPUI/runtime-path test |
| always-live GPUI ingress is authoritative | named passing deterministic behavior proof required; code/readback may support but not replace it |
| single ordinary-runtime minting site | named passing deterministic behavior proof required; code/readback may support but not replace it |
| same-id reselection semantics (`Loading`/`Ready` no-op, `Error` retry) | named passing deterministic behavior proof required; code/readback may support but not replace it |
| streaming/thinking preservation across selection changes | named passing deterministic test |
| `FinalizeStream` durable transcript materialization | named passing deterministic test plus streamed-interaction duplicate-check proof |
| bounded `ConversationCleared` exception behavior | named passing deterministic same-turn test |
| anti-mirror / single-authority behavior | named passing deterministic test |
| ignored/no-op/stale inputs do not publish | named passing deterministic observer proof |

## Reachability / Integration Verification

- [ ] startup/manual convergence verified in project context
- [ ] popup-independent snapshot correctness remains verified in project context
- [ ] architecture-specific GPUI/store/bridge/remount tests added or extended in Phases 05-07 are included in final verification
- [ ] provider/helpers regressions remain green
- [ ] quality helper remains green after full suite, or the final evidence proves no regression beyond the explicit Phase 00a baseline exception scope

## Semantic Verification Checklist

- [ ] full named suite passes without downgrading failures as unrelated
- [ ] convergence architecture survives full project validation
- [ ] preserved behaviors remain intact under integrated verification
- [ ] integrated verification still preserves active GPUI streaming/thinking behavior where that surface was touched by the recovery
- [ ] `FinalizeStream` durable transcript materialization is explicitly covered in final evidence
- [ ] bounded `ConversationCleared` exception behavior is explicitly covered in final evidence
- [ ] anti-mirror / single-authority proof is present in the final architecture risk matrix
- [ ] any unchanged GPUI handling of `ShowToolCall` / `UpdateToolCall` remains explicit in final evidence rather than assumed

## Success Criteria

- All required final commands pass, or any explicit Phase 00a baseline-exception-scoped `scripts/check-quality.sh` failure is accompanied by no-regression evidence for recovery-touched files and is documented exactly as such in final evidence
- No conditional pass language is used
