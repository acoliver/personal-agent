# Feature Specification: GPUI Chat State Delivery Recovery Architecture

Plan ID: PLAN-20260304-GPUIREMEDIATE
Created: 2026-03-04
Total Phase Pairs: 13
Execution Steps: 26 (`P00 -> P00a -> ... -> P12 -> P12a`)
Requirements: REQ-ARCH-001, REQ-ARCH-002, REQ-ARCH-003, REQ-ARCH-004, REQ-ARCH-005, REQ-ARCH-006, REQ-INT-001, REQ-INT-002

---

## Purpose

Stabilize GPUI chat state delivery by replacing popup-coupled command replay with one authoritative durable app state/store that drives both startup rendering and ordinary runtime updates.

This is a planning-only recovery architecture. The diagnosed failure is not missing data. The failing seam is integration: startup succeeds because `src/main_gpui.rs` builds synchronous bootstrap commands and `src/ui_gpui/views/main_panel.rs` applies `startup_commands`, while manual conversation selection depends on a separate async presenter-to-bridge-to-popup delivery path that does not reliably deliver `ConversationMessagesLoaded` back into the mounted `ChatView` after startup.

The plan must converge those paths so startup and ordinary runtime interactions use the same state flow, preserve first-frame correctness, and make popup lifetime irrelevant to whether state delivery works.

---

## Architecture Diagnosis

### Confirmed Current Behavior

1. `src/main_gpui.rs` eagerly builds startup commands through `build_startup_view_commands(...)`.
2. `src/ui_gpui/views/main_panel.rs` reads `MainPanelAppState.startup_commands` and applies them via `apply_startup_commands(...)`.
3. `ChatView` and `HistoryView` currently emit raw `UserEvent::SelectConversation { id }` into `GpuiBridge`.
4. `src/ui_gpui/bridge/user_event_forwarder.rs` forwards those raw user events to the tokio-side `EventBus`.
5. `ChatPresenter` currently handles that raw selection event in `handle_select_conversation(...)`, emits `ConversationActivated`, and then emits `ConversationMessagesLoaded`.
6. `src/ui_gpui/views/main_panel.rs::ensure_bridge_polling(...)` currently owns the production `GpuiBridge::drain_commands()` loop via `cx.spawn(...)` plus `window_handle.update(...)` while a popup-mounted `MainPanel` exists.
7. `ChatView` ignores `ConversationMessagesLoaded` when `active_conversation_id` does not match at delivery time.
8. Current GPUI stream durability is popup-local: `FinalizeStream` in `ChatView` appends the assistant message from the ephemeral streaming buffer into visible transcript state.
9. `ConversationCleared` currently clears mounted local `ChatView` state and triggers `HistoryView` refresh behavior on the old path.

### Root Problem Statement

The GPUI runtime currently has dual, non-equivalent state delivery paths:

1. **Synchronous bootstrap path**  
   `build_startup_view_commands` -> `MainPanelAppState.startup_commands` -> `MainPanel::apply_startup_commands`

2. **Live async presenter path**  
   `tokio presenters -> mpsc -> flume -> GpuiBridge -> MainPanel popup poll loop`

Startup correctness depends on the first path. Manual conversation switching depends on the second path. Because these paths are not unified behind one authoritative store, the popup can render a transcript on first mount yet fail to receive equivalent transcript replacement later.

---

## Target Architecture

### Pattern

Authoritative store + intent-driven views + snapshot rendering.

### Required Data Flow

```text
User intent from GPUI view
  -> GPUI runtime selection handler records authoritative selection transition
  -> enriched selection request crosses existing user-event transport
  -> presenter/service work uses authoritative token/context
  -> authoritative app store mutation
  -> snapshot publication
  -> mounted popup views render latest snapshot
```

### Architectural Rules

1. There is one authoritative durable app store / app state for GPUI runtime state.
2. The concrete store module path is `src/ui_gpui/app_store.rs` unless preflight proves an existing equivalent module is a strictly better fit.
3. Popup views render snapshots of that state and emit intents only.
   - Mounted popup views may keep only ephemeral render cache / UI state; they must not own a durable semantic transcript model independent of the authoritative store snapshot.
   - Any local transcript collection that still exists during migration must be treated as transient render cache overwritten from store snapshot on relevant revision changes and on bounded-clear restoration.
   - No mounted view may accept presenter-originated transcript/state updates as an authority path after Phase 05.
   - After Phase 05, presenter-originated transcript-durable commands such as `ConversationMessagesLoaded`, `ConversationLoadFailed`, `MessageAppended`, and `FinalizeStream` must not mutate `ChatView` transcript authority directly; mounted transcript rendering must derive from authoritative snapshot application only, except for the bounded local clear/readback tolerance defined for `ConversationCleared`.
4. Popup open/close must not determine whether command or state delivery works.
5. Startup uses the same authoritative state flow as ordinary runtime interactions.
6. Conversation selection uses an explicit loading protocol/state, not "clear and hope replay arrives".
7. `MainPanel` becomes a thinner composition root, not the primary owner of chat-state delivery semantics.
8. Startup hydration must be batched into one coherent initial committed store batch so the first rendered frame stays correct and there is no transient loading/empty flash.
9. The `ViewCommand` contract change for selection freshness is mandatory across the repo, not an optional local tweak.
10. Ignored/no-op/stale inputs must not create extra visible publications or revisions.
11. `ConversationCleared` may remain a bounded transitional local behavior during migration, but it must not become a second durable transcript authority.
12. `FinalizeStream` uses the direct-finalize durable model in this recovery: accepted finalize writes the assistant transcript message into authoritative transcript state exactly once and clears related ephemeral stream/thinking buffers in the same accepted reducer mutation.

### Store Synchronization / Publication Strategy

