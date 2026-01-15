# Settings Panel Implementation Plan

## Overview

Step-by-step implementation plan for the settings panel redesign. Each phase is independently testable.

---

## Phase 1: Config Changes

**Goal:** Add MCP config structure and mcps field to Config.

### Tasks

1. **Add McpConfig struct to `src/config/settings.rs`:**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
   pub struct McpConfig {
       pub id: Uuid,
       pub name: String,
       pub description: String,
       pub command: String,
       pub args: Vec<String>,
       pub enabled: bool,
   }
   ```

2. **Add mcps field to Config struct:**
   ```rust
   pub struct Config {
       // ... existing fields ...
       pub mcps: Vec<McpConfig>,
   }
   ```

3. **Update Config::default() to include empty mcps vec:**
   ```rust
   mcps: Vec::new(),
   ```

4. **Add helper methods to Config:**
   ```rust
   pub fn add_mcp(&mut self, mcp: McpConfig) { ... }
   pub fn remove_mcp(&mut self, id: &Uuid) -> Result<(), ConfigError> { ... }
   pub fn get_enabled_mcps(&self) -> Vec<&McpConfig> { ... }
   ```

5. **Export McpConfig from `src/config/mod.rs`**

### Verification
- `cargo build` succeeds
- `cargo test` passes
- Existing config.json loads without error (mcps defaults to empty)

---

## Phase 2: Settings View Structure

**Goal:** Rebuild settings_view.rs with new layout structure (sections, no content yet).

### Task 2.1: Update Ivars and Cargo.toml

**Update ivars struct:**
```rust
pub struct SettingsViewIvars {
    scroll_view: RefCell<Option<Retained<NSScrollView>>>,
    profiles_list: RefCell<Option<Retained<NSStackView>>>,
    profiles_toolbar: RefCell<Option<Retained<NSView>>>,
    mcps_list: RefCell<Option<Retained<NSStackView>>>,
    mcps_toolbar: RefCell<Option<Retained<NSView>>>,
    hotkey_field: RefCell<Option<Retained<NSTextField>>>,
    // Use Option<Uuid> instead of index for robustness
    selected_profile_id: RefCell<Option<Uuid>>,
    selected_mcp_id: RefCell<Option<Uuid>>,
}
```

**Update Cargo.toml** - add `"NSSwitch"` to objc2-app-kit features.

### Task 2.2: Create Section Builder Helper

```rust
fn build_section_with_list(
    &self,
    title: &str,
    list_height: f64,
    mtm: MainThreadMarker,
) -> (Retained<NSView>, Retained<NSStackView>, Retained<NSView>)
// Returns: (section_container, list_stack, toolbar)
```

This helper builds:
- Section label (12pt bold, 16px left margin)
- List box container with border and rounded corners
- Inner stack for list items
- Toolbar row at bottom

### Task 2.3: Build Profiles Section Structure

Use the helper to create:
- "Profiles" label
- Empty list box (~120px height)
- Toolbar with `[−]` `[+]` `[Edit]` buttons (actions wired but no-op for now)

Store references in ivars.

### Task 2.4: Build MCPs Section Structure

Use the helper to create:
- "MCPs" label  
- Empty list box (~100px height)
- Toolbar with `[−]` `[+]` `[Edit]` buttons

Store references in ivars.

### Task 2.5: Build Hotkey Section and Wire Layout

- Add separator between sections
- Add "Global Hotkey" label + text field
- Wire all sections into main scroll view
- Verify constraints for proper sizing

### Verification
- [ ] Settings view displays without crash
- [ ] Three sections visible: Profiles, MCPs, Global Hotkey
- [ ] Each section has correct height proportions
- [ ] Toolbars visible with buttons (non-functional is OK)
- [ ] Top bar still works (back button, refresh button)
- [ ] Scroll works if content exceeds view height

---

## Phase 3: Profiles List Implementation

**Goal:** Implement the profiles list with selection behavior.

### Tasks

1. **Create profile row builder:**
   ```rust
   fn create_profile_row(
       &self,
       profile: &ModelProfile,
       index: usize,
       is_selected: bool,
       mtm: MainThreadMarker,
   ) -> Retained<NSView>
   ```
   - Row format: `▶ name (provider:model)` or `  name (provider:model)`
   - Selected row gets highlight background
   - Row is clickable (gesture recognizer or button overlay)

2. **Implement `load_profiles` method:**
   - Clear existing rows from profiles_list
   - Load config
   - Find default profile index
   - For each profile, create row (selected if index matches default)
   - Update `selected_profile_index` ivar

3. **Implement row click handler:**
   ```rust
   #[unsafe(method(profileRowClicked:))]
   fn profile_row_clicked(&self, sender: Option<&NSObject>)
   ```
   - Get index from sender's tag
   - Update config.default_profile
   - Save config
   - Update UI selection state (deselect old, select new)

4. **Implement toolbar buttons:**
   - `[−]` button → `deleteSelectedProfile:` action
   - `[+]` button → `addProfileClicked:` action (existing)
   - `[Edit]` button → `editSelectedProfile:` action

5. **Implement delete handler:**
   ```rust
   #[unsafe(method(deleteSelectedProfile:))]
   fn delete_selected_profile(&self, sender: Option<&NSObject>)
   ```
   - Get selected index
   - Show confirmation alert
   - On confirm: remove from config, update default, save, reload

6. **Implement edit handler:**
   ```rust
   #[unsafe(method(editSelectedProfile:))]
   fn edit_selected_profile(&self, sender: Option<&NSObject>)
   ```
   - Get selected profile ID
   - Set EDITING_PROFILE_ID
   - Post notification to show profile editor

### Verification
- Profiles display in list
- Clicking row changes selection and updates config.default_profile
- Default profile is pre-selected on view load
- Delete removes profile and updates selection
- Edit opens profile editor with correct profile
- Add opens model selector

---

## Phase 4: MCPs List Implementation

**Goal:** Implement the MCPs list with toggle switches.

### Tasks

1. **Create MCP row builder:**
   ```rust
   fn create_mcp_row(
       &self,
       mcp: &McpConfig,
       index: usize,
       mtm: MainThreadMarker,
   ) -> Retained<NSView>
   ```
   - Row format: `name - description` with toggle on right
   - Use NSSwitch for toggle
   - Row is clickable for selection (Edit/Delete targeting)

2. **Implement `load_mcps` method:**
   - Clear existing rows from mcps_list
   - Load config.mcps
   - For each MCP, create row with toggle state from mcp.enabled

3. **Implement toggle handler:**
   ```rust
   #[unsafe(method(mcpToggled:))]
   fn mcp_toggled(&self, sender: Option<&NSObject>)
   ```
   - Get MCP index from sender's tag
   - Toggle mcp.enabled in config
   - Save config immediately

4. **Implement MCP row click handler:**
   ```rust
   #[unsafe(method(mcpRowClicked:))]
   fn mcp_row_clicked(&self, sender: Option<&NSObject>)
   ```
   - Update selected_mcp_index
   - Update visual selection state

5. **Implement MCP toolbar buttons:**
   - `[−]` → delete selected MCP
   - `[+]` → add new MCP (placeholder alert for now)
   - `[Edit]` → edit selected MCP (placeholder alert for now)

6. **Add placeholder alert for Add/Edit MCP:**
   ```rust
   // Show alert: "MCP editor coming soon"
   ```

### Verification
- MCPs display in list (if any in config)
- Toggle switches reflect enabled state
- Toggling updates config and saves
- Selection works for targeting Edit/Delete
- Delete removes MCP
- Add/Edit show placeholder message

---

## Phase 5: Global Hotkey Field

**Goal:** Implement the global hotkey text field.

### Tasks

1. **Create hotkey field in content area:**
   - Label: "Global Hotkey"
   - Text field showing config.global_hotkey value
   - Full width minus margins

2. **Implement change handler:**
   ```rust
   #[unsafe(method(hotkeyChanged:))]
   fn hotkey_changed(&self, sender: Option<&NSObject>)
   ```
   - Get text from field
   - Update config.global_hotkey
   - Save config

3. **Set up field action:**
   - Action fires on Enter or focus loss

### Verification
- Field displays current hotkey
- Editing and pressing Enter saves new value
- Value persists after leaving and returning to settings

---

## Phase 6: Polish and Edge Cases

**Goal:** Handle edge cases and polish UI.

### Tasks

1. **Empty state for profiles:**
   - If no profiles, show message: "No profiles yet. Click + to add one."
   - Disable `[−]` and `[Edit]` buttons

2. **Empty state for MCPs:**
   - If no MCPs, show message: "No MCPs configured."
   - Disable `[−]` and `[Edit]` buttons

3. **Button state management:**
   - Disable `[−]`/`[Edit]` when nothing selected or list empty
   - Enable `[+]` always

4. **Scroll behavior:**
   - Ensure scroll view starts at top
   - Scroll selected item into view if needed

5. **Visual polish:**
   - Consistent spacing (16px margins, 8px between elements)
   - Proper font weights (bold for labels, regular for content)
   - Selection highlight color matches system/theme

6. **Reload on return:**
   - When returning from profile editor, call reload_profiles()
   - Already implemented but verify it works with new structure

### Verification
- Empty states display correctly
- Buttons enable/disable appropriately
- Scrolling works correctly
- Visual appearance matches design

---

## Testing Checklist

### Profile Operations
- [ ] Profiles load and display correctly
- [ ] Clicking profile changes selection and saves default
- [ ] Default profile is selected on view load
- [ ] Add profile flow works (model selector → editor → back)
- [ ] Edit profile opens editor with correct data
- [ ] Delete profile shows confirmation and removes
- [ ] Delete last profile handles gracefully
- [ ] Delete default profile updates to next/prev

### MCP Operations  
- [ ] MCPs load and display (when present in config)
- [ ] Toggle switches reflect enabled state
- [ ] Toggle updates config and saves
- [ ] Add MCP shows placeholder (for now)
- [ ] Edit MCP shows placeholder (for now)
- [ ] Delete MCP works

### Global Hotkey
- [ ] Field shows current value
- [ ] Editing saves new value
- [ ] Value persists

### Edge Cases
- [ ] Empty profiles list shows message
- [ ] Empty MCPs list shows message
- [ ] Buttons disable when appropriate
- [ ] View scrolls correctly
- [ ] Returning from editor reloads data

---

## Files Modified

1. `src/config/settings.rs` - McpConfig struct, mcps field
2. `src/config/mod.rs` - Export McpConfig
3. `src/ui/settings_view.rs` - Complete rewrite
4. `Cargo.toml` - May need NSSwitch feature

## Dependencies

- Phase 2 depends on Phase 1 (McpConfig struct)
- Phase 3-5 depend on Phase 2 (view structure)
- Phase 6 depends on Phase 3-5

## Estimated Effort

- Phase 1: 15 minutes
- Phase 2: 30 minutes
- Phase 3: 45 minutes
- Phase 4: 30 minutes
- Phase 5: 15 minutes
- Phase 6: 30 minutes

**Total: ~2.5-3 hours**
