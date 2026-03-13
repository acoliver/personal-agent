# Analysis 01: Current GPUI Chat State Delivery Paths

## Purpose

Document the current split delivery model so the implementation plan can replace it with one authoritative state flow without losing already-working startup behavior.

## Observed Active Paths

### Path A: Startup Bootstrap Path

1. `src/main_gpui.rs:174-293` constructs synchronous startup replay payloads in `build_startup_view_commands(...)`, including `ConversationListRefreshed`, `ConversationActivated`, and `ConversationMessagesLoaded` for the first conversation when one exists.
2. `src/main_gpui.rs:797-800` stores those replay payloads on `MainPanelAppState.startup_commands` before any popup window exists.
3. `src/ui_gpui/views/main_panel.rs:211-225` reads `startup_commands` in `apply_startup_commands(...)`.
4. `src/ui_gpui/views/main_panel.rs:228-306` calls `apply_startup_commands(...)` during `MainPanel::init(...)`, immediately after constructing child views.
5. `src/ui_gpui/views/main_panel.rs:750-760` and `src/ui_gpui/views/main_panel.rs:597-610` forward `ConversationActivated` and `ConversationMessagesLoaded` directly into `ChatView`.
6. Because this replay is applied synchronously during initialization instead of arriving through the runtime bridge, first-frame transcript rendering depends on bootstrap replay rather than the presenter polling path.

### Path B: Runtime Presenter Path

1. `src/ui_gpui/views/chat_view.rs:372-403` updates local dropdown selection state and emits `UserEvent::SelectConversation { id }` when the user selects a conversation from ChatView.
2. `src/ui_gpui/views/history_view.rs:297-306` emits the same `UserEvent::SelectConversation { id }` from the HistoryView Load action.
3. `src/presentation/chat_presenter.rs:710-753` handles that event in `handle_select_conversation(...)`, calls `conversation_service.set_active(id)`, emits `ViewCommand::ConversationActivated { id }`, replays `ViewCommand::ConversationMessagesLoaded { conversation_id, messages }`, then refreshes the conversation list.
4. `src/main_gpui.rs:85-105` and `src/main_gpui.rs:772-773` route presenter commands through a tokio `mpsc` receiver into a flume channel owned by `GpuiBridge`.
5. `src/ui_gpui/views/main_panel.rs:308-351` only drains that bridge in `ensure_bridge_polling(...)`, which requires `MainPanelAppState.popup_window` to be present.
6. `src/ui_gpui/views/main_panel.rs:460-479` starts runtime polling only from `start_runtime(...)`, after the popup window handle exists.
7. `src/ui_gpui/views/main_panel.rs:597-610` forwards runtime `ConversationMessagesLoaded` into `ChatView` after bridge delivery.
8. `src/ui_gpui/views/chat_view.rs:823-851` applies `ConversationMessagesLoaded` only when `state.active_conversation_id == Some(conversation_id)` and otherwise logs that the replay is ignored for an inactive conversation.

## File-Cited Failure Evidence

### `src/main_gpui.rs`

- `src/main_gpui.rs:174-293` proves startup correctness comes from synthesized bootstrap replay commands, not from a persistent application state store.
- `src/main_gpui.rs:780-800` proves those startup commands are injected into `MainPanelAppState` before popup runtime startup.
- `src/main_gpui.rs:85-105` plus `src/main_gpui.rs:772-773` show ordinary presenter output is delivered through the bridge pipeline instead of that startup bootstrap path.

### `src/ui_gpui/views/main_panel.rs`

- `src/ui_gpui/views/main_panel.rs:156-160` shows `MainPanelAppState` mixes `gpui_bridge`, `popup_window`, and `startup_commands` in one coordinator-owned struct.
- `src/ui_gpui/views/main_panel.rs:211-225` and `src/ui_gpui/views/main_panel.rs:228-306` show startup replay is applied inside `MainPanel::init(...)` before runtime polling starts.
- `src/ui_gpui/views/main_panel.rs:308-351` and `src/ui_gpui/views/main_panel.rs:460-479` show runtime command delivery is popup-coupled because bridge polling cannot start until a popup-mounted `MainPanel` has a window handle.
- `src/ui_gpui/views/main_panel.rs:493-810` shows `MainPanel::handle_command(...)` is also the dispatch hub for chat/history/settings/model/profile/MCP commands, not only chat transcript routing.
- Concretely, `MainPanel` is overloaded with:
  - popup lifecycle and window-handle coordination (`src/main_gpui.rs:568-620`, `src/ui_gpui/views/main_panel.rs:156-160`),
  - startup bootstrap replay (`src/ui_gpui/views/main_panel.rs:211-225`),
  - runtime bridge polling (`src/ui_gpui/views/main_panel.rs:308-351`),
  - root navigation ownership (`src/ui_gpui/views/main_panel.rs:167-180`, `src/ui_gpui/views/main_panel.rs:493-545`),
  - per-view command forwarding across chat/history/settings/model/profile/MCP subviews (`src/ui_gpui/views/main_panel.rs:493-810`).

### `src/ui_gpui/views/chat_view.rs`

- `src/ui_gpui/views/chat_view.rs:385-402` shows manual selection sets `active_conversation_id` locally before emitting `UserEvent::SelectConversation`, which means the view pre-optimistically changes selection state.
- `src/ui_gpui/views/chat_view.rs:823-851` shows `ConversationMessagesLoaded` is discarded whenever the incoming conversation id is not the currently active id.
- `src/ui_gpui/views/chat_view.rs:1078-1110` shows `ConversationActivated` mutates selection metadata and clears transient state, so selection/title updates can succeed independently of transcript replay application.

### `src/presentation/chat_presenter.rs`

- `src/presentation/chat_presenter.rs:100-144` shows the presenter also performs a startup activate-and-replay sequence, but that sequence still goes through runtime command delivery rather than the synchronous bootstrap replay used by `build_startup_view_commands(...)`.
- `src/presentation/chat_presenter.rs:710-753` shows runtime manual selection depends on `handle_select_conversation(...)` emitting transient commands in order, not on mutating an authoritative shared state object.
- `src/presentation/chat_presenter.rs:306-339` proves the runtime replay payload is materially the same kind of `ConversationMessagesLoaded` payload synthesized in startup bootstrap, which points to delivery/state ownership as the seam rather than missing conversation data.

## Architectural Conclusion

The codebase currently has two distinct assumptions for the same UI outcome:

- Startup correctness comes from synchronous bootstrap replay that is precomputed in `src/main_gpui.rs:174-293` and injected directly by `MainPanel::init(...)` (`src/ui_gpui/views/main_panel.rs:211-225`, `228-306`).
- Manual selection correctness depends on popup-coupled runtime delivery: views emit `SelectConversation`, the presenter emits transient commands, and `MainPanel` only forwards them while a popup-mounted bridge poll loop is running (`src/ui_gpui/views/main_panel.rs:308-351`, `460-479`).

That separation is the failure seam. Startup works because it bypasses the popup-coupled runtime path. Manual selection is fragile because transcript state is not recovered from an authoritative store; it is only correct if transient runtime commands arrive while `MainPanel` is mounted, polling, and the active-conversation guard in `ChatView` matches at delivery time.

## Recovery Implication

The plan is justified in moving to an authoritative store model. The existing evidence shows that adding more replay commands would preserve the same architectural fragility, while a shared authoritative state would let both startup hydration and runtime selection mutate the same source of truth and let popup views render snapshots from that state.
