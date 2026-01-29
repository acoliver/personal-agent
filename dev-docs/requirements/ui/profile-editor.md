# Profile Editor View Requirements

The Profile Editor is the second step in adding a profile (after Model Selector), or for editing an existing profile. User configures name, API key, system prompt, and model parameters. On save, returns to Settings. **The view is purely presentational** - it renders data and forwards user actions to ProfileService.

---

## Visual Reference

```
┌──────────────────────────────────────────────────────────────┐
│ TOP BAR (44px, #1a1a1a)                                      │
│                                                              │
│  [Cancel]           Edit Profile            [Save]           │
│   70px               14pt bold               60px            │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ CONTENT SCROLL AREA (flex height, #121212)                   │
│                                                              │
│  12px padding                                                │
│                                                              │
│  NAME                                      ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ My Claude Profile                                      │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  MODEL                                     ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ claude-3-5-sonnet-20241022                    [Change] │  │
│  └────────────────────────────────────────────────────────┘  │
│   Read-only model ID, Change returns to Model Selector       │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  API TYPE                                  ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ anthropic                                            v │  │
│  └────────────────────────────────────────────────────────┘  │
│   Dropdown: "openai", "anthropic" (auto-detected, editable)  │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  BASE URL                                  ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ https://api.anthropic.com/v1                           │  │
│  └────────────────────────────────────────────────────────┘  │
│   Pre-filled from model selection, editable                  │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  AUTH METHOD                               ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ API Key                                              v │  │
│  └────────────────────────────────────────────────────────┘  │
│   Dropdown: "None", "API Key", "Key File"                    │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  ─── If "API Key" selected: ───                              │
│                                                              │
│  API KEY                               [x] Mask              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●              │  │
│  └────────────────────────────────────────────────────────┘  │
│   Single-line, masked by default                             │
│                                                              │
│  ─── If "Key File" selected: ───                             │
│                                                              │
│  KEY FILE                                  ← 11pt, #888888   │
│  ┌──────────────────────────────────────────────┐ [Browse]   │
│  │ ~/.config/anthropic/api_key                  │            │
│  └──────────────────────────────────────────────┘            │
│   Single-line, plain text path                               │
│                                                              │
│  ─── If "None" selected: ───                                 │
│                                                              │
│  (no auth fields shown)                                      │
│                                                              │
│  16px gap                                                    │
│                                                              │
│  ═══════════════════════════════════════════════════════════ │
│  PARAMETERS                                                  │
│  ═══════════════════════════════════════════════════════════ │
│                                                              │
│  TEMPERATURE                               ← 11pt, #888888   │
│  ┌──────────────┐ ┌───┐                                      │
│  │ 1.0          │ │▲▼│                                       │
│  └──────────────┘ └───┘                                      │
│   Number field (80px) + stepper, range 0.0-2.0, step 0.1     │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  MAX TOKENS                                ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ 4096                                                   │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  CONTEXT LIMIT                             ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ 200000                                                 │  │
│  └────────────────────────────────────────────────────────┘  │
│   Required, pre-filled from model selection                  │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  [x] Show Thinking                                           │
│   Default visibility for thinking content in Chat View       │
│                                                              │
│  12px gap                                                    │
│                                                              │
│  [ ] Enable Extended Thinking                                │
│                                                              │
│  ─── If checked: ───                                         │
│                                                              │
│  THINKING BUDGET                           ← 11pt, #888888   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ 10000                                                  │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  16px gap                                                    │
│                                                              │
│  ═══════════════════════════════════════════════════════════ │
│  SYSTEM PROMPT                                               │
│  ═══════════════════════════════════════════════════════════ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ You are a helpful assistant.                           │  │
│  │                                                        │  │
│  │                                                        │  │
│  └────────────────────────────────────────────────────────┘  │
│   Multi-line text area, 360px wide, 100px tall               │
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
| Button labels | System Medium | 12pt | #e5e5e5 |

---

## Component Requirements

### Top Bar

**Layout:** 44px height, #1a1a1a background

```
[12px] [Cancel 70px] [spacer] [Title] [spacer] [Save 60px] [12px]
```

| ID | Element | Spec | Behavior |
|----|---------|------|----------|
| TB-1 | Cancel button | 70px, left | Discard changes, return to Settings |
| TB-2 | Title | "Edit Profile" or "New Profile" | Context-dependent |
| TB-3 | Save button | 60px, right | Validate and save |
| TB-4 | Save enabled | When name non-empty AND (auth_method="None" OR has valid auth) | Validation |
| TB-5 | Save disabled style | Grayed out, not clickable | Visual feedback |

### Name Field

| ID | Element | Spec |
|----|---------|------|
| NF-1 | Label | "NAME", 11pt, #888888 |
| NF-2 | Field | NSTextField, 360px x 24px |
| NF-3 | Background | #2a2a2a |
| NF-4 | Border | 1px #444444, 4px radius |
| NF-5 | Placeholder | "Profile name" |
| NF-6 | Required | Yes |
| NF-7 | Default (new) | Model ID (e.g., "claude-3-5-sonnet-20241022") |

### Model Display

| ID | Element | Spec |
|----|---------|------|
| MD-1 | Label | "MODEL", 11pt, #888888 |
| MD-2 | Container | HStack, 360px wide |
| MD-3 | Display text | Read-only, model ID only |
| MD-4 | Display style | 12pt, #e5e5e5 |
| MD-5 | Change button | 60px, right side |
| MD-6 | Change action | Navigate to Model Selector, preserve other fields |

### API Type Dropdown

| ID | Element | Spec |
|----|---------|------|
| AT-1 | Label | "API TYPE", 11pt, #888888 |
| AT-2 | Dropdown | NSPopUpButton, 360px wide |
| AT-3 | Options | "Anthropic", "OpenAI" (display), maps to `ApiType` enum |
| AT-4 | Default | From backend (parsed from npm field) |
| AT-5 | Editable | Yes, user can override |
| AT-6 | Purpose | Determines which API client to use |

**ApiType Enum:**

| Display | Enum Value | Use Case |
|---------|------------|----------|
| "Anthropic" | `ApiType::Anthropic` | Anthropic Claude API |
| "OpenAI" | `ApiType::OpenAI` | OpenAI API and OpenAI-compatible (Ollama, LM Studio, etc.) |

**Backend provides clean values (from Model Selector):**

| Backend npm field | ApiType |
|-------------------|---------|
| `@ai-sdk/anthropic` | `ApiType::Anthropic` |
| `@ai-sdk/openai` | `ApiType::OpenAI` |
| `@ai-sdk/openai-compatible` | `ApiType::OpenAI` |
| Any other | `ApiType::OpenAI` |

### Base URL Field

| ID | Element | Spec |
|----|---------|------|
| BU-1 | Label | "BASE URL", 11pt, #888888 |
| BU-2 | Field | NSTextField, 360px x 24px |
| BU-3 | Pre-filled | From selected model's API URL |
| BU-4 | Editable | Yes |
| BU-5 | Required | Yes |
| BU-6 | Placeholder | "https://api.example.com/v1" |

### Auth Method Dropdown

| ID | Element | Spec |
|----|---------|------|
| AM-1 | Label | "AUTH METHOD", 11pt, #888888 |
| AM-2 | Dropdown | NSPopUpButton, 360px wide |
| AM-3 | Options | "None", "API Key", "Key File" (display), maps to `AuthMethod` enum |
| AM-4 | Default | "API Key" |
| AM-5 | Effect | Shows/hides appropriate auth field below |

**AuthMethod Enum Mapping:**

| Display | Enum Value | Use Case |
|---------|------------|----------|
| "None" | `AuthMethod::None` | Local models (Ollama, LM Studio) that don't need auth |
| "API Key" | `AuthMethod::ApiKey` | API key stored securely in SecretsService |
| "Key File" | `AuthMethod::Keyfile { path }` | Read key from file at runtime |

**Conditional Display:**

| Auth Method | Shows | Hides |
|-------------|-------|-------|
| "None" | Nothing | API Key field, Key File field |
| "API Key" | API Key field + Mask checkbox | Key File field |
| "Key File" | Key File field + Browse button | API Key field + Mask checkbox |

### API Key Field (visible when Auth Method = "API Key")

| ID | Element | Spec |
|----|---------|------|
| AK-1 | Label | "API KEY", 11pt, #888888, with Mask checkbox right |
| AK-2 | Field | NSTextField (single-line), 360px x 24px |
| AK-3 | Mode: masked | NSSecureTextField behavior, shows dots |
| AK-4 | Mode: unmasked | Regular NSTextField, shows text |
| AK-5 | Placeholder | "sk-..." |
| AK-6 | Background | #2a2a2a |
| AK-7 | Border | 1px #444444, 4px radius |

**Mask Checkbox:**

| ID | Element | Spec |
|----|---------|------|
| MK-1 | Checkbox | "Mask", small, right of API KEY label |
| MK-2 | Default | Checked |
| MK-3 | Visibility | Only when Auth Method = "API Key" |
| MK-4 | Effect | Toggles secure/plain text field |

**Input Sanitization (API Key):**

| ID | Rule | Spec |
|----|------|------|
| AS-1 | Single-line | Field does NOT allow line breaks |
| AS-2 | Trim | Remove leading/trailing whitespace on save |
| AS-3 | Strip newlines | Remove all `\n` and `\r` on every input/paste |
| AS-4 | On paste | Sanitize immediately (strip newlines, trim), then apply mask if enabled |
| AS-5 | On type | Strip `\n` and `\r` on each keystroke |

### Key File Field (visible when Auth Method = "Key File")

| ID | Element | Spec |
|----|---------|------|
| KF-1 | Label | "KEY FILE", 11pt, #888888 |
| KF-2 | Container | HStack |
| KF-3 | Field | NSTextField (single-line), 290px x 24px |
| KF-4 | Placeholder | "/path/to/api_key" |
| KF-5 | Browse button | 60px, right of field |
| KF-6 | Browse action | Open file picker dialog |
| KF-7 | Display | Plain text (never masked) |

**Input Sanitization (Key File):**

| ID | Rule | Spec |
|----|------|------|
| KS-1 | Single-line | Field does NOT allow line breaks |
| KS-2 | Trim | Remove leading/trailing whitespace on save |
| KS-3 | Strip newlines | Remove all `\n` and `\r` on every input/paste |
| KS-4 | No masking | Always show plain path text |

### Parameters Section

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| PH-1 | Divider | Horizontal line, 1px, #333333 |
| PH-2 | Label | "PARAMETERS", 11pt bold, #888888 |
| PH-3 | Spacing | 8px above divider, 8px below label |

**Temperature Field:**

| ID | Element | Spec |
|----|---------|------|
| TM-1 | Label | "TEMPERATURE", 11pt, #888888 |
| TM-2 | Container | HStack |
| TM-3 | Number field | NSTextField, 80px x 24px |
| TM-4 | Stepper | NSStepper, right of field |
| TM-5 | Range | 0.0 - 2.0 |
| TM-6 | Step | 0.1 |
| TM-7 | Default | 1.0 |
| TM-8 | Format | One decimal place (e.g., "1.0", "0.7") |
| TM-9 | Validation | Clamp to range on blur |

**Max Tokens Field:**

| ID | Element | Spec |
|----|---------|------|
| MT-1 | Label | "MAX TOKENS", 11pt, #888888 |
| MT-2 | Field | NSTextField (number), 360px x 24px |
| MT-3 | Default | 4096 |
| MT-4 | Validation | Positive integer |
| MT-5 | Placeholder | "4096" |

**Context Limit Field:**

| ID | Element | Spec |
|----|---------|------|
| CL-1 | Label | "CONTEXT LIMIT", 11pt, #888888 |
| CL-2 | Field | NSTextField (number), 360px x 24px |
| CL-3 | Default (new) | Pre-filled from Model Selector's `context` value |
| CL-4 | Default (edit) | Existing value from profile |
| CL-5 | Validation | Required, positive integer |
| CL-6 | Placeholder | "128000" |
| CL-7 | Purpose | Used by HistoryProcessor to truncate context before sending to model |

**Important:** This field must always have a value. Models don't report their context limit at runtime, so we must capture it from models.dev during profile creation.

**Show Thinking Checkbox:**

| ID | Element | Spec |
|----|---------|------|
| ST-1 | Checkbox | "Show Thinking" |
| ST-2 | Style | NSButton checkbox type |
| ST-3 | Default | Checked |
| ST-4 | Purpose | Default visibility for thinking content in Chat View |
| ST-5 | Note | Chat View can toggle at runtime, but this is the default on profile change/app start |

**Extended Thinking Checkbox:**

| ID | Element | Spec |
|----|---------|------|
| ET-1 | Checkbox | "Enable Extended Thinking" |
| ET-2 | Style | NSButton checkbox type |
| ET-3 | Default | Unchecked |
| ET-4 | Effect | Shows/hides Thinking Budget field |

**Thinking Budget Field (visible when Extended Thinking checked):**

| ID | Element | Spec |
|----|---------|------|
| TB-1 | Visibility | Only when Extended Thinking checked |
| TB-2 | Label | "THINKING BUDGET", 11pt, #888888 |
| TB-3 | Field | NSTextField (number), 360px x 24px |
| TB-4 | Default | 10000 |
| TB-5 | Validation | Positive integer |
| TB-6 | Placeholder | "10000" |

### System Prompt Section

**Section Header:**

| ID | Element | Spec |
|----|---------|------|
| SH-1 | Divider | Horizontal line, 1px, #333333 |
| SH-2 | Label | "SYSTEM PROMPT", 11pt bold, #888888 |

**Prompt Field:**

| ID | Element | Spec |
|----|---------|------|
| SP-1 | Field | NSTextView (multi-line), 360px x 100px |
| SP-2 | Background | #2a2a2a |
| SP-3 | Border | 1px #444444, 4px radius |
| SP-4 | Default | "You are a helpful assistant." |
| SP-5 | Scrollable | Yes, vertical |
| SP-6 | Text color | #e5e5e5 |
| SP-7 | Font | System 12pt |

---

## Behavioral Requirements

### New Profile Flow (from Model Selector)

| Step | Action |
|------|--------|
| 1 | Receive model info (model_id, api_type, base_url, context) |
| 2 | Set title to "New Profile" |
| 3 | Pre-fill name with model_id |
| 4 | Pre-fill model display with model_id |
| 5 | Pre-fill API Type from backend |
| 6 | Pre-fill Base URL from backend |
| 7 | Set Auth Method to "API Key", Mask checked |
| 8 | Set parameter defaults |
| 9 | Focus name field, select all text |

### Edit Profile Flow (from Settings)

| Step | Action |
|------|--------|
| 1 | Receive profile ID |
| 2 | Call ProfileService.get(id) |
| 3 | Set title to "Edit Profile" |
| 4 | Populate all fields from profile data |
| 5 | Set Auth Method based on which auth field has value |
| 6 | Set Mask checkbox checked (API Key mode) |
| 7 | Focus name field |

### Save Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Save | |
| 2 | | Validate all fields |
| 3 | | If invalid: highlight first error field |
| 4 | | Sanitize API key / keyfile (trim, strip newlines) |
| 5 | | Build ModelProfile from fields |
| 6 | | If new: call ProfileService.create(profile) |
| 7 | | If edit: call ProfileService.update(profile) |
| 8 | | Navigate back to Settings |

### Cancel Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Cancel | |
| 2 | | Check if any field changed from original |
| 3a | If no changes | Navigate to Settings immediately |
| 3b | If changes | Show "Discard changes?" alert |
| 4 | | [Cancel] dismisses alert |
| 5 | | [Discard] navigates to Settings |

### Change Model Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click [Change] | |
| 2 | | Store current field values |
| 3 | | Navigate to Model Selector |
| 4 | | On model select: return to Profile Editor |
| 5 | | Update model display with new model_id |
| 6 | | Update API Type from new model |
| 7 | | Update Base URL from new model |
| 8 | | Keep name, auth, parameters unchanged |

### Auth Method Change Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Select different Auth Method | |
| 2a | If "None" | Hide API Key field, hide Key File field |
| 2b | If "API Key" | Show API Key field + Mask, hide Key File field |
| 2c | If "Key File" | Show Key File field + Browse, hide API Key field |
| 3 | | Clear the hidden field's value |

### Mask Toggle Flow

| Step | Trigger | Action |
|------|---------|--------|
| 1 | Click Mask checkbox | |
| 2a | If checking | Convert API Key field to secure (masked) |
| 2b | If unchecking | Convert API Key field to plain text |
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

---

## Validation Rules

| Field | Rule | Error Handling |
|-------|------|----------------|
| Name | Non-empty after trim | Highlight field, disable Save |
| Base URL | Non-empty, valid URL format | Highlight field |
| API Key (if method=API Key) | Non-empty after trim | Highlight field, disable Save |
| Keyfile (if method=Key File) | Non-empty, file exists | Highlight field, show error |
| Temperature | 0.0 - 2.0 | Clamp to range |
| Max Tokens | Positive integer | Show error, revert to default |
| Context Limit | Required, positive integer | Highlight field, disable Save |
| Thinking Budget | Positive integer | Show error, revert to default |

**Save enabled when:**
- Name is non-empty AND
- Base URL is non-empty AND
- (Auth Method = "None") OR (Auth Method = "API Key" AND api_key non-empty) OR (Auth Method = "Key File" AND keyfile non-empty)

---

## Data Model

**Input from Model Selector:**

```rust
struct SelectedModel {
    model_id: String,       // "claude-3-5-sonnet-20241022"
    api_type: ApiType,      // ApiType::Anthropic or ApiType::OpenAI
    base_url: String,       // "https://api.anthropic.com/v1"
    context: u64,           // 200000 (used to pre-fill context_limit)
}
```

**Output to ProfileService:**

```rust
struct NewProfile {
    name: String,
    model_id: String,               // "claude-3-5-sonnet-20241022"
    api_type: ApiType,              // Enum: Anthropic, OpenAI
    base_url: Option<String>,       // None = use default for api_type
    auth_method: AuthMethod,
    api_key: Option<String>,        // If AuthMethod::ApiKey, key to store (sanitized)
    system_prompt: String,
    parameters: ModelParameters,
}

