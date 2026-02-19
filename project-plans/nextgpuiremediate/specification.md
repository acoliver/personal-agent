# Feature Specification: GPUI Remediation - Event Pipeline and Main Panel Routing

Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
Created: 2026-02-19
Total Phases: TBD (to be determined during implementation planning)

---

## Purpose

This specification defines the comprehensive remediation of the GPUI-based UI system's event pipeline and main panel routing. The goal is to ensure robust, fully-functional integration between UI views, the EventBus, presenters, and services following the established architectural patterns documented in `dev-docs/architecture/gpui-architecture.md` and `dev-docs/requirements/events.md`.

The remediation focuses on six critical flows:
1. **Event Pipeline** - Core event routing infrastructure
2. **Main Panel Routing** - Navigation and view switching
3. **Profile Flow** - Profile CRUD and default selection
4. **Conversation Flow** - Chat messaging, streaming, and persistence
5. **MCP Flow** - MCP configuration, lifecycle, and tool execution
6. **Settings Flow** - Global settings and preferences management

---

## Architectural Decisions

### Pattern: Event-Driven Architecture with Presenter Layer

```
┌─────────────────────────────────────────────────────────────┐
│  GPUI Views (smol runtime)                                  │
│  - Pure rendering, no business logic                        │
│  - Emit UserEvent on user actions                           │
│  - Receive ViewCommand updates via ViewCommandSink          │
└─────────────────────────────────────────────────────────────┘
                          │
          emit(UserEvent) │ ViewCommand
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  GpuiBridge                                                 │
│  - Bridges GPUI (smol) ↔ tokio runtimes                    │
│  - UserEvent forwarding via user_event_forwarder            │
│  - ViewCommand delivery via ViewCommandSink                 │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  EventBus (tokio runtime)                                   │
│  - broadcast::channel<AppEvent>                             │
│  - All events flow through here                             │
│  - Enables logging, debugging, replay                       │
└─────────────────────────────────────────────────────────────┘
                          │
       subscribe + handle │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Presenters (tokio runtime)                                 │
│  - Subscribe to relevant events                             │
│  - Call services                                            │
│  - Send ViewCommands to update UI                           │
│  - Emit domain events                                       │
└─────────────────────────────────────────────────────────────┘
                          │
                   calls  │  emits domain events
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Services (tokio runtime)                                   │
│  - Business logic                                           │
│  - Emit domain events (ChatEvent, McpEvent, etc.)           │
└─────────────────────────────────────────────────────────────┘
```

### Technology Stack

- **Language**: Rust
- **UI Framework**: GPUI (smol async runtime)
- **Backend Runtime**: tokio async runtime
- **Event System**: tokio::sync::broadcast channels
- **Data Persistence**: JSON files in ~/Library/Application Support/PersonalAgent/

### Data Flow Principles

1. **Unidirectional Data Flow**: UserEvent → EventBus → Presenter → Service → ViewCommand → UI
2. **Runtime Isolation**: GPUI runs on smol, business logic on tokio
3. **Bridge Communication**: Async channels bridge the two runtimes
4. **View Purity**: Views only render state and emit events, no business logic

### Integration Points

| Component | Integrates With | Direction |
|-----------|-----------------|-----------|
| ChatView | ChatPresenter | UserEvent → ViewCommand |
| HistoryView | HistoryPresenter | UserEvent → ViewCommand |
| SettingsView | SettingsPresenter | UserEvent → ViewCommand |
| ProfileEditorView | ProfileEditorPresenter | UserEvent → ViewCommand |
| McpAddView | McpAddPresenter | UserEvent → ViewCommand |
| McpConfigureView | McpConfigurePresenter | UserEvent → ViewCommand |
| ModelSelectorView | ModelSelectorPresenter | UserEvent → ViewCommand |
| MainPanel | NavigationState | ViewId routing |
| GpuiBridge | EventBus | Runtime bridging |
| All Presenters | Services (Chat, Profile, Mcp, Conversation, AppSettings) | Service calls |

