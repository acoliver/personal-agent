# Phase 07: Prove Popup Independence and Reopen Correctness

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P07`

## Prerequisites

- Required: Phase 06a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P06a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P06a.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-004: Popup Independence

**Full Text**: Popup open/close must not determine whether command/state delivery works.

**Behavior**:
- GIVEN: state changes while popup is closed or while popup is reopened
- WHEN: the popup mounts again
- THEN: the current authoritative snapshot renders correctly without special replay dependency

**Why This Matters**: This removes the architectural dependency that currently hides runtime correctness bugs.

### REQ-INT-001: Test-First Recovery

**Full Text**: Semantic tests must verify popup-independent reachability.

**Behavior**:
- GIVEN: store-backed state mutations
- WHEN: popup mount/unmount is exercised in tests
- THEN: correctness is proven through snapshot rendering, not transient queue timing

**Why This Matters**: The architectural target fails if popup timing still secretly owns correctness.

## Implementation Tasks

### Files to Modify

- popup/runtime integration tests in existing project conventions
- prefer placing the anti-mirror / popup-absent mutation proof in `tests/gpui_integration_tests.rs` or `tests/gpui_wiring_command_routing_tests.rs`; if a new dedicated test file is required, record its exact path in phase evidence
- `src/ui_gpui/views/main_panel.rs`
- `src/ui_gpui/app_store.rs`
- `src/main_gpui.rs`
- any store subscription / snapshot delivery glue required for popup reopen semantics

### Implementation Requirements

- add or extend tests for popup close/reopen around selection/transcript updates
- prefer deterministic remount and snapshot-subscription tests over fragile real-window automation
- first inspect existing GPUI-oriented test files (for example `tests/gpui_integration_tests.rs`, `tests/gpui_wiring_command_routing_tests.rs`, and `tests/ui_gpui/`) for reusable harness patterns before adding new seams
- if no existing harness can mount/unmount the relevant views deterministically, add one tightly bounded fallback seam that can construct `MainPanel`/child views against a shared store handle without opening a real popup window; this seam is allowed only to prove remount/render behavior and store-handle identity, never runtime ingress independence by itself
- this phase must still include at least one deterministic behavior proof that popup-absent mutation occurred through the production `spawn_runtime_bridge_pump(...)` path while popup subscriber count/object identity for the mounted popup path was zero/dropped
- treat popup reopen as unsubscribe/remount/resubscribe against the same authoritative store handle in tests
- require one explicit anti-mirror proof artifact in this phase with this exact production-path recipe:
  1. create runtime with authoritative store and production `spawn_runtime_bridge_pump(...)` already running,
  2. mount popup and record the store handle identity/subscription object identities,
  3. fully unmount popup and name the exact popup-local subscription/proxy objects intentionally dropped,
  4. prove popup absence with both of these mandatory observables together: one deterministic popup-absence witness from subscription plumbing (for example zero subscriber count, explicit invalidation of all popup subscription tokens, or equivalent readback from subscription state), and teardown assertion on the exact popup-local subscription/proxy objects intentionally dropped,
  5. assert the production ingress used during popup absence is still `spawn_runtime_bridge_pump(...)`, not direct store mutation and not popup-retained proxying,
  6. trigger selection/transcript change through the production selection/event/presenter path, not by directly mutating the store,
  7. assert store revision/snapshot changed while both popup-absence observables above prove no popup-local subscriber/proxy remained authoritative,
  8. remount popup against the same store handle identity,
  9. use one repo-acceptable same-store identity witness and name it explicitly in evidence: stable store instance id exposed by the test seam, `Arc::ptr_eq` on the authoritative store handle, or an equivalent repo-idiomatic allocation-identity witness; subscription/proxy identity alone is insufficient,
 10. assert first remount render reads `current_snapshot()` and shows the updated transcript without replay/bootstrap consumption,
 11. include a negative-control expectation showing the same harness would fail if runtime ingress were moved back under popup lifetime or if a popup-local proxy remained the semantic state path
- ensure reopening popup renders latest selected conversation transcript snapshot
- ensure closed popup does not block state mutation into the authoritative store
- verify reopened popup state by observable snapshot/render assertions, not by timing-dependent queue drain assumptions
- by the end of this phase, bridge polling may still exist as transport glue, but the evidence must show popup reopen correctness comes from store snapshot subscription rather than a replay queue draining at just the right time
- by the end of this phase, any mounted-popup code path that invokes `GpuiBridge::drain_commands()`, schedules a production drain task, or retains a drainer/proxy object capable of initiating production reduction is an explicit FAIL even if behavioral tests still appear to pass
- tests in this phase must not satisfy popup independence via a popup-local mirror/proxy that survives unmount
- if any runtime helper or proxy object survives popup unmount, evidence must prove it is transport-only and incapable of holding independent selected transcript, selected title, selection generation, load/error state, or other semantic chat-state authority outside `src/ui_gpui/app_store.rs`
- anti-mirror proof must still use the production runtime ingress and must fail if that surviving object becomes a hidden second semantic owner

### Required Code Markers

Every created or materially updated production item or test/helper in this phase must include markers matching project conventions:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P07
/// @requirement[: ]REQ-ARCH-004.3
/// @pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:037-144
```

Use exact requirement IDs per item, including `REQ-ARCH-004.1`, `REQ-ARCH-004.2`, `REQ-ARCH-004.3`, and `REQ-INT-001.3` where relevant.

## Pseudocode References

- `analysis/pseudocode/03-main-panel-integration.md` lines 037-144

## Verification Commands

```bash
cargo check -q
cargo test --test chat_view_conversation_switch_regression_tests --test chat_startup_scrollback_layout_regression_tests --test gpui_integration_tests --test gpui_wiring_event_flow_tests --test gpui_wiring_command_routing_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P07" src tests --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-004.1\|@requirement[: ]REQ-ARCH-004.2\|@requirement[: ]REQ-ARCH-004.3\|@requirement[: ]REQ-INT-001.3" src tests --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src tests --include="*.rs"
grep -rn "popup_window\|open_popup\|close_popup\|current_snapshot\|subscribe\|revision\|drain_commands\|SelectConversation\|cx\\.spawn\|AppState" src/main_gpui.rs src/ui_gpui/views/main_panel.rs src/ui_gpui tests --include="*.rs"
```

## Reachability / Integration Verification

- [ ] state can change while popup is absent
- [ ] reopened popup renders latest store snapshot
- [ ] production-path runtime ingress remains live while popup is absent
- [ ] no special replay dependency remains for reopen correctness

## Semantic Verification Checklist

- [ ] popup visibility is no longer a correctness boundary for transcript delivery
- [ ] manual selection remains correct across reopen
- [ ] deterministic remount/subscription tests prove reopen correctness without fragile real-window automation
- [ ] explicit anti-mirror proof shows production-path ingress reaches the same reducer boundary with no mounted popup and no retained popup-local subscription object
- [ ] explicit anti-mirror proof names dropped popup-local objects, surviving store handle identity, and exact runtime ingress function used during popup absence
- [ ] startup/manual convergence remains intact

## Success Criteria

- Popup lifecycle no longer determines whether selected transcript is correct
