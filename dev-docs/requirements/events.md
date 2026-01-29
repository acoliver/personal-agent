# Event System Requirements

The Event System provides a centralized event bus for decoupling components. All user actions, domain events, and system events flow through the EventBus, enabling logging, debugging, and clean separation of concerns.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│  Views (UI Layer)                                           │
│  - Render state from Presenters                             │
│  - emit(UserEvent::*) on user actions                       │
└─────────────────────────────────────────────────────────────┘
                          │
          emit(UserEvent) │ 
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  EventBus (Event Layer)                                     │
│  - broadcast::channel<AppEvent>                             │
│  - All events flow through here                             │
│  - Enables logging, debugging, replay                       │
└─────────────────────────────────────────────────────────────┘
                          │
       subscribe + handle │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Presenters (Presentation Layer)                            │
│  - Subscribe to events they care about                      │
│  - Call services                                            │
│  - Update views                                             │
│  - emit() result events                                     │
└─────────────────────────────────────────────────────────────┘
                          │
                   calls  │  emit(ChatEvent, McpEvent, ...)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Services (Domain Layer)                                    │
│  - Business logic                                           │
│  - emit() domain events as things happen                    │
└─────────────────────────────────────────────────────────────┘
```

---

## EventBus Interface

```rust
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    /// Create a new EventBus with specified channel capacity
    pub fn new(capacity: usize) -> Self;
    
    /// Emit an event to all subscribers
    pub fn emit(&self, event: AppEvent);
    
    /// Subscribe to receive all events
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent>;
    
    /// Emit with context for debugging
    pub fn emit_with_context(&self, event: AppEvent, source: &str);
}

// Global access
pub fn emit(event: impl Into<AppEvent>);
pub fn subscribe() -> broadcast::Receiver<AppEvent>;
```

---

## Event Hierarchy

```rust
/// Top-level event enum - all events in the system
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// User-initiated actions from UI
    User(UserEvent),
    
    /// Chat and streaming events
    Chat(ChatEvent),
    
    /// MCP server lifecycle events
    Mcp(McpEvent),
    
    /// Profile and settings events
    Profile(ProfileEvent),
    
    /// Conversation events
    Conversation(ConversationEvent),
    
    /// Navigation events
    Navigation(NavigationEvent),
    
    /// System-level events
    System(SystemEvent),
}
```

---

## UserEvent - User Actions

Events emitted by Views when users interact with the UI.

```rust
#[derive(Debug, Clone)]
pub enum UserEvent {
    // ===== Chat Actions =====
    /// User clicked send or pressed Enter
    SendMessage { text: String },
    
    /// User clicked stop during streaming
    StopStreaming,
    
    /// User clicked new conversation
    NewConversation,
    
    /// User selected a conversation from history
    SelectConversation { id: Uuid },
    
    /// User toggled thinking display
    ToggleThinking,
    
    /// User started renaming conversation
    StartRenameConversation { id: Uuid },
    
    /// User confirmed rename
    ConfirmRenameConversation { id: Uuid, title: String },
    
    /// User cancelled rename
    CancelRenameConversation,
    
    // ===== Profile Actions =====
    /// User selected a profile as default
    SelectProfile { id: Uuid },
    
    /// User clicked create new profile
    CreateProfile,
    
    /// User clicked edit profile
    EditProfile { id: Uuid },
    
    /// User clicked save on profile editor
    SaveProfile { profile: ProfileData },
    
    /// User clicked delete profile
    DeleteProfile { id: Uuid },
    
    /// User confirmed delete in dialog
    ConfirmDeleteProfile { id: Uuid },
    
    /// User clicked test connection
    TestProfileConnection { id: Uuid },
    
    // ===== MCP Actions =====
    /// User toggled MCP enabled/disabled
    ToggleMcp { id: Uuid, enabled: bool },
    
    /// User clicked add MCP
    AddMcp,
    
    /// User searched MCP registry
    SearchMcpRegistry { query: String, source: RegistrySource },
    
