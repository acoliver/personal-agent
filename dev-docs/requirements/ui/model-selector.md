# Model Selector View Requirements

The Model Selector is the first step in adding a new profile. Users select a provider and model from the models.dev registry. On selection, navigates to Profile Editor to complete setup. **The view is purely presentational** - it renders data from ModelsRegistryService and forwards selections.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, #1a1a1a)                                      │
│                                                              │
│  [Cancel]           Select Model                             │
│   70px               14pt bold, centered                     │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ FILTER BAR (36px, #0f0f0f)                                   │
│                                                              │
│  [Search models...____________]  Provider: [All        v]    │
│   flex width                      100px dropdown             │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ CAPABILITY TOGGLES (28px, #0f0f0f)                           │
│                                                              │
│  [ ] Reasoning   [ ] Vision                                  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ COLUMN HEADER (20px, sticky)                                 │
│                                                              │
│  Model                          Context   In $    Out $      │
│  ─────────────────────────────────────────────────────────── │
├──────────────────────────────────────────────────────────────┤
│ MODEL LIST (flex height, scrollable, #0f0f0f)                │
│                                                              │
│  ┌─ Anthropic ─────────────────────────────────────────────┐ │
│  │ claude-3-5-sonnet-20241022   200K  R V    $3      $15   │ │
│  │ claude-3-opus-20240229       200K  R V    $15     $75   │ │
│  │ claude-3-haiku-20240307      200K    V    $0.25   $1.25 │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─ OpenAI ────────────────────────────────────────────────┐ │
│  │ gpt-4o                       128K    V    $5      $15   │ │
│  │ gpt-4o-mini                  128K    V    $0.15   $0.60 │ │
│  │ gpt-4-turbo                  128K    V    $10     $30   │ │
│  └─────────────────────────────────────────────────────────┘ │
│  ┌─ Google ────────────────────────────────────────────────┐ │
│  │ gemini-1.5-pro               1M      V    $3.50   $10.5 │ │
│  │ gemini-1.5-flash             1M      V    $0.075  $0.30 │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ STATUS BAR (24px, #1a1a1a)                                   │
│                                                              │
│  142 models from 12 providers                                │
│                                                              │
└──────────────────────────────────────────────────────────────┘

── EMPTY STATE (when no results) ────────────────────────────────

│                                                              │
│              No models match your filters.                   │
│                                                              │
│              Try adjusting the capability                    │
│              filters or search term.                         │
│                                                              │
```

---

## Layout Specifications

### Overall Dimensions

| Property | Value | Notes |
|----------|-------|-------|
| Popover width | 400px | Same as other views |
| Popover height | 500px | Same as other views |
| Background | #0f0f0f | Theme.BG_DARKEST |

### Spacing Standards

| Context | Value | Notes |
|---------|-------|-------|
| Content padding | 12px | Horizontal edges |
| Row height | 28px | Model rows |
| Provider header height | 24px | Section headers |
| Row spacing | 0px | Compact, no gaps |

### Typography

| Element | Font | Size | Color |
|---------|------|------|-------|
| "Select Model" title | System Bold | 14pt | #e5e5e5 |
| Provider header | System Bold | 12pt | #e5e5e5 |
| Model ID | System Regular | 11pt | #e5e5e5 |
| Column header | System Regular | 10pt | #888888 |
| Status text | System Regular | 11pt | #888888 |

---

## Component Requirements

### Top Bar

**Layout:** 44px height, #1a1a1a background

```
[12px] [Cancel 70px] [spacer] [Select Model] [spacer] [12px]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | Cancel button | 70px wide, left side | Navigate back to Settings |
| TB-2 | Title | "Select Model", 14pt bold, centered | Static |
| TB-3 | Layout | Title centered between spacers | Balanced |

### Filter Bar

**Layout:** 36px height, #0f0f0f background

```
[12px] [Search field, flex] [8px] [Provider:] [Dropdown 100px] [12px]
```

| ID | Element | Spec |
|----|---------|------|
| FB-1 | Search field | NSSearchField, flexible width |
| FB-2 | Search placeholder | "Search models..." |
| FB-3 | Search background | #2a2a2a |
| FB-4 | Provider label | "Provider:", 12pt, #888888 |
| FB-5 | Provider dropdown | NSPopUpButton, 100px wide |
| FB-6 | Dropdown default | "All" |
| FB-7 | Dropdown items | One per provider from registry |

### Capability Toggles

**Layout:** 28px height, #0f0f0f background

```
[12px] [Reasoning checkbox] [12px] [Vision checkbox] [spacer] [12px]
```

| ID | Element | Spec |
|----|---------|------|
| CT-1 | Reasoning checkbox | NSButton Switch type |
| CT-2 | Vision checkbox | NSButton Switch type |
| CT-3 | Default state | Both unchecked (show all) |
| CT-4 | Checked behavior | Filter to models with capability |

### Column Header

**Layout:** 20px height, sticky at top of scroll area

```
Model                          Context   In $    Out $
───────────────────────────────────────────────────────
```

| ID | Element | Spec |
|----|---------|------|
| CH-1 | Header row | Sticky, doesn't scroll |
| CH-2 | Background | #0f0f0f (matches list) |
| CH-3 | Text | 10pt, #888888 |
| CH-4 | Columns | Model (flex), Context (50px), In $ (50px), Out $ (50px) |
| CH-5 | Separator | 1px line below header, #333333 |
| CH-6 | Alignment | Model left, others right-aligned |

### Provider Sections

| ID | Element | Spec |
|----|---------|------|
| PS-1 | Header height | 24px |
| PS-2 | Header background | #121212 (slightly lighter) |
| PS-3 | Header text | Provider name, 12pt bold, #e5e5e5 |
| PS-4 | Header padding | 8px left |
| PS-5 | Collapsible | No (always expanded) |

### Model Rows

**Layout:** 28px height, full width, compact table style

```
[8px] [Model ID, flex] [Context 50px] [Caps 40px] [In$ 50px] [Out$ 50px] [8px]
```

| ID | Element | Spec |
|----|---------|------|
| MR-1 | Row height | 28px |
| MR-2 | Row width | Full width of list |
| MR-3 | Background | Transparent (inherits #0f0f0f) |
| MR-4 | Hover background | #1a1a1a |
| MR-5 | Model ID | 11pt, #e5e5e5, left-aligned, truncate tail |
| MR-6 | Context column | 50px, right-aligned |
| MR-7 | Context format | "200K", "128K", "1M" (from service) |
| MR-8 | Capabilities column | 40px, center-aligned |
| MR-9 | Capability indicators | "R" = Reasoning, "V" = Vision |
| MR-10 | Input cost column | 50px, right-aligned |
| MR-11 | Output cost column | 50px, right-aligned |
| MR-12 | Cost format | "$X.XX" or "free" |
| MR-13 | Click target | Entire row |
| MR-14 | Click action | Navigate to Profile Editor with model |

### Cost Formatting (UI Only)

**The UI receives numeric cost values from the service (already in $/million tokens). UI only formats for display:**

| Service Value | UI Display |
|---------------|------------|
| 0.0 | "free" |
| 0.25 | "$0.25" |
| 3.0 | "$3" |
| 15.0 | "$15" |
| 0.075 | "$0.08" (round to 2 decimals) |

**Rules:**
- If value is 0, display "free"
- If value is whole number, no decimals: "$3"
- If value has decimals, show up to 2: "$0.25", "$3.50"
- Add "$" prefix
- **NO multiplication or division** - service provides ready values

### Context Formatting (UI Only)

**The UI receives numeric context values from the service. UI only formats for display:**

| Service Value | UI Display |
|---------------|------------|
| 131072 | "131K" |
| 200000 | "200K" |
| 1000000 | "1M" |
| 32000 | "32K" |

**Rules:**
- >= 1,000,000: show as "XM"
- >= 1,000: show as "XK"
- < 1,000: show raw number
- **NO other manipulation** - just format for display

### Status Bar

**Layout:** 24px height, #1a1a1a background

| ID | Element | Spec |
|----|---------|------|
| SB-1 | Status text | "X models from Y providers" |
| SB-2 | Text style | 11pt, #888888 |
| SB-3 | Alignment | Left, 12px padding |
| SB-4 | Updates | On filter change |

### Empty State

| ID | Element | Spec |
|----|---------|------|
| ES-1 | Visibility | When filtered list is empty |
| ES-2 | Primary text | "No models match your filters." |
| ES-3 | Secondary text | "Try adjusting the capability filters or search term." |
| ES-4 | Text style | 13pt, #888888, centered |
| ES-5 | Layout | Centered in scroll area |

---

## Behavioral Requirements

### View Loading Flow

| Step | Action | Visual |
|------|--------|--------|
| 1 | View appears | |
| 2 | Call ModelsRegistryService.providers() | |
| 3 | Call ModelsRegistryService.all_models() | |
| 4 | Populate provider dropdown | "All" + provider names |
| 5 | Render column header | Sticky |
| 6 | Render provider sections + model rows | Grouped |
| 7 | Update status bar | "X models from Y providers" |
| 8 | Focus search field | Ready for input |

### Search Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User types in search field | |
| 2 | | Filter models where ID contains query (case-insensitive) |
| 3 | | Re-render rows instantly |
| 4 | | Update status bar count |
| 5 | | If no results, show empty state |

### Provider Filter Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User selects provider from dropdown | |
| 2 | | If "All": show all providers |
| 3 | | If specific: show only that provider's section |
| 4 | | Re-render rows |
| 5 | | Update status bar |
| 6 | | Search filter still applies within selection |

### Capability Filter Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User checks Reasoning checkbox | |
| 2 | | Filter to models where reasoning=true |
| 3 | | Re-render rows |
| 4 | | Same for Vision checkbox |
| 5 | | Filters combine (AND logic) |

### Model Selection Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User clicks model row | |
| 2 | | Get model info (provider_id, model_id, base_url, context) |
| 3 | | Navigate to Profile Editor |
| 4 | | Pass model info for pre-population |

### Cancel Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | User clicks Cancel | |
| 2 | | Navigate back to Settings |
| 3 | | No model selected |

---

## Data Model (from Service)

```rust
struct ModelInfo {
    id: String,              // "claude-3-5-sonnet-20241022"
    provider_id: String,     // "anthropic"
    context: u64,            // 200000 (raw value, UI formats)
    reasoning: bool,         // true
    vision: bool,            // true
    cost_input: f64,         // 3.0 ($/million, UI formats)
    cost_output: f64,        // 15.0 ($/million, UI formats)
}

struct Provider {
    id: String,              // "anthropic"
    name: String,            // "Anthropic"
    api_url: String,         // "https://api.anthropic.com/v1"
}
```

**Important:** Cost values from service are already in dollars per million tokens. UI does NOT multiply or divide - only formats with "$" prefix and decimal handling.

---

## State Management

### View State

| Field | Type | Purpose |
|-------|------|---------|
| search_query | String | Current search text |
| selected_provider | Option<String> | Provider filter ("All" = None) |
| reasoning_filter | bool | Reasoning checkbox state |
| vision_filter | bool | Vision checkbox state |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| search_field | NSSearchField | Search input |
| provider_popup | NSPopUpButton | Provider dropdown |
| reasoning_checkbox | NSButton | Filter toggle |
| vision_checkbox | NSButton | Filter toggle |
| models_container | NSStackView | Row container |
| scroll_view | NSScrollView | List scroll |
| status_label | NSTextField | Status bar text |

---

## Service Dependencies

| Action | Service | Method |
|--------|---------|--------|
| Get providers | ModelsRegistryService | providers() |
| Get all models | ModelsRegistryService | all_models() |
| Search models | ModelsRegistryService | search(query) |
| Refresh cache | ModelsRegistryService | refresh() |

## Service Calls

| User Action | Service Method | Success Response | Error Response | UI State Change |
|-------------|----------------|------------------|----------------|-----------------|
| View appears | ModelsRegistryService.providers() + all_models() | Providers + ModelInfo list | Error {code,message} | Render model list or show #error-banner |
| Search input | ModelsRegistryService.search(query) | ModelInfo list | Error {code,message} | Filter results or show empty state |
| Refresh models | ModelsRegistryService.refresh() | Success | Error {code,message} | Update list or show #error-banner |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| UI-MS-NT1 | Search query > 200 chars | Show "Query too long" in #error-banner |
| UI-MS-NT2 | Refresh fails with no cache | Show "Network error" in #error-banner, list empty |
| UI-MS-NT3 | Provider filter yields no results | Show empty state text in #empty-state-label |

---

## Test Coverage

### Visual Tests

- [ ] Column header visible and aligned
- [ ] Model rows align with column header
- [ ] Provider sections visually grouped
- [ ] Costs display with "$" prefix
- [ ] Context displays as "XK" or "XM"
- [ ] Capabilities show "R" and/or "V"
- [ ] Empty state centered when no results

### Interaction Tests

- [ ] Search filters models in real-time
- [ ] Provider dropdown filters to single provider
- [ ] Reasoning checkbox filters correctly
- [ ] Vision checkbox filters correctly
- [ ] Filters combine (search + provider + capabilities)
- [ ] Row click navigates to Profile Editor
- [ ] Cancel returns to Settings

### Data Tests

- [ ] Cost values displayed without multiplication
- [ ] "free" shown when cost is 0
- [ ] Context formatted correctly (K/M)
- [ ] Status bar shows accurate counts
