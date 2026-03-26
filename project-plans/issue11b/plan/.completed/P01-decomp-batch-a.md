# P01 Decomposition Map — Batch A: chat_view.rs + main_panel.rs

## Source files

| File | Lines |
|------|-------|
| `src/ui_gpui/views/chat_view.rs` | 2527 |
| `src/ui_gpui/views/main_panel.rs` | 2019 |

---

## `chat_view.rs` — Decomposition Map

### Code Groups with Exact Line Ranges

| Group | Lines | Description |
|-------|-------|-------------|
| Imports / file header | 1–25 | `use` statements, module doc |
| Data types: `ChatMessage`, `MessageRole` | 26–75 | Public message model, constructors, builders |
| Data types: `StreamingState`, `ChatState` | 77–238 | Streaming enum, full state struct + `impl Default` |
| `ChatState` impl (pure state helpers) | 139–236 | `selected_conversation`, `sync_*` helpers, `add_message`, `set_*` |
| `ChatView` struct | 237–244 | Struct fields |
| `ChatView` impl — construction + bridge | 245–312 | `new`, `refresh_autoscroll_state_*`, `maybe_scroll_chat_to_bottom`, `messages_from_payload`, `streaming_state_from_snapshot`, `set_bridge` |
| `ChatView` impl — store application | 313–403 | `apply_store_snapshot`, `apply_settings_snapshot`, `set_conversation_id` |
| `ChatView` impl — conversation dropdown | 404–572 | `emit`, `current_or_active_conversation_id`, `select_conversation_at_index`, `toggle_conversation_dropdown`, `move_conversation_dropdown_selection`, `confirm_conversation_dropdown_selection`, `select_conversation_by_id`, `start_rename_conversation`, `submit_rename_conversation`, `cancel_rename_conversation`, `handle_rename_backspace`, `conversation_dropdown_open`, `conversation_title_editing` |
| `ChatView` impl — profile dropdown | 573–708 | `select_profile_at_index`, `toggle_profile_dropdown`, `profile_dropdown_open`, `active_input_text`, `active_cursor_position`, `move_profile_dropdown_selection`, `confirm_profile_dropdown_selection` |
| `ChatView` impl — input/clipboard/cursor | 709–848 | `handle_paste`, `handle_select_all`, `move_cursor_left`, `move_cursor_right`, `move_cursor_home`, `move_cursor_end`, `handle_backspace`, `handle_enter` |
| `handle_command` | 849–1245 | Full command dispatch — 396 lines, covers all `ViewCommand` variants |
| `render_top_bar` | 1246–1389 | 144 lines — toolbar buttons, history/settings/exit navigation |
| `render_title_bar` | 1390–1577 | 188 lines — conversation dropdown row, profile dropdown row |
| `render_conversation_dropdown` | 1578–1678 | Floating overlay + scrollable menu |
| `render_profile_dropdown` | 1679–1776 | Floating profile menu overlay |
| `render_chat_area` | 1777–1850 | Scroll container, message list composition |
| `render_message` / `render_user_message` / `render_assistant_message` / `render_thinking_block` | 1851–1963 | Per-message sub-renderers |
| `render_input_bar` | 1964–2124 | 161 lines — text field, send/stop button, IME canvas |
| `impl Focusable` | 2125–2146 | Single method |
| `impl EntityInputHandler` (IME) | 2147–2299 | Full IME protocol: `text_for_range`, `selected_text_range`, `marked_text_range`, `unmark_text`, `replace_text_in_range`, `replace_and_mark_text_in_range`, `bounds_for_range`, `character_index_for_point` |
| `impl Render` | 2300–2527 | Root render + `on_key_down` handler |
| No `#[cfg(test)]` module | — | `chat_view.rs` has **no inline test module** |

### Proposed Extraction Targets (3 files)

#### 1. `chat_state.rs`
**Contents:**
- `ChatMessage` (lines 26–75)
- `MessageRole` (lines 36–40)
- `StreamingState` (lines 77–85)
- `ChatState` struct + `impl Default` + full `impl ChatState` (lines 86–236)
- `messages_from_payload` (lines ~278–312, currently on `ChatView` impl but operates purely on payload types)
- `streaming_state_from_snapshot` (lines ~313–329, same reasoning)

