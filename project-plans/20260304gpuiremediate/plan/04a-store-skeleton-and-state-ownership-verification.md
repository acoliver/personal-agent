# Phase 04a: Store Skeleton and State Ownership Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P04a`

## Prerequisites

- Required: Phase 04 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P04.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P04.md`

## Verification Commands

```bash
cargo check -q
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P04" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-001.1\|@requirement[: ]REQ-ARCH-001.3\|@requirement[: ]REQ-ARCH-004.1" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/01-app-store.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "pub mod app_store;" src/ui_gpui/mod.rs
grep -rn "todo!\|unimplemented!" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
cargo test --test presenter_selection_and_settings_tests --test chat_view_conversation_switch_regression_tests -- --nocapture
```

## Structural Verification Checklist

- [ ] Authoritative store code exists in `src/ui_gpui/app_store.rs` or an explicitly documented preflight-approved replacement path
- [ ] Runtime startup owns store initialization
- [ ] MainPanel / views reference store snapshots in active code paths
- [ ] At least one active startup-visible render-driving field (minimum: selected conversation id/title and matching HistoryView selection/highlight) now reads from store snapshots authoritatively
- [ ] `src/ui_gpui/mod.rs` exports `app_store` when that module path is used
- [ ] Required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched production items

## Semantic Verification Checklist

- [ ] Store outlives popup lifecycle
- [ ] State ownership no longer depends exclusively on popup-local view state
- [ ] No placeholder implementation remains in changed paths
- [ ] Default-valued transcript/loading state is acceptable in this phase only if the required minimum authority transfer already happened and the store type, process-lifetime construction, module export, and subscription plumbing are real rather than hollow placeholders
- [ ] This phase established real store ownership/plumbing rather than a compile-only shell
- [ ] structural grep/readback in this phase is supporting evidence only; PASS requires deterministic evidence that startup-visible render-driving state is now store-owned rather than still sourced solely from popup-local view state
- [ ] Verification distinguishes Phase 04 ownership/subscription wiring from the still-pending Phase 05 runtime reduction and Phase 06 startup-hydration migration

## Success Criteria

- Verification can trace startup -> store initialization -> snapshot render path
