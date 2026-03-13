# Analysis 02: Authoritative Store Contract

## Purpose

Define the minimum authoritative store required to converge startup and runtime transcript delivery.

## Concrete Store Module Target

The implementation target is `src/ui_gpui/app_store.rs`.

If preflight discovers an existing GPUI app-state module that is a clearly better fit, Phase 00a must document the replacement path explicitly and every downstream phase must be updated to match. Otherwise, implementation agents should not invent alternate store locations.

## Store Responsibilities

The authoritative store owns the durable GPUI application snapshot relevant to popup rendering. It is the state authority during process lifetime, independent of popup mount state.

## Repo-Grounded Runtime Ownership

This recovery is grounded in the repo's existing GPUI app-root runtime shape.

- `src/main_gpui.rs` already has a real global `AppState` and app-root GPUI task pattern via `cx.spawn(...)`.
- `src/ui_gpui/views/main_panel.rs::ensure_bridge_polling(...)` is currently popup-bound and drains inside popup/window update logic.
- `src/ui_gpui/bridge/gpui_bridge.rs::drain_commands()` already drains queued commands in one call.
- `src/ui_gpui/bridge/gpui_bridge.rs::emit(...)` can fail synchronously.

The production target after this recovery is therefore explicit:

- authoritative store lifetime/ownership is rooted in app-root GPUI runtime state (`src/main_gpui.rs::AppState`)
- one app-root runtime helper (`spawn_runtime_bridge_pump(...)`) becomes the sole production drainer after Phase 05
- popup/window lifetime becomes a subscriber concern only, not the semantic ingress owner

## Synchronization and Publication Model

- The store is constructed during GPUI runtime startup, before popup mount.
- Runtime producers reduce `ViewCommand` values through one reducer boundary owned by the store.
- Store state is mutated behind one synchronization boundary consistent with the project runtime model; implementation must not split write authority across popup-local state and the store.
- Publication is snapshot-based and revision-based.
- One logical mutation batch produces one revision increment and one published snapshot.
- Startup hydration is a single initial batch publication so first-frame correctness is preserved without an empty/loading flash.
- No subscriber may observe partially hydrated startup state; all startup reductions complete before the first published snapshot becomes visible.
- Popup reopen subscribes to the latest snapshot and renders it immediately; reopen does not depend on replaying transient queues.
- Publication discipline is reducer-entrypoint-scoped: one reducer invocation may publish at most once, but one user-visible selection lifecycle may legitimately produce more than one publication across separate reducer invocations (for example `Loading` then later `Ready` or `Error`).
- The single-flight guard lives on app-root runtime state (`runtime_ingress_in_flight`) and covers one whole production drain/publish critical section: one eligible pump tick -> at most one `drain_commands()` call -> one reducer entrypoint -> at most one publication attempt for that drained batch.

## Minimum State Domains

### Chat Domain

- selected conversation id
- selected conversation title
- selected title provenance (`HistoryBacked(non_empty_title)` or `LiteralFallback("Untitled Conversation")`)
- conversation selection generation / request token
- transcript snapshot for selected conversation
- conversation loading state
- streaming/thinking/tool-call state for selected conversation
- exact duplicate-finalize guard (`last_finalized_stream_guard`) for narrow reducer-side dedupe of streamed assistant finalization

### History Domain

- conversation list snapshot
- selected conversation id as a render-selection slice aligned to authoritative chat selection, not a second semantic owner
- metadata needed for title/message-count/history ordering

### Settings/Profile Domain

- selected profile id
- profile list snapshot needed by chat/settings startup correctness

## Loading Protocol

Conversation selection requires explicit states instead of implicit clear/replay ordering.

### Required States

- `Idle`
- `Loading { conversation_id, generation }`
- `Ready { conversation_id, generation }`
- `Error { conversation_id, generation, message }`

### Required Rules

