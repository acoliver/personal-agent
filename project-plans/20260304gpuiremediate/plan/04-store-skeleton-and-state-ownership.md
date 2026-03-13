# Phase 04: Implement Store Skeleton and State Ownership

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P04`

## Prerequisites

- Required: Phase 03a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P03a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P03a.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-001: Authoritative App Store

**Full Text**: GPUI runtime MUST expose one authoritative store for chat-facing application state.

**Behavior**:
- GIVEN: runtime startup and popup mount/unmount
- WHEN: the store is introduced
- THEN: one durable owner exists for selected conversation state and transcript snapshot

**Why This Matters**: This is the ownership change that eliminates popup-local correctness.

### REQ-ARCH-004: Popup Independence

**Full Text**: Popup visibility MUST NOT determine whether presenter-originated chat state reaches the authoritative store.

**Behavior**:
- GIVEN: popup closed or reopened
- WHEN: state changes occur
- THEN: the store still holds the latest snapshot needed for rendering

**Why This Matters**: Delivery correctness must no longer depend on mounted popup state.

## Implementation Tasks

### Files to Modify

- `src/main_gpui.rs`
- `src/ui_gpui/app_store.rs`
- `src/ui_gpui/mod.rs`
- `src/ui_gpui/views/main_panel.rs`
- `src/ui_gpui/views/chat_view.rs`
- `src/ui_gpui/views/history_view.rs`

### Implementation Requirements

- create the authoritative store skeleton in `src/ui_gpui/app_store.rs`
- register the module in `src/ui_gpui/mod.rs` with `pub mod app_store;` if that export is still absent at implementation time
- make store lifetime independent from popup open/close
- define the store-owned snapshot/types needed by chat/history/settings rendering
- initialize the store during GPUI runtime startup with default/placeholder-free state ownership, but without yet migrating startup hydration or runtime command reduction into it
- wire snapshot construction and subscription plumbing into mounted views/MainPanel so mounted views can read store-backed snapshots once later phases begin mutating them
- transfer at least one live render-driving field to store authority in this phase so the store is not a hollow parallel structure; the minimum acceptable authority transfer is startup-visible selected conversation id/title plus matching HistoryView selection/highlight state rendering from the store snapshot in an active path
- keep the existing `startup_commands` bootstrap path as the active startup transcript source during this phase; Phase 06 owns migrating startup hydration onto the store path
- it is acceptable in this phase for transcript/loading snapshots to remain mostly default-valued, but not for every view-driving field to remain popup-local; ordinary runtime selection/highlight updates still become store-authoritative in Phase 05, and this phase must not fake semantic convergence by claiming runtime/store reduction already exists
- reserve the single synchronization/publication boundary for the store design, but do not yet route ordinary presenter/runtime commands through it in this phase
- transitional ownership rule for Phase 04: startup transcript durability may remain on the legacy bootstrap path until Phases 05/06 complete the semantic cutover; the prohibition in this phase is against creating a second live semantic transcript authority or dual durable transcript owners, not against temporarily leaving transcript durability on the legacy path while the store skeleton/subscription wiring is established
- do not yet implement the full reducer semantics or protocol migration in this phase; Phase 05 owns selection-generation protocol changes, reducer freshness rules, explicit failure-command handling, and presenter/runtime mutation wiring into the reducer

### Required Code Markers

Every created or materially updated production item in this phase must include project-convention markers:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P04
/// @requirement[: ]REQ-ARCH-001.1
/// @pseudocode[: ]analysis/pseudocode/01-app-store.md:001-098
```

Use exact requirement and pseudocode ranges per item. Snapshot-subscription and popup-lifetime wiring should cite `analysis/pseudocode/03-main-panel-integration.md:001-035` where applicable.

## Pseudocode References

- `analysis/pseudocode/01-app-store.md` lines 001-098
- `analysis/pseudocode/03-main-panel-integration.md` lines 001-035

## Verification Commands

```bash
cargo check -q
cargo test --test presenter_selection_and_settings_tests --test chat_view_conversation_switch_regression_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P04" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-001.1\|@requirement[: ]REQ-ARCH-001.3\|@requirement[: ]REQ-ARCH-004.1" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/01-app-store.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "pub mod app_store;" src/ui_gpui/mod.rs
grep -rn "todo!\|unimplemented!" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/main_gpui.rs src/ui_gpui src/presentation --include="*.rs"
```

## Reachability / Integration Verification

- [ ] Store is initialized at runtime startup, not popup mount time
- [ ] MainPanel can read/store snapshots without being the only durable owner
- [ ] Chat/history rendering can subscribe to store-backed snapshot state in at least one active path even if startup/runtime mutation still remains on the old paths during this phase
- [ ] At least one active startup-visible render-driving field (minimum: selected conversation id/title and matching HistoryView selection/highlight) is authoritative from the store rather than popup-local state by the end of this phase
- [ ] Existing `startup_commands` remains the active startup transcript source until Phase 06 migration
- [ ] Module registration/export for `src/ui_gpui/app_store.rs` is present when that file path is used

## Semantic Verification Checklist

- [ ] Store ownership exists in code, not only in comments
- [ ] Popup closure does not destroy authoritative state
- [ ] Placeholder detection returns no matches in implemented paths
- [ ] Phase boundary is respected: this phase creates the store/types/subscriptions and startup-time initialization only, while leaving runtime reduction to Phase 05 and startup hydration migration to Phase 06
- [ ] The required minimum authority transfer really occurred and is not just a compile-time shell or unused snapshot field
- [ ] No claim of startup/runtime convergence is made yet in Phase 04 evidence

## Success Criteria

- Store skeleton compiles and is reachable from active GPUI runtime
- Failing tests move closer to passing for the right reasons
