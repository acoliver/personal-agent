# Phase 01 Artifact: Behavior Contract Inventory

This inventory exists so decomposition stays tied to observable behavior instead of arbitrary file splitting.

## Required test style for this plan

For GPUI refactoring, a qualifying behavioral safety-net test should:
- drive real GPUI state through `#[gpui::test]` / `TestAppContext`, real view state transitions, or real presenter/service boundaries
- assert user-visible state, emitted events, save payloads, or stable routing/state contracts
- avoid `include_str!()` source-text assertions unless the assertion is explicitly classified as a legitimate static contract

For pure refactoring, the primary safety-net rule is **green-before-move and green-after-move**:
1. the behavioral contract test passes before extraction
2. the same test passes after extraction
3. if new missing coverage is added, that new coverage may follow a red→green cycle, but that is separate from the extraction-safety proof

## `chat_view.rs`

Primary contracts to protect:
- conversation transcript loads replace the active transcript for the selected conversation
- incremental append/stream/finalize/cancel/error commands produce the correct visible message and streaming state
- thinking visibility toggles affect visible assistant-thinking blocks without corrupting transcript state
- conversation dropdown navigation and confirmation select the intended conversation
- profile dropdown navigation/selection updates selected profile and emits selection intent
- starting/stopping send while streaming honors the current stream state and does not double-send

Expected strong safety-net tests:
- `tests/chat_view_handle_command_tests.rs` (currently the primary external `chat_view` safety-net file and containing 9 `#[gpui::test]` tests that must be mapped to the required contracts)

Files that must not count toward the minimum:
- `tests/gpui_chat_view_tests.rs` (currently empty)
- `tests/chat_view_title_visibility_tests.rs`

Minimum expectation before extraction:
- at least 4 passing behavioral safety-net tests covering transcript replacement, stream lifecycle, thinking visibility, and one dropdown interaction
- verify those contracts are actually covered by the cited tests rather than assumed from file names alone

## `main_panel.rs`

Primary contracts to protect:
- `ViewCommand` routing reaches the correct child view or navigation target
- navigation state changes route the visible panel correctly
- store snapshots propagate to child views correctly
- profile/settings/API key command forwarding reaches the intended view family

Expected strong safety-net tests:
- new or expanded external GPUI behavioral tests created for this batch
- inline `#[gpui::test]` coverage inside `main_panel.rs`

Structural support tests that may assist but do not count by themselves:
- `route_view_command` / `CommandTargets` routing tests
- routing-counter assertions that do not prove visible state or child-view effects

Minimum expectation before extraction:
- at least 4 passing safety-net tests, with at least 2 of them proving behavioral outcomes rather than routing counters alone
- acknowledge that `tests/main_panel_state_tests.rs` is currently empty and therefore external safety-net coverage must be created before extraction

## `settings_view.rs`

Primary contracts to protect:
- profile summary updates preserve/fix selection correctly
- MCP updates preserve/fix selection correctly
- profile/MCP action helpers emit the correct user events
- profile and MCP sections reflect the correct visible state for empty/error/selected cases

Expected strong safety-net tests:
- `tests/settings_view_tests.rs`
- `tests/settings_view_mcp_saved_state_tests.rs`
- `tests/settings_panel_behavior_tests.rs`
- `tests/settings_view_display_tests.rs`

Minimum expectation before extraction:
- at least 3 passing behavioral safety-net tests covering selection fallback, command handling, and one emitted-event path

## `profile_editor_view.rs`

Primary contracts to protect:
- profile editor load fills visible state correctly
- save payload mapping preserves provider/auth/base-url/system-prompt semantics
- active-field editing and IME behavior modify the intended field only
- API key refresh/navigation behavior remains intact

Expected strong safety-net tests:
- `tests/profile_editor_view_tests.rs`
- `tests/profile_editor_save_payload_mapping_tests.rs`

Minimum expectation before extraction:
- at least 4 passing behavioral safety-net tests covering load mapping, save mapping, active-field editing, and API key refresh behavior

## `mcp_add_view.rs`

Primary contracts to protect:
- search result projection retains enough source information for follow-on actions
- input/search/select behavior obeys real state rules
- chosen result turns into the intended follow-on action or draft state

Expected strong safety-net tests:
- `tests/mcp_add_view_projection_tests.rs`
- inline `#[gpui::test]` coverage inside `mcp_add_view.rs`

Minimum expectation before extraction:
- at least 3 passing behavioral safety-net tests covering projection, search/input, and selection/action behavior

## `mcp_configure_view.rs`

Primary contracts to protect:
- can-save logic and auth/config validation remain correct
- draft loading and save mapping preserve visible editor state
- OAuth/auth state transitions preserve correct action availability

Expected strong safety-net tests:
- `tests/mcp_configure_view_can_save_tests.rs`
- inline `#[gpui::test]` coverage inside `mcp_configure_view.rs`

Minimum expectation before extraction:
- at least 3 passing behavioral safety-net tests covering validation, draft/load behavior, and auth/OAuth transitions

## `model_selector_view.rs`

Primary contracts to protect:
- provider/model search and selection behavior remain stable
- visible selection/filter state remains correct after command updates
- choosing a model produces the intended downstream profile/editor action

Expected strong safety-net tests:
- inline GPUI tests inside `model_selector_view.rs`

Minimum expectation before extraction:
- at least 3 passing behavioral safety-net tests covering filtering, selection, and downstream action behavior

## `api_key_manager_view.rs`

Primary contracts to protect:
- listing/storing/deleting keys updates visible state correctly
- edit mode and field behavior stay coherent
- save/delete actions emit the correct user events and preserve labels
- masking and paste behavior work through real state/input handling, not source-text grep

Expected strong safety-net tests:
- inline GPUI tests inside `api_key_manager_view.rs`
- presenter-backed behavior tests where appropriate

Minimum expectation before extraction:
- at least 3 passing behavioral safety-net tests covering edit state, save/delete flow, and mask/paste behavior

## `history_view.rs`

Primary contracts to protect if touched:
- conversation list refresh updates visible history state
- create/delete/activate flows update selection/list contents correctly
- refresh requests still emit correctly

Expected strong safety-net tests:
- `tests/history_view_tests.rs`
- inline GPUI tests inside `history_view.rs`

Minimum expectation before extraction:
- at least 2 passing behavioral safety-net tests if `history_view.rs` is touched
