# Domain Model Analysis: GPUI Remediation

Plan ID: PLAN-20260219-NEXTGPUIREMEDIATE
Document: Domain Model Analysis
Created: 2026-02-19

---

## Overview

This document defines the domain model for the GPUI UI system remediation, covering the core entities, their relationships, state transitions, and data flows across the six key areas: Event Pipeline, Main Panel Routing, Profile Flow, Conversation Flow, MCP Flow, and Settings Flow.

---

## Core Domain Entities

### 1. Event System Entities

#### EventBus

The central nervous system of the application, routing all events between components.

```
┌─────────────────────────────────────────────┐
│                 EventBus                     │
├─────────────────────────────────────────────┤
│ - sender: broadcast::Sender<AppEvent>       │
│ - capacity: usize (default 256)             │
├─────────────────────────────────────────────┤
│ + emit(event: AppEvent)                     │
│ + emit_with_context(event, source: &str)    │
│ + subscribe() -> Receiver<AppEvent>         │
└─────────────────────────────────────────────┘
```

**State**: Stateless (pure message passing)

**Lifecycle**:
1. Created on app startup
2. Available globally via events::global module
3. Destroyed on app shutdown

#### AppEvent Hierarchy

```
AppEvent
├── User(UserEvent)           # UI-initiated actions
├── Chat(ChatEvent)           # Streaming lifecycle
├── Mcp(McpEvent)             # MCP server lifecycle
├── Profile(ProfileEvent)     # Profile operations
├── Conversation(ConversationEvent)  # Conversation lifecycle
├── Navigation(NavigationEvent)      # View navigation
└── System(SystemEvent)       # App-level events
```

### 2. Bridge Entities

#### GpuiBridge

Bridges the GPUI (smol) and tokio async runtimes.

```
┌─────────────────────────────────────────────┐
│               GpuiBridge                     │
├─────────────────────────────────────────────┤
│ - user_event_sender: Sender<UserEvent>      │
│ - view_command_receiver: Receiver<ViewCmd>  │
│ - cx: AsyncAppContext                       │
├─────────────────────────────────────────────┤
│ + emit_user_event(event: UserEvent)         │
│ + receive_view_command() -> Option<ViewCmd> │
│ + run_event_loop()                          │
└─────────────────────────────────────────────┘
```

#### ViewCommandSink

Delivers view commands from tokio to GPUI.

```
┌─────────────────────────────────────────────┐
│            ViewCommandSink                   │
├─────────────────────────────────────────────┤
│ - sender: Sender<ViewCommand>               │
│ - notifier: Arc<Notify>                     │
├─────────────────────────────────────────────┤
│ + send(command: ViewCommand)                │
│ + send_batch(commands: Vec<ViewCommand>)    │
└─────────────────────────────────────────────┘
```

### 3. Navigation Entities

#### NavigationState

Stack-based navigation manager.

```
┌─────────────────────────────────────────────┐
│            NavigationState                   │
├─────────────────────────────────────────────┤
│ - stack: Vec<ViewId>                        │
├─────────────────────────────────────────────┤
│ + current() -> ViewId                       │
│ + can_go_back() -> bool                     │
│ + navigate(to: ViewId)                      │
│ + navigate_back() -> bool                   │
│ + stack_depth() -> usize                    │
└─────────────────────────────────────────────┘
```

**State Diagram**:
```
                 ┌────────────────────────┐
                 │                        │
     ┌───────────▼───────────┐            │
     │        Chat           │ ◄──────────┼───── Initial State
     │    (stack depth: 1)   │            │
     └───────────┬───────────┘            │
                 │                        │
    navigate()   │                        │ navigate_back()
                 ▼                        │
     ┌───────────────────────┐            │
     │       Settings        │            │
     │    (stack depth: 2)   │────────────┤
     └───────────┬───────────┘            │
                 │                        │
    navigate()   │                        │ navigate_back()
                 ▼                        │
     ┌───────────────────────┐            │
     │    ProfileEditor      │────────────┘
     │    (stack depth: 3)   │
     └───────────────────────┘
```

#### ViewId Enumeration

