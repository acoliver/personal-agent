# P01 Decomposition Map — Batch B: settings_view.rs + profile_editor_view.rs + mcp_add_view.rs

## Source files

| File | Lines |
|------|-------|
| `src/ui_gpui/views/settings_view.rs` | 1485 |
| `src/ui_gpui/views/profile_editor_view.rs` | 1875 |
| `src/ui_gpui/views/mcp_add_view.rs` | 1696 |

---

## `settings_view.rs` — Decomposition Map

### Code Groups with Exact Line Ranges

| Group | Lines | Description |
|-------|-------|-------------|
| Imports | 1–17 | Use statements |
| Data types: `ProfileItem`, `McpStatus`, `McpItem` | 18–110 | Public data models for settings list items |
| `SettingsState` struct + impl | 111–130 | State container, `new()` |
| `SettingsView` struct | 131–136 | View struct fields |
| `impl SettingsView` — construction + bridge | 138–156 | `new`, `set_bridge` |
| `impl SettingsView` — profile projection/selection | 157–242 | `set_profiles`, `apply_profile_summaries`, `selected_profile_index`, `select_profile_by_index`, `scroll_profiles`, `select_profile` |
| `impl SettingsView` — profile action helpers | 243–270 | `delete_selected_profile`, `edit_selected_profile`, `navigate_to_chat`, `navigate_to_profile_editor` |
| `impl SettingsView` — MCP projection/selection | 271–298 | `toggle_mcp`, `select_mcp`, `delete_selected_mcp`, `navigate_to_mcp_add`, `edit_selected_mcp` |
| `impl SettingsView` — key handler | 299–334 | `handle_key_down`, `emit` |
| `handle_command` | 335–460 | 126-line command dispatch — profile/MCP list refresh, navigation reactions |
| `render_top_bar` | 461–535 | 75-line top bar — back button, title, action buttons |
| `render_profile_row` | 536–574 | Single profile list row |
| `render_profiles_section` | 575–720 | 146-line section — add button, empty state, profile list with scroll |
| `render_mcp_row` | 721–806 | 86-line MCP list row |
| `render_mcp_section` | 807–949 | 143-line section — add button, empty state, MCP list |
| `render_hotkey_section` | 950–982 | Keyboard shortcut reference panel |
| `impl Focusable` | 983–988 | Single method |
| `impl Render` | 989–1028 | Root render, assembles sections |
| `#[cfg(test)]` module | 1029–1485 | 457 lines, 4 `#[gpui::test]` tests, helpers |

### Proposed Extraction Targets (3 files)

#### 1. `settings_state.rs`
**Contents:**
- `ProfileItem` struct + impl (lines 18–63)
- `McpStatus` enum (lines 64–72)
- `McpItem` struct + impl (lines 73–110)
- `SettingsState` struct + impl (lines 111–130)
- `apply_profile_summaries` (lines 157–192) — pure data-mapping helper that belongs with state, not with the view
- `select_profile_by_index` and `scroll_profiles` (lines 216–242) — pure index-arithmetic on state

**Why:** These are the stable public data types that appear in `mod.rs` re-exports (`McpItem`, `McpStatus`, `ProfileItem`, `SettingsState`). Extracting them to a sibling module lets them be tested, re-used, and imported without pulling in the full GPUI render tree. `apply_profile_summaries` is a pure projection — it maps incoming `ProfileSummary` slices to `ProfileItem` lists and preserves selection; it has no `cx` argument and no rendering side effects.

**Re-exports needed from `settings_view.rs`:**
```rust
pub use settings_state::{McpItem, McpStatus, ProfileItem, SettingsState};
```
**Re-exports needed from `mod.rs`:** none — `mod.rs` already imports these from `settings_view`; the chain stays intact.

#### 2. `settings_command.rs`
**Contents:**
- `handle_command` method body (lines 335–460) — extracted as `impl SettingsView { pub fn handle_command(…) }` block