1. `ConversationActivated { id, selection_generation }` no longer owns ordinary-runtime selection advancement. It is an idempotent protocol echo once authoritative store state already matches the selected id/generation.
2. Ordinary activation does not clear the existing transcript immediately.
3. `ConversationMessagesLoaded { conversation_id, selection_generation, messages }` performs bulk replacement only if its conversation id and generation match the current loading/selected state.
4. `ConversationLoadFailed { conversation_id, selection_generation, message }` records explicit error state only if its conversation id and generation match the current loading/selected state.
5. If an outdated payload arrives, the store ignores it without corrupting visible state.
6. Mounted views render `Loading` or `Error` explicitly rather than showing an empty transcript that looks authoritative.
7. Empty-conversation readiness is reached only by an explicit `ConversationMessagesLoaded` payload carrying an empty message list for the active generation.
8. Same-id reselection while authoritative state is `Loading` or `Ready` is a strict no-op: no new generation, no publication, no async load dispatch, no ephemera clear, and no selected-title rewrite.
9. Same-id reselection from `Error` is an explicit retry: mint one new generation and re-enter `Loading`.
10. `handle_select_conversation_intent(...)` calls `begin_selection(...)` before presenter async work begins and is the only ordinary-runtime entrypoint allowed to initiate selected-conversation async transcript load dispatch.
11. The presenter's matching `ConversationActivated { id, selection_generation }` becomes an idempotent protocol echo when store state already matches.
12. In ordinary runtime, `ConversationActivated` is non-authoritative except for one bounded case: if id and generation already match authoritative store state but the current selected title provenance is `LiteralFallback("Untitled Conversation")`, and authoritative history data now provides a non-empty title for that same id, activation may correct the selected title to that history-backed value only. It must not mint a generation, replace transcript, or create a second loading transition.
13. Stale `ConversationActivated` for a generation lower than the current authoritative generation is always ignored.
14. `ConversationActivated` for a generation greater than the current authoritative generation during ordinary runtime is a protocol violation and must also be ignored as a no-op; higher-generation advancement is not a defensive recovery path because `begin_selection(...)` is the sole ordinary-runtime minting site.
15. Success/failure payloads for that request lifecycle must carry the same token.

## Exact `begin_selection(...)` Semantics

- different conversation id -> mint new generation, enter `Loading`, publish once
- same conversation id while `Loading` or `Ready` -> strict no-op: no new generation, no publication, no async load dispatch, no ephemera clear, and no selected-title rewrite
- same conversation id while `Error` -> mint new generation, re-enter `Loading`, publish once
- stale in-flight loads do not need cancellation to satisfy this plan, but stale success/failure payloads must be ignored on arrival
- title fallback at mint time is owned by the GPUI/store boundary using the current authoritative history snapshot if available; the exact fallback rule is the existing repo rule already visible in `src/main_gpui.rs` startup conversation-summary creation and `src/ui_gpui/views/chat_view.rs::sync_conversation_title_from_active()`: if the title is empty after trim, use the literal fallback string `"Untitled Conversation"`
- title provenance/strength is literal in this recovery, not helper-implied. The authoritative selected title must be treated as exactly one of: `HistoryBacked(non_empty_title)` or `LiteralFallback("Untitled Conversation")`.
- title-source precedence is fixed for this recovery: `HistoryBacked(non_empty_title)` > `LiteralFallback("Untitled Conversation")`
- bounded selected-title correction may only upgrade `LiteralFallback("Untitled Conversation")` to `HistoryBacked(non_empty_title)` for the same selected conversation id when authoritative history snapshot data now provides that stronger non-empty title. This bounded upgrade must use the same exact reducer helper rule whether triggered by `ConversationListRefreshed` or by a matching idempotent `ConversationActivated` echo. It must not overwrite an existing `HistoryBacked(...)` title, and it must not invent a string-comparison-based “stronger title” rule or any second title source
- popup views never mint freshness tokens independently
- tokio-side presenters do not own an independent monotonic counter
- a bridge-side helper may transport the minted token, but it may not become the authoritative source of sequence advancement or a second minting site

## Selection-Generation Ownership and Dispatch

The monotonic `selection_generation` counter is owned by the authoritative store domain for selected-conversation state.

