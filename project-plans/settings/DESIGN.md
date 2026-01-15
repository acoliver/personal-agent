# Settings Panel Redesign

## Overview

Redesign the settings panel to use a cleaner list-box style interface with proper alignment and more intuitive controls.

## Current Problems

1. **Jagged left alignment** - Profile rows have inconsistent left edges
2. **Cramped horizontal layout** - Too many elements crammed into each row
3. **Cryptic UI** - "[OK]" status indicator is unclear
4. **No visual indication of active/default profile**
5. **Button overload** - Select/Edit/Del buttons on every row

## New Design

### Layout Structure

```
┌────────────────────────────────────────────────────────────┐
│ [<] Settings                            [Refresh Models]   │  ← 44px top bar
├────────────────────────────────────────────────────────────┤
│                                                            │
│  Profiles                                                  │  ← Section label
│  ┌──────────────────────────────────────────────────────┐  │
│  │ ▶ claude-sonnet (anthropic:claude-3-5-sonnet)       │  │  ← Selected/default
│  │   gpt-4o (openai:gpt-4o)                            │  │
│  │   llama3 (groq:llama-3)                             │  │
│  ├──────────────────────────────────────────────────────┤  │
│  │ [−] [+]                                      [Edit] │  │  ← Table toolbar
│  └──────────────────────────────────────────────────────┘  │
│                                                            │
│  ─────────────────────────────────────────────────────────  │  ← Separator
│                                                            │
│  MCPs                                                      │  ← Section label
│  ┌──────────────────────────────────────────────────────┐  │
│  │ filesystem - local files access              [====] │  │  ← Toggle ON
│  │ web-search - search the web                  [====] │  │  ← Toggle ON  
│  │ github - GitHub API access                   [    ] │  │  ← Toggle OFF
│  ├──────────────────────────────────────────────────────┤  │
│  │ [−] [+]                                      [Edit] │  │  ← Table toolbar
│  └──────────────────────────────────────────────────────┘  │
│                                                            │
│  ─────────────────────────────────────────────────────────  │  ← Separator
│                                                            │
│  Global Hotkey: [Cmd+Shift+Space________________________]  │  ← Text field
│                                                            │
└────────────────────────────────────────────────────────────┘
```

### Profiles Section

**List Box Behavior:**
- Single selection (click to select)
- Selected row is highlighted with accent color background
- Selected profile IS the default/active profile
- Selection persists to `config.default_profile`

**Row Format:**
```
│ ▶ profile-name (provider:model-id)                        │
```
- `▶` indicator on selected row only
- Profile name in primary text color
- `(provider:model)` in secondary/dimmer text color
- Full width, clean left alignment

**Table Toolbar:**
- `[−]` - Delete selected profile (with confirmation dialog)
- `[+]` - Add new profile (opens model selector flow)
- `[Edit]` - Edit selected profile (opens profile editor)
- Toolbar is inside the table border, at the bottom

**Delete Behavior:**
- Shows confirmation dialog
- After delete, selects next profile (or previous if deleting last)
- If no profiles remain, shows empty state message

### MCPs Section

**List Box Behavior:**
- Click row to select (for Edit/Delete targeting)
- Toggle switch on right side of each row controls enabled/disabled state
- Multiple MCPs can be enabled simultaneously

**Row Format:**
```
│ mcp-name - description                          [toggle] │
```
- MCP name in primary text color
- Description in secondary/dimmer text color
- Toggle switch (NSSwitch) right-aligned

**Table Toolbar:**
- Same as Profiles: `[−]` `[+]` `[Edit]`
- `[−]` deletes selected MCP
- `[+]` adds new MCP (opens MCP editor - TBD)
- `[Edit]` edits selected MCP

**Toggle Behavior:**
- Toggling saves immediately to config
- Does NOT change selection
- Visual: ON = filled/colored, OFF = empty/gray

### Global Hotkey Section

- Simple label + text field
- Editable text field showing current hotkey
- Changes save on focus loss or Enter

## Component Specifications

### Profiles List Box
- **Height:** ~120px (shows ~4-5 rows)
- **Row height:** 24px
- **Selection color:** System accent or Theme::ACCENT
- **Border:** 1px, rounded corners (4px)
- **Background:** Theme::BG_DARKER

### MCPs List Box  
- **Height:** ~100px (shows ~3-4 rows)
- **Row height:** 28px (slightly taller for toggle)
- **Toggle:** NSSwitch or custom toggle button
- **Border:** Same as Profiles
- **Background:** Same as Profiles

### Table Toolbar
- **Height:** 28px
- **Background:** Slightly different from list (Theme::BG_DARK)
- **Button style:** Small, borderless or minimal bezel
- **Layout:** `[−] [+]` left-aligned, `[Edit]` right-aligned

## Data Model Changes

### Config Additions

```rust
// In src/config/settings.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub command: String,        // Command to run MCP server
    pub args: Vec<String>,      // Arguments
    pub env: HashMap<String, String>, // Environment variables
    pub enabled: bool,          // Toggle state
}

// Add to Config struct:
pub struct Config {
    // ... existing fields ...
    
    #[serde(default)]  // CRITICAL: allows loading old configs without mcps field
    pub mcps: Vec<McpConfig>,
}
```

### Profile Selection Sync

```rust
// When settings view loads:
1. Load config
2. Find index where profiles[i].id == config.default_profile
3. Select that row in table

// When user clicks a row:
1. Get profile at clicked index
2. Set config.default_profile = profile.id
3. Save config

// When delete:
1. Remove profile from config.profiles
2. If deleted was default, set default to next/prev
3. Save config
4. Update selection in UI
```

## Files to Modify

1. **src/config/settings.rs** - Add McpConfig struct, add mcps field to Config
2. **src/ui/settings_view.rs** - Complete rewrite of UI building and event handling
3. **src/ui/theme.rs** - May need additional colors for selection highlighting

## Implementation Notes

### NSTableView vs Custom Stack

**Option A: NSTableView**
- Native list selection behavior
- Built-in scrolling
- More complex setup (delegate, data source)
- Better for large lists

**Option B: Custom NSStackView with selection logic**
- Simpler to implement with current codebase patterns
- Manual selection state management
- Already familiar pattern from current code
- Fine for small lists (<20 items)

**Recommendation:** Use NSStackView with manual selection for consistency with existing code. Can migrate to NSTableView later if needed.

### Toggle Switch Implementation

macOS has `NSSwitch` (10.15+) for native toggle switches. Use this for MCP enabled/disabled state.

**Cargo.toml:** Add `"NSSwitch"` to objc2-app-kit features.

```rust
use objc2_app_kit::{NSSwitch, NSControlStateValueOn, NSControlStateValueOff};

let toggle = NSSwitch::new(mtm);
toggle.setState(if enabled { NSControlStateValueOn } else { NSControlStateValueOff });
toggle.setTarget(Some(self));
toggle.setAction(Some(sel!(mcpToggled:)));
```

### Row Identification

Use UUIDs instead of index-based tags for robust row identification:

```rust
// Store UUID in row's identifier
row.setIdentifier(Some(&NSString::from_str(&profile.id.to_string())));

// On click, retrieve UUID
let uuid_str = sender.identifier()?.to_string();
let uuid = Uuid::parse_str(&uuid_str).ok()?;
```

## Out of Scope (Phase 1)

- MCP actually being wired to functionality (just UI/config storage)
- Hotkey recording/capture UI (just text field for now)
- Profile reordering via drag-and-drop
- Import/export of profiles or MCPs
