# Phase 06: Migrate Startup Hydration onto the Same State Path

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P06`

## Prerequisites

- Required: Phase 05a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P05a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P05a.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-002: Startup/Runtime Convergence

**Full Text**: Startup hydration MUST use the same authoritative state semantics used for ordinary runtime interactions.

**Behavior**:
- GIVEN: app startup before popup mount
- WHEN: startup state is hydrated
- THEN: the authoritative store receives equivalent selected-conversation semantics through the same reducer contract used at runtime, preserving first-frame correctness

**Why This Matters**: This phase removes the architectural split that made startup and runtime non-equivalent.

### REQ-ARCH-006: Behavior Preservation

**Full Text**: Preserve startup first-frame correctness while converging the architecture.

**Behavior**:
- GIVEN: existing startup transcript correctness
- WHEN: startup migrates onto the authoritative store contract
- THEN: first-frame transcript remains correct without regressing selection/transcript/layout behavior

**Why This Matters**: Recovery is invalid if startup gets worse.

## Implementation Tasks

### Files to Modify

- `src/main_gpui.rs`
- startup hydration/store bootstrap module(s)
- `src/ui_gpui/app_store.rs`
- `src/ui_gpui/views/main_panel.rs`
- any directly required view/store glue files

### Implementation Requirements

- move startup bootstrap onto authoritative store hydration/reduction path
- preserve first-frame chat snapshot correctness
- maintain selected conversation/profile/history snapshots needed at startup
- startup hydration must use one canonical semantic form: a single startup transaction entrypoint in `src/ui_gpui/app_store.rs`; this plan standardizes that entrypoint as `reduce_startup_batch(startup_inputs)`
- no dedicated startup-only semantic mutator is allowed beyond that one entrypoint; helper wrappers are allowed only if they assemble startup inputs and invoke `reduce_startup_batch(startup_inputs)`
- startup-specific helpers must not directly mutate selected-conversation id/title/generation/load/transcript fields before, after, or instead of that shared authoritative entrypoint
- no third startup mutation form is allowed; in particular, compatibility glue must not become a second semantic mutator outside the reducer module
- `reduce_startup_batch(startup_inputs)` must itself reuse the same ordinary-runtime semantics by calling `begin_selection(conversation_id, BatchNoPublish)` when startup has a selected conversation and then using only the matching success/failure payload as the authoritative startup completion step in that same transaction
- if startup still synthesizes `ConversationActivated` / `ConversationMessagesLoaded`, those commands are compatibility-only/readback-only material and must not define authoritative startup state; authoritative startup semantics remain `begin_selection(..., BatchNoPublish)` plus only the matching success/failure completion inside `reduce_startup_batch(startup_inputs)`
- startup mode for this phase is explicit and must be carried in `startup_inputs` rather than inferred from a bare pending union:
  - **Mode A: known selected conversation and synchronously known transcript outcome**
    - `startup_inputs.startup_mode` must be `ModeA { transcript_result: Success(messages) | Failure(message) }`
    - call `reduce_startup_batch(startup_inputs)` once
    - have that transaction invoke `begin_selection(conversation_id, BatchNoPublish)` and the matching transcript success/failure reduction
    - bump revision at most once for the whole startup transaction
    - publish at most once for the whole startup transaction
    - publish only the final visible `Ready` or `Error` state for generation `1`
  - **Mode B: known selected conversation but transcript outcome genuinely unavailable at startup**
    - `startup_inputs.startup_mode` must be `ModeB { transcript_unavailable_reason, pending_generation: 1 }`
    - call `reduce_startup_batch(startup_inputs)` once
    - have that transaction invoke `begin_selection(conversation_id, BatchNoPublish)` exactly once and commit explicit `Loading` for generation `1`
    - evidence must state exactly which repo-grounded seam class made transcript outcome unavailable from the current startup seam: `StartupServiceSeamUnavailable`, `AsyncOnlySourceBeforeMount`, or `StartupCompositionDoesNotProvideTranscriptOutcome`
    - evidence must tie that seam class to exact source file/function origin
    - evidence must explicitly rule out the other two seam classes with exact source file/function grounding rather than prose preference

    - later success/failure for generation `1` must arrive through the ordinary runtime pump/reducer path
  - a bare `Pending` startup transcript result is non-compliant because it does not prove why Mode B was required instead of Mode A
- the canonical proof seam for startup atomicity in this phase is one mandatory combined harness artifact: a subscriber-visible snapshot/revision observer plus first-subscriber `current_snapshot()` readback proving no intermediate `Loading`/default snapshot became visible
- batch startup hydration into one coherent reduction/publication step before first popup render subscription consumes the initial snapshot
- do not publish an intermediate empty/loading snapshot when startup already has the selected transcript data available
- make startup atomicity explicit in code shape: all startup reductions complete before the first publication/subscription handoff
- ensure no subscriber can observe an intermediate `Loading` snapshot for transcript data already known during startup hydration
- if compatibility glue still materializes startup `ConversationActivated` for local readback/evidence, it must remain non-authoritative; only the final success/failure completion state may be publishable from the startup transaction, and the batch must not publish per-command intermediate states
- keep any temporary compatibility shim explicitly bounded and removable
- if compatibility shim remains, it must populate the authoritative store first and must not become a second semantic state owner

### Required Code Markers

Every created or materially updated production item in this phase must include markers matching project conventions:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P06
/// @requirement[: ]REQ-ARCH-002.5
/// @pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:001-013
```

Use exact requirement IDs per item, including `REQ-ARCH-002.1`, `REQ-ARCH-002.2`, `REQ-ARCH-002.5`, and `REQ-ARCH-006.3` where relevant.

## Pseudocode References

- `analysis/pseudocode/01-app-store.md` lines 133-195, 196-270
- `analysis/pseudocode/03-main-panel-integration.md` lines 001-013, 045-049

## Verification Commands

```bash
cargo check -q
cargo test --test chat_startup_scrollback_layout_regression_tests --test presenter_selection_and_settings_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P06" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-002.1\|@requirement[: ]REQ-ARCH-002.2\|@requirement[: ]REQ-ARCH-002.5\|@requirement[: ]REQ-ARCH-006.3" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/01-app-store.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -rn "startup_commands\|apply_startup_commands" src/main_gpui.rs src/ui_gpui/views/main_panel.rs
```

## Reachability / Integration Verification

- [ ] Startup hydration mutates store before popup rendering
- [ ] Popup first render uses current snapshot, not a separate view-only replay path
- [ ] Any remaining bootstrap shim is documented as temporary and redundant

## Semantic Verification Checklist

- [ ] Startup transcript still renders correctly on first frame
- [ ] Startup and manual selection now share the same authoritative reducer semantics/state contract
- [ ] No empty/loading flash is introduced for the already-known startup transcript
- [ ] No subscriber can observe partially reduced startup hydration for already-known transcript data
- [ ] Mode B evidence is explicit, repo-grounded, and bounded
- [ ] No regressions in startup scrollback/layout behavior

## Success Criteria

- Startup correctness is preserved through the same authoritative state contract used at runtime
