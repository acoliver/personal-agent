# Phase 03: Settings and Editor Family Refactor

## Phase ID

`PLAN-20260325-ISSUE11B.P03`

## Objective

Refactor the large settings/editor-style GPUI files into maintainable modules with strong behavioral tests.

## Baseline debt driving this phase

- `profile_editor_view.rs` — `1875` lines
- `mcp_add_view.rs` — `1696` lines
- `settings_view.rs` — `1485` lines
- lizard hotspots:
  - `settings_view.rs::handle_command` — 123 length
  - `settings_view.rs::render_profiles_section` — 143 length
  - `settings_view.rs::render_mcp_section` — 140 length
  - `mcp_add_view.rs` has long test code and large render/input surface
  - `profile_editor_view.rs` has large field/render/input sections even when individual functions are less obviously over threshold

## Grounded decomposition targets

### `settings_view.rs`
Likely seams based on the real file:
- profile projection / selection helpers
- MCP projection / selection helpers
- command handling reducer branches
- profiles section rendering
- MCP section rendering
- top-level action bar / refresh controls

### `profile_editor_view.rs`
Likely seams based on the real file:
- editable data model and normalization helpers
- active-field editing helpers / input handler behavior
- save payload mapping
- provider/API type handling
- individual field/section renderers

### `mcp_add_view.rs`
Likely seams based on the real file:
- search state and result projection
- active field/input handling
- search-result selection and command mapping
- section rendering / footer actions

## Required test-first work

Before moving code in this phase:
- extend existing strong suites such as `tests/settings_view_tests.rs`, `tests/settings_view_mcp_saved_state_tests.rs`, `tests/profile_editor_view_tests.rs`, `tests/profile_editor_save_payload_mapping_tests.rs`, and `tests/mcp_add_view_projection_tests.rs`
- prefer GPUI behavior tests and mapping tests that verify user-visible state, save payloads, validation, and selection behavior
- inventory and remediate `include_str!()` tests touching `profile_editor_view.rs`

Known source-text dependencies include:
- `tests/model_profile_flow_regression_tests.rs`
- `tests/api_key_manager_ui_regression_tests.rs`
- `tests/seven_bugs_regression_tests.rs`

## Public API / import guardrails

- keep `views/mod.rs` exports stable where practical for `SettingsState`, `ProfileItem`, `McpItem`, `ProfileEditorData`, `ApiType`, `AuthMethod`, `McpAddState`, `McpRegistry`, `McpSearchResult`, and `SearchState`
- if internals move into submodules, re-export from the original file/module path unless a deliberate consumer update is clearly better

## Success Criteria

- `profile_editor_view.rs`, `mcp_add_view.rs`, and `settings_view.rs` are each `<= 1000` lines after the batch, with `<= 750` as the target where practical
- no newly created extracted `.rs` file exceeds `750` lines without written justification
- touched functions satisfy lizard `-L 100` and `-C 50`
- save/edit/search/select flows are protected by behavioral tests added first
- source-text dependencies were intentionally remediated
- full batch verification passes, including `cargo coverage`