**Why:** At 126 lines, `handle_command` is the primary lizard hotspot (`-L 100` violation). Its logic is entirely about reacting to `ViewCommand` variants by updating state and triggering `cx.notify()` — it has no render output. Extracting it as a separate `impl SettingsView` block lets it be reviewed and tested in isolation from the render tree.

**Implementation note:** Same Rust split-impl pattern as Batch A — declare `impl SettingsView { pub fn handle_command(…) }` in `settings_command.rs` and `mod settings_command;` in `settings_view.rs`.

**Re-exports needed:** None — `handle_command` remains as a method on `SettingsView`.

#### 3. `settings_render.rs`
**Contents:**
- `render_top_bar` (lines 461–535)
- `render_profile_row` (lines 536–574)
- `render_profiles_section` (lines 575–720) — 146 lines, lizard `-L 100` violation
- `render_mcp_row` (lines 721–806)
- `render_mcp_section` (lines 807–949) — 143 lines, lizard `-L 100` violation
- `render_hotkey_section` (lines 950–982)
- `impl Render for SettingsView` (lines 989–1028)

**Why:** The three oversized render functions (`render_profiles_section`, `render_mcp_section`) are responsible for the file length problem. Grouping all render logic in one extracted file removes ~570 lines from the root file and cleanly separates visual composition from data handling.

**Re-exports needed:** None — all render functions are private `impl SettingsView` methods.

### Post-extraction `settings_view.rs` estimated size
Original: 1485 lines
- Remove to `settings_state.rs`: ~170 lines
- Remove to `settings_command.rs`: ~130 lines (including surrounding impl block)
- Remove to `settings_render.rs`: ~590 lines
- Remaining in `settings_view.rs` (struct, construction, projection helpers, action helpers, key handler, inline tests): ~595 lines

Target: ≤750 lines. The inline test module (457 lines) is the largest remaining block. See decision below.

### Inline Tests — Decision
`settings_view.rs` has 4 `#[gpui::test]` tests in lines 1029–1485.
- Tests exercise selection behavior, profile/MCP command handling, and navigation — they need access to the view internals after splitting.
- Decision: **keep inline in `settings_view.rs`** (the root coordination file) since they drive the public `handle_command` interface. However, if the test module itself causes `settings_view.rs` to exceed 750 lines after other extractions, the helpers should be externalized to `tests/settings_view_tests.rs` which already exists and already has 6 tests.

### `include_str!()` Tests Referencing `settings_view.rs`
No tests in the surveyed set use `include_str!("../src/ui_gpui/views/settings_view.rs")`. No source-text remediation required for this file.

### Safety-Net Tests (Green-Before-Move)

| Test | File | Count | Contract covered |
|------|------|-------|-----------------|
| `settings_view_tests.rs` | `tests/` | 6 tests | Profile/MCP selection, profile list refresh |
| `settings_view_mcp_saved_state_tests.rs` | `tests/` | 2 tests | MCP saved state preservation |
| `settings_view_display_tests.rs` | `tests/` | 7 tests | Display state, empty/populated rendering |
| `settings_panel_behavior_tests.rs` | `tests/` | 8 tests | User action flows, event emission |
| Inline tests | `settings_view.rs` | 4 tests | Command handling, selection fallback, navigation |

**Contracts confirmed covered (minimum 3 required):**
1. Profile summary updates preserve selection — `settings_view_tests.rs`
2. MCP updates preserve selection — `settings_view_mcp_saved_state_tests.rs`
3. Profile/MCP action helpers emit correct events — `settings_panel_behavior_tests.rs`
4. Profile/MCP empty/selected display state — `settings_view_display_tests.rs`

**Gaps requiring new tests BEFORE extraction:**
- `apply_profile_summaries` selection-fallback logic when previously selected profile disappears — verify this specific path is in an existing test before moving the method to `settings_state.rs`

---

## `profile_editor_view.rs` — Decomposition Map

### Code Groups with Exact Line Ranges