```rust
pub enum ViewId {
    Chat,                           // Root view
    History,                        // Conversation list
    Settings,                       // Profile & MCP management
    ProfileEditor { id: Option<Uuid> },  // New or edit profile
    McpAdd,                         // Search/add MCP
    McpConfigure { id: Uuid },      // Configure existing MCP
    ModelSelector,                  // Choose provider/model
}
```

### 4. Presenter Entities

#### Presenter Trait

```rust
pub trait Presenter: Send + Sync {
    fn start(&self);  // Subscribe to events
    fn stop(&self);   // Unsubscribe
}
```

#### ChatPresenter

```
┌─────────────────────────────────────────────┐
│            ChatPresenter                     │
├─────────────────────────────────────────────┤
│ - event_bus: Arc<EventBus>                  │
│ - view_command_sink: Arc<ViewCommandSink>   │
│ - chat_service: Arc<dyn ChatService>        │
│ - conversation_service: Arc<dyn ConvSvc>    │
│ - app_settings: Arc<dyn AppSettingsSvc>     │
│ - current_conversation_id: Option<Uuid>     │
│ - stream_handle: Option<StreamHandle>       │
│ - is_streaming: bool                        │
│ - show_thinking: bool                       │
├─────────────────────────────────────────────┤
│ + handle_send_message(text: String)         │
│ + handle_stop_streaming()                   │
│ + handle_new_conversation()                 │
│ + handle_select_conversation(id: Uuid)      │
│ + handle_toggle_thinking()                  │
│ + handle_stream_event(event: ChatEvent)     │
└─────────────────────────────────────────────┘
```

**Subscriptions**:
- UserEvent::SendMessage
- UserEvent::StopStreaming
- UserEvent::NewConversation
- UserEvent::SelectConversation
- UserEvent::ToggleThinking
- ChatEvent::* (all streaming events)
- ConversationEvent::Loaded
- ConversationEvent::Created

#### SettingsPresenter

```
┌─────────────────────────────────────────────┐
│           SettingsPresenter                  │
├─────────────────────────────────────────────┤
│ - event_bus: Arc<EventBus>                  │
│ - view_command_sink: Arc<ViewCommandSink>   │
│ - profile_service: Arc<dyn ProfileService>  │
│ - mcp_service: Arc<dyn McpService>          │
│ - app_settings: Arc<dyn AppSettingsSvc>     │
│ - selected_profile_id: Option<Uuid>         │
│ - selected_mcp_id: Option<Uuid>             │
├─────────────────────────────────────────────┤
│ + handle_select_profile(id: Uuid)           │
│ + handle_delete_profile(id: Uuid)           │
│ + handle_toggle_mcp(id: Uuid, enabled)      │
│ + handle_delete_mcp(id: Uuid)               │
│ + load_data()                               │
└─────────────────────────────────────────────┘
```

**Subscriptions**:
- UserEvent::SelectProfile
- UserEvent::DeleteProfile / ConfirmDeleteProfile
- UserEvent::ToggleMcp
- UserEvent::DeleteMcp / ConfirmDeleteMcp
- ProfileEvent::* (CRUD events)
- McpEvent::* (lifecycle events)
- NavigationEvent::Navigated { view: Settings }

### 5. View Entities

#### MainPanel

Root container managing view routing.

```
┌─────────────────────────────────────────────┐
│              MainPanel                       │
├─────────────────────────────────────────────┤
│ - navigation: Model<NavigationState>        │
│ - chat_view: ChatView                       │
│ - history_view: HistoryView                 │
│ - settings_view: SettingsView               │
│ - profile_editor_view: ProfileEditorView    │
│ - mcp_add_view: McpAddView                  │
│ - mcp_configure_view: McpConfigureView      │
│ - model_selector_view: ModelSelectorView    │
├─────────────────────────────────────────────┤
│ + render_current_view()                     │
│ + handle_navigation_command(ViewId)         │
└─────────────────────────────────────────────┘
```

#### ChatView (and ChatState)