**Why:** These are all pure data and state-transition logic. They have no GPUI rendering, no bridge, no scroll handles. They are the contracts that tests already exercise and that new tests should protect before extraction. Extracting to `chat_state.rs` reduces `chat_view.rs` by ~250 lines and gives the state model its own coherent home.

**Re-exports needed from `chat_view.rs`:**
```rust
pub use chat_state::{ChatMessage, ChatState, MessageRole, StreamingState};
```
`mod.rs` currently re-exports `ChatState, ChatView` from `chat_view` — `ChatState` stays re-exported from `chat_view` via the above chain; no `mod.rs` change needed.

#### 2. `chat_render.rs`
**Contents:**
- All `render_*` private methods: `render_top_bar` (1246–1389), `render_title_bar` (1390–1577), `render_conversation_dropdown` (1578–1678), `render_profile_dropdown` (1679–1776), `render_chat_area` (1777–1850), `render_message` / `render_user_message` / `render_assistant_message` / `render_thinking_block` (1851–1963), `render_input_bar` (1964–2124)
- `impl gpui::Render for ChatView` (2300–2527) — delegates to the extracted helpers

**Why:** Render subtrees are compositional and pure output. They reference `self.state` and `Theme` but have no command-handling or event-emission logic. Grouping them together eliminates the largest line-count contributor and makes `chat_view.rs` purely about coordination. The `impl Render` block must stay accessible so it stays in this file (or as an `impl ChatView` extension block imported from here).

**Implementation note:** Rust `impl` blocks for a type can be split across files within the same crate only via `impl SomeView { … }` in a file that `use`s the type. The canonical pattern is to put helper methods in `impl ChatView { … }` blocks in the extracted file and `mod chat_render; use chat_render::*` (or `pub(super)`) in `chat_view.rs`. Since all render methods are `fn` items on the `ChatView` impl, the extracted file declares `impl ChatView { … render_* … }` and the root `impl Render` block remains in `chat_view.rs` calling `self.render_*()`. This is idiomatic for GPUI views in this codebase.

**Re-exports needed from `chat_view.rs`:** None — all render methods are private/crate-internal.

#### 3. `chat_ime.rs`
**Contents:**
- `impl gpui::EntityInputHandler for ChatView` (lines 2147–2299) — all IME protocol methods
- `impl gpui::Focusable for ChatView` (lines 2125–2146)

**Why:** The IME handler is a self-contained protocol implementation. It involves only UTF-8/UTF-16 index arithmetic and `self.state.input_text` / `self.state.marked_range`. Isolating it removes ~175 lines of boilerplate from the main file and makes `chat_view.rs` the coordination hub it should be. This follows the same pattern used in `api_key_manager_view.rs` where the IME implementation is already well-understood.

**Re-exports needed:** None — these are `impl Trait for ChatView` blocks.

### Post-extraction `chat_view.rs` estimated size
Original: 2527 lines
- Remove to `chat_state.rs`: ~250 lines
- Remove to `chat_render.rs`: ~950 lines
- Remove to `chat_ime.rs`: ~175 lines
- Remaining in `chat_view.rs`: ~1150 lines

That alone is not yet ≤750. A second-pass within Batch A can address `handle_command` (396 lines) by extracting a `chat_command_reducer.rs` carrying the command dispatch logic. However that fourth extraction requires scope-creep guard review — see notes below.

**Scope-creep guard note:** 3 extractions keep us within the plan guard. The `handle_command` function is the remaining hotspot (396 lines) and is a candidate for an additional extraction in a focused follow-on batch step, not a new separate file in this phase. The guard counts newly created `.rs` files; the plan allows up to 3 per decomposition. Four extractions would require an explicit scope update.

### Inline Tests — Decision
`chat_view.rs` has **no** `#[cfg(test)]` module. Decision: N/A.

### `include_str!()` Tests Referencing `chat_view.rs`

