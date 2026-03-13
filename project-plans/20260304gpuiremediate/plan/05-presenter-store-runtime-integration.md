# Phase 05: Integrate Presenter and Runtime Updates Through the Store

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P05`

## Prerequisites

- Required: Phase 04a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P04a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P04a.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-002: Startup/Runtime Convergence

**Full Text**: Startup hydration MUST use the same authoritative reducer semantics and state contract used for ordinary runtime presenter updates, even though startup may use a dedicated startup transaction instead of the ordinary runtime bridge/event transport choreography.

**Behavior**:
- GIVEN: presenter-originated `ViewCommand` traffic during normal runtime
- WHEN: runtime integration is implemented
- THEN: those commands mutate the authoritative store rather than depending on popup-local replay correctness

**Why This Matters**: This phase turns the store into the real runtime owner instead of a passive side structure.

### REQ-ARCH-003: Explicit Conversation Loading Protocol

**Full Text**: Selection should be an explicit loading protocol/state, not “clear and hope replay arrives”.

**Behavior**:
- GIVEN: `ConversationActivated` followed by `ConversationMessagesLoaded`
- WHEN: runtime updates are reduced through the store
- THEN: activation enters loading, transcript replacement is generation-aware, and no ordinary clear occurs

**Why This Matters**: This is the semantic fix for manual selection correctness.

## Implementation Tasks

### Files to Modify

- `src/presentation/chat_presenter.rs`
- `src/presentation/history_presenter.rs`
- `src/presentation/view_command.rs`
- `src/events/types.rs`
- `src/ui_gpui/bridge/user_event_forwarder.rs`
- `src/main_gpui.rs`
- `src/ui_gpui/app_store.rs`
- `src/ui_gpui/views/main_panel.rs`
- `src/ui_gpui/views/chat_view.rs`
- `src/ui_gpui/views/history_view.rs`
- deterministic proof coverage should land primarily in `tests/gpui_wiring_command_routing_tests.rs` and `tests/presenter_selection_and_settings_tests.rs`; if additional proof files are required, record their exact paths in phase evidence
- any additional directly affected match/forwarding/test-support files touched by the `ViewCommand` / `UserEvent` protocol migration

### Event Wiring Contract

This phase must make the existing repo event flow concrete rather than inventing a new parallel dispatcher.

Required runtime wiring:

1. `ChatView` and `HistoryView` continue originating selection intent, but production selection must route into a GPUI-owned handler rooted in `src/main_gpui.rs`; this plan standardizes that seam as `handle_select_conversation_intent(...)`.
2. `handle_select_conversation_intent(...)` must call `begin_selection(conversation_id)` before async transcript loading begins.
3. `begin_selection(conversation_id)` must synchronously decide no-op vs retry/new selection, mint the next `selection_generation` when required, update authoritative selected-conversation/loading state, and publish exactly once when authoritative state changed.
4. When `begin_selection(...)` returns a minted token, `handle_select_conversation_intent(...)` must forward it through the existing user-event transport by emitting the enriched event shape `UserEvent::SelectConversation { id, selection_generation }`.
5. `src/ui_gpui/bridge/user_event_forwarder.rs` remains transport only; it forwards the enriched event onto the tokio-side `EventBus` path without mutating freshness state.
6. `ChatPresenter::handle_select_conversation(...)` (or its explicit delegate) must consume the enriched event and reuse the provided token rather than minting one independently.
7. `HistoryView` and `ChatView` selection must use this same path; no second selection entry point is allowed.
8. Title fallback at mint time is owned by the GPUI/store boundary, not by presenter land.

Evidence for this phase must name the exact file/function that owns steps 1-4.
Evidence for this phase must also include one named deterministic negative-control proof that would fail if any production path could still dispatch selection async load work without first passing through `handle_select_conversation_intent(...)` and `begin_selection(...)`, including same-id reselection while `Loading`/`Ready`.

### Runtime Ingress Ownership Contract

This phase must replace popup-owned draining with one concrete production ingress seam.

Required runtime ingress model:

- add `spawn_runtime_bridge_pump(...)` in `src/main_gpui.rs`
- launch it from the `Application::new().run(|cx: &mut App| { ... })` closure using `cx.spawn(...)`
- implement it as the bounded periodic GPUI runtime task class fixed by the store contract
- use one fixed GPUI background-executor timer loop at the current repo polling cadence (`std::time::Duration::from_millis(16)` unless Phase 00a explicitly approves an equivalent fixed bounded cadence)
- make it the sole production callsite for `GpuiBridge::drain_commands()` once Phase 05 completes
- keep it alive with zero mounted popups/windows
- enforce one-iteration/one-batch discipline: each eligible tick performs at most one iteration, that iteration calls `GpuiBridge::drain_commands()` exactly once, passes the resulting batch to one reducer entrypoint, and that reducer entrypoint may publish at most once for that batch
- do not add a second inner drain-until-empty loop around `drain_commands()`; commands arriving after the drain begins are handled by a later tick
- enforce single-flight runtime ingress: if a pump tick occurs while a prior iteration is still inside `drain_commands()`, reducer execution, revision bump, or publication completion, it must be skipped or coalesced rather than reentered
- enforce the same guard against nested production ingress reentry from publication/subscriber side effects while the guard is held; publication, redraw, or subscriber callbacks must not recursively trigger another production drain/reduce path during an active iteration
- treat any overlap between two iterations in any portion of `drain_commands()` -> reducer execution -> revision bump -> publication completion as a phase failure
- the task exits only when the runtime-owned bridge/store context is shutting down or no longer valid; popup-owned code must not respawn it
- forbid popup-created timer tasks, popup-retained proxy objects, dual production drainers, or direct child-view forwarding as the production authority path

### Implementation Requirements

- route presenter/runtime command delivery into authoritative store mutation
- preserve existing presenter command semantics where possible
- this is the highest-risk implementation phase in the plan because it combines protocol migration, reducer semantics, presenter wiring, and test-helper conversion; complete it in a deliberately staged order rather than editing opportunistically
- before modifying any protocol-bearing file, rerun the Phase 00a variant/match inventory (`grep -rn -l "ConversationActivated\|ConversationMessagesLoaded\|SelectConversation" src tests --include="*.rs"`) and use every hit as a checklist for the migration
- perform the migration in this safe order:
  1. extend `ViewCommand` payload shapes, extend `UserEvent::SelectConversation` to carry `selection_generation`, and add `ConversationLoadFailed`
  2. update every constructor/match site in source and tests until the repo compiles under the new serialized contract while existing delivery paths still function
  3. introduce `spawn_runtime_bridge_pump(...)` in `src/main_gpui.rs` and make it the only production `GpuiBridge::drain_commands()` owner
  4. introduce `handle_select_conversation_intent(...)` plus `begin_selection(conversation_id)` and update `ChatView` / `HistoryView` selection to use that path instead of directly sending a raw selection id to tokio
  5. prove that `begin_selection(conversation_id)` is the single ordinary-runtime minting site and record its exact file/function in evidence
  6. convert Phase 03 helpers such as `SelectionTracker` / `assert_load_failure_protocol(...)` into real assertions over the new fields/variant
  7. route bridge-drained runtime command batches into the store reducer on the GPUI side
  8. switch `ChatView` / `HistoryView` render authority to store snapshots for selection/loading semantics
  9. after Phase 05, mounted views may keep only ephemeral render cache / UI state; they must not own durable semantic transcript/loading state independent of authoritative snapshot state
  10. any local transcript collection that remains during migration must be overwritten from authoritative snapshot on relevant revision changes and on bounded-clear restoration
  11. after Phase 05, presenter-originated transcript-durable commands (`ConversationMessagesLoaded`, `ConversationLoadFailed`, `MessageAppended`, and `FinalizeStream`) must not mutate `ChatView` transcript authority directly; mounted transcript rendering must derive from authoritative snapshot application only, except for the bounded local clear/readback tolerance defined for `ConversationCleared`

- treat bridge draining as transport only: tokio presenters still emit commands, but only the GPUI/store side may reduce and publish snapshots
- implement reducer semantics for explicit loading state transitions for conversation selection
- implement a concrete GPUI-owned `selection_generation` minting/hand-off API at the store/service boundary; presenter-side async transcript loads must receive the minted token from that boundary rather than incrementing an independent presenter-local counter
- because current repo `GpuiBridge::emit(...)` may fail synchronously on full/disconnected channel, the selection handoff path must explicitly handle that failure: if emitting enriched `UserEvent::SelectConversation { id, selection_generation }` fails, immediately reduce `ConversationLoadFailed { conversation_id: id, selection_generation, message }` for that same generation rather than leaving `Loading` stuck
- evidence for this phase must identify exactly one ordinary-runtime minting implementation site for `selection_generation`; if more than one minting site exists, the phase fails
- same-id reselection semantics are fixed by this plan and must not be improvised:
  - same id while `Loading` or `Ready` -> strict no-op: no new generation, no async load dispatch, no publication, no ephemera clear, and no selected-title rewrite
  - same id while `Error` -> retry by minting a new generation and re-entering `Loading`
- carry a concrete freshness token through the loading protocol using `selection_generation`
- ensure ignored/unhandled commands do not bump the store revision or publish a new snapshot
- require a named behavior proof in this phase showing that ignored/no-op/stale runtime commands do not bump revision or publish
- update `src/presentation/view_command.rs` as a mandatory protocol migration, not an optional cleanup
- update `src/events/types.rs` as a mandatory user-event migration so selection transport carries the minted token
- add `selection_generation` to `ConversationActivated`
- add `selection_generation` to `ConversationMessagesLoaded`
- add `ConversationLoadFailed { conversation_id, selection_generation, message }`
- identify the transcript-load failure path in `ChatPresenter::handle_select_conversation` or its delegate and emit `ConversationLoadFailed` there with a meaningful error message; if no explicit failure path exists yet, add one for conversation-not-found and storage/read failure cases
- update all affected constructors, match arms, bridge forwarding sites, reducer entry points, and test fixtures that consume these variants across the relevant files
- specifically convert Phase 03 test-local helpers such as `SelectionTracker` and `assert_load_failure_protocol(...)` from intentional-failure scaffolding into real generation-aware assertions once the new protocol fields/variants exist
- treat this as a repo-wide serialized contract change because `ViewCommand` derives `Serialize` and `Deserialize`; implementation is not complete until the renamed payload shapes are consistent everywhere they cross layer boundaries
- preserve currently active GPUI streaming/thinking behavior for `ShowThinking`, `HideThinking`, `AppendThinking`, `AppendStream`, `FinalizeStream`, `StreamCancelled`, and `StreamError`, including the current active-conversation / `Uuid::nil()` targeting semantics already used by `ChatPresenter` and `ChatView`
- `FinalizeStream` handling in this phase must use the direct-finalize durable model from the store contract:
  - accepted `FinalizeStream` itself appends exactly one durable assistant transcript payload in the same authoritative replay-compatible shape already used by `ConversationMessagesLoaded`: `ConversationMessagePayload { role: MessageRole::Assistant, content, thinking_content, timestamp }`,
  - `content` comes from the accepted current stream buffer,
  - `thinking_content` comes from buffered thinking content if non-empty,
  - `timestamp` remains `None` unless an already-existing repo source provides a concrete finalize-time timestamp,
  - mounted rendering may continue to derive `ChatMessage::assistant(..., current_model)` from that payload using the same replay mapping style already present in `ChatView::messages_from_payload(...)`,
  - clears `streaming.active_target` plus stream/thinking buffers in the same accepted reducer mutation/publication,
  - resolves `Uuid::nil()` before acceptance,
  - accepts only when resolved target equals both the currently selected conversation and the active stream lifecycle target and the stream buffer is non-empty before acceptance,
  - rejects stale/off-target/empty-buffer finalize as a no-op,
- duplicate prevention must prove that replayed or duplicate finalize for an already-cleared stream lifecycle is a no-op,
- reducer-side dedupe guard support for streamed assistant durable finalization is mandatory in the final design,
- the mandatory guard is one store-owned just-finalized stream record conceptually equivalent to `last_finalized_stream_guard { conversation_id, transcript_len_after_finalize }`,
- accepted finalize must set that guard in the same accepted mutation that appends the assistant payload and clears stream/thinking ephemera,
- a later assistant-side `MessageAppended` targeting the same conversation must be ignored as a no-op only when all exact duplicate-predicate conditions hold together: same conversation id, transcript length still equals `transcript_len_after_finalize`, current transcript tail entry exists, current transcript tail entry role is assistant, and current transcript tail entry content exactly equals incoming `MessageAppended.content`,

- matching bulk replacement or a new selection must clear that just-finalized guard,
- repo-wide callsite inventory plus a named deterministic streamed-interaction test are still required evidence, but they validate the mandatory dedupe guard coverage rather than deciding whether duplication handling exists at all,
- leaving duplication safety to proof-only without that reducer-side dedupe guard is a phase failure,

  - must not require a second durable append command for streamed assistant output
- `MessageAppended` remains the durable append path for user messages and existing non-stream append cases, but it must not duplicate accepted streamed assistant finalization
- do not invent new GPUI tool-call semantics for `ShowToolCall` / `UpdateToolCall` in this phase unless that scope addition is made explicit and tested
- keep `ConversationCleared` on the existing non-store path during this plan; do not silently expand the reducer scope to own clear-conversation behavior in Phase 05
- while `ConversationCleared` remains on the old path, evidence for this phase must state and prove the post-clear invariant explicitly: mounted local transcript may clear transiently, selected title/id remain authoritative, local stream/thinking buffers reset, and `src/ui_gpui/views/chat_view.rs::ChatView::handle_command(ViewCommand::ConversationCleared)` has deterministic same-turn read access to authoritative snapshot state so it can restore all store-backed render fields before control returns to the event loop rather than waiting for an unrelated future publication; equivalent repo-idiomatic same-turn repair mechanisms are acceptable if they preserve that invariant and do not introduce a second durable mirror
- forbidden near-miss implementations for bounded clear are part of this phase contract: subscriber-driven repair, `cx.spawn(...)`/task/timer/frame callback repair, remount-only repair, or introducing a second popup-local transcript authority to survive the clear
- ensure `ConversationActivated` does not clear transcript on ordinary activation
- ensure `ConversationMessagesLoaded` remains bulk replacement
- reject stale or off-target transcript payloads and stale failure payloads
- make ignored/unhandled command behavior explicit: commands outside this recovery reducer's scope must leave store state unchanged
- this phase owns presenter/runtime wiring into the reducer plus the explicit success/failure generation protocol; do not defer those semantics back into a Phase 04 shell

### Required Code Markers

Every created or materially updated production item in this phase must include markers matching project conventions:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P05
/// @requirement[: ]REQ-ARCH-003.6
/// @pseudocode[: ]analysis/pseudocode/01-app-store.md:196-329
```

