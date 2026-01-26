# MCP Add View Requirements

The MCP Add View allows users to discover and add new MCP (Model Context Protocol) servers. Users can either enter a manual command/URL or search registries. **The view is purely presentational** - it renders data from McpRegistryService and forwards selections.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, #1a1a1a)                                      │
│                                                              │
│  [Cancel]              Add MCP               [Next]          │
│   70px                  14pt bold             60px           │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ CONTENT (flex height, #121212)                               │
│                                                              │
│  12px padding                                                │
│                                                              │
│  MANUAL ENTRY                              ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ npx @modelcontextprotocol/server-github               │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, single-line                                    │
│   Accepts: npx command, docker image, http URL               │
│                                                              │
│  ─────────────── or search registry ───────────────          │
│   centered divider, 11pt, #666666                            │
│                                                              │
│  REGISTRY                                  ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ Both                                                 v │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, dropdown                                       │
│   Options: "Official", "Smithery", "Both"                    │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  SEARCH                                    ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ github                                                 │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, search triggers on Enter or 500ms debounce     │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  RESULTS                                   ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  │
│  │▓ server-github                            [Official]  ▓│  │
│  │▓ GitHub API integration for repos, PRs               ▓│  │
│  │▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓│  │
│  │ @anthropic/github-mcp                     [Smithery]   │  │
│  │ Alternative GitHub MCP implementation                  │  │
│  │ mcp-github-tools                          [Smithery]   │  │
│  │ GitHub tools for code review                           │  │
│  └────────────────────────────────────────────────────────┘  │
│   360px wide, 200px tall (scrollable)                        │
│   Selected row: full-width accent highlight                  │
│                                                              │
│  ── Empty state (when no results) ──                         │
│                                                              │
│  No MCPs found matching "xyz".                               │
│  Try a different search term.                                │
│   centered, 13pt, #888888                                    │
│                                                              │
│  ── Loading state ──                                         │
│                                                              │
│  Searching...                                                │
│   centered, with spinner                                     │
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
| Field gap | 12px | Between fields |
| Results height | 200px | Scrollable list |
| Result row height | 48px | Two-line rows |

### Typography

| Element | Font | Size | Color |
|---------|------|------|-------|
| Title | System Bold | 14pt | #e5e5e5 |
| Field labels | System Regular | 11pt | #888888 |
| Divider text | System Regular | 11pt | #666666 |
| Result name | System Bold | 12pt | #e5e5e5 |
| Result description | System Regular | 11pt | #888888 |
| Badge text | System Medium | 9pt | #ffffff |

---

## Component Requirements

### Top Bar

**Layout:** 44px height, #1a1a1a background

```
[12px] [Cancel 70px] [spacer] [Add MCP] [spacer] [Next 60px] [12px]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | Cancel button | 70px, left | Return to Settings, discard |
| TB-2 | Title | "Add MCP", 14pt bold, centered | Static |
| TB-3 | Next button | 60px, right | Proceed to Configure |
| TB-4 | Next enabled | When manual entry has text OR result selected | Validation |
| TB-5 | Next disabled style | Grayed out | Visual feedback |

### Manual Entry Field

| ID | Element | Spec |
|----|---------|------|
| ME-1 | Label | "MANUAL ENTRY", 11pt, #888888 |
| ME-2 | Field | NSTextField, 360px x 24px |
| ME-3 | Background | #2a2a2a |
| ME-4 | Border | 1px #444444, 4px radius |
| ME-5 | Placeholder | "npx @scope/package or docker image or URL" |
| ME-6 | Single-line | No line breaks allowed |

**Accepted Formats:**

| Format | Example | Detection |
|--------|---------|-----------|
| npx command | `npx @modelcontextprotocol/server-github` | Starts with "npx " |
| Docker image | `ghcr.io/owner/mcp-server:latest` | Contains "/" and no space |
| HTTP URL | `https://mcp.example.com/sse` | Starts with "http://" or "https://" |

### Divider

| ID | Element | Spec |
|----|---------|------|
| DV-1 | Text | "── or search registry ──" |
| DV-2 | Style | 11pt, #666666, centered |
| DV-3 | Spacing | 16px above and below |

### Registry Dropdown

| ID | Element | Spec |
|----|---------|------|
| RD-1 | Label | "REGISTRY", 11pt, #888888 |
| RD-2 | Dropdown | NSPopUpButton, 360px wide |
| RD-3 | Options | "Official", "Smithery", "Both" |
| RD-4 | Default | "Both" |

### Search Field

| ID | Element | Spec |
|----|---------|------|
| SF-1 | Label | "SEARCH", 11pt, #888888 |
| SF-2 | Field | NSSearchField, 360px x 24px |
| SF-3 | Placeholder | "Search MCPs..." |
| SF-4 | Trigger | On Enter key OR 500ms debounce after typing |
| SF-5 | Clear button | Built-in X to clear |

### Results List

| ID | Element | Spec |
|----|---------|------|
| RL-1 | Container | NSScrollView, 360px x 200px |
| RL-2 | Background | #1e1e1e |
| RL-3 | Border | 1px #333333, 4px radius |
| RL-4 | Row height | 48px |
| RL-5 | Row spacing | 0px (compact) |

### Result Rows

| ID | Element | Spec |
|----|---------|------|
| RR-1 | Row width | Full width of list |
| RR-2 | Row padding | 8px left/right, 6px top/bottom |
| RR-3 | Name | 12pt bold, #e5e5e5, left-aligned |
| RR-4 | Badge | Right of name, pill shape |
| RR-5 | Description | 11pt, #888888, below name, single line, truncate |
| RR-6 | Normal background | Transparent |
| RR-7 | Hover background | #2a2a2a |
| RR-8 | Selected background | Accent blue, full width |
| RR-9 | Click target | Entire row |

### Badge Styling

| Source | Text | Background | Text Color |
|--------|------|------------|------------|
| Official | "Official" | #2563eb (blue) | #ffffff |
| Smithery | "Smithery" | #16a34a (green) | #ffffff |

**Badge specs:** 6px horizontal padding, 2px vertical padding, 4px radius, 9pt font

### Empty State

| ID | Element | Spec |
|----|---------|------|
| ES-1 | Visibility | When search returns no results |
| ES-2 | Primary text | "No MCPs found matching \"{query}\"." |
| ES-3 | Secondary text | "Try a different search term." |
| ES-4 | Style | 13pt, #888888, centered |
| ES-5 | Layout | Centered in results area |

### Loading State

| ID | Element | Spec |
|----|---------|------|
| LS-1 | Visibility | While search API call in progress |
| LS-2 | Text | "Searching..." |
| LS-3 | Spinner | NSProgressIndicator, small |
| LS-4 | Layout | Centered in results area |

---

## Behavioral Requirements

### View Loading Flow

| Step | Action |
|------|--------|
| 1 | View appears |
| 2 | Set Registry dropdown to "Both" |
| 3 | Clear search field |
| 4 | Clear results |
| 5 | Disable Next button |
| 6 | Focus manual entry field |

### Manual Entry Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User types in manual entry | |
| 2 | | If text non-empty: enable Next, clear result selection |
| 3 | | If text empty: disable Next (unless result selected) |
| 4 | Click Next | |
| 5 | | Detect format (npx/docker/http) |
| 6 | | Create manual McpSource |
| 7 | | Navigate to MCP Configure with manual entry data |

### Search Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User types in search field | |
| 2 | | Start 500ms debounce timer |
| 3 | Timer fires OR Enter pressed | |
| 4 | | Show loading state |
| 5 | | Call McpRegistryService.search(query, registry) |
| 6 | | On success: populate results |
| 7 | | On empty: show empty state |
| 8 | | On error: show error message |
| 9 | | Hide loading state |

### Registry Change Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User selects different registry | |
| 2 | | If search field has text: re-trigger search |
| 3 | | If search field empty: do nothing |

### Result Selection Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User clicks result row | |
| 2 | | Highlight selected row (full width) |
| 3 | | Unhighlight previous selection |
| 4 | | Clear manual entry field |
| 5 | | Enable Next button |

### Proceed to Configure Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Next | |
| 2a | If manual entry has text | |
| 3a | | Parse manual entry format |
| 4a | | Create SelectedMcp with manual source |
| 2b | If result selected | |
| 3b | | Get selected result data |
| 4b | | Create SelectedMcp with registry source |
| 5 | | Navigate to MCP Configure |
| 6 | | Pass SelectedMcp context |

### Cancel Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Cancel | |
| 2 | | Navigate back to Settings |
| 3 | | No confirmation needed |

---

## Data Model

**Search Result (from service):**

```rust
struct McpSearchResult {
    name: String,                    // "server-github"
    display_name: String,            // "GitHub"
    description: String,             // "GitHub API integration..."
    source: McpRegistrySource,       // Official or Smithery
    package: String,                 // "npx @modelcontextprotocol/server-github"
    env_vars: Vec<EnvVarSpec>,       // Required environment variables
    config_schema: Option<JsonSchema>, // Custom config fields
}

enum McpRegistrySource {
    Official { name: String, version: String },
    Smithery { qualified_name: String },
}

struct EnvVarSpec {
    name: String,                    // "GITHUB_TOKEN"
    required: bool,
    secret: bool,                    // Should be masked
    description: Option<String>,
}
```

**Output to MCP Configure:**

```rust
struct SelectedMcp {
    name: String,                    // Display name
    source: McpSource,               // Official, Smithery, or Manual
    package: String,                 // How to run it
    env_vars: Vec<EnvVarSpec>,       // Auth requirements
    config_schema: Option<JsonSchema>,
}
```

## Service Calls

| User Action | Service Method | Success Response | Error Response | UI State Change |
|-------------|----------------|------------------|----------------|-----------------|
| Search MCPs | McpRegistryService.search(query, registry) | Vec<McpSearchResult> | Error {code,message} | Populate results or show empty state |
| Load details | McpRegistryService.get_details(source) | McpSearchResult | Error {code,message} | Enable Next or show #error-banner |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| UI-MA-NT1 | Search with empty query | Show empty state, no request |
| UI-MA-NT2 | Registry request fails | Show "Service unavailable" in #error-banner |
| UI-MA-NT3 | Result selection cleared | Next disabled, no selection highlight |

enum McpSource {
    Official { name: String, version: String },
    Smithery { qualified_name: String },
    Manual,                          // From manual entry
}
```

---

## State Management

### View State

| Field | Type | Purpose |
|-------|------|---------|
| manual_entry | String | Direct command/URL |
| selected_registry | RegistrySource | Dropdown selection |
| search_query | String | Search text |
| search_results | Vec<McpSearchResult> | API results |
| selected_index | Option<usize> | Selected result |
| is_searching | bool | Loading state |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| manual_field | NSTextField | Manual entry input |
| registry_popup | NSPopUpButton | Registry dropdown |
| search_field | NSSearchField | Search input |
| results_scroll | NSScrollView | Results container |
| results_stack | NSStackView | Result rows |
| next_button | NSButton | Proceed button |
| loading_indicator | NSProgressIndicator | Spinner |

---

## Service Dependencies

| Action | Service | Method |
|--------|---------|--------|
| Search MCPs | McpRegistryService | search(query, registry) |
| Get MCP details | McpRegistryService | get_details(source) |

---

## Test Coverage

### Visual Tests

- [ ] Manual entry field 360px wide
- [ ] Registry dropdown shows all options
- [ ] Results list scrollable
- [ ] Selected row full-width highlight
- [ ] Badges show correct colors
- [ ] Empty state centered
- [ ] Loading spinner visible during search

### Interaction Tests

- [ ] Manual entry enables Next
- [ ] Manual entry clears result selection
- [ ] Result selection enables Next
- [ ] Result selection clears manual entry
- [ ] Search triggers on Enter
- [ ] Search triggers on debounce
- [ ] Registry change re-triggers search
- [ ] Cancel returns to Settings
- [ ] Next navigates to Configure

### Data Tests

- [ ] npx format detected correctly
- [ ] Docker format detected correctly
- [ ] HTTP URL format detected correctly
- [ ] Search results parsed correctly
- [ ] Selected data passed to Configure
