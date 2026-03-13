# Phase 08a: MainPanel Simplification and Bootstrap Deprecation Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P08a`

## Prerequisites

- Required: Phase 08 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P08.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P08.md`

## Verification Commands

```bash
cargo check -q
cargo test --test presenter_selection_and_settings_tests --test chat_view_conversation_switch_regression_tests --test chat_startup_scrollback_layout_regression_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P08" src/ui_gpui src/main_gpui.rs --include="*.rs"

grep -R -n "@requirement[: ]REQ-ARCH-002.3\|@requirement[: ]REQ-ARCH-005.1\|@requirement[: ]REQ-ARCH-005.2\|@requirement[: ]REQ-ARCH-005.3" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -rn "apply_startup_commands\|startup_commands\|ensure_bridge_polling" src/main_gpui.rs src/ui_gpui/views/main_panel.rs
```

## Structural Verification Checklist

- [ ] MainPanel simplification changes exist
- [ ] redundant bootstrap authority removed or explicitly bounded
- [ ] required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched production items

## Semantic Verification Checklist

- [ ] no second semantic state path remains for startup correctness
- [ ] MainPanel is no longer the hidden transcript durability owner
- [ ] convergence and popup independence still pass
- [ ] deterministic negative-control evidence would fail if `apply_startup_commands(...)` or popup-local forwarding were reintroduced as semantic authority
- [ ] any remaining `ensure_bridge_polling(...)` / transport glue is proven non-authoritative by behavior evidence, not only by comments
- [ ] structural grep/readback in this phase is supporting evidence only; PASS requires the negative-control and behavior-led authority proofs above rather than structural absence/presence alone

## Success Criteria

- Verification can explain the remaining MainPanel responsibilities in a short, bounded list