| Test file | Line | Assertion content | Planned disposition |
|-----------|------|-------------------|---------------------|
| `chat_startup_scrollback_layout_regression_tests.rs` | 5 | `ChatView` export/type presence | **Narrow**: if the type is still re-exported from the same path, this assertion remains valid; confirm re-export chain after extraction |
| `chat_view_conversation_switch_regression_tests.rs` | 5, 22, 40, 95 | Transcript replacement ordering / source snippets | **Replace** with behavioral tests in `chat_view_handle_command_tests.rs` covering `ConversationMessagesLoaded` ordering and `ConversationActivated` state reset |
| `seven_bugs_regression_tests.rs` | 242, 262, 280, 441, 482 | Bug 2 chat dropdown overlay, bug 3 profile dropdown, bug 5 finalize-stream, bug 7 model-editable | **Replace** with behavioral GPUI tests; dropdown overlay placement → `render_conversation_dropdown` behavioral test; finalize-stream → `handle_command(FinalizeStream)` behavioral test |

### Safety-Net Tests (Green-Before-Move)

**Existing tests covering `chat_view.rs` contracts:**

| Test | File | Count | Contract covered |
|------|------|-------|-----------------|
| `chat_view_handle_command_tests.rs` | `tests/` | 9 `#[gpui::test]` | Command dispatch, streaming, conversation selection, thinking toggle |
| `chat_rename_overwrite_behavior_tests.rs` | `tests/` | 4 tests | Rename/overwrite flow |
| `chat_view_title_visibility_tests.rs` | `tests/` | 5 tests (all `assert!(true)`) | **ZERO VALUE** — must not count |
| `conversation_rename_behavior_tests.rs` | `tests/` | 4 tests | Rename behavior |
| `conversation_title_behavior_tests.rs` | `tests/` | 5 tests | Title sync behavior |
| `new_conversation_behavior_tests.rs` | `tests/` | 5 tests | New conversation flow |
| `regression_hardening_preserved_behaviors_tests.rs` | `tests/` | 10 tests | Various regression guards |
| Inline tests | `chat_view.rs` | 0 | — |

**Contracts confirmed covered (minimum 4 required before extraction):**
1. [OK] Transcript replacement (`ConversationMessagesLoaded`) — `chat_view_handle_command_tests.rs`
2. [OK] Stream lifecycle (`AppendStream`, `FinalizeStream`, `StreamCancelled`) — `chat_view_handle_command_tests.rs`
3. [OK] Thinking visibility toggle — `chat_view_handle_command_tests.rs`
4. [OK] Conversation dropdown selection — `chat_view_handle_command_tests.rs` / `conversation_rename_behavior_tests.rs`

**Gaps that need new tests BEFORE extraction:**
- `ChatState` pure helper methods (`sync_conversation_title_from_active`, `sync_profile_dropdown_index`, `selected_conversation`, `selected_profile`) — none of these are directly exercised by name in external tests; they are covered only indirectly. Adding 2–3 unit tests for `ChatState` in `chat_state.rs` after extraction (or in the existing test infra before) is required.
- `apply_settings_snapshot` profile/model sync — not directly covered; needs a dedicated behavioral test.

---

## `main_panel.rs` — Decomposition Map

### Code Groups with Exact Line Ranges

| Group | Lines | Description |
|-------|-------|-------------|
| Imports | 1–19 | Core use statements (trimmed set before second use block) |
| `mod _actions` + `pub use` | 20–31 | GPUI action definitions: `NavigateToHistory`, `NavigateToSettings`, `NewConversation`, `NavigateBack` |
| `CommandTargets` struct | 46–83 | Observable counter struct for routing tests |
| `route_view_command` | 84–179 | 96-line free function — the routing matrix, dispatches `ViewCommand` variants to target counters |
| Second use block (child view imports) | 158–179 | All child view `use` lines (inside same file, before structs) |
| `MainPanelAppState` | 180–188 | Global app state struct holding bridge, popup window, app store |
| `MainPanel` struct | 190–210 | Struct fields — all child view `Entity<T>` handles, navigation, tasks, subscriptions |
| `impl MainPanel` — construction/snapshot | 212–270 | `new`, `apply_store_snapshot` |
| `impl MainPanel` — subscription/startup | 271–518 | `ensure_store_subscription`, `request_runtime_snapshots`, `apply_startup_state`, `init`, `ensure_bridge_polling`, `maybe_start_test_conversation_switch`, `is_initialized`, `is_runtime_started`, `start_runtime` |
| `current_view` | 544–553 | Simple accessor |
| `handle_command` | 554–1202 | 649-line method, routes all `ViewCommand` variants to child views, handles navigation |
| `impl Focusable` | 1203–1208 | Single method |
| `impl Render` | 1209–1442 | 234-line render — view routing, overlay rendering, keyboard dispatch |
| `#[cfg(test)]` module | 1443–2019 | 577 lines, 6 `#[gpui::test]` tests, test helpers (`build_app_state`, `conversation_summary`, `profile_summary`, `transcript_message`, `remote_model`, `registry_result`) |