- The authoritative store lives for the process lifetime inside GPUI runtime state and is created before popup mount.
- `src/main_gpui.rs` remains the concrete GPUI-runtime composition root that constructs the store, the bridge, and the presenter tasks.
- Tokio-side presenters continue to emit `ViewCommand` values over the existing channel/bridge boundary during migration; they do not mutate the store directly from tokio tasks.
- The GPUI side remains the only store-mutation boundary: after `GpuiBridge::drain_commands()` yields a batch, one GPUI-owned reducer call in `src/ui_gpui/app_store.rs` applies that batch.
- The sole production bridge drainer after Phase 05 is a runtime-owned GPUI task launched from the `Application::new().run(|cx: &mut App| { ... })` closure in `src/main_gpui.rs` using `cx.spawn(...)`. This plan standardizes that task/helper as `spawn_runtime_bridge_pump(...)`.
- The function name `spawn_runtime_bridge_pump(...)` is normative shorthand for this plan, not mandatory spelling. An implementation may use a repo-idiomatic equivalent name only if Phase 00a or Phase 05 evidence maps it one-to-one to this sole-production-drainer responsibility in `src/main_gpui.rs`.
- Because current repo evidence exposes `GpuiBridge::drain_commands()` as a non-blocking batch seam and already shows app-root `cx.spawn(...)` background task patterns in `src/main_gpui.rs`, the canonical ingress scheduling class for this plan is a runtime-owned **bounded periodic GPUI task**.
- The runtime-pump execution contract is fixed by this plan:
  - the cadence is one fixed GPUI background-executor timer loop matching the current repo polling shape (`std::time::Duration::from_millis(16)` unless Phase 00a explicitly approves an equivalent fixed cadence with the same bounded periodic behavior),
  - each eligible tick performs at most one pump iteration,
  - one pump iteration calls `GpuiBridge::drain_commands()` exactly once,
  - because `drain_commands()` already drains all currently queued commands into one vector, the pump must not add a second inner drain-until-empty loop around that call,
  - commands arriving after that `drain_commands()` call begins are handled by a later tick,
  - if a tick fires while a prior iteration is still inside `drain_commands()`, reducer execution, revision bump, or publication completion, that later tick is skipped and thereby coalesced; it must not start a second overlapping iteration,
  - the task exits when the app runtime is shutting down or when the runtime-owned bridge/store context required by the pump is no longer valid; it must not be respawned from popup-owned code.
- `spawn_runtime_bridge_pump(...)` is the only allowed production callsite for `GpuiBridge::drain_commands()` once popup independence is claimed.
- `spawn_runtime_bridge_pump(...)` must remain alive with zero mounted popups/windows and must not depend on `MainPanel`, `popup_window`, or child-view lifetimes.
- `spawn_runtime_bridge_pump(...)` may call only runtime-owned transport glue plus store-owned reduction/publication functions; it must not delegate authority through popup-owned callbacks, child-view forwarding adapters, or any helper that retains durable transcript/loading state outside the store.
- Any migration helper between the pump and reducer must be runtime-owned stateless transport glue, not a second authority boundary.
- Pump reduction/publication discipline is fixed by this plan: one `spawn_runtime_bridge_pump(...)` iteration drains one command batch, passes that batch to one reducer entrypoint, and that reducer entrypoint may publish at most once for the batch. Per-command publication inside a drained batch is forbidden unless the batch is intentionally split before reduction.
- Pump execution must be single-flight: a new pump iteration may not begin `drain_commands()`, reducer execution, revision bump, or snapshot publication while a prior iteration is still in any of those stages. If a tick fires while a prior iteration is active, it must be skipped or coalesced rather than reentered.
- Single-flight must be enforced by the explicit `runtime_ingress_in_flight` field on `src/main_gpui.rs::AppState`, and that field may be toggled only inside `cx.update_global::<AppState>(...)` closures. Executor scheduling alone is not accepted as proof of serialization.

- The same guard also forbids nested production ingress reentry from publication/subscriber side effects while the guard is held. Publication, redraw, or subscriber callbacks must not recursively trigger another production `drain_commands()` / reducer entry while a prior runtime-pump iteration is active.
- If two pump iterations can overlap in any portion of `drain_commands()` -> reducer execution -> revision bump -> publication completion, ingress is non-compliant.
- Forbidden production shapes after Phase 05:
  - popup-owned timer/task as sole drainer,
  - popup-retained proxy object as sole drainer,
  - dual reducer drainers mutating the same store,
  - direct child-view forwarding from a drain loop as an authority path.
- `MainPanel` and child views subscribe/read snapshots from the store; they do not become a second reducer boundary.
- Store state is mutated behind one synchronization boundary consistent with the current runtime model; implementation must not split write authority across popup-local state and the store.
- Publication limits in this recovery apply per reducer invocation / drained bridge batch, not per entire end-user selection lifecycle; one ordinary selection may legitimately publish `Loading` first and later publish `Ready` or `Error` in a separate reducer invocation.

- Snapshot publication is revision-based and change-sensitive: after a logically complete mutation batch that actually changes authoritative state, increment revision once and publish one snapshot to subscribers.
- Ignored/no-op/stale inputs do not bump revision and do not publish.
- Startup hydration is treated as one reduction batch, not as a sequence of separately rendered partial publications.
- The startup contract is fixed by this plan: startup correctness is satisfied by one committed startup batch before popup mount, not by requiring one subscriber-delivered startup event.
- The canonical production startup visibility model is fixed by this plan through two explicit startup modes.
- **Startup Mode A: selected conversation and transcript outcome are synchronously known before popup mount.** This is the preferred/canonical mode for this recovery whenever current repo seams can supply the outcome during startup assembly.
  - no popup subscriber may be created before the startup batch commits
  - required ordering is:
    1. construct the authoritative store,
    2. run `reduce_startup_batch(startup_inputs)` to completion,
    3. commit the resulting authoritative snapshot/revision,
    4. any publication at that moment is a silent no-op by contract because popup subscribers do not yet exist,
    5. only then allow popup subscription/mount to read `current_snapshot()`
  - subscriber-visible startup correctness is therefore proven by two facts together:
    - the committed startup batch already contains the final `Ready` or `Error` selected snapshot,
    - the first subscriber reads that committed snapshot immediately via `current_snapshot()` rather than depending on queued replay
  - mounting `MainPanel` against pre-hydration default state is forbidden, and publishing visible startup `Loading` for that already-known transcript outcome is forbidden
