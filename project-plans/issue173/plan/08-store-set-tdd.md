# Phase 08: Store active_streaming_targets Set TDD

Plan ID: `PLAN-20260416-ISSUE173.P08`

## Prerequisites

- P07 verified PASS.

## Requirements implemented (tests only)

- REQ-173-004.1, REQ-173-004.2

## Tasks

Add unit tests to the app_store test suite (find the existing module — most
store tests live in `src/ui_gpui/app_store/` and `src/ui_gpui/app_store_streaming/tests.rs`). Add a new `src/ui_gpui/app_store_streaming/tests_concurrent.rs`
module and wire it into the existing `tests` mod.

Required tests, each with P08 plan markers and appropriate REQ-173-004.x tags:

1. `multiple_targets_can_be_tracked_concurrently`:
   - Build an `AppStoreInner` via whatever existing test constructor is used.
   - Call the reducer for stream-start on conversations A and B (use the same
     public/crate-private reducer entry points the current tests use).
   - Assert `inner.active_streaming_targets.contains(&A)` and
     `.contains(&B)`.

2. `finalize_removes_only_targeted_conversation_from_set`:
   - With both A and B in the set, finalize the stream for A.
   - Assert A not in the set, B still in the set.

3. `streaming_state_snapshot_active_flag_reflects_set_membership`:
   - Selected conversation = A. Set contains A and B.
   - `project_streaming_snapshot(...)` with `selected = A` sets
     `active_target = Some(A)`.
   - With `selected = C` (not in set), `active_target = None`.

NOTE: the existing signature `project_streaming_snapshot(..., active_streaming_target: Option<Uuid>)` must change to accept a set.
Option A: change the signature to accept `&HashSet<Uuid>`. Option B: add a new
projector. We prefer A; call-sites update in P09.

The test WILL fail to compile until P09. That is the desired fail state.

## Verification

- `cargo build --all-targets 2>&1 | tail -10`
- `grep -c "@plan PLAN-20260416-ISSUE173.P08" src/ui_gpui/`

Deliverable: `project-plans/issue173/plan/.completed/P08.md`.
