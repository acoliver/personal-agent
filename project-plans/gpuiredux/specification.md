# GPUI Redux: Complete UI Implementation

**Plan ID:** PLAN-20250130-GPUIREDUX
**Date:** 2025-01-30
**Status:** Draft

## 1. Overview

This plan implements the PersonalAgent UI in GPUI **exactly as specified** in `dev-docs/requirements/ui/` and the mockup at `project-plans/initial/ui-mockup-v2.html`. The UI will be fully wired to the existing EventBus/Presenter architecture.

### What We're Building

A macOS menu bar application with:
1. **Chat View** - Main conversation interface with streaming, thinking blocks, model labels
2. **History View** - Saved conversations list with load/delete
3. **Settings View** - Profile management, MCP configuration, global hotkey
4. **Model Selector View** - Provider/model picker from models.dev registry
5. **Profile Editor View** - Full profile configuration (auth, parameters, system prompt)
6. **MCP Add View** - Registry search and manual entry for MCPs
7. **MCP Configure View** - Credentials and settings for MCPs

### Architecture Constraints

- Views emit `UserEvent` only - never call services directly
- Views receive `ViewCommand` from presenters - apply state changes
- Bridge connects GPUI (smol) to EventBus (tokio) via flume channels
- All UI matches mockup dimensions, colors, typography exactly

## 2. Existing Assets

### Already Implemented (Keep)
- `src/ui_gpui/bridge/` - flume-based bridge (GpuiBridge, ViewCommandSink, forwarder)
- `src/ui_gpui/theme.rs` - Color palette matching mockup
- `src/main_gpui.rs` - NSStatusItem setup, popup window positioning
- `src/events/types.rs` - UserEvent enum (30 variants)
- `src/presentation/view_command.rs` - ViewCommand enum (42 variants)
- All presenters: ChatPresenter, HistoryPresenter, SettingsPresenter, ErrorPresenter

### To Be Rewritten
- `src/ui_gpui/views/` - All view components (chat, history, settings, main_panel)
- `src/ui_gpui/components/` - Reusable UI components
- `src/ui_gpui/app.rs` - GPUI application setup

### To Be Added
- Model Selector view
- Profile Editor view
- MCP Add view
- MCP Configure view
- Navigation system
- ViewCommand handling in views

## 3. View Specifications Summary

### 3.1 Chat View (dev-docs/requirements/ui/chat.md)

**Layout:** 400x500px popover
- Top bar: Icon, "PersonalAgent", [T][S][H][+][Settings]
- Title bar: Conversation dropdown (200px), model label
- Chat area: Scrollable, user bubbles right (green), assistant left (dark)
- Input bar: Text field, Send/Stop buttons

**Key Features:**
- Streaming with cursor animation
- Thinking blocks (collapsible, blue tint)
- Model label per assistant message
- Editable conversation title
- Stop button during streaming

**Events Emitted:**
- `UserEvent::SendMessage { text }`
- `UserEvent::StopStreaming`
- `UserEvent::NewConversation`
- `UserEvent::SelectConversation { id }`
- `UserEvent::ToggleThinking`
- `UserEvent::Navigate { to: ViewId::* }`

### 3.2 History View (dev-docs/requirements/ui/history.md)

**Layout:** 400x500px
- Top bar: [<] Back, "History"
- Scrollable cards: Title, date, message count, [Load][Delete]
- Empty state when no conversations

**Events Emitted:**
- `UserEvent::Navigate { to: ViewId::Chat }`
- `UserEvent::SelectConversation { id }`
- `UserEvent::DeleteConversation { id }`
- `UserEvent::ConfirmDeleteConversation { id }`

### 3.3 Settings View (dev-docs/requirements/ui/settings.md)

**Layout:** 400x500px
- Top bar: [<] Back, "Settings", [Refresh Models]
- Profiles section: List with [-][+][Edit] toolbar
- MCP Tools section: List with status indicators, toggles, [-][+][Edit]
- Global Hotkey field

**Events Emitted:**
- `UserEvent::Navigate { to: ViewId::Chat }`
- `UserEvent::SelectProfile { id }`
- `UserEvent::DeleteProfile { id }`
- `UserEvent::EditProfile { id }`
- `UserEvent::Navigate { to: ViewId::ModelSelector }`
- `UserEvent::ToggleMcp { id, enabled }`
- `UserEvent::DeleteMcp { id }`
- `UserEvent::ConfigureMcp { id }`
- `UserEvent::RefreshModelsRegistry`

### 3.4 Model Selector View (dev-docs/requirements/ui/model-selector.md)

**Layout:** 400x500px
- Top bar: [Cancel], "Select Model"
- Filter bar: Search field, Provider dropdown
- Capability toggles: Reasoning, Vision
- Column header: Model, Context, In$, Out$
- Provider sections with model rows (name, context, caps, costs)
- Status bar: "X models from Y providers"

**Events Emitted:**
- `UserEvent::NavigateBack`
- `UserEvent::SearchModels { query }`
- `UserEvent::FilterModelsByProvider { provider_id }`
- `UserEvent::SelectModel { provider_id, model_id }`

### 3.5 Profile Editor View (dev-docs/requirements/ui/profile-editor.md)

**Layout:** 400x500px scrollable form
- Top bar: [Cancel], "New/Edit Profile", [Save]
- Name field
- Model display with [Change] button
- API Type dropdown (Anthropic/OpenAI)
- Base URL field
- Auth Method dropdown (None/API Key/Key File)
- Conditional auth fields (API key with mask, keyfile with browse)
- Parameters section: Temperature (stepper), Max Tokens, Context Limit
- Extended Thinking checkbox + budget
- System Prompt textarea