| Group | Lines | Description |
|-------|-------|-------------|
| Imports | 1–22 | Use statements |
| `AuthMethod` enum + impl | 23–39 | Public enum: `Keychain`, `Value`, `None` |
| `ApiType` enum + impl | 40–78 | Public enum: `Anthropic`, `OpenAI`, etc. — `display()`, `provider_id()` |
| `ActiveField` enum | 67–79 | Private enum for field focus tracking |
| `ProfileEditorData` struct + impl | 80–131 | Public editable data model, `new()`, `can_save()` |
| `ProfileEditorState` struct + impl | 132–159 | Public state container, `new_profile()` |
| `ProfileEditorView` struct | 160–168 | View struct fields |
| `impl ProfileEditorView` — construction | 169–196 | `new`, `set_bridge`, `request_api_key_refresh`, `set_profile` |
| `impl ProfileEditorView` — active-field editing | 197–365 | `append_to_active_field`, `backspace_active_field`, `cycle_active_field`, `remove_trailing_bytes_from_active_field`, `active_field_text` — IME-adjacent input handling, 169 lines |
| `impl ProfileEditorView` — emit helpers | 366–420 | `emit`, `emit_save_profile` (45-line payload builder) |
| `handle_command` | 421–501 | 81-line command dispatch |
| Render helpers | 502–1266 | `render_top_bar`, `render_label`, `render_text_field`, `render_name_section`, `render_model_section`, `render_api_type_section`, `render_base_url_section`, `render_key_label_section`, `render_section_divider`, `render_temperature_section`, `render_max_tokens_section`, `render_context_limit_section`, `render_show_thinking_section`, `render_extended_thinking_section`, `render_system_prompt_section`, `render_content` |
| `impl Focusable` | 1267–1272 | Single method |
| `impl EntityInputHandler` (IME) | 1273–1380 | Full IME protocol |
| `impl Render` | 1381–1476 | Root render, 96 lines |
| `#[cfg(test)]` module | 1477–1875 | 399 lines, 6 `#[gpui::test]` tests |

### Proposed Extraction Targets (3 files)

#### 1. `profile_editor_data.rs`
**Contents:**
- `AuthMethod` enum + impl (lines 23–39)
- `ApiType` enum + impl (lines 40–78)
- `ActiveField` enum (lines 67–79) — made `pub(super)` to allow access from `profile_editor_view.rs`
- `ProfileEditorData` struct + impl (lines 80–131)
- `ProfileEditorState` struct + impl (lines 132–159)
- `emit_save_profile` (lines 376–420) — pure payload-mapping function that maps `ProfileEditorData` fields to the `UserEvent::SaveProfile` payload; no GPUI/render dependencies

**Why:** `ProfileEditorData`, `AuthMethod`, `ApiType`, and `ProfileEditorState` are the stable public types re-exported from `mod.rs`. `emit_save_profile` is the save payload mapping that the `profile_editor_save_payload_mapping_tests.rs` regression tests target — co-locating it with the data model makes the mapping logic directly testable. This is ~250 lines.

**Re-exports needed from `profile_editor_view.rs`:**
```rust
pub use profile_editor_data::{ApiType, AuthMethod, ProfileEditorData, ProfileEditorState};
```
**Re-exports needed from `mod.rs`:** unchanged (re-exports from `profile_editor_view`).

#### 2. `profile_editor_input.rs`
**Contents:**
- `impl ProfileEditorView` — active-field editing block (lines 197–365): `append_to_active_field`, `backspace_active_field`, `cycle_active_field`, `remove_trailing_bytes_from_active_field`, `active_field_text`
- `impl gpui::EntityInputHandler for ProfileEditorView` (lines 1273–1380) — IME protocol

**Why:** The active-field editing helpers and IME handler are a coherent group: both operate on the same `active_field` enum and text buffer. Together they are ~270 lines. Grouping them avoids scattering the text-entry contract across the file and mirrors the `chat_ime.rs` extraction in Batch A.

