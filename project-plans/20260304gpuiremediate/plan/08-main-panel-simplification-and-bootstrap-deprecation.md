# Phase 08: MainPanel Simplification and Bootstrap Deprecation

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P08`

## Prerequisites

- Required: Phase 07a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P07a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P07a.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-005: MainPanel Responsibility Reduction

**Full Text**: `MainPanel` becomes a thinner composition root and not the semantic owner of transcript durability or replay correctness.

**Behavior**:
- GIVEN: store-backed snapshot ownership is already in place
- WHEN: MainPanel is simplified
- THEN: popup lifecycle, routing, and child composition remain, but transcript correctness no longer depends on MainPanel-specific forwarding/bootstrap behavior

**Why This Matters**: The recovery architecture is incomplete if MainPanel remains the hidden authority.

### REQ-ARCH-002: Startup/Runtime Convergence

**Full Text**: Redundant startup-only command application MUST be removable once the authoritative path is in place.

**Behavior**:
- GIVEN: authoritative store hydration now exists
- WHEN: bootstrap-specific logic is deprecated or removed
- THEN: no second semantic state path remains for startup correctness

**Why This Matters**: Leaving redundant bootstrap authority behind recreates drift risk.

## Implementation Tasks

### Files to Modify

- `src/ui_gpui/views/main_panel.rs`
- `src/main_gpui.rs`
- `src/ui_gpui/app_store.rs`
- any directly related state/bootstrap glue files

### Implementation Requirements

- simplify MainPanel command forwarding to snapshot/render composition duties
- reduce `MainPanel` responsibilities to a bounded set: popup lifecycle, navigation, child-view composition, store subscription, redraw notification, and any temporarily retained transport pumping
- deprecate redundant `startup_commands` / `apply_startup_commands` authority as soon as tests prove equivalence
- if `ensure_bridge_polling(...)` remains after this phase, document exactly why it still exists and prove it no longer owns transcript correctness
- any surviving `ensure_bridge_polling(...)` / transport glue after this phase must not invoke `GpuiBridge::drain_commands()` and must not schedule or trigger production ingress; its remaining role, if any, is transport-only/non-authoritative observation glue
- keep any remaining transport polling as implementation detail, not semantic state owner
- document any intentionally retained compatibility shim and the exact reason it still exists

### Required Code Markers

Every created or materially updated production item in this phase must include markers matching project conventions:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P08
/// @requirement[: ]REQ-ARCH-005.1
/// @pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:014-127
```

Use exact requirement IDs per item, including `REQ-ARCH-002.3`, `REQ-ARCH-005.1`, `REQ-ARCH-005.2`, and `REQ-ARCH-005.3` where relevant.

## Pseudocode References

- `analysis/pseudocode/03-main-panel-integration.md` lines 014-127

## Verification Commands

```bash
cargo check -q
cargo test --test presenter_selection_and_settings_tests --test chat_view_conversation_switch_regression_tests --test chat_startup_scrollback_layout_regression_tests --test gpui_integration_tests --test gpui_wiring_command_routing_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P08" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-002.3\|@requirement[: ]REQ-ARCH-005.1\|@requirement[: ]REQ-ARCH-005.2\|@requirement[: ]REQ-ARCH-005.3" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -rn "apply_startup_commands\|startup_commands\|ensure_bridge_polling" src/main_gpui.rs src/ui_gpui/views/main_panel.rs
```

## Reachability / Integration Verification

- [ ] MainPanel remains render/composition root
- [ ] MainPanel is no longer sole transcript/state authority
- [ ] redundant startup path is removed or clearly bounded as compatibility-only

## Semantic Verification Checklist

- [ ] startup/manual convergence still holds after simplification
- [ ] popup-independent correctness still holds
- [ ] transcript replacement and loading semantics remain intact
- [ ] deterministic behavior evidence shows removing or bounding `apply_startup_commands(...)` / `startup_commands` does not change rendered startup transcript correctness
- [ ] deterministic behavior evidence shows any remaining `ensure_bridge_polling(...)` path is non-authoritative because correctness still holds through `spawn_runtime_bridge_pump(...)` plus store snapshots when popup-local forwarding is absent
- [ ] remaining MainPanel responsibilities can be enumerated concretely and do not include durable transcript ownership

## Success Criteria

- MainPanel responsibilities are materially reduced and no redundant state authority remains hidden there