enum ApiType {
    Anthropic,
    OpenAI,  // Also used for OpenAI-compatible (Ollama, LM Studio, etc.)
}

enum AuthMethod {
    None,                           // No auth (local models)
    ApiKey,                         // Stored in SecretsService
    Keyfile { path: PathBuf },      // Read from file at runtime
}

struct ModelParameters {
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    context_limit: u32,             // Required - pre-filled from models.dev
    enable_thinking: bool,
    thinking_budget: Option<u32>,
    show_thinking: bool,            // Default visibility in Chat View
}
```

**Note:** For edits, use `ProfileUpdate` which has all fields as `Option<T>` for partial updates.

---

## State Management

### View State

| Field | Type | Purpose |
|-------|------|---------|
| profile_id | Option<Uuid> | None for new, Some for edit |
| is_dirty | bool | Track unsaved changes |
| original_values | HashMap<String, String> | For dirty detection |
| auth_method | AuthMethod | Current auth mode |
| mask_enabled | bool | Whether to mask API key |

### UI References

| Field | Type | Purpose |
|-------|------|---------|
| name_field | NSTextField | Profile name |
| model_label | NSTextField | Model ID display |
| api_type_popup | NSPopUpButton | API type selection |
| base_url_field | NSTextField | Endpoint URL |
| auth_method_popup | NSPopUpButton | Auth method selection |
| api_key_field | NSTextField | API key input |
| mask_checkbox | NSButton | Toggle masking |
| keyfile_field | NSTextField | Keyfile path |
| browse_button | NSButton | File picker |
| temperature_field | NSTextField | Temp value |
| temperature_stepper | NSStepper | Temp control |
| max_tokens_field | NSTextField | Token limit |
| context_limit_field | NSTextField | Context window size |
| show_thinking_checkbox | NSButton | Default thinking visibility |
| thinking_checkbox | NSButton | Enable extended thinking |
| thinking_budget_field | NSTextField | Budget value |
| system_prompt_view | NSTextView | System prompt |
| save_button | NSButton | Save action |

---

## Event Emissions

The Profile Editor View emits `UserEvent` variants on user actions. **The view never calls services directly.**

| User Action | Event Emitted |
|-------------|---------------|
| Click Cancel | `UserEvent::NavigateBack` |
| Click Save | `UserEvent::SaveProfile { profile: ProfileData }` |
| Click [Change] model | `UserEvent::Navigate { to: ViewId::ModelSelector }` |
| Click [Test Connection] | `UserEvent::TestProfileConnection { id }` |

---

## Event Subscriptions

The Profile Editor View receives updates via events (handled by ProfileEditorPresenter, which calls view methods):

| Event | View Update |
|-------|-------------|
| `ProfileEvent::TestStarted` | Show "Testing..." indicator |
| `ProfileEvent::TestCompleted { success, response_time_ms }` | Show success/failure with timing |
| `ProfileEvent::ValidationFailed { errors }` | Highlight invalid fields, show errors |
| `NavigationEvent::Navigated { view: ProfileEditor }` | Load profile data if editing |

---

## Test Coverage

### Visual Tests

- [ ] All fields 360px wide
- [ ] Section dividers visible
- [ ] Auth fields hidden/shown based on Auth Method
- [ ] Thinking Budget hidden when checkbox unchecked
- [ ] API key masked when Mask checked
- [ ] API key visible when Mask unchecked
- [ ] Temperature shows stepper arrows
- [ ] Context Limit field visible
- [ ] Show Thinking checkbox visible

### Interaction Tests

- [ ] Save disabled when name empty
- [ ] Save enabled when Auth Method = "None"
- [ ] Save disabled when Auth Method = "API Key" and key empty
- [ ] Save disabled when Auth Method = "Key File" and path empty
- [ ] Browse opens file picker
- [ ] Change returns to Model Selector and back
- [ ] Cancel with changes shows confirmation
- [ ] Temperature stepper increments by 0.1
- [ ] Mask toggle switches field type
- [ ] Auth Method change shows/hides correct fields
- [ ] Show Thinking checkbox toggles correctly

## Service Calls

| User Action | Service Method | Success Response | Error Response | UI State Change |
|-------------|----------------|------------------|----------------|-----------------|
| View appears (edit) | ProfileService.get(id) | ModelProfile | Error {code,message} | Populate fields or show #error-banner |
| Click Save (new) | ProfileService.create(profile) | Success | Error {code,message,field} | Navigate back to Settings |
| Click Save (edit) | ProfileService.update(profile) | Success | Error {code,message,field} | Navigate back to Settings |
| Validate profile | ProfileService.validate(profile) | ValidationResult | Error {code,message} | Highlight invalid fields |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| UI-PR-NT1 | Save with empty name | Highlight name field, show "Name is required" |
| UI-PR-NT2 | Save with invalid base URL | Highlight base URL field, show "Base URL invalid" |
| UI-PR-NT3 | Save with missing auth | Highlight auth section, disable Save |
| UI-PR-NT4 | Update missing profile id | Show "Profile not found" in #error-banner |


### Sanitization Tests

- [ ] API key trimmed on save
- [ ] API key `\n` stripped on input
- [ ] API key `\r` stripped on input
- [ ] Keyfile path trimmed on save
- [ ] Keyfile path newlines stripped
- [ ] Paste into API key field sanitizes before masking
- [ ] Multi-line paste becomes single line

### Validation Tests

- [ ] Empty name prevents save
- [ ] Empty base URL prevents save
- [ ] Invalid keyfile path shows error
- [ ] Non-numeric max tokens shows error
- [ ] Empty context limit prevents save
- [ ] Non-numeric context limit shows error
- [ ] Temperature clamped to 0-2 range