---

## Project Structure

```
src/
  events/
    mod.rs           # Module exports
    types.rs         # AppEvent, UserEvent, ChatEvent, etc.
    bus.rs           # EventBus implementation
    global.rs        # Global event bus access
    error.rs         # Event-related errors
  presentation/
    mod.rs           # Module exports
    view_command.rs  # ViewCommand and ViewId types
    chat_presenter.rs
    history_presenter.rs
    settings_presenter.rs
    profile_editor_presenter.rs
    mcp_add_presenter.rs
    mcp_configure_presenter.rs
    model_selector_presenter.rs
    error_presenter.rs
  ui_gpui/
    mod.rs           # GPUI module exports
    app.rs           # GpuiApp entry point
    navigation.rs    # NavigationState
    navigation_channel.rs  # Navigation command channel
    theme.rs         # UI theming
    popup_window.rs  # Popup window management
    tray_bridge.rs   # System tray integration
    bridge/
      mod.rs         # Bridge exports
      gpui_bridge.rs # Runtime bridge
      view_command_sink.rs
      user_event_forwarder.rs
    views/
      mod.rs         # View exports
      main_panel.rs  # Root container with navigation
      chat_view.rs
      history_view.rs
      settings_view.rs
      profile_editor_view.rs
      mcp_add_view.rs
      mcp_configure_view.rs
      model_selector_view.rs
    components/
      mod.rs         # Component exports
      message_bubble.rs
      input_bar.rs
      button.rs
      tab_bar.rs
  services/
    mod.rs
    chat_service.rs
    profile_service.rs
    mcp_service.rs
    conversation_service.rs
    app_settings_service.rs
    models_registry_service.rs
    secrets_service.rs
tests/
  events_tests.rs
  presenter_tests.rs
  ui_gpui_tests.rs
```

---

## Technical Environment

- **Type**: macOS Menu Bar App
- **Runtime**: Native macOS with tokio async + GPUI/smol async
- **UI Framework**: GPUI via gpui crate
- **Dependencies** (from Cargo.toml):
  - gpui: GPUI framework
  - tokio: Async runtime
  - uuid: UUID generation
  - serde/serde_json: Serialization
  - chrono: Date/time handling
  - tracing: Structured logging

---

## Integration Points (MANDATORY SECTION)

### Existing Code That Will Use This Feature

