# Model Selector Wireframe

**Version:** 1.0  
**Date:** 2026-01-14

## Design Principles

1. **Provider filter at top** - Primary filter, not buried at bottom
2. **Table layout** - Compact, scannable, not chunky bubbles
3. **Auto-filter by tools** - Default ON since we need tool support
4. **Capability toggles** - Quick filter by reasoning, vision
5. **Search box** - Filter by model name/ID

---

## Model Selector View (replaces current profile editor step 1)

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44)------------] | |
| | | [Cancel] "Select Model" <spacer>       | | |
| | [----------------------------------------] | |
| |                                            | |
| | [--------FiltersBar (horiz, h=36)--------] | |
| | | [Search......................] Provider: [All v]| | |
| | [----------------------------------------] | |
| |                                            | |
| | [--------CapabilityToggles (horiz, h=28)-] | |
| | | [x Tools]  [ ] Reasoning  [ ] Vision   | | |
| | [----------------------------------------] | |
| |                                            | |
| | {------ModelsList (flex)-----------------} | |
| | {                                        } | |
| | { anthropic                              } | |
| | { ┌────────────────────┬─────┬────┬─────┐} | |
| | { │ claude-opus-4      │200K │ R V│15/75│} | |
| | { │ claude-sonnet-4    │200K │ R V│ 3/15│} | |
| | { │ claude-3.5-sonnet  │200K │ R V│ 3/15│} | |
| | { └────────────────────┴─────┴────┴─────┘} | |
| | {                                        } | |
| | { openai                                 } | |
| | { ┌────────────────────┬─────┬────┬─────┐} | |
| | { │ gpt-4-turbo        │128K │  V │10/30│} | |
| | { │ gpt-4o             │128K │  V │ 5/15│} | |
| | { │ gpt-4o-mini        │128K │  V │ 0/1 │} | |
| | { └────────────────────┴─────┴────┴─────┘} | |
| | {                                        } | |
| | { openrouter                             } | |
| | { ┌────────────────────┬─────┬────┬─────┐} | |
| | { │ claude-opus-4      │200K │ R V│ 4/16│} | |
| | { │ deepseek-r1        │128K │ R  │ 1/3 │} | |
| | { └────────────────────┴─────┴────┴─────┘} | |
| | {                                        } | |
| | { groq                                   } | |
| | { ┌────────────────────┬─────┬────┬─────┐} | |
| | { │ llama-3.3-70b      │128K │    │ free│} | |
| | { └────────────────────┴─────┴────┴─────┘} | |
| | {                                        } | |
| | {------------------------------------------} | |
| |                                            | |
| | [--------StatusBar (h=24)----------------] | |
| | | "47 models from 12 providers"          | | |
| | [----------------------------------------] | |
| |--------------------------------------------| |
===================================================
```

### Layout Structure

**Provider Section Header:**
- Full provider name as section header (bold, slightly larger)
- Background: slightly lighter than table rows (#1f1f1f)
- Not clickable, just visual grouping

**Model Rows (under each provider):**
| Col | Width | Content |
|-----|-------|---------|
| Model | flex | Model ID |
| Ctx | 45px | Context window: 8K, 32K, 128K, 200K, 1M |
| Caps | 35px | R=reasoning, V=vision (only show if model has it) |
| $/M | 45px | Cost per million tokens (input/output) or "free" |

### Capability Badges

Show in the Caps column:
- R = has reasoning capability
- V = has vision capability
- (blank) = neither

These are informational - the toggle filters above control what's shown.

---

## Interaction Flow

### 1. Open Model Selector
- From Profile Editor "Select Model" button
- Or from Settings "Add Profile" (goes straight here)

### 2. Filter by Provider (optional)
- Dropdown: All, anthropic, openai, google, groq, mistral, ollama, ...
- Default: "All"

### 3. Capability Filters
- **Tools**: ON by default (required for our use case)
- **Reasoning**: OFF by default, toggle to filter
- **Vision**: OFF by default, toggle to filter

### 4. Search (optional)
- Type to filter by model ID or name
- Case-insensitive substring match

### 5. Select Model
- Click row to select
- Proceeds to Step 2: Configure Auth & Parameters

---

## Step 2: Configure Profile (after model selection)

```
===============NSPopover (400x500)=================
| |----------MainStack (vert)------------------| |
| | [--------TopBar (horiz, h=44)------------] | |
| | | [< Back] "Configure Profile" [Save]    | | |
| | [----------------------------------------] | |
| |                                            | |
| | {------FormScroll (flex)----------------} | |
| | { |----FormStack (vert, spacing=12)----| } | |
| | { |                                    | } | |
| | { | [Selected: anthropic:claude-sonnet-4] } | |
| | { | [Change Model]                     | } | |
| | { |                                    | } | |
| | { | "Profile Name"                     | } | |
| | { | [Claude Sonnet________________]    | } | |
| | { |                                    | } | |
| | { | "Base URL"                         | } | |
| | { | [https://api.anthropic.com/v1]     | } | |
| | { |                                    | } | |
| | { | "Authentication"                   | } | |
| | { | [API Key ▼] [sk-...____________]   | } | |
| | { |                                    | } | |
| | { | ─────── Parameters ───────         | } | |
| | { |                                    | } | |
| | { | Temperature    [====●=====] 0.7    | } | |
| | { | Max Tokens     [4096_______]       | } | |
| | { |                                    | } | |
| | { | [[OK]] Enable Thinking                | } | |
| | { |     Budget: [10000_____]           | } | |
| | { | [[OK]] Show Thinking                  | } | |
| | { |                                    | } | |
| | { |------------------------------------| } | |
| | {------------------------------------------} | |
| |--------------------------------------------| |
===================================================
```

---

## Provider Abbreviations

| Abbrev | Provider |
|--------|----------|
| ANT | anthropic |
| OAI | openai |
| GGL | google |
| GRQ | groq |
| MIS | mistral |
| OLL | ollama |
| COH | cohere |
| TOG | together |
| FWK | fireworks |
| DPS | deepseek |
| XAI | xai |
| BDR | bedrock |
| VTX | vertex |

---

## Implementation Notes

### NSTableView vs NSStackView
Use **NSTableView** for the model list:
- Built-in scrolling and selection
- Column sorting (click header)
- More efficient for large lists
- Native look and feel

### Data Flow
```
1. Load ModelRegistry from cache/API
2. Apply filters (provider, capabilities, search)
3. Map to table rows
4. On selection: store (provider_id, model_id)
5. Proceed to config step with pre-filled values
```

### Filter Logic
```rust
fn filter_models(registry: &ModelRegistry, filters: &Filters) -> Vec<ModelRow> {
    registry.search_models(|model| {
        // Tools filter (default ON)
        if filters.require_tools && !model.tool_call {
            return false;
        }
        // Reasoning filter (optional)
        if filters.require_reasoning && !model.reasoning {
            return false;
        }
        // Vision filter (optional)
        if filters.require_vision {
            let has_vision = model.modalities
                .as_ref()
                .map(|m| m.input.contains(&"image".to_string()))
                .unwrap_or(false);
            if !has_vision {
                return false;
            }
        }
        // Search filter
        if !filters.search.is_empty() {
            let search_lower = filters.search.to_lowercase();
            if !model.id.to_lowercase().contains(&search_lower) 
                && !model.name.to_lowercase().contains(&search_lower) {
                return false;
            }
        }
        true
    })
}
```

---

## Cost Formatting

| Raw Cost | Display |
|----------|---------|
| 0.0 / 0.0 | "free" |
| 0.15 / 0.60 | "0.2/1" |
| 3.0 / 15.0 | "3/15" |
| 15.0 / 75.0 | "15/75" |

Round to 1 decimal place, drop trailing zeros.

---

## Context Window Formatting

| Raw Value | Display |
|-----------|---------|
| 8192 | "8K" |
| 32768 | "32K" |
| 128000 | "128K" |
| 200000 | "200K" |
| 1000000 | "1M" |
| 2000000 | "2M" |

---

## Empty State

If no models match filters:
```
┌────────────────────────────────────┐
│                                    │
│    No models match your filters    │
│                                    │
│    Try adjusting the capability    │
│    filters or search term.         │
│                                    │
└────────────────────────────────────┘
```
