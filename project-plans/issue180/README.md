# Issue #180 — Unify popin History view and popout sidebar into a single ConversationList component

This plan implements the refactor in three phases. Tests are migrated/written FIRST in each phase, code follows. The full verification suite is run at every phase boundary so we can stop and fix at the smallest possible diff.

## Goal

Replace the parallel `history_view.rs` (popin) and `chat_view/render_sidebar.rs` (popout) with a single `ConversationListView` shared component used in two modes:

- `Inline` — embedded in the chat sidebar (popout). Click selects only.
- `FullPanel` — wrapped by `HistoryPanelView` (popin). Click selects AND navigates back to Chat. Container provides the Back button.

Sidebar wins as the base implementation: it has search, inline rename, inline delete confirmation, streaming indicator, emoji-filter compliance, and preview rows. The popin loses its current "card grid + Load/Delete buttons" look in favor of the shared component.

## Acceptance criteria (from issue)

- [ ] Single `ConversationListView` used in both modes.
- [ ] All sidebar features (search, rename, inline delete, streaming dot) available in both modes.
- [ ] Popin click: select + navigate to Chat. Popout click: select only.
- [ ] Popout sidebar width unchanged (260px container-controlled).
- [ ] Popin view fills its panel (container-controlled).
- [ ] Popin keeps a Back button on the popin container.
- [ ] Concurrent streams: per-row indicator highlights right rows in both modes.
- [ ] `history_view.rs` deleted.
- [ ] All snapshot/projection tests from `history_view.rs` migrated and passing.
- [ ] All existing UI/automation tests pass.

## Module layout

```
src/ui_gpui/views/
  conversation_list/
    mod.rs              # ConversationListView, ConversationListMode, public API
    state.rs            # ConversationListState (with cfg(test) unit tests)
    render.rs           # All visual rendering (header, list, item, search, delete)
    history_panel.rs    # HistoryPanelView (FullPanel container with Back button)
  chat_view/
    render_sidebar.rs   # Reduced to a thin wrapper that embeds the entity in Inline mode
  history_view.rs       # DELETED
```

## ConversationListMode

```rust
pub enum ConversationListMode { Inline, FullPanel }
```

Single behavioural branch in row click handlers:

```rust
selection_intent_channel().request_select(conv_id);
if matches!(self.mode, ConversationListMode::FullPanel) {
    navigation_channel().request_navigate(ViewId::Chat);
}
```

## ConversationListState fields (extracted from ChatState)

- `conversations: Vec<ConversationSummary>` — moved out of ChatState
- `active_conversation_id: Option<Uuid>` — moved out of ChatState
- `streaming_conversation_ids: HashSet<Uuid>` — moved out of ChatState
- `sidebar_search_query: String`
- `sidebar_search_focused: bool`
- `sidebar_search_results: Option<Vec<ConversationSearchResult>>`
- `delete_confirming_id: Option<Uuid>`
- `conversation_title_editing: bool`
- `conversation_title_input: String`
- `rename_replace_on_next_char: bool`

ChatState **keeps** the conversation_dropdown_* fields (those belong to the title-bar dropdown, not the list) and the `conversation_title` (display, sourced from selected summary). It uses Entity<ConversationListView> for any list-related state, and reads conversation/active id through the entity when needed for non-list behaviour (escape stops streaming, Cmd+N clears, etc.).

To minimise blast radius the ChatView retains read-through accessors that delegate to the embedded entity (e.g. `active_conversation_id()` → reads from entity). Where ChatView previously mutated `state.active_conversation_id`, the mutation is forwarded to the entity instead.

## Phases

### Phase 1 — Foundation (test-first)

1. Create `conversation_list/{mod.rs, state.rs, render.rs}` skeleton.
2. Define `ConversationListMode` and `ConversationListState` in `state.rs`. Implement `Default`, `new`, builders, and `apply_history_snapshot`.
3. Migrate the 3 tests from `history_view.rs` (snapshot projection, transitions, and conversation-cleared) to `state.rs` `#[cfg(test)]`. They must compile and pass against `ConversationListState`.
4. Add minimal stub `ConversationListView` so the module compiles. No render logic yet.
5. Run `cargo build --tests` + the new tests.