**Re-exports needed:** None — these are method blocks on `ProfileEditorView`.

#### 3. `profile_editor_render.rs`
**Contents:**
- All `render_*` helper methods (lines 502–1266): `render_top_bar`, `render_label`, `render_text_field`, `render_name_section`, `render_model_section`, `render_api_type_section`, `render_base_url_section`, `render_key_label_section`, `render_section_divider`, `render_temperature_section`, `render_max_tokens_section`, `render_context_limit_section`, `render_show_thinking_section`, `render_extended_thinking_section`, `render_system_prompt_section`, `render_content`
- `impl Render for ProfileEditorView` (lines 1381–1476)

**Why:** Render helpers are 765 lines — the single largest block in the file. All are pure compositional UI output with no event emission or state mutation beyond display logic. Extracting them eliminates the primary size driver.

**Re-exports needed:** None — private render helpers, `impl Render` is a trait implementation.

### Post-extraction `profile_editor_view.rs` estimated size
Original: 1875 lines
- Remove to `profile_editor_data.rs`: ~250 lines
- Remove to `profile_editor_input.rs`: ~270 lines
- Remove to `profile_editor_render.rs`: ~860 lines
- Remaining (construction, bridge, `handle_command`, Focusable, inline tests): ~495 lines

Target: well within ≤750 lines.

### Inline Tests — Decision
`profile_editor_view.rs` has 6 `#[gpui::test]` tests in lines 1477–1875 (399 lines).
- These tests drive `handle_command`, `set_profile`, field editing, and save behavior through the public interface.
- Decision: **keep inline in `profile_editor_view.rs`** — they test the coordination layer that remains in the root file. The 399-line test block contributes to an estimated ~495-line total which is acceptable.

### `include_str!()` Tests Referencing `profile_editor_view.rs`

| Test file | Line | Assertion content | Planned disposition |
|-----------|------|-------------------|---------------------|
| `api_key_manager_ui_regression_tests.rs` | 54, 87 | Source assertions about profile editor API key navigation / keychain auth fields | **Replace** with behavioral tests: one test verifying `RefreshApiKeys` event is emitted during profile edit, one verifying `AuthMethod::Keychain` produces correct save payload |
| `model_profile_flow_regression_tests.rs` | 154, 186, 200 | Source assertions about shared provider defaults / keychain behavior in profile editor | **Replace** with behavioral tests: `emit_save_profile` payload tests already partially covered by `profile_editor_save_payload_mapping_tests.rs`; expand those tests to cover the specific cases currently asserted by source text |
| `seven_bugs_regression_tests.rs` | 503 | Bug 7 — model-editable source assertion in `profile_editor_view.rs` | **Replace** with behavioral test verifying `ApiType` display or model field editability via state assertion |

### Safety-Net Tests (Green-Before-Move)

| Test | File | Count | Contract covered |
|------|------|-------|-----------------|
| `profile_editor_view_tests.rs` | `tests/` | 9 tests | Load mapping, field editing, save behavior, navigation |
| `profile_editor_save_payload_mapping_tests.rs` | `tests/` | No `#[gpui::test]` markers detected by grep | **Verify actual test count before treating as safety net** |
| Inline tests | `profile_editor_view.rs` | 6 tests | Command handling, field editing, API key refresh |

**Note on `profile_editor_save_payload_mapping_tests.rs`:** The grep for `#[gpui::test]` and `#[test]` returned 0, but the file is 322 lines and contains `async fn test_save_*` functions with `assert!` calls. These use a custom async executor pattern, not the standard markers. Must confirm these tests run and pass before extraction.

**Contracts confirmed covered (minimum 4 required):**
1. Profile editor load fills visible state — `profile_editor_view_tests.rs`
2. Save payload mapping for provider/auth/base-url/system-prompt — `profile_editor_save_payload_mapping_tests.rs` (verify runs)
3. Active-field editing and IME modifies intended field only — inline `profile_editor_view.rs` tests
4. API key refresh/navigation — inline tests + `profile_editor_view_tests.rs`