- freshness is defined relative to the store's current selected-conversation snapshot, so the store must remain the source of truth for the current generation
- popup views must never mint freshness tokens independently
- multiple runtime producers can only stay coherent if one authority defines the current generation sequence
- without one concrete minting boundary, an implementation agent could accidentally recreate split state ownership in presenter land

The production dispatch contract is equally explicit:

1. `handle_select_conversation_intent(...)` is the only allowed ordinary-runtime dispatch entrypoint.
2. `begin_selection(...)` is the sole ordinary-runtime minting site.
3. `src/ui_gpui/bridge/user_event_forwarder.rs` remains transport only and forwards the enriched event onto the existing `EventBus` path without changing freshness ownership.
4. For ordinary runtime production selection, `handle_select_conversation_intent(...)` is the only allowed dispatch entrypoint. No other production path may directly or indirectly call `begin_selection(...)`, emit enriched selection transport, or initiate selected-conversation async transcript load dispatch, including compatibility shims, remount hooks, history refresh callbacks, or retained transport helpers.
5. `handle_select_conversation_intent(...)` calls `app_store.begin_selection(conversation_id)` synchronously before any presenter async transcript load begins.
6. `begin_selection(conversation_id)` is the only ordinary-runtime minting site. It must:
   - decide whether the selection is a no-op or retry,
   - mint the next generation only when required,
   - record selected id/title fallback plus `Loading` state in authoritative store state,
   - clear only streaming/thinking ephemera,
   - publish exactly once if authoritative state changed.
7. `handle_select_conversation_intent(...)` then forwards the minted token through the existing user-event transport using an enriched event shape; this plan standardizes that as `UserEvent::SelectConversation { id, selection_generation }`.
8. `src/ui_gpui/bridge/user_event_forwarder.rs` remains transport only and forwards that enriched event onto the existing `EventBus` path without changing freshness ownership.
9. `ChatPresenter::handle_select_conversation(...)` (or its explicit delegate) consumes that enriched event and must reuse the provided token exactly as received.
10. Current repo evidence shows `GpuiBridge::emit(...)` uses `try_send(...)` and may fail synchronously. If emitting the enriched selection event fails, the GPUI/store boundary must immediately reduce `ConversationLoadFailed { conversation_id, selection_generation, message }` for that same generation rather than leaving `Loading` stuck.
11. The presenter's matching `ConversationActivated { id, selection_generation }` becomes an idempotent protocol echo when store state already matches.
12. In ordinary runtime, `ConversationActivated` is non-authoritative except for one bounded case: if id and generation already match authoritative store state but the current selected title provenance is `LiteralFallback("Untitled Conversation")`, and authoritative history data now provides a non-empty title for that same id, activation may correct the selected title to that history-backed value only. It must not mint a generation, replace transcript, or create a second loading transition.
13. Stale `ConversationActivated` for a generation lower than the current authoritative generation is always ignored.
14. `ConversationActivated` for a generation greater than the current authoritative generation during ordinary runtime is a protocol violation and must also be ignored as a no-op; higher-generation advancement is not a defensive recovery path because `begin_selection(...)` is the sole ordinary-runtime minting site.
15. Success/failure payloads for that request lifecycle must carry the same token.

## Startup Transaction Contract

Startup initializes the store with generation `0` before publication.

- if startup has no selected conversation, generation stays `0` until the first ordinary runtime selection mints `1`
- startup transaction shape is fixed by this plan to one canonical entrypoint owned by `src/ui_gpui/app_store.rs`; this plan standardizes that entrypoint as `reduce_startup_batch(startup_inputs)`
- The function names `begin_selection(...)` and `reduce_startup_batch(startup_inputs)` are normative shorthand for this plan, not mandatory spelling. An implementation may use repo-idiomatic equivalent names only if evidence maps them one-to-one to the same sole minting site and sole startup transaction responsibilities.
- `reduce_startup_batch(startup_inputs)` is the only allowed public production startup transaction API for selected-conversation hydration in this recovery and must:
  - run only against fresh startup store state before any popup subscriber exists,
  - assemble startup history/profile/chat inputs,
  - require `startup_inputs` to carry an explicit startup mode discriminator rather than an unqualified pending/success/failure union,
  - call `begin_selection(conversation_id, BatchNoPublish)` exactly once when startup has a selected conversation,
  - use only the matching transcript success or failure payload as the authoritative startup completion step in that same transaction using the same reducer helper path as runtime commands,
  - treat any startup-synthesized `ConversationActivated` as compatibility-only/readback-only material rather than an authoritative startup state transition,
  - bump revision at most once for the whole startup transaction,
  - publish at most once for the whole startup transaction,
  - forbid any second startup-specific mutator from writing selected-conversation id/title/generation/load/transcript state directly
