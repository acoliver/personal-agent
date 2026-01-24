# Settings View Requirements

The Settings View manages model profiles and MCP configurations. **The view is purely presentational** - it renders data from services and forwards user actions.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, #1a1a1a)                                      │
│                                                              │
│  [<]  Settings                          [Refresh Models]     │
│  28px  14pt bold                              text button    │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ CONTENT SCROLL AREA (flex height, #121212)                   │
│                                                              │
│  12px padding                                                │
│                                                              │
│  PROFILES                              ← 11pt, #888888       │
│  ┌────────────────────────────────────────────────────────┐  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  │
│  │▓ zai (anthropic:claude-3-5-sonnet)                    ▓│  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  │
│  │ zai glm (openrouter:deepseek/deepseek-r1)              │  │
│  │ hf:moonshotai (huggingface:Kimi-K2)                    │  │
│  └────────────────────────────────────────────────────────┘  │
│   ↑ 360px wide, 100px tall                                   │
│   ↑ Selected row: FULL WIDTH highlight, left-justified text  │
│                                                              │
│  [-]  [+]                                           [Edit]   │
│   ↑ toolbar below list                                       │
│                                                              │
│  16px gap                                                    │
│                                                              │
│  MCP TOOLS                             ← 11pt, #888888       │
│  ┌────────────────────────────────────────────────────────┐  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  │
│  │▓ ● exa                                        [ON ]   ▓│  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  │
│  │ ○ ...server-filesystem                        [OFF]    │  │
│  └────────────────────────────────────────────────────────┘  │
│   ↑ Row layout: [status dot] [name, left] [spacer] [toggle]  │
│   ↑ Status: ● green=running, ○ gray=stopped, ● red=error     │
│                                                              │
│  [-]  [+]                                           [Edit]   │
│                                                              │
│  16px gap                                                    │
│                                                              │
│  GLOBAL HOTKEY                         ← 11pt, #888888       │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Cmd+Shift+P                                            │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, 24px tall                                      │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## Layout Specifications

### Overall Dimensions

| Property | Value | Notes |
|----------|-------|-------|
| Popover width | 400px | Same as other views |
| Popover height | 500px | Same as other views |
| Background | #121212 | Theme.BG_BASE |

### Spacing Standards

| Context | Value | Notes |
|---------|-------|-------|
| Content padding | 12px | All edges |
| Section spacing | 16px | Between sections |
| Header to list | 6px | Section header to list box |
| List to toolbar | 6px | List box to button row |

### Typography

| Element | Font | Size | Color |
|---------|------|------|-------|
| "Settings" title | System Bold | 14pt | #e5e5e5 |
| Section headers | System Regular | 11pt | #888888 |
| Profile row text | System Regular | 12pt | #e5e5e5 |
| MCP row text | System Regular | 12pt | #e5e5e5 |
| Button labels | System Medium | 12pt | #e5e5e5 |

---

## Component Requirements

### Top Bar

**Layout:** Horizontal stack, 44px height, #1a1a1a background

```
[12px] [<] [8px] [Settings] [spacer] [Refresh Models] [12px]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | Back button | 28x28, "<" label | Navigate to Chat View |
| TB-2 | Title | "Settings", 14pt bold | Static |
| TB-3 | Refresh button | Text button, right side | Fetch models from models.dev |
| TB-4 | Layout | Back + title left, refresh right | Spacer between |

### Profiles Section

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| PH-1 | Label | "PROFILES", 11pt, #888888 |
| PH-2 | Alignment | Left-aligned with list box |
| PH-3 | Spacing | 6px below header |

**List Box:**

| ID | Element | Spec |
|----|---------|------|
| PL-1 | Container | 360px wide, 100px tall |
| PL-2 | Background | #1e1e1e (Theme.BG_DARKER) |
| PL-3 | Border | 1px #333333, 4px radius |
| PL-4 | Scrollable | Vertical scroll if content exceeds |

**Profile Rows:**

| ID | Element | Spec |
|----|---------|------|
| PR-1 | Row height | 24px |
| PR-2 | Row width | FULL WIDTH of list box |
| PR-3 | Text alignment | LEFT-justified |
| PR-4 | Text format | "{name} ({provider}:{model})" |
| PR-5 | Text padding | 8px left |
| PR-6 | Normal background | Transparent (inherits list bg) |
| PR-7 | Selected background | Accent blue, FULL ROW WIDTH |
| PR-8 | Selected text | White |
| PR-9 | Hover background | Subtle highlight #2a2a2a |
| PR-10 | Click target | Entire row (full width) |

**Profile Toolbar:**

| ID | Element | Spec |
|----|---------|------|
| PT-1 | Layout | `[-] [+] [spacer] [Edit]` |
| PT-2 | [-] button | 28x28, delete selected |
| PT-3 | [+] button | 28x28, open Model Selector |
| PT-4 | [Edit] button | 60px wide, open Profile Editor |
| PT-5 | Spacing | 8px between [-] and [+] |
| PT-6 | [-] disabled | When no selection |
| PT-7 | [Edit] disabled | When no selection |
| PT-8 | Toolbar width | Matches list box (360px) |

### MCP Tools Section

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| MH-1 | Label | "MCP TOOLS", 11pt, #888888 |
| MH-2 | Alignment | Left-aligned with list box |
| MH-3 | Spacing | 6px below header |

**List Box:**

| ID | Element | Spec |
|----|---------|------|
| ML-1 | Container | 360px wide, 100px tall |
| ML-2 | Background | #1e1e1e (Theme.BG_DARKER) |
| ML-3 | Border | 1px #333333, 4px radius |
| ML-4 | Scrollable | Vertical scroll if content exceeds |

**MCP Rows:**

| ID | Element | Spec |
|----|---------|------|
| MR-1 | Row height | 28px (taller for toggle) |
| MR-2 | Row width | FULL WIDTH of list box |
| MR-3 | Row layout | `[8px] [status] [8px] [name] [spacer] [toggle] [8px]` |
| MR-4 | Status indicator | 8px circle, left side |
| MR-5 | Status: running | Green (#4a9f4a) filled circle |
| MR-6 | Status: stopped | Gray (#666666) hollow circle |
| MR-7 | Status: error | Red (#9f4a4a) filled circle |
| MR-8 | Name alignment | LEFT-justified after status |
| MR-9 | Name truncation | Truncate from LEFT (show end) "...filesystem" |
| MR-10 | Toggle | NSSwitch, right side |
| MR-11 | Normal background | Transparent |
| MR-12 | Selected background | Accent blue, FULL ROW WIDTH |
| MR-13 | Hover background | Subtle highlight #2a2a2a |
| MR-14 | Click row | Selects row (not toggle) |
| MR-15 | Click toggle | Toggles enabled (not selection) |

**MCP Toolbar:**

| ID | Element | Spec |
|----|---------|------|
| MT-1 | Layout | `[-] [+] [spacer] [Edit]` |
| MT-2 | [-] button | 28x28, delete selected |
| MT-3 | [+] button | 28x28, open MCP Add View |
| MT-4 | [Edit] button | 60px wide, open MCP Configure |
| MT-5 | Spacing | 8px between [-] and [+] |
| MT-6 | [-] disabled | When no selection |
| MT-7 | [Edit] disabled | When no selection |
| MT-8 | Toolbar width | Matches list box (360px) |

### Global Hotkey Section

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| HH-1 | Label | "GLOBAL HOTKEY", 11pt, #888888 |
| HH-2 | Alignment | Left-aligned with field |
| HH-3 | Spacing | 6px below header |

**Hotkey Field:**

| ID | Element | Spec |
|----|---------|------|
| HF-1 | Width | 360px (matches list boxes) |
| HF-2 | Height | 24px |
| HF-3 | Background | #2a2a2a |
| HF-4 | Border | 1px #444444, 6px radius |
| HF-5 | Placeholder | "Cmd+Shift+P" |
| HF-6 | Text | 12pt, #e5e5e5 |
| HF-7 | Editable | Yes |
| HF-8 | Save trigger | On blur or Enter |

---

## Behavioral Requirements

### View Loading Flow

| Step | Action | Visual |
|------|--------|--------|
| 1 | View appears | |
| 2 | Call ProfileService.list() | |
| 3 | Call McpService.list() | |
| 4 | Render profile rows | |
| 5 | Render MCP rows with status | |
| 6 | Pre-select default profile | Highlight row |
| 7 | Scroll to TOP | Content at top |
| 8 | Update button states | Enable/disable |

### Profile Selection Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click profile row | |
| 2 | | Highlight clicked row (full width) |
| 3 | | Unhighlight previous selection |
| 4 | | Call ProfileService.set_default(id) |
| 5 | | Enable [-] and [Edit] buttons |

### Profile Add Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [+] button | |
| 2 | | Navigate to Model Selector View |
| 3 | | User selects provider and model |
| 4 | | Navigate to Profile Editor View |
| 5 | | User fills name, API key, settings |
| 6 | | User clicks Save |
| 7 | | Navigate back to Settings |
| 8 | | New profile appears in list |

### Profile Edit Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Edit] button | |
| 2 | | Get selected profile ID |
| 3 | | Navigate to Profile Editor with ID |
| 4 | | User edits, clicks Save |
| 5 | | Navigate back to Settings |
| 6 | | Profile row updated |

### Profile Delete Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [-] button | |
| 2 | | Show confirmation alert |
| 3 | | Title: "Delete Profile?" |
| 4 | | Message: "Delete '{name}'? This cannot be undone." |
| 5a | Click [Cancel] | Dismiss alert |
| 5b | Click [Delete] | |
| 6 | | Call ProfileService.delete(id) |
| 7 | | Remove row from list |
| 8 | | Select next profile (or first) |
| 9 | | Update button states |

### MCP Selection Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click MCP row (not toggle) | |
| 2 | | Highlight clicked row (full width) |
| 3 | | Unhighlight previous selection |
| 4 | | Enable [-] and [Edit] buttons |

### MCP Toggle Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click toggle switch | |
| 2 | | Toggle mcp.enabled state |
| 3 | | Call McpService config update |
| 4 | | If enabled: start MCP → status green |
| 5 | | If disabled: stop MCP → status gray |
| 6 | | Row selection unchanged |

### MCP Add Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [+] button | |
| 2 | | Navigate to MCP Add View |
| 3 | | User searches/selects MCP |
| 4 | | Navigate to MCP Configure View |
| 5 | | User fills credentials |
| 6 | | User clicks Save |
| 7 | | Navigate back to Settings |
| 8 | | New MCP appears in list |

### MCP Edit Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Edit] button | |
| 2 | | Get selected MCP ID |
| 3 | | Navigate to MCP Configure with ID |
| 4 | | User edits, clicks Save |
| 5 | | Navigate back to Settings |
| 6 | | MCP row updated |

### MCP Delete Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [-] button | |
| 2 | | Show confirmation alert |
| 3 | | Title: "Delete MCP?" |
| 4 | | Message: "Delete '{name}'? Credentials will be removed." |
| 5a | Click [Cancel] | Dismiss alert |
| 5b | Click [Delete] | |
| 6 | | Stop MCP if running |
| 7 | | Call McpService.delete(id) |
| 8 | | Delete stored credentials |
| 9 | | Remove row from list |
| 10 | | Update button states |

### Refresh Models Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Refresh Models] | |
| 2 | | Call ModelsRegistryService.refresh() |
| 3 | | Show spinner/feedback (optional) |
| 4 | | On complete: ready for next add |

---

## State Management

### View State

| Field | Type | Purpose |
|-------|------|---------|
| selected_profile_id | Option<Uuid> | Currently selected profile |
| selected_mcp_id | Option<Uuid> | Currently selected MCP |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| profiles_list | NSStackView | Profile rows container |
| mcps_list | NSStackView | MCP rows container |
| profile_delete_btn | NSButton | [-] for profiles |
| profile_add_btn | NSButton | [+] for profiles |
| profile_edit_btn | NSButton | [Edit] for profiles |
| mcp_delete_btn | NSButton | [-] for MCPs |
| mcp_add_btn | NSButton | [+] for MCPs |
| mcp_edit_btn | NSButton | [Edit] for MCPs |
| hotkey_field | NSTextField | Hotkey input |
| scroll_view | NSScrollView | Content container |

---

## Service Dependencies

| Action | Service | Method |
|--------|---------|--------|
| List profiles | ProfileService | list() |
| Set default profile | AppSettingsService | set_default_profile_id(id) |
| Delete profile | ProfileService | delete(id) |
| List MCPs | McpService | list() |
| Get MCP status | McpService | status(id) |
| Toggle MCP | McpService | start(id) / stop(id) |
| Delete MCP | McpService | delete(id) |
| Refresh models | ModelsRegistryService | refresh() |

---

## Test Coverage

### Visual Tests

- [ ] Profile rows fill full width
- [ ] Profile text left-justified
- [ ] Selected profile row fully highlighted
- [ ] MCP rows fill full width
- [ ] MCP status indicator on left
- [ ] MCP name left-justified
- [ ] MCP toggle on right
- [ ] Selected MCP row fully highlighted
- [ ] Scroll starts at top

### Interaction Tests

- [ ] Click profile row selects it
- [ ] Click MCP row selects it (not toggle)
- [ ] Click MCP toggle changes enabled state
- [ ] [+] opens add flow
- [ ] [-] shows delete confirmation
- [ ] [Edit] opens edit flow
- [ ] [-] and [Edit] disabled when no selection

### State Tests

- [ ] Default profile pre-selected on load
- [ ] Selection persists after toggle
- [ ] Button states update on selection change
- [ ] MCP status reflects running state