- **Startup Mode B: selected conversation is known, but transcript outcome is genuinely unavailable at startup.**
  - first visible startup `Loading` for generation `1` is allowed only in this bounded mode
- In Startup Mode B, `reduce_startup_batch(startup_inputs)` commits `Loading { generation: 1 }` into the authoritative store before popup mount, may perform at most one silent pre-subscription publication for that committed snapshot, and first-subscriber correctness is proven by immediate `current_snapshot()` readback of that already-committed state rather than by replaying a queued bootstrap event.

  - this is an explicit exception, not the default startup shape for the recovery
  - implementation/evidence must state exactly why transcript outcome was unavailable from the current repo startup seam rather than merely choosing a lazier transport order
  - later success/failure for generation `1` must arrive through the ordinary runtime pump/reducer path
- No subscriber may observe partially hydrated startup state because no popup subscriber exists until the startup batch has committed.

- Popup remounts subscribe to the latest already-published snapshot and do not require replay of transient command queues to become correct.

### Concrete Runtime Boundary

Observed current concrete seams that this plan intentionally refactors rather than duplicates:

- `src/main_gpui.rs::build_startup_view_commands(...)` currently synthesizes startup `ConversationActivated` / `ConversationMessagesLoaded` payloads.
- `src/ui_gpui/views/main_panel.rs::apply_startup_commands(...)` currently applies that startup bootstrap directly into mounted views.
- `src/ui_gpui/views/main_panel.rs::ensure_bridge_polling(...)` currently drains the bridge only while a popup-mounted `MainPanel` exists.
- `src/presentation/chat_presenter.rs::handle_select_conversation(...)` currently emits runtime `ConversationActivated` plus `ConversationMessagesLoaded` after receiving a raw selection event.

Target recovery boundary:

- startup data and runtime presenter commands both cross into one store reducer boundary before they become visible UI state
- `src/main_gpui.rs::spawn_runtime_bridge_pump(...)` becomes the sole production `GpuiBridge::drain_commands()` owner once Phase 05 lands
- `MainPanel` may temporarily proxy/schedule that ingress during migration before Phase 05 completes, but it must not remain the sole or semantically relevant drainer afterward
- popup independence is only proven when no mounted popup, no popup-local subscription object, and no popup-local proxy object are required for production-path presenter traffic to reach the same reducer boundary
- zero-subscriber runtime mutation is still required to reach the store and become visible on later popup reopen
- no tokio task, popup-local view, compatibility shim, or retained runtime helper may become a second long-lived owner of selected-conversation transcript state
- any runtime object that survives popup unmount while correctness continues must be proven transport-only: it may queue, forward, or schedule commands, but it may not hold independent selected transcript, selected title, selection generation, load/error state, or other semantic chat-state authority outside `src/ui_gpui/app_store.rs`

### Selection-Generation Ownership

The authoritative store owns the monotonic `selection_generation` sequence for selected-conversation freshness.

#### Canonical Ordinary-Runtime Sequence

1. `ChatView` / `HistoryView` no longer satisfy this recovery by directly calling `bridge.emit(UserEvent::SelectConversation { id })` for production selection.
2. Instead, they route selection intent into a GPUI-owned handler rooted in `src/main_gpui.rs`; this plan standardizes that seam as `handle_select_conversation_intent(...)`.
3. The function name `handle_select_conversation_intent(...)` is normative shorthand for this plan, not mandatory spelling. An implementation may use a repo-idiomatic equivalent name only if Phase 00a or Phase 05 evidence maps it one-to-one to this responsibility in `src/main_gpui.rs`.
4. For ordinary runtime production selection, `handle_select_conversation_intent(...)` is the only allowed dispatch entrypoint. No other production path may directly or indirectly call `begin_selection(...)`, emit enriched selection transport, or initiate selected-conversation async transcript load dispatch, including compatibility shims, remount hooks, history refresh callbacks, or retained transport helpers.
5. `handle_select_conversation_intent(...)` must call `app_store.begin_selection(conversation_id)` synchronously before any presenter async transcript load begins.
6. `begin_selection(conversation_id)` is the single ordinary-runtime minting site and must:
   - read current authoritative selected id/title/load state,
   - decide whether the selection is a no-op or a new generation,
   - if changed, mint the next generation token,
   - record selected id, selected title fallback, generation, and `Loading` state in authoritative store state,
   - clear only streaming/thinking ephemeral state,
   - perform the selection publication exactly once if authoritative selection/loading state changed.
7. `handle_select_conversation_intent(...)` must then forward the minted token across the existing user-event transport using an enriched selection event; this plan standardizes that event as `UserEvent::SelectConversation { id, selection_generation }`.
8. Current repo evidence shows `GpuiBridge::emit(...)` uses `try_send(...)` and may return `false` on full/disconnected channel, so selection handoff is not implicitly infallible.
9. If that enriched user-event emit fails synchronously, the GPUI/store boundary must immediately reduce `ConversationLoadFailed { conversation_id: id, selection_generation, message }` for that same generation rather than leaving the selection stuck in `Loading`.
10. `src/ui_gpui/bridge/user_event_forwarder.rs` remains transport only; it forwards the enriched event onto the existing `EventBus` path without minting or modifying freshness state.
11. `ChatPresenter::handle_select_conversation(...)` (or its explicit delegate) consumes the enriched event, calls service/storage work, and must reuse the provided token exactly as received.
12. The presenter may still emit `ConversationActivated`, but once `begin_selection(...)` has already established that id/generation in the store, matching activation is only an idempotent protocol echo and must not create a second logical transition or second publication.
13. Ordinary-runtime `ConversationActivated` is non-authoritative except for one bounded case: if id and generation already match authoritative store state but the current selected title provenance is `LiteralFallback("Untitled Conversation")`, and authoritative history data now provides a non-empty title for that same id, activation may correct the selected title to that history-backed value only. It must not mint a generation, replace transcript, or create a second loading transition.
14. `ConversationActivated` for a generation lower than the current authoritative generation is always ignored as stale.
15. `ConversationActivated` for a generation greater than the current authoritative generation during ordinary runtime is a protocol violation and must also be ignored as a no-op; higher-generation advancement is not a defensive recovery path because `begin_selection(...)` is the sole ordinary-runtime minting site.
16. The presenter then emits exactly one matching success or failure payload for that request lifecycle.
17. A later different selection may begin before an earlier load completes. Cancellation is optional, but stale success/failure payloads must be ignored on arrival.