```
┌─────────────────────────────────────────────┐
│               ChatView                       │
├─────────────────────────────────────────────┤
│ state: ChatState                            │
│   - messages: Vec<MessageViewModel>         │
│   - input_text: String                      │
│   - is_streaming: bool                      │
│   - show_thinking: bool                     │
│   - conversation_title: String              │
│   - model_id: String                        │
│   - conversations: Vec<ConversationItem>    │
├─────────────────────────────────────────────┤
│ + render(cx: &mut ViewContext)              │
│ + handle_send_click()                       │
│ + handle_stop_click()                       │
│ + handle_input_change(text: String)         │
│ + handle_conversation_select(id: Uuid)      │
└─────────────────────────────────────────────┘
```

---

## Entity Relationships

```
                          ┌───────────────┐
                          │   EventBus    │
                          └───────┬───────┘
                                  │
           ┌──────────────────────┼──────────────────────┐
           │                      │                      │
           ▼                      ▼                      ▼
    ┌─────────────┐       ┌─────────────┐       ┌─────────────┐
    │ GpuiBridge  │◄─────►│ Presenters  │◄─────►│  Services   │
    └──────┬──────┘       └─────────────┘       └─────────────┘
           │                      │
           │              ViewCommandSink
           │                      │
           ▼                      ▼
    ┌─────────────┐       ┌─────────────┐
    │    Views    │◄──────│  MainPanel  │
    └─────────────┘       └──────┬──────┘
                                 │
                          NavigationState
```

### Relationship Details

| From | To | Relationship | Description |
|------|----|--------------|-------------|
| GpuiBridge | EventBus | Forwards to | UserEvents forwarded to EventBus |
| EventBus | Presenters | Broadcasts to | All presenters receive relevant events |
| Presenters | Services | Calls | Business logic execution |
| Presenters | ViewCommandSink | Sends to | UI update commands |
| ViewCommandSink | Views | Updates | View state modifications |
| MainPanel | NavigationState | Owns | Navigation stack management |
| MainPanel | Views | Contains | All view instances |
| Views | GpuiBridge | Emits through | UserEvent emissions |

---

## State Transitions

### ChatState Transitions

```
┌────────────────────────────────────────────────────────────────┐
│                         ChatState                              │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│   IDLE                                                         │
│     │                                                          │
│     │ SendMessage                                              │
│     ▼                                                          │
│   STREAMING ─────────────────────────────────────────────────► │
│     │         │                    │                           │
│     │ TextDelta (multiple)         │ StopStreaming             │
│     │         │                    │                           │
│     │         ▼                    ▼                           │
│     │   STREAMING (updated)    CANCELLED                       │
│     │                             │                            │
│     │ StreamCompleted             │                            │
│     │         │                   │                            │
│     ▼         ▼                   ▼                            │
│   IDLE ◄──────────────────────────                             │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### McpStatus Transitions

```
┌────────────────────────────────────────────────────────────────┐
│                        MCP Status                              │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│   DISABLED ◄───────────────────────────────────────────────────┤
│     │                                                          │
│     │ ToggleMcp(enabled: true)                                 │
│     ▼                                                          │
│   STARTING                                                     │
│     │                                                          │
│     ├──── McpEvent::Started ────► RUNNING                      │
│     │                               │                          │
│     │                               │ ToggleMcp(enabled: false)│
│     │                               ▼                          │
│     │                            STOPPING ──► DISABLED         │
│     │                               │                          │
│     │                               │ McpEvent::Unhealthy      │
│     │                               ▼                          │
│     └──── McpEvent::StartFailed ► ERROR ◄────────────────────  │
│                                     │                          │
│                                     │ Recovery or Restart      │
│                                     ▼                          │
│                                  STARTING                      │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### ProfileEditor Flow States