1. **src/main_gpui.rs** - Application entry point initializes the event pipeline
2. **src/ui_gpui/app.rs** - GpuiApp coordinates bridge setup
3. **src/ui_gpui/views/*.rs** - All views emit UserEvents and receive ViewCommands
4. **src/presentation/*.rs** - All presenters subscribe to events and emit ViewCommands
5. **src/services/*.rs** - All services emit domain events

### Existing Code To Be Replaced

1. **Direct service calls from views** - Replace with UserEvent emissions
2. **Manual state updates in views** - Replace with ViewCommand pattern
3. **Ad-hoc event handling** - Consolidate into presenter subscriptions
4. **Inconsistent navigation** - Standardize through NavigationState

### User Access Points

- **UI**: Click actions on all views (send, stop, navigate, etc.)
- **Menu bar**: Tray icon click shows/hides popover
- **Hotkey**: Global keyboard shortcut to summon/dismiss

### Runtime Scope Requirements

- No backward-compatibility shim layer will be introduced
- No standalone data migration subsystem will be introduced
- Remediation applies only to active GPUI runtime wiring and behavior
- Runtime behavior must match specification exactly

---

## Formal Requirements

### REQ-EVT: Event Pipeline Requirements

[REQ-EVT-001] EventBus Broadcast
  [REQ-EVT-001.1] EventBus MUST deliver events to all active subscribers
  [REQ-EVT-001.2] Events MUST be delivered in emission order per subscriber
  [REQ-EVT-001.3] Slow subscribers MUST NOT block fast subscribers
  [REQ-EVT-001.4] Lagged subscribers MUST receive lagged notification

[REQ-EVT-002] Runtime Bridge
  [REQ-EVT-002.1] GpuiBridge MUST forward UserEvents from GPUI to tokio EventBus
  [REQ-EVT-002.2] GpuiBridge MUST deliver ViewCommands from tokio to GPUI
  [REQ-EVT-002.3] Bridge MUST handle runtime shutdown gracefully

[REQ-EVT-003] Event Logging
  [REQ-EVT-003.1] All events MUST be logged with structured tracing
  [REQ-EVT-003.2] Logs MUST include event type name and payload
  [REQ-EVT-003.3] Sensitive data in events MUST be redacted in logs

### REQ-NAV: Navigation Requirements

[REQ-NAV-001] Stack-Based Navigation
  [REQ-NAV-001.1] Navigation MUST use a stack with Chat as root
  [REQ-NAV-001.2] Navigate forward MUST push new view onto stack
  [REQ-NAV-001.3] Navigate back MUST pop current view from stack
  [REQ-NAV-001.4] Navigation to same view MUST be a no-op

[REQ-NAV-002] View Rendering
  [REQ-NAV-002.1] MainPanel MUST render the current view from NavigationState
  [REQ-NAV-002.2] View changes MUST trigger re-render
  [REQ-NAV-002.3] Navigation MUST emit NavigationEvent::Navigated

[REQ-NAV-003] Navigation Routing
  [REQ-NAV-003.1] UserEvent::Navigate MUST be handled by navigation system
  [REQ-NAV-003.2] UserEvent::NavigateBack MUST trigger back navigation
  [REQ-NAV-003.3] ViewId MUST map to correct view component

### REQ-PRF: Profile Flow Requirements

[REQ-PRF-001] Profile List Display
  [REQ-PRF-001.1] SettingsView MUST display all profiles from ProfileService
  [REQ-PRF-001.2] Default profile MUST be visually highlighted
  [REQ-PRF-001.3] Profile format: "{name} ({provider}:{model})"

[REQ-PRF-002] Profile Selection
  [REQ-PRF-002.1] Clicking profile row MUST emit UserEvent::SelectProfile
  [REQ-PRF-002.2] SettingsPresenter MUST call AppSettingsService.set_default_profile_id()
  [REQ-PRF-002.3] ProfileEvent::DefaultChanged MUST update view highlight

[REQ-PRF-003] Profile CRUD
  [REQ-PRF-003.1] Add profile MUST navigate to ModelSelector then ProfileEditor
  [REQ-PRF-003.2] Edit profile MUST navigate to ProfileEditor with profile ID
  [REQ-PRF-003.3] Delete profile MUST show confirmation then call ProfileService.delete()
  [REQ-PRF-003.4] ProfileEvent::Created/Updated/Deleted MUST refresh profile list

[REQ-PRF-004] Profile Editor
  [REQ-PRF-004.1] ProfileEditorView MUST populate fields from profile data
  [REQ-PRF-004.2] Save MUST validate and call ProfileService.create() or .update()
  [REQ-PRF-004.3] Auth method change MUST show/hide appropriate auth fields

### REQ-CONV: Conversation Flow Requirements

[REQ-CONV-001] Message Sending
  [REQ-CONV-001.1] Send button/Enter MUST emit UserEvent::SendMessage
  [REQ-CONV-001.2] ChatPresenter MUST call ChatService.send_message()
  [REQ-CONV-001.3] User message MUST appear immediately in chat
  [REQ-CONV-001.4] Assistant response MUST stream via ChatEvent::TextDelta

[REQ-CONV-002] Streaming
  [REQ-CONV-002.1] ChatEvent::StreamStarted MUST show assistant placeholder with cursor
  [REQ-CONV-002.2] ChatEvent::TextDelta MUST append to assistant bubble
  [REQ-CONV-002.3] ChatEvent::ThinkingDelta MUST append to thinking section
  [REQ-CONV-002.4] ChatEvent::StreamCompleted MUST finalize message and remove cursor
  [REQ-CONV-002.5] ChatEvent::StreamCancelled MUST show partial + "[cancelled]"

[REQ-CONV-003] Stop Streaming
  [REQ-CONV-003.1] Stop button MUST emit UserEvent::StopStreaming
  [REQ-CONV-003.2] ChatPresenter MUST call ChatService.cancel()
  [REQ-CONV-003.3] Stream MUST stop and emit ChatEvent::StreamCancelled

[REQ-CONV-004] Conversation Switching
  [REQ-CONV-004.1] Dropdown selection MUST emit UserEvent::SelectConversation
  [REQ-CONV-004.2] ChatPresenter MUST call ConversationService.load()
  [REQ-CONV-004.3] Chat area MUST render loaded conversation messages

[REQ-CONV-005] New Conversation
  [REQ-CONV-005.1] + button MUST emit UserEvent::NewConversation
  [REQ-CONV-005.2] ChatPresenter MUST call ConversationService.create()
  [REQ-CONV-005.3] Chat area MUST clear and show new empty conversation

### REQ-MCP: MCP Flow Requirements

[REQ-MCP-001] MCP List Display
  [REQ-MCP-001.1] SettingsView MUST display all MCPs from McpService
  [REQ-MCP-001.2] MCP status indicator MUST reflect running/stopped/error state
  [REQ-MCP-001.3] Toggle switch MUST enable/disable MCP

[REQ-MCP-002] MCP Toggle
  [REQ-MCP-002.1] Toggle MUST emit UserEvent::ToggleMcp
  [REQ-MCP-002.2] SettingsPresenter MUST call McpService.start() or .stop()
  [REQ-MCP-002.3] McpEvent::Started/Stopped MUST update status indicator

[REQ-MCP-003] MCP Add Flow
  [REQ-MCP-003.1] + button MUST navigate to McpAdd view
  [REQ-MCP-003.2] Search MUST emit UserEvent::SearchMcpRegistry
  [REQ-MCP-003.3] Next MUST navigate to McpConfigure with selected MCP data

[REQ-MCP-004] MCP Configure Flow
  [REQ-MCP-004.1] McpConfigureView MUST populate fields from MCP data
  [REQ-MCP-004.2] Save MUST emit UserEvent::SaveMcpConfig
  [REQ-MCP-004.3] McpConfigurePresenter MUST call McpService.create() or .update()

### REQ-SET: Settings Flow Requirements

[REQ-SET-001] Settings View Loading
  [REQ-SET-001.1] SettingsView MUST load profiles and MCPs on navigation
  [REQ-SET-001.2] SettingsPresenter MUST call ProfileService.list() and McpService.list()
  [REQ-SET-001.3] View MUST render both lists with current states

[REQ-SET-002] Refresh Models
  [REQ-SET-002.1] Refresh button MUST emit UserEvent::RefreshModelsRegistry
  [REQ-SET-002.2] SettingsPresenter MUST call ModelsRegistryService.refresh()
  [REQ-SET-002.3] SystemEvent::ModelsRegistryRefreshed MUST confirm success

---

## Data Schemas

### Event Types (from src/events/types.rs)

```rust
pub enum AppEvent {
    User(UserEvent),
    Chat(ChatEvent),
    Mcp(McpEvent),
    Profile(ProfileEvent),
    Conversation(ConversationEvent),
    Navigation(NavigationEvent),
    System(SystemEvent),
}

pub enum UserEvent {
    SendMessage { text: String },
    StopStreaming,
    NewConversation,
    SelectConversation { id: Uuid },
    ToggleThinking,
    Navigate { to: ViewId },
    NavigateBack,
    SelectProfile { id: Uuid },
    // ... additional variants per requirements
}

pub enum ViewId {
    Chat,
    History,
    Settings,
    ProfileEditor { id: Option<Uuid> },
    McpAdd,
    McpConfigure { id: Uuid },
    ModelSelector,
}
```

### ViewCommand (from src/presentation/view_command.rs)

```rust
pub enum ViewCommand {
    // Chat commands
    AppendUserMessage { text: String },
    AppendAssistantMessage { text: String },
    AppendTextDelta { text: String },
    SetStreaming { streaming: bool },
    // Settings commands
    SetProfiles { profiles: Vec<ProfileItem> },
    SetMcps { mcps: Vec<McpItem> },
    SetDefaultProfile { id: Option<Uuid> },
    // Navigation commands
    Navigate { to: ViewId },
    // ... additional commands per view requirements
}
```

---

## Example Data

### Event Flow: Send Message

```json
{
  "step1_user_action": {
    "event": "UserEvent::SendMessage",
    "payload": { "text": "Hello, Claude!" }
  },
  "step2_presenter_handles": {
    "presenter": "ChatPresenter",
    "action": "calls ChatService.send_message(conversation_id, text)"
  },
  "step3_service_emits": [
    { "event": "ChatEvent::StreamStarted", "payload": { "conversation_id": "...", "model_id": "claude-3-5-sonnet" } },
    { "event": "ChatEvent::TextDelta", "payload": { "text": "Hi" } },
    { "event": "ChatEvent::TextDelta", "payload": { "text": " there!" } },
    { "event": "ChatEvent::StreamCompleted", "payload": { "conversation_id": "...", "total_tokens": 42 } }
  ],
  "step4_view_commands": [
    { "command": "SetStreaming", "payload": { "streaming": true } },
    { "command": "AppendUserMessage", "payload": { "text": "Hello, Claude!" } },
    { "command": "AppendTextDelta", "payload": { "text": "Hi" } },
    { "command": "AppendTextDelta", "payload": { "text": " there!" } },
    { "command": "SetStreaming", "payload": { "streaming": false } }
  ]
}
```

---

## Constraints

1. **No blocking operations on main thread** - All async operations use tokio runtime
2. **UI updates on GPUI thread** - ViewCommands delivered via ViewCommandSink
3. **Event serialization** - All events must be Clone + Debug + Serialize
4. **Follow existing error patterns** - Use ServiceError shape consistently
5. **Respect project conventions** - Match existing code style and patterns

---

## Performance Requirements

| ID | Requirement | Target |
|----|-------------|--------|
| PF-EVT-1 | Event delivery latency | <10ms |
| PF-EVT-2 | ViewCommand delivery latency | <50ms |
| PF-NAV-1 | View switch time | <100ms |
| PF-STR-1 | Streaming latency to first delta | <200ms |
| PF-MEM-1 | Event channel capacity | 256 events |

---

## Test Requirements

| ID | Test Category | Description |
|----|---------------|-------------|
| EVT-T1 | EventBus | Delivers events to all subscribers |
| EVT-T2 | EventBus | Events delivered in order |
| EVT-T3 | EventBus | Slow subscriber doesn't block |
| NAV-T1 | Navigation | Stack starts with Chat |
| NAV-T2 | Navigation | Forward navigation pushes |
| NAV-T3 | Navigation | Back navigation pops |
| NAV-T4 | Navigation | Same-view navigation is no-op |
| PRF-T1 | Profile | List loads on Settings navigation |
| PRF-T2 | Profile | Selection updates default |
| PRF-T3 | Profile | CRUD operations work end-to-end |
| CONV-T1 | Conversation | Send message flow complete |
| CONV-T2 | Conversation | Stop streaming works |
| CONV-T3 | Conversation | Switch conversation loads messages |
| MCP-T1 | MCP | Toggle starts/stops server |
| MCP-T2 | MCP | Add flow navigates correctly |
| MCP-T3 | MCP | Configure saves configuration |