#### Exact `begin_selection(...)` Semantics

- **Different conversation id** -> mint a new generation, enter `Loading`, publish once.
- **Same conversation id while `Loading` or `Ready`** -> strict no-op: no new generation, no publication, no async load dispatch, no streaming/thinking ephemera clear, and no selected-title rewrite.
- **Same conversation id while `Error`** -> mint a new generation and retry by re-entering `Loading`, publish once.
- Title fallback at mint time is owned by the GPUI/store boundary. It uses current authoritative history snapshot data if available; otherwise it uses the current repo fallback rule already visible in `src/main_gpui.rs` startup conversation-summary creation and `src/ui_gpui/views/chat_view.rs::sync_conversation_title_from_active()`: if the title is empty after trim, use the literal fallback string `"Untitled Conversation"`. The presenter is not a second title authority.
- Title provenance/strength is literal in this recovery, not helper-implied. The authoritative selected title must be treated as exactly one of: `HistoryBacked(non_empty_title)` or `LiteralFallback("Untitled Conversation")`.
- Title-source precedence is fixed for this recovery: `HistoryBacked(non_empty_title)` > `LiteralFallback("Untitled Conversation")`.
- Bounded `ConversationActivated` title correction may only upgrade `LiteralFallback("Untitled Conversation")` to `HistoryBacked(non_empty_title)` for the same selected conversation id when authoritative history snapshot data now provides that stronger non-empty title. This bounded upgrade must use the same exact reducer helper rule whether triggered by `ConversationListRefreshed` or by a matching idempotent `ConversationActivated` echo. It must not overwrite an existing `HistoryBacked(...)` title, and it must not invent a string-comparison-based “stronger title” rule or any second title source.
- Popup-local views never mint generations.
- Tokio tasks do not independently increment or authoritatively own the sequence.
- Store initialization starts at generation `0`.
- If startup has no selected conversation, generation remains `0` until the first ordinary runtime selection mints `1`.
- If startup already knows the selected conversation and transcript, startup must use one canonical startup transaction entrypoint owned by `src/ui_gpui/app_store.rs`; this plan standardizes that entrypoint as `reduce_startup_batch(startup_inputs)`.
- The function names `begin_selection(...)` and `reduce_startup_batch(startup_inputs)` are normative shorthand for this plan, not mandatory spelling. An implementation may use repo-idiomatic equivalent names only if evidence maps them one-to-one to the same sole minting site and sole startup transaction responsibilities.
- `reduce_startup_batch(startup_inputs)` must:
  - be the only public production startup transaction API for selected-conversation hydration in this recovery,
  - assemble startup history/profile/chat inputs,
  - require `startup_inputs` to carry an explicit startup mode discriminator rather than an unqualified pending/success/failure union,
  - call `begin_selection(conversation_id, BatchNoPublish)` when startup has a selected conversation,
  - use only the matching transcript success or failure payload as the authoritative startup completion step in that same transaction using the same reducer helper path as runtime commands,
  - treat any startup-synthesized `ConversationActivated` as compatibility-only/readback-only material rather than an authoritative startup state transition,
  - bump revision at most once for the whole startup transaction,
  - publish at most once for the whole startup transaction,
  - leave no other startup-specific mutation API free to write selected-conversation id/title/generation/load/transcript state directly.
- Structured `startup_inputs` are therefore the only normative production representation for startup-selected hydration in this recovery. Its startup-mode shape must be explicit:
  - `ModeA { transcript_result: Success(messages) | Failure(message) }`
  - `ModeB { transcript_unavailable_reason, pending_generation: 1 }`
- A bare `Pending` transcript result is non-compliant because it does not prove why Mode B was required instead of Mode A.
- Converting startup data into synthesized `ViewCommand` values is acceptable only as an internal implementation detail inside `reduce_startup_batch(startup_inputs)` or in tests/evidence; it is not a second equally normative startup entry form.
- `reduce_startup_batch(startup_inputs)` is defined for this recovery only against fresh startup store state before any popup subscriber exists; it is not a general-purpose re-entry path after runtime state already exists.
- If startup already knows the selected conversation and transcript, that one canonical startup transaction must call `begin_selection(conversation_id, BatchNoPublish)` exactly once, must mint generation `1`, and must finish in committed `Ready` state for generation `1` before popup mount.
- If startup knows the selected conversation but the initial transcript load fails, that same canonical startup transaction must call `begin_selection(conversation_id, BatchNoPublish)` exactly once, must mint generation `1`, and must finish in committed explicit `Error` state for generation `1` before popup mount.
- If startup uses Mode B, the handoff/proof evidence must name the exact repo-grounded seam class `transcript_unavailable_reason` that made synchronous startup transcript outcome unavailable.
- Acceptable `transcript_unavailable_reason` values in this recovery must map to one cited repo seam class, not ad hoc prose: `StartupServiceSeamUnavailable`, `AsyncOnlySourceBeforeMount`, or `StartupCompositionDoesNotProvideTranscriptOutcome`. Phase 06/12 evidence must tie the chosen value to exact source file/function origin.


### ViewCommand Protocol Migration

`src/presentation/view_command.rs` currently derives `Serialize` and `Deserialize`, so payload-shape changes are protocol changes across the repo.

This plan requires:

- `ConversationActivated { id, selection_generation }`
- `ConversationMessagesLoaded { conversation_id, selection_generation, messages }`
- `ConversationLoadFailed { conversation_id, selection_generation, message }`

Implementation must update all affected constructors, forwarding sites, reducers, match arms, bridges, and tests together. The migration is mandatory for convergence correctness.

### Hard Phase Cutover Matrix

- **End of P04**: the authoritative store exists, is process-lifetime, supports snapshot construction/subscription, and owns at least one real active render path; runtime command semantics may still arrive through old delivery plumbing, but popup-local forwarding/bootstrap paths remain explicitly transitional rather than target authority.
- **End of P05**: ordinary runtime presenter commands reduce into the authoritative store first through `spawn_runtime_bridge_pump(...)`; `begin_selection(...)` is the sole ordinary-runtime minting site; mounted popup forwarding is no longer semantic state authority and may remain only as bounded transport glue.
- **End of P06**: startup selected-conversation hydration uses `reduce_startup_batch(startup_inputs)` as the sole startup semantic mutator; no second startup-selected mutation form remains outside the store reducer module.
- **End of P08**: no remaining startup/bootstrap semantic authority lives in `MainPanel`; any retained polling/transport glue is demonstrably transport-only and incapable of initiating an alternate semantic reduction path.


### Safe Protocol Migration Sequence

1. Re-run the Phase 00a constructor/match inventory and use every hit as a migration checklist.
2. Extend `ViewCommand` payload shapes in `src/presentation/view_command.rs`, extend the user-event selection transport to carry `selection_generation`, and update every constructor/match site in source and tests so the repository compiles under the new serialized contract while the old delivery path still works.
3. Introduce `spawn_runtime_bridge_pump(...)` in `src/main_gpui.rs` and make it the only production `GpuiBridge::drain_commands()` owner.
4. Introduce `handle_select_conversation_intent(...)` plus `begin_selection(conversation_id)` and update `ChatView` / `HistoryView` selection to use that path instead of directly sending a raw selection id to tokio.
5. Only after the repo compiles cleanly under the new contract, switch runtime selection/loading semantics to the authoritative store reducer and snapshot publication path.
6. Startup hydration must then reuse that same `begin_selection(...)` + reducer-batch semantic path inside startup batching; startup does not get a second semantically distinct mutation form.
7. Only after Phase 06/07 convergence evidence is green may startup-only bootstrap replay and any redundant direct transcript forwarding be deprecated or removed.

### Streaming / Thinking / Tool-Call Preservation

Current source evidence shows two distinct preservation categories:

- Active GPUI-rendered ephemeral chat commands: `ShowThinking`, `HideThinking`, `AppendThinking`, `AppendStream`, `FinalizeStream`, `StreamCancelled`, and `StreamError` are emitted from `src/presentation/chat_presenter.rs`, forwarded in `src/ui_gpui/views/main_panel.rs`, and handled in `src/ui_gpui/views/chat_view.rs`.
- Emitted-but-not-currently-rendered tool-call commands: `ShowToolCall` and `UpdateToolCall` are emitted from `src/presentation/chat_presenter.rs` but do not currently have active GPUI handlers in `MainPanel` / `ChatView`.

This recovery therefore requires:

- preserving the current active GPUI semantics for the streaming/thinking commands above, including the current `Uuid::nil()` sentinel behavior used by some chat-event deltas to mean "apply to the currently active conversation"
- treating `AppendStream` / `AppendThinking` as ephemeral buffer updates only unless an already-existing repo contract proves otherwise
- using the **direct-finalize durable model** for recovery: accepted `FinalizeStream` directly appends one assistant transcript message into authoritative transcript state from the currently buffered stream content, attaches buffered thinking content if present, and clears stream/thinking buffers in the same accepted reducer mutation/publication
- the concrete durable transcript payload boundary for streamed assistant output is the same replay-compatible payload shape already used by `ConversationMessagesLoaded`: `ConversationMessagePayload { role: MessageRole::Assistant, content, thinking_content, timestamp }`
- the concrete mounted-GPUI render shape remains `ChatView`'s `ChatMessage { role, content, thinking, model_id, timestamp }`; when snapshots are rendered, the replay-compatible payload is mapped into that view shape using the same style of conversion already present in `ChatView::messages_from_payload(...)`
- direct-finalize mapping must therefore preserve at least:
  - `role = Assistant`
  - `content = current accepted stream buffer`
  - `thinking_content = buffered thinking content if non-empty`
  - `timestamp = None` unless an already-existing repo source provides a concrete timestamp at finalize time
- current repo evidence shows streamed finalize rendering uses `self.state.current_model` in `ChatView` for the mounted `ChatMessage` model id; if the authoritative store snapshot keeps replay-compatible payloads only, model/provider metadata must continue to be derived at render time exactly as replay mapping already does rather than inventing a second persisted streamed-message shape in this recovery
- duplicate prevention for streamed assistant output is not buffer-emptiness alone; this plan defines one active stream lifecycle by the current `streaming.active_target` plus non-empty active stream buffer before clear
- accepted `FinalizeStream` therefore requires all of these conditions:
  - resolved target from `conversation_id` / `Uuid::nil()` is `Some(target)`
  - `streaming.active_target == Some(target)`
  - resolved target equals the currently selected conversation
  - stream buffer is non-empty before acceptance