Use exact requirement IDs per item, including `REQ-ARCH-003.2`, `REQ-ARCH-003.3`, `REQ-ARCH-003.4`, `REQ-ARCH-003.6`, `REQ-ARCH-006.6`, and `REQ-ARCH-006.7` where relevant.

## Pseudocode References

- `analysis/pseudocode/01-app-store.md` lines 196-329
- `analysis/pseudocode/02-selection-loading-protocol.md` lines 001-087
- `analysis/pseudocode/03-main-panel-integration.md` lines 014-136


## Verification Commands

```bash
cargo check --all-targets -q
cargo test --test presenter_selection_and_settings_tests --test chat_view_conversation_switch_regression_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test gpui_wiring_command_routing_tests --test gpui_wiring_event_flow_tests --test gpui_integration_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P05" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-003.2\|@requirement[: ]REQ-ARCH-003.3\|@requirement[: ]REQ-ARCH-003.4\|@requirement[: ]REQ-ARCH-003.6" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/01-app-store.md:\|@pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "ConversationLoadFailed\|selection_generation\|SelectConversation\|drain_commands\|try_send\|current_snapshot\|revision" src/ui_gpui src/presentation src/main_gpui.rs src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -R -n "ShowThinking\|HideThinking\|AppendThinking\|AppendStream\|FinalizeStream\|StreamCancelled\|StreamError\|ShowToolCall\|UpdateToolCall" src/ui_gpui src/presentation --include="*.rs"
grep -rn "todo!\|unimplemented!" src/main_gpui.rs src/ui_gpui src/presentation src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/main_gpui.rs src/ui_gpui src/presentation src/events/types.rs src/ui_gpui/bridge/user_event_forwarder.rs --include="*.rs"
```