**Events Emitted:**
- `UserEvent::NavigateBack`
- `UserEvent::SaveProfile { profile }`
- `UserEvent::Navigate { to: ViewId::ModelSelector }`

### 3.6 MCP Add View (dev-docs/requirements/ui/mcp-add.md)

**Layout:** 400x500px
- Top bar: [Cancel], "Add MCP", [Next]
- Manual entry field (npx/docker/URL)
- Divider "or search registry"
- Registry dropdown (Official/Smithery/Both)
- Search field (debounced)
- Results list with badges
- Empty/loading states

**Events Emitted:**
- `UserEvent::NavigateBack`
- `UserEvent::SearchMcpRegistry { query, source }`
- `UserEvent::SelectMcpFromRegistry { source }`

### 3.7 MCP Configure View (dev-docs/requirements/ui/mcp-configure.md)

**Layout:** 400x500px scrollable form
- Top bar: [Cancel], "Configure MCP", [Save]
- Name field (editable)
- Package display (read-only)
- Auth Method dropdown (None/API Key/Key File/OAuth)
- Conditional auth fields
- Dynamic config fields from schema (strings, booleans, arrays)

**Events Emitted:**
- `UserEvent::NavigateBack`
- `UserEvent::SaveMcpConfig { id, config }`
- `UserEvent::StartMcpOAuth { id, provider }`

## 4. Component Library

### 4.1 Atomic Components

| Component | Purpose | GPUI Pattern |
|-----------|---------|--------------|
| `Button` | Standard button with variants | Render + click handler |
| `IconButton` | 28x28 icon button | Render + click handler |
| `TextField` | Single-line input | Render + Focusable + key handler |
| `SecureTextField` | Masked input | Same + mask state |
| `TextArea` | Multi-line input | Render + Focusable |
| `Dropdown` | Popup selection | Render + overlay state |
| `Toggle` | On/off switch | Render + click handler |
| `Checkbox` | Checkbox with label | Render + click handler |
| `Stepper` | Number with +/- | Render + value state |
| `List` | Scrollable list | Render + children |
| `ListRow` | Selectable row | Render + click handler |
| `Badge` | Colored pill | Render only |
| `Divider` | Horizontal line | Render only |
| `Spinner` | Loading indicator | Render with animation |

### 4.2 Composite Components

| Component | Contains | Purpose |
|-----------|----------|---------|
| `TopBar` | Title, buttons | View header |
| `MessageBubble` | Text, model label | Chat message |
| `ThinkingBlock` | Collapsible content | Thinking display |
| `ConversationCard` | Title, meta, buttons | History item |
| `ProfileRow` | Name, model, selection | Settings list item |
| `McpRow` | Status, name, toggle | Settings list item |
| `ModelRow` | Name, context, caps, costs | Model selector item |
| `FormField` | Label, input, error | Form layout |
| `ArrayField` | List with add/remove | Config arrays |

## 5. Navigation System

### 5.1 View Stack

```rust
pub enum ViewId {
    Chat,
    History,
    Settings,
    ModelSelector,
    ProfileEditor { id: Option<Uuid> },
    McpAdd,
    McpConfigure { id: Option<Uuid> },
}

pub struct NavigationState {
    stack: Vec<ViewId>,
    current: ViewId,
}
```

### 5.2 Navigation Flow

- Views emit `UserEvent::Navigate { to }` or `UserEvent::NavigateBack`
- Presenter handles via EventBus
- Presenter sends `ViewCommand::Navigate { to }` or `ViewCommand::NavigateBack`
- MainPanel receives command, updates navigation state
- MainPanel renders appropriate view

## 6. ViewCommand Handling

Each view implements a `handle_command` method:

```rust
impl ChatView {
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut Context<Self>) {
        match cmd {
            ViewCommand::Chat(ChatCommand::AppendTextDelta { text }) => {
                self.current_message.push_str(&text);
                cx.notify();
            }
            ViewCommand::Chat(ChatCommand::StreamStarted { model_id }) => {
                self.start_streaming(model_id);
                cx.notify();
            }
            // ... all ChatCommand variants
            _ => {}
        }
    }
}
```

## 7. Test-First Development

Each phase follows TDD:

1. **Write failing tests first** - Component behavior, event emission, command handling
2. **Implement minimally** - Just enough to pass tests
3. **Refactor** - Clean up while tests stay green

### Test Categories

| Category | What We Test |
|----------|--------------|
| Render tests | Component produces expected element tree |
| Event tests | User actions emit correct UserEvent |
| Command tests | ViewCommand updates state correctly |
| Integration tests | Full flow from click to state change |

## 8. Success Criteria

- [ ] All 7 views implemented matching mockup specs exactly
- [ ] All UserEvent variants emitted correctly from views
- [ ] All ViewCommand variants handled correctly in views
- [ ] Bridge connects views to EventBus without data loss
- [ ] Tests cover all event/command paths
- [ ] `cargo build --bin personal_agent_gpui` succeeds
- [ ] `cargo test` passes all UI tests
- [ ] Visual inspection matches mockup

## 9. References

- `dev-docs/requirements/ui/chat.md` - Chat View spec
- `dev-docs/requirements/ui/history.md` - History View spec
- `dev-docs/requirements/ui/settings.md` - Settings View spec
- `dev-docs/requirements/ui/model-selector.md` - Model Selector spec
- `dev-docs/requirements/ui/profile-editor.md` - Profile Editor spec
- `dev-docs/requirements/ui/mcp-add.md` - MCP Add spec
- `dev-docs/requirements/ui/mcp-configure.md` - MCP Configure spec
- `project-plans/initial/ui-mockup-v2.html` - Visual mockup
- `src/events/types.rs` - UserEvent enum
- `src/presentation/view_command.rs` - ViewCommand enum