```
┌────────────────────────────────────────────────────────────────┐
│                    ProfileEditor Flow                          │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│   [Settings] ──── Click [+] ────► [ModelSelector]             │
│       │                                │                       │
│       │                                │ SelectModel           │
│       │ Click [Edit]                   ▼                       │
│       │                      [ProfileEditor(new)]              │
│       ▼                                │                       │
│   [ProfileEditor(edit)]                │ Save                  │
│       │                                │                       │
│       │ Save                           ▼                       │
│       │                           [Settings]                   │
│       ▼                                                        │
│   [Settings]                                                   │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Diagrams

### Send Message Flow

```
User          ChatView       GpuiBridge      EventBus      ChatPresenter     ChatService
  │               │               │              │               │               │
  │ Type + Enter  │               │              │               │               │
  │───────────────►               │              │               │               │
  │               │ emit(SendMsg) │              │               │               │
  │               │───────────────►              │               │               │
  │               │               │ forward      │               │               │
  │               │               │──────────────►               │               │
  │               │               │              │ broadcast     │               │
  │               │               │              │───────────────►               │
  │               │               │              │               │ send_message  │
  │               │               │              │               │───────────────►
  │               │               │              │               │               │
  │               │               │              │ emit(StreamStarted)           │
  │               │               │              │◄──────────────│               │
  │               │               │              │ emit(TextDelta) (multiple)    │
  │               │               │              │◄──────────────│               │
  │               │               │              │ emit(StreamCompleted)         │
  │               │               │              │◄──────────────│               │
  │               │               │              │               │               │
  │               │               │              │ ChatPresenter handles events  │
  │               │               │              │───────────────►               │
  │               │               │              │               │               │
  │               │◄──────────────│◄─────────────│◄──────────────│ ViewCommands  │
  │◄──────────────│ Updated UI    │              │               │               │
```

### MCP Toggle Flow

```
User       SettingsView    EventBus    SettingsPresenter   McpService
  │             │              │               │               │
  │ Toggle MCP  │              │               │               │
  │─────────────►              │               │               │
  │             │ emit(Toggle) │               │               │
  │             │──────────────►               │               │
  │             │              │ broadcast     │               │
  │             │              │───────────────►               │
  │             │              │               │ start/stop    │
  │             │              │               │───────────────►
  │             │              │               │               │
  │             │              │ emit(Starting/Started/Stopped) │
  │             │              │◄──────────────│◄──────────────│
  │             │              │               │               │
  │             │◄─────────────│◄──────────────│ ViewCommand   │
  │◄────────────│ Status update│               │               │
```

---

## Aggregate Boundaries

### Event System Aggregate

- **Root**: EventBus
- **Entities**: AppEvent variants
- **Invariants**:
  - Events are immutable once emitted
  - Event delivery is at-least-once to active subscribers
  - Lagged subscribers are notified

### Navigation Aggregate

- **Root**: NavigationState
- **Entities**: ViewId, navigation stack
- **Invariants**:
  - Stack always has at least one element (Chat)
  - Current view is always top of stack
  - Navigate to same view is no-op

### Presenter Aggregate

- **Root**: Individual Presenter
- **Entities**: Presenter state, subscriptions
- **Invariants**:
  - Presenter handles only its subscribed events
  - ViewCommands are the only way to update views
  - Service calls are async and non-blocking

---

## Consistency Rules

1. **Event Ordering**: Events from a single emitter arrive in emission order per subscriber
2. **View State Consistency**: View state only changes via ViewCommand
3. **Navigation Consistency**: Only one view is "current" at any time
4. **Service State**: Services are the source of truth for domain data
5. **Presenter Mediation**: All UI↔Service communication goes through presenters

---

## Error Handling

### Event Delivery Errors

| Error | Handling |
|-------|----------|
| Channel closed | Log and return gracefully |
| Subscriber lagged | Log warning, subscriber catches up |
| Send failed | Log error, event lost |

### Service Call Errors

| Error | Presenter Action | UI Feedback |
|-------|------------------|-------------|
| ValidationError | Emit validation failed event | Highlight field |
| NetworkError | Log and emit error event | Show error banner |
| NotFound | Log and emit error event | Show not found message |
| Unauthorized | Log and emit error event | Show auth error |

### Navigation Errors

| Error | Handling |
|-------|----------|
| Invalid ViewId | Log error, stay on current view |
| Navigate back at root | No-op, stay on Chat |

---

## Performance Considerations

1. **Event Channel Capacity**: 256 events prevents backpressure in normal operation
2. **ViewCommand Batching**: Multiple related commands can be sent in batch
3. **Lazy View Loading**: Views are created once and reused
4. **Debounced Events**: Search inputs debounced to reduce event volume
5. **Async Service Calls**: All I/O operations are async, never blocking UI