- any finalize failing one of those conditions is a no-op
- one accepted `FinalizeStream` for one accepted active stream lifecycle may materialize at most one new durable assistant transcript entry; clearing `streaming.active_target` and stream/thinking buffers in the same accepted reducer mutation is part of that lifecycle closure
- current repo-grounded evidence available to this plan already shows `src/presentation/chat_presenter.rs` emits `MessageAppended` for the user-message send path and separately emits `FinalizeStream` for stream completion; no source evidence gathered for this plan shows a later assistant-side `MessageAppended` being emitted after streamed completion
- because the plan must be execution-safe even if that assumption later proves false, reducer-side dedupe guard support is mandatory in the final design for streamed assistant durable finalization
- the mandatory dedupe state for this recovery is one store-owned guard describing the just-finalized streamed assistant lifecycle; this plan standardizes it conceptually as `last_finalized_stream_guard { conversation_id, transcript_len_after_finalize }`
- accepted `FinalizeStream` must set that guard in the same accepted mutation that appends the assistant payload and clears stream/thinking ephemera
- a later assistant-side `MessageAppended` targeting the same conversation must be ignored as a no-op only when all exact duplicate-predicate conditions hold together: same conversation id as `last_finalized_stream_guard`, transcript length still equals `transcript_len_after_finalize`, current transcript tail entry exists, current transcript tail entry role is assistant, and current transcript tail entry content exactly equals incoming `MessageAppended.content`; user-message append behavior is unchanged, and if any one of those checks fails the append is treated as an ordinary non-stream append rather than suppressed
- that dedupe predicate is intentionally narrow and exact; if transcript length or current tail no longer matches, implementation must treat the later `MessageAppended` as an ordinary append rather than inventing fuzzy content-based suppression
- `ConversationMessagesLoaded` remains the authoritative bulk-replacement path for the matching generation and always overwrites the current selected transcript snapshot for that generation, including any earlier finalize-materialized assistant content currently held in store memory; matching bulk replacement also clears `last_finalized_stream_guard`
- a new selection or bulk transcript replacement clears the just-finalized guard; the guard is for duplicate streamed-assistant durable materialization, not for long-lived transcript dedupe across unrelated sessions
- the repo-wide callsite inventory plus named deterministic streamed-interaction test remain required evidence, but they now validate the mandatory guard coverage rather than deciding whether duplication handling exists at all
- leaving streamed-output duplication safety to proof-only without that reducer-side dedupe guard is non-compliant
- `MessageAppended` remains the durable append path for user messages and any already-existing non-stream assistant append cases, but it must not duplicate accepted streamed assistant finalization
- accepted finalize therefore creates authoritative store state for current rendering/reopen correctness, but it does not become a second persistence authority competing with later matching bulk replacement

- rejecting stale/non-selected stream finalization rather than allowing it to create durable transcript mutations for the wrong target
- resolving `Uuid::nil()` to the currently selected conversation before any accepted `FinalizeStream` durable write; if no selected conversation exists, the finalize is ignored as a no-op
- clearing post-finalize `streaming.active_target` and stream/thinking buffers exactly once in the same accepted reducer mutation that writes the durable assistant message
- not silently broadening scope by inventing new tool-call UI semantics during this recovery
- documenting in evidence that `ShowToolCall` / `UpdateToolCall` remain explicit no-op transport variants in GPUI unless intentionally migrated with their own tests

### Bounded `ConversationCleared` Exception

`ConversationCleared` remains outside the store-backed transcript reducer scope for this recovery unless explicitly expanded with new tests and requirements.

That bounded exception means:

- it may continue to clear mounted popup-local visible transcript state on the old path during migration
- it must not mutate or replace the authoritative store transcript snapshot, selection identity, selected title, generation token, load/error state, or revision
- selected conversation header/title remain derived from authoritative selection identity while the popup is mounted
- local clear resets local stream/thinking ephemera to idle/empty
- the only preserved post-condition for transcript body is this: after control returns to the event loop, mounted transcript rendering must again match authoritative `current_snapshot()`; a durable empty transcript after handler return is not compliant
- the concrete owner of the bounded clear recovery path is the mounted `ChatView` clear handler while that old-path behavior remains in scope
- the concrete recovery seam is one fixed invariant shape: `ChatView` must have deterministic same-turn read-only access to authoritative `current_snapshot()`. `MainPanel` or any caller must not synthesize a one-off semantic repair accessor into the clear path.
- the concrete mechanism is: apply local clear side effects, synchronously read authoritative `current_snapshot()` through that retained construction-time access path in the same update closure, then overwrite all store-backed render fields from that snapshot before control returns to the event loop
- acceptable shape: same mounted handler/update closure performing local clear then immediate authoritative snapshot reapplication
- forbidden near-miss shapes: subscriber-driven repair, `cx.spawn(...)`/task/timer/frame callback repair, remount-only repair, or creating a second popup-local transcript authority just to survive the clear
- restoration must not be scheduled onto a future executor tick, async task, subscriber callback, frame callback, or remount path
- the required timing is the same mounted clear-handling update/redraw cycle, not a later unrelated event turn
- popup remount must always read `current_snapshot()` and show authoritative store state even if no intervening runtime publication occurred
- it must not become a second durable empty transcript owner


#### Bounded `ConversationCleared` State Table

| Facet | Before local clear | Immediately after local clear on mounted popup | After immediate authoritative redraw/readback or popup remount |
|-------|--------------------|-----------------------------------------------|---------------------------------------------------------------|
| Authoritative transcript snapshot | selected conversation transcript snapshot | unchanged | unchanged and rendered again |
| Selected conversation id/title | authoritative selected id/title | unchanged | unchanged |
| `selection_generation` | current authoritative generation | unchanged | unchanged |
| Load/error state | current authoritative `Idle` / `Loading` / `Ready` / `Error` | unchanged | unchanged |
| Local visible transcript | authoritative transcript currently rendered | may become transiently empty on mounted old path | overwritten by authoritative snapshot from `current_snapshot()` |
| Local streaming/thinking buffers | current mounted local ephemera | cleared to idle/empty | reflect authoritative snapshot on redraw/remount |
| Store revision | current revision | unchanged | unchanged unless some other accepted store mutation happened |
| Durable ownership | authoritative store | authoritative store | authoritative store |

Interpretation rules:
- any proof for bounded `ConversationCleared` must show the rightmost column is restored without requiring an unrelated future store mutation

---

## Integration Points

### Existing Files That Define the Problem