- Structured `startup_inputs` are therefore the only normative production representation for startup-selected hydration in this recovery. Its startup-mode shape must be explicit:
  - `ModeA { transcript_result: Success(messages) | Failure(message) }`
  - `ModeB { transcript_unavailable_reason, pending_generation: 1 }`
- A bare `Pending` transcript result is non-compliant because it does not prove why Mode B was required instead of Mode A.
- if startup already knows the selected conversation and transcript, that canonical startup transaction must mint generation `1` and finish in `Ready` before any popup subscriber may exist
- if startup knows the selected conversation but the initial transcript load fails, that same canonical startup transaction must mint generation `1` and finish in explicit `Error` state before any popup subscriber may exist
- if startup uses Mode B, handoff/proof evidence must name the exact repo-grounded seam class `transcript_unavailable_reason`
- acceptable `transcript_unavailable_reason` values in this recovery must map to one cited repo seam class, not ad hoc prose: `StartupServiceSeamUnavailable`, `AsyncOnlySourceBeforeMount`, or `StartupCompositionDoesNotProvideTranscriptOutcome`; Phase 06/12 evidence must tie the chosen value to exact source file/function origin
- any startup-specific wrapper may only assemble startup inputs and invoke `reduce_startup_batch(startup_inputs)`; it must not directly mutate selected-conversation id/title/generation/load/transcript fields outside that entrypoint

## ViewCommand Protocol Migration

`src/presentation/view_command.rs` is a serialized cross-layer contract, not a popup-local implementation detail. This recovery requires an explicit repo-wide protocol migration.

Implementation must update `ViewCommand` so that:

- `ConversationActivated` carries `selection_generation`
- `ConversationMessagesLoaded` carries `selection_generation`
- `ConversationLoadFailed { conversation_id, selection_generation, message }` exists as a first-class variant

Because `ViewCommand` derives `Serialize` and `Deserialize`, all constructors, match arms, bridge forwarding sites, reducer entry points, tests, and any persisted/transported protocol expectations touching these variants must be updated together. This is mandatory for protocol consistency; it is not an optional cleanup.

## Streaming / Finalize Preservation Contract

The repo-grounded current GPUI stream durability behavior is local-view finalization: `FinalizeStream` in `src/ui_gpui/views/chat_view.rs` materializes assistant output from the streaming buffer.

This recovery preserves streamed-output correctness by moving the durable finalization responsibility into the authoritative reducer contract without inventing fuzzy duplicate suppression:

- `FinalizeStream` must remain a direct-finalize durable model in the store reducer: the reducer materializes the assistant payload exactly once from the active stream buffer into the authoritative transcript snapshot
- dedupe must be intentionally narrow/exact, not heuristic/content-fuzzy
- the exact reducer-side duplicate guard is `last_finalized_stream_guard`, evaluated together with exact transcript-tail comparison on conversation id, transcript length after finalize, assistant role, and assistant content
- timestamp/model/provider/finalized-thinking are intentionally not part of the duplicate predicate because the incoming `MessageAppended` path does not carry enough information to compare them safely here
- repo-wide callsite inventory plus one named deterministic streamed-interaction regression must verify the guard coverage; duplicate-safety may not be left as an assumption
- `MessageAppended` remains the durable append path for user messages and existing non-stream append cases

## Tool-Call Scope Boundary

