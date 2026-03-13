# Phase 07a: Popup Independence Integration Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P07a`

## Prerequisites

- Required: Phase 07 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P07.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P07.md`

## Verification Commands

```bash
cargo check -q
cargo test --test chat_view_conversation_switch_regression_tests --test chat_startup_scrollback_layout_regression_tests --test gpui_integration_tests --test gpui_wiring_event_flow_tests --test gpui_wiring_command_routing_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P07" src tests --include="*.rs"

grep -R -n "@requirement[: ]REQ-ARCH-004.1\|@requirement[: ]REQ-ARCH-004.2\|@requirement[: ]REQ-ARCH-004.3\|@requirement[: ]REQ-INT-001.3" src tests --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src tests --include="*.rs"
grep -rn "current_snapshot\|subscribe\|revision\|ensure_bridge_polling" src/ui_gpui src/main_gpui.rs tests --include="*.rs"
```

## Structural Verification Checklist

- [ ] popup lifecycle tests exist or were extended
- [ ] store/snapshot integration supports reopen rendering
- [ ] required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched production items/tests
- [ ] verification artifact names the exact production runtime ingress function/path used by the anti-mirror proof

## Semantic Verification Checklist

- [ ] popup close/open no longer gates transcript correctness
- [ ] reopened popup renders latest snapshot without bootstrap replay dependence
- [ ] deterministic remount/subscription assertions are used instead of fragile real-window automation assumptions
- [ ] verification evidence shows snapshot read/subscription correctness, not merely that polling still occurs
- [ ] explicit anti-mirror proof demonstrates in one deterministic unmount/remount harness/test: popup fully unmounted, popup absence is evidenced by both one deterministic popup-absence witness from subscription plumbing and dropped popup-local subscription/proxy object identity, production `spawn_runtime_bridge_pump(...)` traffic still mutates authoritative store revision while popup path is absent, the same store handle identity survives across unmount/remount via a named acceptable identity witness (stable store instance id, `Arc::ptr_eq`, or equivalent repo-idiomatic allocation identity), remount reads the already-current snapshot without replay, and the proof would fail if a popup-local proxy or second durable transcript mirror still owned correctness
- [ ] selection/loading behavior remains explicit and coherent

## Success Criteria

- Verification evidence demonstrates popup independence, not just continued polling