- `src/main_gpui.rs`
- `src/ui_gpui/views/main_panel.rs`
- `src/ui_gpui/views/chat_view.rs`
- `src/ui_gpui/views/history_view.rs`
- `src/presentation/chat_presenter.rs`
- `src/presentation/history_presenter.rs`
- `src/presentation/view_command.rs`
- `src/events/types.rs`
- `src/ui_gpui/bridge/user_event_forwarder.rs`
- `src/ui_gpui/bridge/gpui_bridge.rs`

### Existing Planning/Architecture References To Preserve Alignment

- `project-plans/gpui-migration/specification.md`
- `project-plans/gpui-migration/appendix-integration-contracts.md`
- `project-plans/gpui-migration/analysis/pseudocode/app.md`
- `project-plans/gpui-migration/analysis/pseudocode/bridge.md`
- `project-plans/gpui-migration/analysis/pseudocode/chat_view.md`
- `project-plans/gpui-migration/analysis/pseudocode/main_panel.md`
- `project-plans/nextgpuiremediate/*`
- `dev-docs/PLAN.md`
- `dev-docs/PLAN-TEMPLATE.md`
- `dev-docs/COORDINATING.md`

### Existing Code To Be Replaced Or Deprecated In Scope

- Startup-only transcript delivery logic that bypasses ordinary runtime state flow
- Popup-coupled assumptions that `MainPanel` polling is the authoritative transport boundary
- Any redundant bootstrap replay logic that duplicates store hydration behavior after the authoritative store exists
- Selection behavior that clears visible chat state without entering an explicit loading state

This plan does **not** expand into unrelated UI redesign, transport rewrites unrelated to chat-state delivery, or broader architecture migrations outside the seams required to stabilize this flow.

---

## Formal Requirements

### REQ-ARCH-001: Authoritative App Store

[REQ-ARCH-001.1] GPUI runtime MUST expose one authoritative store for chat-facing application state.  
[REQ-ARCH-001.2] The store MUST hold the selected conversation identity, transcript snapshot, loading status, profile snapshot, and conversation list snapshot required by mounted popup views.  
[REQ-ARCH-001.3] The store MUST outlive popup open/close cycles within the running app process.  
[REQ-ARCH-001.4] Popup views MUST derive render state from store snapshots, not from transient replay side effects.  
[REQ-ARCH-001.5] The implementation plan MUST treat `src/ui_gpui/app_store.rs` as the target store module path unless preflight documents a better existing module and updates all downstream phase references.

### REQ-ARCH-002: Startup/Runtime Convergence

[REQ-ARCH-002.1] Startup hydration MUST use the same authoritative reducer semantics and state contract used for ordinary runtime presenter updates, even though startup may use a dedicated startup transaction instead of the ordinary runtime bridge/event transport choreography.  
[REQ-ARCH-002.2] First-frame correctness currently achieved by `build_startup_view_commands` + `apply_startup_commands` MUST be preserved.  
[REQ-ARCH-002.3] Redundant startup-only command application MUST be removable once the authoritative path is in place.  
[REQ-ARCH-002.4] Manual conversation selection and startup selection MUST converge on the same transcript replacement semantics.  
[REQ-ARCH-002.5] Startup hydration MUST complete as one coherent committed startup batch before popup subscription/mount when startup already knows the selected transcript outcome, so the app does not flash an empty or loading transcript before converging.

### REQ-ARCH-003: Explicit Conversation Loading Protocol

[REQ-ARCH-003.1] Selecting a conversation MUST enter an explicit loading protocol/state.  
[REQ-ARCH-003.2] `begin_selection(...)` MUST be the only ordinary-runtime authority transition that may mint a new selection generation and enter `Loading`.  
[REQ-ARCH-003.3] Bulk transcript replacement via `ConversationMessagesLoaded` MUST remain supported.  
[REQ-ARCH-003.4] Transcript replacement MUST apply only when the payload corresponds to the currently selected conversation generation/request.  
[REQ-ARCH-003.5] The UI MUST not rely on "clear and hope replay arrives" semantics.  
[REQ-ARCH-003.6] Selection freshness MUST use an explicit token carried through `ConversationActivated`, `ConversationMessagesLoaded`, and `ConversationLoadFailed`, using `selection_generation`.

### REQ-ARCH-004: Popup Independence

[REQ-ARCH-004.1] Popup visibility MUST NOT determine whether presenter-originated chat state reaches the authoritative store.  
[REQ-ARCH-004.2] Opening the popup after background state changes MUST render the latest store snapshot without requiring special bootstrap commands.  
[REQ-ARCH-004.3] Closing and reopening the popup MUST preserve the same selected conversation snapshot and transcript snapshot already held in the store.

### REQ-ARCH-005: MainPanel Responsibility Reduction

[REQ-ARCH-005.1] `MainPanel` MUST remain a navigation/composition root, but no longer be the semantic owner of transcript durability or replay correctness.  
[REQ-ARCH-005.2] `MainPanel` forwarding logic MUST become thin and deterministic relative to the authoritative store contract.  
[REQ-ARCH-005.3] Bridge polling, popup lifecycle, startup bootstrap, and command routing responsibilities MUST be explicitly separated in the implementation plan.

### REQ-ARCH-006: Behavior Preservation

[REQ-ARCH-006.1] Preserve bulk transcript replacement via `ConversationMessagesLoaded`.  
[REQ-ARCH-006.2] Preserve no-clear-on-ordinary-`ConversationActivated`.  
[REQ-ARCH-006.3] Preserve startup first-frame correctness.  
[REQ-ARCH-006.4] Preserve Kimi/provider quirks behavior.  
[REQ-ARCH-006.5] Preserve existing transcript-loading and layout fixes.  
[REQ-ARCH-006.6] Preserve currently active GPUI streaming/thinking behavior for `ShowThinking`, `HideThinking`, `AppendThinking`, `AppendStream`, `FinalizeStream`, `StreamCancelled`, and `StreamError`, including the current active-conversation / `Uuid::nil()` targeting semantics until intentionally changed by separate scope.  
[REQ-ARCH-006.7] Do not implicitly expand this recovery into new tool-call UI behavior for `ShowToolCall` / `UpdateToolCall`; any change to their current GPUI semantics must be called out explicitly in plan evidence and tests.