### Proposed Extraction Targets (3 files)

#### 1. `main_panel_routing.rs`
**Contents:**
- `mod _actions` + `pub use _actions::*` (lines 20–31)
- `CommandTargets` struct (lines 46–83)
- `route_view_command` free function (lines 84–179)

**Why:** `route_view_command` is called out explicitly in the plan as "an especially strong early extraction candidate because it already has a standalone signature." It has zero GPUI context dependencies — it takes only a `ViewCommand` and a `&mut CommandTargets`. Extracting it removes ~160 lines from `main_panel.rs` and gives the routing matrix its own testable module. The GPUI actions are tightly coupled to this routing surface and belong in the same module.

**Re-exports needed from `main_panel.rs`:**
```rust
pub use main_panel_routing::{CommandTargets, route_view_command};
pub use main_panel_routing::*;  // for actions
```

**Re-exports needed from `mod.rs`:** `CommandTargets` and `route_view_command` are not currently in `mod.rs` re-exports; no `mod.rs` changes required.

#### 2. `main_panel_startup.rs`
**Contents:**
- `MainPanelAppState` struct + `impl Global` (lines 180–188)
- `ensure_store_subscription` (lines 271–292)
- `request_runtime_snapshots` (lines 293–304)
- `apply_startup_state` (lines 305–311)
- `init` (lines 312–401)
- `ensure_bridge_polling` (lines 402–409)
- `maybe_start_test_conversation_switch` (lines 410–518)
- `is_initialized`, `is_runtime_started`, `start_runtime` (lines 503–543)

**Why:** These methods are entirely about lifecycle and startup orchestration — bridge setup, store subscription, runtime start, bridge polling, and the test conversation switch experiment. None of these touch the rendering path or direct command handling. Grouping them lets the startup surface be understood and tested independently. `MainPanelAppState` belongs here because it is consumed only during startup.

**Re-exports needed from `main_panel.rs`:**
```rust
pub use main_panel_startup::MainPanelAppState;
```

#### 3. `main_panel_render.rs`
**Contents:**
- `impl Render for MainPanel` (lines 1209–1442) — all render logic, view routing render, key dispatch, overlay rendering

**Why:** The `render` function is 234 lines and contains significant conditional logic for routing the active view, rendering overlays, and dispatching keyboard events. Extracting it mirrors the `chat_render.rs` pattern and removes one of the two remaining lizard hotspots (`main_panel.rs::render` at 230 length). The `handle_command` function remains in `main_panel.rs` for now; a follow-on step within the batch can further decompose its branches if lines allow.

**Re-exports needed:** None — `impl Render for MainPanel` is a trait implementation.

### Post-extraction `main_panel.rs` estimated size
Original: 2019 lines
- Remove to `main_panel_routing.rs`: ~160 lines
- Remove to `main_panel_startup.rs`: ~260 lines
- Remove to `main_panel_render.rs`: ~234 lines
- Remove inline test module to keep or move: 577 lines (see below)
- Remaining in `main_panel.rs`: ~788 lines

With the test module staying inline the file will still exceed 750. The test helpers and test functions are the dominant mass. The plan allows the inline tests to **move with the extracted module they most depend on** — the startup and routing tests go to their respective extracted files, and the render/command tests can stay inline or be externalized to `main_panel_state_tests.rs` (which must be populated before extraction in any case).

### Inline Tests — Decision
`main_panel.rs` has 6 `#[gpui::test]` tests in lines 1443–2019 (577 lines total including helpers):
- `init_and_startup_state_seed_child_views_from_store` → tests startup behavior → **move with `main_panel_startup.rs`** (needs private access to startup state)
- `start_runtime_requires_popup_window_before_emitting_refreshes` → tests runtime start → **move with `main_panel_startup.rs`**
- `ensure_store_subscription_only_subscribes_once_and_applies_published_updates` → tests subscription → **move with `main_panel_startup.rs`**
- `handle_command_navigates_and_forwards_model_results_to_real_selector` → tests command handling → **externalize to `tests/main_panel_state_tests.rs`** (public interface)
- `handle_command_forwards_registry_results_and_errors_to_real_mcp_add_view` → tests command handling → **externalize to `tests/main_panel_state_tests.rs`**
- `handle_command_forwards_settings_profiles_and_routes_mcp_commands_to_expected_targets` → tests command handling → **externalize to `tests/main_panel_state_tests.rs`**