**Gaps requiring new tests BEFORE extraction:**
- Confirm `profile_editor_save_payload_mapping_tests.rs` tests actually run; if test framework is non-standard, add at least 2 `#[gpui::test]` equivalents to `profile_editor_view_tests.rs` before extraction
- `ApiType::provider_id` and `ApiType::display` methods in `profile_editor_data.rs` need unit test coverage after extraction

---

## `mcp_add_view.rs` — Decomposition Map

### Code Groups with Exact Line Ranges

| Group | Lines | Description |
|-------|-------|-------------|
| Imports | 1–20 | Use statements |
| `McpRegistry` enum + impl | 21–41 | Public enum with `display()` |
| `McpSearchResult` struct + builder impl | 42–126 | Public search result model, extensive builder chain |
| `ActiveField` enum | 127–134 | Private field focus enum |
| `SearchState` enum | 135–146 | Public search lifecycle enum |
| `McpAddState` struct + impl | 147–172 | Public state container, `new()`, `can_proceed()` |
| `McpAddView` struct | 173–179 | View struct fields |
| `impl McpAddView` — construction + bridge + setters | 180–241 | `new`, `set_bridge`, `set_results`, `set_loading`, `set_search_query`, `set_manual_entry` |
| `impl McpAddView` — input editing helpers | 242–300 | `append_to_active_field`, `backspace_active_field`, `remove_trailing_bytes_from_active_field`, `active_field_text` |
| `impl McpAddView` — search/selection logic | 301–466 | `select_registry`, `toggle_registry_dropdown`, `select_result`, `handle_key_down`, `filtered_results`, `command_preview`, `emit_search_registry`, `emit` |
| `handle_command` | 468–558 | 91-line command dispatch |
| Render helpers | 559–1264 | `render_top_bar`, `render_label`, `render_manual_entry`, `render_divider`, `render_registry_dropdown`, `render_registry_overlay`, `render_search_field`, `render_result_row`, `render_results`, `render_content` |
| `impl Focusable` | 1078–1083 | Single method |
| `impl EntityInputHandler` (IME) | 1084–1187 | Full IME protocol |
| `impl Render` | 1188–1264 | Root render |
| `#[cfg(test)]` module | 1265–1696 | 432 lines, 6 `#[gpui::test]` tests, helpers |

### Proposed Extraction Targets (3 files)

#### 1. `mcp_add_data.rs`
**Contents:**
- `McpRegistry` enum + impl (lines 21–41)
- `McpSearchResult` struct + builder impl (lines 42–126)
- `SearchState` enum (lines 135–146)
- `McpAddState` struct + impl (lines 147–172)
- `filtered_results` method (lines 375–411) — pure filtering logic operating on `McpSearchResult` list; no GPUI, no `cx`
- `command_preview` static method (lines 412–430) — pure string formatting from `McpSearchResult`

**Why:** `McpRegistry`, `McpSearchResult`, `SearchState`, and `McpAddState` are all in `mod.rs` re-exports. `filtered_results` and `command_preview` are pure functions on the result/state model — they are the exact contracts exercised by `tests/mcp_add_view_projection_tests.rs`. Extracting them to a dedicated data module makes those contracts directly testable without spinning up a GPUI view.

**Re-exports needed from `mcp_add_view.rs`:**
```rust
pub use mcp_add_data::{McpAddState, McpRegistry, McpSearchResult, SearchState};
```
**Re-exports needed from `mod.rs`:** unchanged.

#### 2. `mcp_add_input.rs`
**Contents:**
- `ActiveField` enum (lines 127–134) — `pub(super)`
- `impl McpAddView` — input editing helpers (lines 242–300): `append_to_active_field`, `backspace_active_field`, `remove_trailing_bytes_from_active_field`, `active_field_text`
- `impl McpAddView` — interaction logic (lines 301–466): `select_registry`, `toggle_registry_dropdown`, `select_result`, `handle_key_down`, `emit_search_registry`, `emit`
- `impl gpui::EntityInputHandler for McpAddView` (lines 1084–1187)

