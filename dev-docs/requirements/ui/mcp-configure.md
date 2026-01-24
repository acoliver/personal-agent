# MCP Configure View Requirements

The MCP Configure View allows users to configure credentials and settings for an MCP server. This is the second step after MCP Add (for new MCPs) or accessed directly from Settings (for editing). **The view is purely presentational** - it renders forms and forwards user actions to McpService.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, #1a1a1a)                                      │
│                                                              │
│  [Cancel]          Configure MCP            [Save]           │
│   70px               14pt bold               60px            │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ CONTENT SCROLL AREA (flex height, #121212)                   │
│                                                              │
│  12px padding                                                │
│                                                              │
│  NAME                                      ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ GitHub                                                 │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, editable                                       │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  PACKAGE                                   ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ npx @modelcontextprotocol/server-github               │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, read-only, muted text                          │
│                                                              │
│  16px gap                                                    │
│                                                              │
│  ═══════════════════════════════════════════════════════════ │
│  AUTHENTICATION                                              │
│  ═══════════════════════════════════════════════════════════ │
│                                                              │
│  AUTH METHOD                               ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ API Key                                              v │  │
│  └────────────────────────────────────────────────────────┘  │
│   Options: "None", "API Key", "Key File", "OAuth"            │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  ─── If "API Key" selected: ───                              │
│                                                              │
│  GITHUB_TOKEN                          [x] Mask              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●              │  │
│  └────────────────────────────────────────────────────────┘  │
│   Label shows env var name from registry                     │
│   Single-line, masked by default                             │
│                                                              │
│  ─── If "Key File" selected: ───                             │
│                                                              │
│  KEY FILE                                  ← 11pt, #888888   │
│  ┌──────────────────────────────────────────────┐ [Browse]   │
│  │ ~/.github_token                              │            │
│  └──────────────────────────────────────────────┘            │
│   290px field + 60px browse button                           │
│                                                              │
│  ─── If "OAuth" selected: ───                                │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │              Authorize with GitHub                     │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, 36px tall, prominent button                    │
│                                                              │
│  Status: Not connected                     ← 11pt, #888888   │
│   or: Connected as @username               ← 11pt, #4ade80   │
│                                                              │
│  ─── If "None" selected: ───                                 │
│                                                              │
│  No authentication required for this MCP.                    │
│   11pt, #888888                                              │
│                                                              │
│  16px gap                                                    │
│                                                              │
│  ═══════════════════════════════════════════════════════════ │
│  CONFIGURATION (only if MCP has configSchema)                │
│  ═══════════════════════════════════════════════════════════ │
│                                                              │
│  ALLOWED_PATHS                             ← from schema     │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ ~/Documents                                       [-]  │  │
│  │ ~/Projects                                        [-]  │  │
│  │ [+ Add Path]                                           │  │
│  └────────────────────────────────────────────────────────┘  │
│   Array field: list with add/remove                          │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  [ ] READ_ONLY                             ← boolean field   │
│   Checkbox from configSchema                                 │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  DEFAULT_BRANCH                            ← string field    │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ main                                                   │  │
│  └────────────────────────────────────────────────────────┘  │
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
| Field width | 360px | All input fields |
| Field height | 24px | Single-line fields |
| Section gap | 16px | Between major sections |
| Field gap | 12px | Between fields in same section |
| Label gap | 4px | Between label and field |

### Typography

| Element | Font | Size | Color |
|---------|------|------|-------|
| Title | System Bold | 14pt | #e5e5e5 |
| Section headers | System Bold | 11pt | #888888 |
| Field labels | System Regular | 11pt | #888888 |
| Field text | System Regular | 12pt | #e5e5e5 |
| Read-only text | System Regular | 12pt | #888888 |
| Button labels | System Medium | 12pt | #e5e5e5 |

---

## Component Requirements

### Top Bar

**Layout:** 44px height, #1a1a1a background

```
[12px] [Cancel 70px] [spacer] [Configure MCP] [spacer] [Save 60px] [12px]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | Cancel button | 70px, left | Discard changes, return |
| TB-2 | Title | "Configure MCP", 14pt bold | Static |
| TB-3 | Save button | 60px, right | Validate and save |
| TB-4 | Save enabled | When name non-empty AND auth valid | Validation |
| TB-5 | Save disabled style | Grayed out | Visual feedback |

### Name Field

| ID | Element | Spec |
|----|---------|------|
| NF-1 | Label | "NAME", 11pt, #888888 |
| NF-2 | Field | NSTextField, 360px x 24px |
| NF-3 | Background | #2a2a2a |
| NF-4 | Border | 1px #444444, 4px radius |
| NF-5 | Pre-filled | From registry display_name or manual entry |
| NF-6 | Editable | Yes |
| NF-7 | Required | Yes |

### Package Display

| ID | Element | Spec |
|----|---------|------|
| PD-1 | Label | "PACKAGE", 11pt, #888888 |
| PD-2 | Field | NSTextField, 360px x 24px |
| PD-3 | Background | #1e1e1e (darker, read-only look) |
| PD-4 | Text color | #888888 (muted) |
| PD-5 | Editable | No (read-only) |
| PD-6 | Content | "npx @scope/pkg" or "docker image" or "https://..." |

### Authentication Section

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| AH-1 | Divider | Horizontal line, 1px, #333333 |
| AH-2 | Label | "AUTHENTICATION", 11pt bold, #888888 |
| AH-3 | Spacing | 8px above divider, 8px below label |

**Auth Method Dropdown:**

| ID | Element | Spec |
|----|---------|------|
| AM-1 | Label | "AUTH METHOD", 11pt, #888888 |
| AM-2 | Dropdown | NSPopUpButton, 360px wide |
| AM-3 | Options | "None", "API Key", "Key File", "OAuth" |
| AM-4 | Default | Auto-detected from env vars, else "None" |
| AM-5 | Effect | Shows/hides appropriate auth fields |

**Auth Detection from Registry:**

| Env Var Pattern | Suggested Auth Method |
|-----------------|----------------------|
| `*_TOKEN`, `*_PAT`, `*_API_KEY`, `*_SECRET` | API Key |
| `*_CLIENT_ID` AND `*_CLIENT_SECRET` | OAuth |
| No secret env vars | None |

**Conditional Display:**

| Auth Method | Shows | Hides |
|-------------|-------|-------|
| "None" | Info message | All auth fields |
| "API Key" | API Key field + Mask checkbox | Key File, OAuth |
| "Key File" | Key File field + Browse button | API Key, OAuth |
| "OAuth" | Authorize button + Status | API Key, Key File |

### API Key Field (visible when Auth Method = "API Key")

| ID | Element | Spec |
|----|---------|------|
| AK-1 | Label | Env var name from registry (e.g., "GITHUB_TOKEN"), 11pt, #888888 |
| AK-2 | Mask checkbox | "Mask", small, right of label |
| AK-3 | Field | NSTextField (single-line), 360px x 24px |
| AK-4 | Mode: masked | NSSecureTextField behavior, shows dots |
| AK-5 | Mode: unmasked | Regular NSTextField, shows text |
| AK-6 | Placeholder | "Enter token..." |
| AK-7 | Background | #2a2a2a |
| AK-8 | Mask default | Checked |

**Input Sanitization (API Key):**

| ID | Rule | Spec |
|----|------|------|
| AS-1 | Single-line | Field does NOT allow line breaks |
| AS-2 | Trim | Remove leading/trailing whitespace on save |
| AS-3 | Strip newlines | Remove all `\n` and `\r` on every input/paste |
| AS-4 | On paste | Sanitize immediately, then apply mask if enabled |

### Key File Field (visible when Auth Method = "Key File")

| ID | Element | Spec |
|----|---------|------|
| KF-1 | Label | "KEY FILE", 11pt, #888888 |
| KF-2 | Container | HStack |
| KF-3 | Field | NSTextField (single-line), 290px x 24px |
| KF-4 | Placeholder | "/path/to/token_file" |
| KF-5 | Browse button | 60px, right of field |
| KF-6 | Browse action | Open file picker dialog |
| KF-7 | Display | Plain text (never masked) |

**Input Sanitization (Key File):**

| ID | Rule | Spec |
|----|------|------|
| KS-1 | Single-line | Field does NOT allow line breaks |
| KS-2 | Trim | Remove leading/trailing whitespace on save |
| KS-3 | Strip newlines | Remove all `\n` and `\r` on every input/paste |

### OAuth Section (visible when Auth Method = "OAuth")

| ID | Element | Spec |
|----|---------|------|
| OA-1 | Button | "Authorize with {Provider}", 360px x 36px |
| OA-2 | Button style | Prominent, provider color or accent |
| OA-3 | Status label | Below button, 11pt |
| OA-4 | Status: not connected | "Status: Not connected", #888888 |
| OA-5 | Status: connected | "Connected as @username", #4ade80 (green) |
| OA-6 | Status: error | "Error: {message}", #ef4444 (red) |

### None Auth Section (visible when Auth Method = "None")

| ID | Element | Spec |
|----|---------|------|
| NA-1 | Message | "No authentication required for this MCP." |
| NA-2 | Style | 11pt, #888888 |

### Configuration Section (only if MCP has configSchema)

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| CH-1 | Divider | Horizontal line, 1px, #333333 |
| CH-2 | Label | "CONFIGURATION", 11pt bold, #888888 |
| CH-3 | Visibility | Only when configSchema exists |

**Dynamic Fields from configSchema:**

| Schema Type | UI Element | Spec |
|-------------|------------|------|
| string | NSTextField | 360px wide, single-line |
| boolean | NSButton checkbox | Checkbox with label |
| array of strings | List with add/remove | See Array Field below |
| number | NSTextField | With validation |

**Array Field (e.g., ALLOWED_PATHS):**

| ID | Element | Spec |
|----|---------|------|
| AF-1 | Container | NSStackView, 360px wide |
| AF-2 | Background | #1e1e1e |
| AF-3 | Border | 1px #333333, 4px radius |
| AF-4 | Item row | Text + [-] remove button |
| AF-5 | Item height | 28px |
| AF-6 | Add row | [+ Add {label}] button at bottom |
| AF-7 | Remove action | Removes that item from array |
| AF-8 | Add action | Adds empty row for input |

---

## Behavioral Requirements

### View Loading (New MCP from Add View)

| Step | Action |
|------|--------|
| 1 | Receive SelectedMcp from Add View |
| 2 | Generate new UUID for MCP |
| 3 | Pre-fill name from display_name |
| 4 | Set package display (read-only) |
| 5 | Detect auth method from env_vars |
| 6 | Set Auth Method dropdown |
| 7 | Generate config fields from configSchema |
| 8 | Focus name field |

### View Loading (Edit MCP from Settings)

| Step | Action |
|------|--------|
| 1 | Receive MCP ID from Settings |
| 2 | Call McpService.get(id) |
| 3 | Populate name from config |
| 4 | Set package display |
| 5 | Set Auth Method from config |
| 6 | Populate auth field (masked if API key) |
| 7 | Populate config fields from saved config |
| 8 | Focus name field |

### Auth Method Change Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Select different Auth Method | |
| 2a | If "None" | Hide all auth fields, show info message |
| 2b | If "API Key" | Show API Key field + Mask, hide others |
| 2c | If "Key File" | Show Key File field + Browse, hide others |
| 2d | If "OAuth" | Show Authorize button + Status, hide others |
| 3 | | Clear the hidden fields' values |

### Mask Toggle Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Mask checkbox | |
| 2a | If checking | Convert field to secure (masked) |
| 2b | If unchecking | Convert field to plain text |
| 3 | | Preserve field content |

### Browse Keyfile Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Browse] | |
| 2 | | Open NSOpenPanel file picker |
| 3 | | Filter: all files |
| 4 | | Start directory: home |
| 5 | On file select | Sanitize path (trim, strip newlines) |
| 6 | | Set keyfile field to selected path |

### OAuth Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Authorize button | |
| 2 | | Generate random state parameter |
| 3 | | Build OAuth authorization URL |
| 4 | | Open URL in default browser |
| 5 | | Set status to "Authorizing..." |
| 6 | User completes auth in browser | |
| 7 | | Callback via URL scheme (personalagent://oauth/callback) |
| 8 | | Exchange code for tokens |
| 9 | | Store tokens securely |
| 10 | | Update status to "Connected as @username" |
| 11 | On error | Set status to error message |

### Array Field - Add Item

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [+ Add {label}] | |
| 2 | | Add new empty row to list |
| 3 | | Focus the new row's text field |

### Array Field - Remove Item

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [-] on item row | |
| 2 | | Remove that row from list |
| 3 | | Reflow remaining rows |

### Save Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Save | |
| 2 | | Validate name non-empty |
| 3 | | Validate auth (if required) |
| 4 | | Sanitize all text fields |
| 5 | | Build McpConfig from form |
| 6 | | Store secrets securely (API key, tokens) |
| 7 | | Call McpService.create() or .update() |
| 8 | | Navigate back to Settings |

### Cancel Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Cancel | |
| 2 | | Discard all changes |
| 3 | | Navigate back (to Add View if new, Settings if edit) |
| 4 | | No confirmation needed |

---

## Validation Rules

| Field | Rule | Error Handling |
|-------|------|----------------|
| Name | Non-empty after trim | Highlight field, disable Save |
| API Key (if method=API Key) | Non-empty after trim | Highlight field, disable Save |
| Keyfile (if method=Key File) | Non-empty, file exists | Highlight field, show error |
| OAuth (if method=OAuth) | Must be connected | Disable Save until authorized |
| Config fields | Per schema (required, type) | Highlight invalid fields |

**Save enabled when:**
- Name is non-empty AND
- (Auth Method = "None") OR
- (Auth Method = "API Key" AND api_key non-empty) OR
- (Auth Method = "Key File" AND keyfile non-empty) OR
- (Auth Method = "OAuth" AND connected)

---

## Data Model

**Input from MCP Add View:**

```rust
struct SelectedMcp {
    name: String,                    // "GitHub"
    source: McpSource,               // Official, Smithery, or Manual
    package: String,                 // "npx @modelcontextprotocol/server-github"
    env_vars: Vec<EnvVarSpec>,       // [{name: "GITHUB_TOKEN", secret: true, ...}]
    config_schema: Option<JsonSchema>,
}
```

**Output to McpService:**

```rust
struct McpConfig {
    id: Uuid,
    name: String,
    enabled: bool,                   // Default true for new
    source: McpSource,
    package: String,
    auth_method: AuthMethod,
    api_key: Option<String>,         // Sanitized, stored in secrets
    keyfile_path: Option<PathBuf>,   // Sanitized
    oauth_tokens: Option<OAuthTokens>,
    config: serde_json::Value,       // From configSchema fields
}

enum AuthMethod {
    None,
    ApiKey { env_var_name: String }, // e.g., "GITHUB_TOKEN"
    KeyFile { env_var_name: String },
    OAuth { provider: String },
}

struct OAuthTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    username: Option<String>,
}
```

---

## State Management

### View State

| Field | Type | Purpose |
|-------|------|---------|
| mcp_id | Option<Uuid> | None for new, Some for edit |
| is_new | bool | Create vs edit mode |
| selected_mcp | Option<SelectedMcp> | Context from Add View |
| auth_method | AuthMethod | Current selection |
| mask_enabled | bool | API key masking |
| oauth_connected | bool | OAuth status |
| oauth_username | Option<String> | Connected user |
| config_values | HashMap<String, Value> | Dynamic config |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| name_field | NSTextField | MCP name |
| package_label | NSTextField | Package display |
| auth_method_popup | NSPopUpButton | Auth selection |
| api_key_field | NSTextField | API key input |
| mask_checkbox | NSButton | Toggle masking |
| keyfile_field | NSTextField | Keyfile path |
| browse_button | NSButton | File picker |
| oauth_button | NSButton | Authorize |
| oauth_status | NSTextField | Connection status |
| config_fields | HashMap<String, NSView> | Dynamic fields |
| save_button | NSButton | Save action |

---

## Service Dependencies

| Action | Service | Method |
|--------|---------|--------|
| Load MCP | McpService | get(id) |
| Create MCP | McpService | create(config) |
| Update MCP | McpService | update(config) |
| Store secret | SecretsService | store(mcp_id, key, value) |
| OAuth exchange | OAuthService | exchange_code(code, state) |

---

## Test Coverage

### Visual Tests

- [ ] Name field 360px wide
- [ ] Package display read-only appearance
- [ ] Auth fields shown/hidden based on dropdown
- [ ] API key masked when Mask checked
- [ ] OAuth button prominent styling
- [ ] Array fields show add/remove buttons
- [ ] Config section only shown when schema exists

### Interaction Tests

- [ ] Save disabled when name empty
- [ ] Save enabled when Auth Method = "None"
- [ ] Save disabled when API Key required but empty
- [ ] Browse opens file picker
- [ ] Mask toggle switches field type
- [ ] Auth Method change shows correct fields
- [ ] Array add creates new row
- [ ] Array remove deletes row
- [ ] Cancel returns without saving

### Sanitization Tests

- [ ] API key trimmed on save
- [ ] API key newlines stripped on input
- [ ] Keyfile path trimmed on save
- [ ] Keyfile path newlines stripped
- [ ] Paste into masked field sanitizes

### OAuth Tests

- [ ] Authorize opens browser
- [ ] Callback updates status
- [ ] Connected shows username
- [ ] Error shows message