Test helper functions (`build_app_state`, `conversation_summary`, `profile_summary`, `transcript_message`, `remote_model`, `registry_result`) must **move with their primary consumer** — startup tests get their helpers, externalized tests get theirs into the external file.

### `include_str!()` Tests Referencing `main_panel.rs`

| Test file | Line | Assertion content | Planned disposition |
|-----------|------|-------------------|---------------------|
| `chat_startup_scrollback_layout_regression_tests.rs` | 20, 41 | `min_h_0` layout assertion, scrollback presence | **Narrow**: check if `min_h_0` assertion targets a stable architectural contract; if the layout call moves with `main_panel_render.rs` the assertion path breaks — replace with behavioral scroll/layout test |
| `api_key_manager_ui_regression_tests.rs` | 66, 76 | `ApiKeyManagerView` presence / navigation routing in `main_panel.rs` | **Replace** with behavioral test verifying navigation to API key manager view from main panel command |
| `gpui_popup_independence_tests.rs` | 262 | `main_panel.rs` source — popup independence | **Replace**: popup independence should be a behavioral test proving that popups do not share state with the main panel; if any static assertion survives it must target a named architectural contract, not raw source |

### Safety-Net Tests (Green-Before-Move)

**Existing tests covering `main_panel.rs` contracts:**

| Test | File | Count | Contract covered |
|------|------|-------|-----------------|
| Inline tests | `main_panel.rs` | 6 `#[gpui::test]` | Startup seeding, store subscription, runtime start, command routing to child views |
| `gpui_integration_tests.rs` | `tests/` | 8 tests | Integration wiring |
| `gpui_popup_independence_tests.rs` | `tests/` | 5 tests | Popup behavior |
| `main_panel_state_tests.rs` | `tests/` | **0 tests — empty file** | **Does not count** |
| `gpui_wiring_command_routing_tests.rs` | `tests/` | **0 tests — empty file** | **Does not count** |

**Contracts confirmed covered (minimum 4 required, at least 2 behavioral):**
1. [OK] Startup state seeding of child views — inline `main_panel.rs` test (behavioral: asserts child view state after snapshot)
2. [OK] Runtime start requires popup window — inline test (behavioral outcome)
3. [OK] Store subscription applied once — inline test (behavioral state outcome)
4. [OK] `handle_command` routes to `ModelSelectorView` — inline behavioral test
5. [OK] `handle_command` routes to `McpAddView` — inline behavioral test

**Gaps requiring new tests BEFORE extraction:**
- `main_panel_state_tests.rs` is empty and must be populated with at least 3 behavioral tests covering `ViewCommand` → child-view forwarding before any code is moved out of `handle_command`
- `route_view_command` has a standalone signature but its live behavioral contract (child view state after dispatch) is not tested from outside the inline module; externalized routing tests must be added to `gpui_wiring_command_routing_tests.rs` before extraction of `route_view_command`

---

## mod.rs Re-export Impact (Batch A)

Current `mod.rs` exports from `chat_view`:
```rust
pub use chat_view::{ChatState, ChatView};
```

After Batch A extraction, `chat_view.rs` must re-export `ChatState` (and `ChatMessage`, `MessageRole`, `StreamingState`) from `chat_state.rs`. The `mod.rs` line remains unchanged if the re-export chain is correct.

Current `mod.rs` exports from `main_panel`:
```rust
pub use main_panel::MainPanel;
```

After Batch A extraction, `MainPanel` is still defined in `main_panel.rs`. `CommandTargets` and `route_view_command` are not currently re-exported from `mod.rs` so no `mod.rs` change is required for those.

`MainPanelAppState` is not currently in `mod.rs` re-exports. After extraction to `main_panel_startup.rs`, `main_panel.rs` re-exports it to preserve existing usages (check import sites before confirming).