### Phase 2 — Shared component (test-first)

1. Add `ConversationListView` with `mode`, `state`, `bridge`, `focus_handle`, `scroll_handle`. Implement `new(mode, cx)`, `set_bridge`, `apply_store_snapshot`, `Focusable`.
2. Port rendering helpers from `render_sidebar.rs` to `render.rs`: header, list, item, search-result item, delete-confirm UI, streaming dot, format helpers, group label, selection colors. Preserve `SIDEBAR_TITLE_LEADING_INDENT`.
3. Implement `Render` for `ConversationListView`: header → scrollable list. NO own width. Click handlers call selection + (FullPanel only) navigation.
4. Port interaction methods: `start_rename_conversation`, `submit_rename_conversation`, `cancel_rename_conversation`, `handle_rename_backspace`, `trigger_sidebar_search`, `clear_search`, plus accessors for ChatView IME (`active_input_text`, `active_cursor_position`).
5. Add unit tests for `trigger_sidebar_search` (empty/non-empty), rename flow (start/submit/cancel), and `apply_store_snapshot` propagation.

### Phase 3 — Integration & cleanup (test-first)

1. Create `HistoryPanelView` in `conversation_list/history_panel.rs`. Holds `Entity<ConversationListView>`. Renders `[component] [Back bar at bottom]`. `apply_store_snapshot` and `set_bridge` are forwarded.
2. Wire ChatView to embed `Entity<ConversationListView>` (Inline mode). Reduce `render_sidebar.rs` to a thin wrapper rendering `[260px column { entity }]`. Forward IME, key down, paste, search, rename, delete state through accessors.
3. Move/delegate ChatView IME paths (`replace_text_in_range`, key handling, `handle_paste`, `active_input_text/cursor`) to read/write the embedded entity instead of `ChatState` fields.
4. Update `MainPanel` to swap `history_view: Option<Entity<HistoryView>>` → `history_panel: Option<Entity<HistoryPanelView>>`. Forward snapshots and bridge.
5. Delete `src/ui_gpui/views/history_view.rs`. Remove its module declaration & re-exports.
6. Delete or archive `tests/history_view_tests.rs` — its content is migrated.
7. Run full verification (fmt, clippy `-D warnings`, `cargo test --lib --tests`, structural/lizard).

## Test strategy

- Unit tests live with the module they cover.
- Existing `tests/history_view_tests.rs` are migrated to `conversation_list::state` tests where they describe state, and to a new `conversation_list/render_tests.rs`-style test where they describe view interaction.
- Existing chat_view tests for sidebar search remain valid; they will be re-routed through the entity (or kept on ChatView as integration tests against the embedded entity).
- Add new tests for `ConversationListMode::FullPanel` click → selection + navigation request.

## Open questions / decisions

- **Width control**: per the issue, the popout sidebar wraps the entity in a 260px container; popin wraps in a full-panel container. The shared component never sets its own width.
- **ChatState mutation churn**: rather than rewrite every site that reads `state.active_conversation_id`, we keep a forwarding accessor `ChatView::active_conversation_id() -> Option<Uuid>` and a setter that writes through to the embedded entity. This keeps Phase 3 surgical.
- **Bridge plumbing**: `ChatView::new` cannot create the entity (it doesn't have a `Context<ChatView>` for the child). We create the entity in `ChatView::new(state, cx)` using `cx.new(...)` since ChatView's `Context<ChatView>` exposes `cx.new`. (Verified pattern used by `MainPanel::init` for child entities.)
- **MainPanel observation**: keep an `observe_in` for the new `history_panel` like the existing one for `chat_view`.

## Done definition

- All acceptance criteria met.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --tests`, structural/lizard checks all green.
- PR opened with `Fixes #180`.
- CI green; CodeRabbit comments addressed.
- App launches manually for user testing.