Current repo evidence shows `ShowToolCall` / `UpdateToolCall` are emitted but not presently active GPUI-rendered behavior. This recovery therefore keeps them explicitly unchanged unless later scope intentionally expands them.

- reducer handling may leave these commands unchanged/no-op in this plan
- they must not silently become new authoritative snapshot domains in this recovery without an explicit scope change
- tests/evidence should confirm that unchanged handling is deliberate, not accidental omission

## Mutation Sources

### Allowed Mutators

- startup hydration path through `reduce_startup_batch(startup_inputs)` only
- presenter-originated runtime updates through the authoritative reducer boundary
- explicit user intents that are reduced into state transitions

### Disallowed Ownership Patterns

- popup-local view state as the only durable owner of selected-conversation transcript state
- popup-bound production draining after Phase 05
- second monotonic selection-generation sequence in presenter/bridge/view state
- second durable transcript mirror used to “repair” `ConversationCleared`
- startup-specific mutators that write selected-conversation state outside `reduce_startup_batch(startup_inputs)`

## `ConversationCleared` Compatibility Boundary

`ConversationCleared` remains bounded old-path behavior only.

- reducer contract for this recovery leaves authoritative store transcript/selection/load state unchanged on `ConversationCleared`
- the mounted clear-handling path (currently `ChatView::handle_command(ViewCommand::ConversationCleared)`, or an evidence-mapped repo-idiomatic equivalent) may repair visible state only by same-turn readback of authoritative `current_snapshot()` already available within the current mounted update transaction
- acceptable structural shape for the bounded clear path: the mounted `ChatView` may hold synchronous read-only access to the authoritative snapshot via a construction-time retained store handle or immutable snapshot accessor supplied during view construction, so long as the same mounted update transaction performs the local clear side effects and the immediate `current_snapshot()`-based render restoration without introducing a second durable mirror


- forbidden near-miss shapes: subscriber callback repair, spawned task/timer/frame callback repair, remount-only repair, or popup-local durable transcript mirror

## MainPanel Relationship

`MainPanel` should compose views from current snapshots and forward intents/commands into the store boundary, but should not itself remain the durable transcript replay authority.

## Hard Cutover Matrix

The phase boundary by end-state is explicit.

- **By end of Phase 04**: authoritative store types, lifetime ownership, app-root runtime initialization, revisioned snapshot construction, module registration, and mounted subscription plumbing exist; popup-local/direct command handling may still exist temporarily for semantics not yet cut over, but it is no longer allowed to become the new durable owner.
- **By end of Phase 05**: ordinary-runtime presenter/bridge traffic reduces into the authoritative store first through app-root `spawn_runtime_bridge_pump(...)`; `begin_selection(...)` is the sole ordinary-runtime minting site; matching `ConversationActivated` is non-authoritative/idempotent; popup-local direct command handling may remain only for explicitly bounded compatibility cases like `ConversationCleared`, not as authority for selected transcript/loading/title/generation.
- **By end of Phase 06**: startup selected-conversation hydration uses the canonical startup transaction `reduce_startup_batch(startup_inputs)` with the same authoritative reducer semantics/state contract; no startup-specific semantic mutator remains outside that entrypoint.
- **By end of Phase 08**: redundant bootstrap authority and popup-local semantic replay authority are removed or left only as tightly bounded non-authoritative compatibility glue explicitly named in evidence.

## Preservation Guarantees

The store contract must preserve:

- startup first-frame transcript presence
- `ConversationMessagesLoaded` bulk replacement semantics
- no clear on ordinary `ConversationActivated`
- provider/Kimi quirks and existing layout fixes
- exact/narrow streamed-finalize dedupe semantics rather than fuzzy duplicate suppression

## Startup Hydration Constraint

Startup hydration must preload the selected conversation, transcript snapshot, history snapshot, and profile snapshot into one coherent store batch before any popup subscriber may exist. The migration may use a temporary compatibility shim while converging, but the shim must still populate the authoritative store first and must not publish an intermediate empty/loading snapshot that would regress first-frame correctness.