## Reachability / Integration Verification

- [ ] Presenter-emitted commands can be traced into store reduction
- [ ] `spawn_runtime_bridge_pump(...)` is the sole production `GpuiBridge::drain_commands()` owner
- [ ] production-path runtime command ingress remains live with no mounted `MainPanel`
- [ ] ChatView snapshot now reflects store loading/ready/error state transitions
- [ ] HistoryView selected-item/highlight state is now store-authoritative alongside ChatView selection/loading state
- [ ] Manual conversation selection no longer depends on transient popup ordering for transcript correctness
- [ ] Protocol migration is complete across all affected `ViewCommand` and `UserEvent` construction and match sites

## Semantic Verification Checklist

- [ ] `begin_selection(...)` is the only ordinary-runtime authority transition that enters `Loading`; matching `ConversationActivated` is only idempotent echo or bounded title-provenance upgrade, without ordinary transcript clear
- [ ] Transcript replacement occurs only for current selection/generation
- [ ] `selection_generation` issuance is concretely GPUI/store-owned rather than presenter-local or popup-local
- [ ] exactly one ordinary-runtime minting implementation site exists and is named in evidence
- [ ] one concrete runtime ingress owner is named in evidence and no popup-created or popup-retained equivalent is counted as compliant
- [ ] same-id reselection behavior matches the fixed no-op/retry rules in the store contract
- [ ] `ConversationLoadFailed` is explicit, generation-aware, and non-ambiguous
- [ ] `FinalizeStream` direct-finalize durable transcript materialization is explicitly preserved
- [ ] bounded `ConversationCleared` behavior is proven rather than assumed
- [ ] Convergence tests now pass or reduce to startup-hydration-specific gaps only

## Success Criteria

- Manual selection path is store-driven rather than popup-forwarding-driven
- Placeholder detection is clean
- Tests prove semantic progress on the original failure mode