    /// User selected MCP from search results
    SelectMcpFromRegistry { source: McpRegistrySource },
    
    /// User clicked configure MCP
    ConfigureMcp { id: Uuid },
    
    /// User saved MCP configuration
    SaveMcpConfig { id: Uuid, config: McpConfigData },
    
    /// User clicked delete MCP
    DeleteMcp { id: Uuid },
    
    /// User confirmed delete in dialog
    ConfirmDeleteMcp { id: Uuid },
    
    /// User initiated OAuth flow
    StartMcpOAuth { id: Uuid, provider: String },
    
    // ===== Model Selector Actions =====
    /// User opened model selector
    OpenModelSelector,
    
    /// User searched models
    SearchModels { query: String },
    
    /// User selected provider filter
    FilterModelsByProvider { provider_id: Option<String> },
    
    /// User selected a model
    SelectModel { provider_id: String, model_id: String },
    
    // ===== Navigation =====
    /// User clicked to navigate to a view
    Navigate { to: ViewId },
    
    /// User clicked back
    NavigateBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

---

## ChatEvent - Streaming & Messages

Events emitted by ChatService during message streaming.

```rust
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// Stream has started
    StreamStarted {
        conversation_id: Uuid,
        message_id: Uuid,
        model_id: String,
    },
    
    /// Text content delta received
    TextDelta { text: String },
    
    /// Thinking content delta received
    ThinkingDelta { text: String },
    
    /// Tool call started
    ToolCallStarted {
        tool_call_id: String,
        tool_name: String,
    },
    
    /// Tool call completed
    ToolCallCompleted {
        tool_call_id: String,
        tool_name: String,
        success: bool,
        result: String,
        duration_ms: u64,
    },
    
    /// Stream completed successfully
    StreamCompleted {
        conversation_id: Uuid,
        message_id: Uuid,
        total_tokens: Option<u32>,
    },
    
    /// Stream was cancelled by user
    StreamCancelled {
        conversation_id: Uuid,
        message_id: Uuid,
        partial_content: String,
    },
    
    /// Stream failed with error
    StreamError {
        conversation_id: Uuid,
        error: String,
        recoverable: bool,
    },
    
    /// Message was saved to conversation
    MessageSaved {
        conversation_id: Uuid,
        message_id: Uuid,
    },
}
```

---

## McpEvent - MCP Lifecycle

Events emitted by McpService during MCP server lifecycle.

```rust
#[derive(Debug, Clone)]
pub enum McpEvent {
    /// MCP server is starting
    Starting { id: Uuid, name: String },
    
    /// MCP server started successfully
    Started {
        id: Uuid,
        name: String,
        tools: Vec<String>,
        tool_count: usize,
    },
    
    /// MCP server failed to start
    StartFailed {
        id: Uuid,
        name: String,
        error: String,
    },
    
    /// MCP server stopped
    Stopped { id: Uuid, name: String },
    
    /// MCP server health check failed
    Unhealthy {
        id: Uuid,
        name: String,
        error: String,
    },
    
    /// MCP server recovered from unhealthy state
    Recovered { id: Uuid, name: String },
    
    /// MCP server is restarting
    Restarting { id: Uuid, name: String },
    
    /// MCP tool was called
    ToolCalled {
        mcp_id: Uuid,
        tool_name: String,
        tool_call_id: String,
    },
    
    /// MCP tool call completed
    ToolCompleted {
        mcp_id: Uuid,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        duration_ms: u64,
    },
    
    /// MCP configuration was saved
    ConfigSaved { id: Uuid },
    
    /// MCP was deleted
    Deleted { id: Uuid, name: String },
}
```

---

## ProfileEvent - Profile Lifecycle

Events emitted by ProfileService and AppSettingsService during profile operations.

```rust
#[derive(Debug, Clone)]
pub enum ProfileEvent {
    /// Profile was created
    Created { id: Uuid, name: String },
    
    /// Profile was updated
    Updated { id: Uuid, name: String },
    
    /// Profile was deleted
    Deleted { id: Uuid, name: String },
    
    /// Default profile was changed (emitted by AppSettingsService)
    DefaultChanged { profile_id: Option<Uuid> },
    
    /// Profile connection test started
    TestStarted { id: Uuid },
    
    /// Profile connection test completed
    TestCompleted {
        id: Uuid,
        success: bool,
        response_time_ms: Option<u64>,
        error: Option<String>,
    },
    
    /// Profile validation failed
    ValidationFailed {
        id: Uuid,
        errors: Vec<String>,
    },
}
```

---

## ConversationEvent - Conversation Lifecycle

Events emitted by ConversationService and AppSettingsService.

```rust
#[derive(Debug, Clone)]
pub enum ConversationEvent {
    /// Conversation was created
    Created { id: Uuid, title: String },
    
    /// Conversation was loaded
    Loaded { id: Uuid },
    
    /// Conversation title was updated
    TitleUpdated { id: Uuid, title: String },
    
    /// Conversation was deleted
    Deleted { id: Uuid },
    
    /// Conversation was set as active (emitted by AppSettingsService)
    Activated { id: Uuid },
    
    /// Active conversation was cleared (emitted by AppSettingsService)
    Deactivated,
    
    /// Conversation list was refreshed
    ListRefreshed { count: usize },
}
```

---

## NavigationEvent - View Navigation

Events for view transitions.

```rust
#[derive(Debug, Clone)]
pub enum NavigationEvent {
    /// Navigation to view started
    Navigating { from: ViewId, to: ViewId },
    
    /// Navigation completed
    Navigated { view: ViewId },
    
    /// Navigation was cancelled
    Cancelled { reason: String },
    
    /// Modal was presented
    ModalPresented { view: ViewId },
    
    /// Modal was dismissed
    ModalDismissed { view: ViewId },
}
```

---

## SystemEvent - Application Lifecycle

Events for application-level concerns.

```rust
#[derive(Debug, Clone)]
pub enum SystemEvent {
    /// Application launched
    AppLaunched,
    
    /// Application will terminate
    AppWillTerminate,
    
    /// Application became active (foreground)
    AppBecameActive,
    
    /// Application resigned active (background)
    AppResignedActive,
    
    /// Global hotkey was pressed
    HotkeyPressed,
    
    /// Hotkey configuration was changed
    HotkeyChanged { hotkey: HotkeyConfig },
    
    /// Popover was shown
    PopoverShown,
    
    /// Popover was hidden
    PopoverHidden,
    
    /// Unhandled error occurred
    Error {
        source: String,
        error: String,
        context: Option<String>,
    },
    
    /// Config was loaded
    ConfigLoaded,
    
    /// Config was saved
    ConfigSaved,
    
    /// Models registry was refreshed
    ModelsRegistryRefreshed { provider_count: usize, model_count: usize },
    
    /// Models registry refresh failed
    ModelsRegistryRefreshFailed { error: String },
}
```

---

## Event Flow Examples

### User Sends Message

```
1. User clicks Send button
   └─▶ View emits: UserEvent::SendMessage { text: "Hello" }

2. EventBus broadcasts to subscribers

3. ChatPresenter handles UserEvent::SendMessage
   ├─▶ Validates input
   ├─▶ Updates view (clear input, show user bubble, show loading)
   └─▶ Calls ChatService.send_message()

4. ChatService emits as it streams:
   ├─▶ ChatEvent::StreamStarted { ... }
   ├─▶ ChatEvent::TextDelta { text: "Hi" }
   ├─▶ ChatEvent::TextDelta { text: " there" }
   ├─▶ ChatEvent::ToolCallStarted { tool_name: "search" }
   ├─▶ ChatEvent::ToolCallCompleted { success: true, ... }
   ├─▶ ChatEvent::TextDelta { text: "..." }
   └─▶ ChatEvent::StreamCompleted { ... }

5. ChatPresenter handles each ChatEvent
   ├─▶ TextDelta → view.append_to_message()
   ├─▶ ToolCallStarted → view.show_tool_indicator()
   └─▶ StreamCompleted → view.hide_loading()
```

### User Toggles MCP

```
1. User clicks MCP toggle
   └─▶ View emits: UserEvent::ToggleMcp { id, enabled: true }

2. SettingsPresenter handles event
   └─▶ Calls McpService.set_enabled(id, true)

3. McpService emits lifecycle events:
   ├─▶ McpEvent::Starting { id, name }
   ├─▶ McpEvent::Started { id, tools: [...] }
   └─▶ (or McpEvent::StartFailed { error })

4. SettingsPresenter handles McpEvent
   ├─▶ Starting → view.show_mcp_loading(id)
   ├─▶ Started → view.show_mcp_active(id, tool_count)
   └─▶ StartFailed → view.show_mcp_error(id, error)

5. ChatPresenter also handles McpEvent::Started
   └─▶ Updates available tools display
```

---

## Presenter Subscriptions

Each presenter subscribes to relevant events:

| Presenter | Subscribes To |
|-----------|---------------|
| ChatPresenter | `UserEvent::{SendMessage, StopStreaming, NewConversation, SelectConversation, ToggleThinking}`, `ChatEvent::*`, `McpEvent::{Started, Stopped}` |
| HistoryPresenter | `UserEvent::{SelectConversation, StartRename, ConfirmRename}`, `ConversationEvent::*` |
| SettingsPresenter | `UserEvent::{SelectProfile, ToggleMcp, ...}`, `ProfileEvent::*`, `McpEvent::*` |
| ProfileEditorPresenter | `UserEvent::{SaveProfile, TestConnection, SelectModel}`, `ProfileEvent::Test*` |
| McpAddPresenter | `UserEvent::{SearchMcpRegistry, SelectMcpFromRegistry}` |
| McpConfigurePresenter | `UserEvent::{SaveMcpConfig, StartMcpOAuth}`, `McpEvent::ConfigSaved` |
| ModelSelectorPresenter | `UserEvent::{SearchModels, FilterModels, SelectModel}` |
| ErrorPresenter | `SystemEvent::Error`, `ChatEvent::StreamError`, `McpEvent::StartFailed` |

---

## Logging & Debugging

The EventBus logs all events for debugging:

```rust
impl EventBus {
    pub fn emit(&self, event: AppEvent) {
        // Structured logging
        tracing::debug!(
            event_type = %event.type_name(),
            event = ?event,
            "Event emitted"
        );
        
        let _ = self.sender.send(event);
    }
}
```

Log output example:
```
DEBUG event_type="UserEvent::SendMessage" event=SendMessage { text: "Hello" } "Event emitted"
DEBUG event_type="ChatEvent::StreamStarted" event=StreamStarted { conversation_id: "...", model_id: "claude-3-5-sonnet" } "Event emitted"
DEBUG event_type="ChatEvent::TextDelta" event=TextDelta { text: "Hi" } "Event emitted"
```

---

## Implementation Notes

### Thread Safety

- EventBus uses `tokio::sync::broadcast` for async multi-consumer
- Events are `Clone` to allow multiple subscribers
- Presenters run on main thread for UI updates

### Event Ordering

- Events are delivered in order to each subscriber
- No guaranteed ordering across subscribers
- Presenters should not depend on cross-presenter ordering

### Backpressure

- Channel capacity: 256 events (configurable)
- Slow subscribers may miss events (lagged)
- Critical events should be handled promptly

### Memory

- Events are cloned for each subscriber
- Large payloads (like full message content) should use Arc or IDs
- StreamCompleted uses message_id, not full content

---

## Test Requirements

| ID | Test |
|----|------|
| EV-T1 | EventBus delivers events to all subscribers |
| EV-T2 | Events are delivered in order |
| EV-T3 | Slow subscriber doesn't block fast subscribers |
| EV-T4 | Events are logged with correct type names |
| EV-T5 | Presenter receives only subscribed event types |
| EV-T6 | Full send message flow emits correct event sequence |
| EV-T7 | MCP toggle flow emits correct event sequence |
| EV-T8 | Error events are captured by ErrorPresenter |