**Why:** The `ActiveField` enum, input mutation helpers, key handler, and IME handler are a coherent input-handling unit. They share the `active_field` state path and together represent the text-entry side of the view — mirroring the `profile_editor_input.rs` extraction. This removes ~375 lines from the root file.

**Re-exports needed:** None.

#### 3. `mcp_add_render.rs`
**Contents:**
- All `render_*` methods (lines 559–1077): `render_top_bar`, `render_label`, `render_manual_entry`, `render_divider`, `render_registry_dropdown`, `render_registry_overlay`, `render_search_field`, `render_result_row`, `render_results`, `render_content`
- `impl Render for McpAddView` (lines 1188–1264)

**Why:** Render helpers are ~520 lines. They are pure compositional UI output. Extracting them to `mcp_add_render.rs` mirrors the Batch A and settings render pattern.

**Re-exports needed:** None.

### Post-extraction `mcp_add_view.rs` estimated size
Original: 1696 lines
- Remove to `mcp_add_data.rs`: ~210 lines
- Remove to `mcp_add_input.rs`: ~375 lines
- Remove to `mcp_add_render.rs`: ~595 lines
- Remaining (construction, bridge, setters, `handle_command`, Focusable, inline tests): ~516 lines

Target: well within ≤750 lines.

### Inline Tests — Decision
`mcp_add_view.rs` has 6 `#[gpui::test]` tests in lines 1265–1696 (432 lines).
- Tests drive bridge-connected behavior, search/selection flows, and command handling.
- Decision: **keep inline in `mcp_add_view.rs`** — they exercise the coordination surface that remains in the root file. 516-line total is acceptable.

### `include_str!()` Tests Referencing `mcp_add_view.rs`
No tests in the surveyed set use `include_str!("../src/ui_gpui/views/mcp_add_view.rs")`. No source-text remediation required for this file.

### Safety-Net Tests (Green-Before-Move)

| Test | File | Count | Contract covered |
|------|------|-------|-----------------|
| `mcp_add_view_projection_tests.rs` | `tests/` | 2 tests | Projection behavior, search result mapping |
| Inline tests | `mcp_add_view.rs` | 6 tests | Search/selection, command handling, input behavior |

**Contracts confirmed covered (minimum 3 required):**
1. Search result projection retains source information — `mcp_add_view_projection_tests.rs`
2. Input/search/select behavior obeys state rules — inline tests
3. Chosen result produces intended follow-on action — inline tests

**Gaps requiring new tests BEFORE extraction:**
- `filtered_results` and `command_preview` in `mcp_add_data.rs` should have direct unit tests added to `mcp_add_view_projection_tests.rs` before extraction to confirm the projection contracts survive the module boundary
- `McpSearchResult` builder chain correctness — add 1 unit test to projection test file

---

## mod.rs Re-export Impact (Batch B)

Current `mod.rs` exports:
```rust
pub use mcp_add_view::{McpAddState, McpAddView, McpRegistry, McpSearchResult, SearchState};
pub use profile_editor_view::{ApiType, AuthMethod, ProfileEditorData, ProfileEditorState, ProfileEditorView};
pub use settings_view::{McpItem, McpStatus, ProfileItem, SettingsState, SettingsView};
```

After Batch B extractions:
- Each root view file (`settings_view.rs`, `profile_editor_view.rs`, `mcp_add_view.rs`) re-exports the public types from its data submodule
- `mod.rs` lines remain **unchanged** — the re-export chain passes through the root file
- Only `mod.rs` change needed: add `pub mod` declarations for the new submodules (handled by parent module `pub mod` in the root view file; `mod.rs` only re-exports the view module, not its submodules)

No `mod.rs` changes needed if the root view files correctly re-export their data types.