### REQ-INT-001: Test-First Recovery

[REQ-INT-001.1] The implementation plan MUST prove the current failure mode before code changes.  
[REQ-INT-001.2] Failing tests for startup/manual-selection convergence MUST be authored before implementation.  
[REQ-INT-001.3] Tests MUST verify semantic behavior, reachability, and integration, not only structural changes.  
[REQ-INT-001.4] Placeholder detection and anti-placeholder verification MUST be explicit in every implementation phase.  
[REQ-INT-001.5] Test-writing and implementation phases MUST require `@plan`, `@requirement`, and `@pseudocode` markers on created or materially updated tests/functions/structs following project plan conventions.

### REQ-INT-002: Verification and Quality Gates

[REQ-INT-002.1] Each implementation phase MUST define structural checks, semantic verification, reachability checks, and placeholder-detection commands.  
[REQ-INT-002.2] Final verification MUST include `cargo fmt --all`, `cargo check -q`, the named regression/integration test suite including the architecture-specific GPUI/store/bridge/remount suites touched by implementation, `cargo clippy --all-targets -- -D warnings`, and `bash scripts/check-quality.sh`.  
[REQ-INT-002.3] Coverage expectations MUST be stated in verification phases and must reject placeholder-style passing behavior.  
[REQ-INT-002.4] The plan MUST stay focused on chat state delivery recovery architecture and not balloon into unrelated redesign.  
[REQ-INT-002.5] The plan MUST define concrete final evidence artifact paths and templates for handoff.

## Requirement Index (Frozen for Phase Marker Traceability)

The following sub-requirement IDs are the canonical traceability surface for this plan's marker-based verification commands:

- `REQ-ARCH-001.1`
- `REQ-ARCH-001.2`
- `REQ-ARCH-001.3`
- `REQ-ARCH-001.4`
- `REQ-ARCH-001.5`
- `REQ-ARCH-002.1`
- `REQ-ARCH-002.2`
- `REQ-ARCH-002.3`
- `REQ-ARCH-002.4`
- `REQ-ARCH-002.5`
- `REQ-ARCH-003.1`
- `REQ-ARCH-003.2`
- `REQ-ARCH-003.3`
- `REQ-ARCH-003.4`
- `REQ-ARCH-003.5`
- `REQ-ARCH-003.6`
- `REQ-ARCH-004.1`
- `REQ-ARCH-004.2`
- `REQ-ARCH-004.3`
- `REQ-ARCH-005.1`
- `REQ-ARCH-005.2`
- `REQ-ARCH-005.3`
- `REQ-ARCH-006.1`
- `REQ-ARCH-006.2`
- `REQ-ARCH-006.3`
- `REQ-ARCH-006.4`
- `REQ-ARCH-006.5`
- `REQ-ARCH-006.6`
- `REQ-ARCH-006.7`
- `REQ-INT-001.1`
- `REQ-INT-001.2`
- `REQ-INT-001.3`
- `REQ-INT-001.4`
- `REQ-INT-001.5`
- `REQ-INT-002.1`
- `REQ-INT-002.2`
- `REQ-INT-002.3`
- `REQ-INT-002.4`
- `REQ-INT-002.5`

This index is frozen for the execution of this plan. If any sub-ID is renumbered, every downstream phase command, marker expectation, and evidence artifact that references that ID must be updated in the same doc change before execution may continue.

---


---

## Quality Helper Relevance

`bash scripts/check-quality.sh` is the repository quality gate helper invocation used in this plan. It enforces project-wide hygiene beyond narrow phase tests. In this plan it is relevant because the recovery touches shared runtime/view/presenter seams, so the final gates must prove the change still satisfies the repository's standard formatting/lint/test or policy checks rather than only the targeted regression suite.
If Phase 00a records an evidence-backed out-of-scope baseline exception for `bash scripts/check-quality.sh`, later phases must treat that as a strict no-regression contract: they may not dismiss helper failures as pre-existing without proving the failure set in recovery-touched files did not grow beyond the Phase 00a exception scope, and final evidence must either show full helper green or that explicit no-regression proof.


---

## Non-Negotiable Preservation Constraints

- Preserve startup first-frame transcript correctness.
- Preserve `ConversationMessagesLoaded` as bulk replacement semantics.
- Do not clear transcript on ordinary `ConversationActivated`.
- Preserve Kimi/provider quirks behavior and related tests.
- Preserve existing transcript-loading/layout fixes.
- Planning only in this plan directory; source-code modifications are out of scope for this task.
- No time estimates anywhere in the plan.

---

## Test and Verification Baseline

The plan must use and cite these verification commands where appropriate:

```bash
cargo fmt --all
cargo check -q
cargo test --test presenter_selection_and_settings_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests --test llm_client_helpers_tests --test kimi_provider_quirks_integration_tests --test gpui_integration_tests --test gpui_bridge_tests --test gpui_chat_view_tests --test gpui_components_tests --test gpui_wiring_event_flow_tests --test gpui_wiring_command_routing_tests -- --nocapture
cargo clippy --all-targets -- -D warnings
bash scripts/check-quality.sh
```

Additional phase-specific grep/search/readback commands must be included to prove state-path ownership, plan-marker presence, placeholder absence, and integration reachability.

---

## Success Criteria

1. The plan proves the current failure mode with file-cited preflight analysis.
2. The plan defines one authoritative store contract and an explicit loading protocol.
3. Startup and manual selection converge onto one state delivery architecture.
4. Popup lifetime is no longer an architectural dependency for state correctness.
5. Each implementation phase is test-first and contains usable commands, pseudocode references, marker requirements, and quality gates.
6. Final phases require full project verification plus quality helper execution.
7. Final handoff artifacts are concrete, reviewable, and complete.
8. The plan remains focused on GPUI chat state delivery recovery rather than unrelated platform redesign.