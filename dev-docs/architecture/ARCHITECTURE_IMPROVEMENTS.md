# Architecture Improvements Plan

## Current Architecture Problems

### 1. UI-Business Logic Coupling

The current code has business logic deeply embedded in UI view controllers:

```
src/ui/chat_view.rs (980 lines!)
  - Message persistence logic
  - Streaming response handling
  - Conversation management
  - Profile loading
  - MCP tool fetching
  - LLM client creation
```

**Problems:**
- Impossible to unit test business logic without mocking NSViews
- UI changes require touching business logic
- Business logic changes risk breaking UI
- Massive file sizes (target: <500 lines per file)

### 2. Scattered State Management

State is spread across:
- `RefCell<Option<T>>` fields in view controllers
- Global singletons (`MCP_SERVICE`)
- Config files on disk
- In-memory conversation objects

**Problems:**
- Race conditions between UI thread and async operations
- State synchronization bugs (dropdown not updating, etc.)
- Hard to reason about what state is "current"

### 3. No Event-Driven Architecture

Current code uses direct method calls:
- Views call services directly
- Services call callbacks directly
- No central place to log/debug what's happening
- Components tightly coupled

### 4. Missing Abstraction Layers

No clear separation between:
- Data access (storage, config)
- Domain logic (conversations, profiles, MCPs)
- Presentation logic (what to show when)
- UI rendering (NSViews)

---

## Target Architecture

### Five-Layer Architecture with Event Bus

```
┌─────────────────────────────────────────────────────────────┐
│                     UI Layer (Views)                        │
│  NSViewControllers, NSViews, UI Components                  │
│  - Renders state from Presenters                            │
│  - Emits UserEvents on user actions                         │
│  - Pure rendering, no business logic                        │
│  - Target: <500 lines per view                              │
└─────────────────────────┬───────────────────────────────────┘
                          │ emit(UserEvent)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Event Layer (EventBus)                   │
│  broadcast::channel<AppEvent>                               │
│  - Central nervous system for the app                       │
│  - Routes events to subscribers                             │
│  - Enables logging, debugging, replay                       │
│  - Decouples producers from consumers                       │
└─────────────────────────┬───────────────────────────────────┘
                          │ subscribe + dispatch
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                  Presentation Layer (Presenters)            │
│  ChatPresenter, SettingsPresenter, HistoryPresenter         │
│  - Subscribe to events they care about                      │
│  - Transform domain models to view state                    │
│  - Handle UI state (loading, error, success)                │
│  - Call services and emit result events                     │
│  - Update views with new state                              │
└─────────────────────────┬───────────────────────────────────┘
                          │ calls services, emit(DomainEvent)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Domain Layer (Services)                  │
│  ConversationService, ProfileService, ChatService, etc.     │
│  - Business rules and validation                            │
│  - Emit domain events as operations progress                │
│  - Pure Rust, no UI dependencies                            │
│  - Orchestrate infrastructure components                    │
└─────────────────────────┬───────────────────────────────────┘
                          │ calls
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                 Infrastructure Layer                        │
│  Repositories, Clients, Storage                             │
│  - ConversationRepository (file storage)                    │
│  - ConfigRepository (config.json)                           │
│  - LlmClient (SerdesAI)                                     │
│  - McpRuntime (SerdesAI MCP)                                │
└─────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
  events/
    mod.rs
    bus.rs                   # EventBus implementation
    types.rs                 # AppEvent, UserEvent, ChatEvent, etc.
    
  presentation/
    mod.rs
    chat_presenter.rs        # Handles chat-related events
    settings_presenter.rs    # Handles settings-related events
    history_presenter.rs     # Handles history-related events
    profile_editor_presenter.rs
    mcp_add_presenter.rs
    mcp_configure_presenter.rs
    model_selector_presenter.rs
    error_presenter.rs       # Global error handling
    
  services/
    mod.rs
    conversation.rs          # Conversation CRUD, activation
    profile.rs               # Profile CRUD, default management
    chat.rs                  # Send message, streaming orchestration
    mcp.rs                   # MCP lifecycle, tool management
    mcp_registry.rs          # Registry search, caching
    secrets.rs               # Credential storage
    app_settings.rs          # Global app settings
    model_registry.rs        # models.dev registry
    
  domain/
    mod.rs
    conversation.rs          # Conversation, Message models
    profile.rs               # ModelProfile, Parameters
    mcp.rs                   # McpConfig, McpStatus
    
  infrastructure/
    mod.rs
    storage/
      conversations.rs       # File-based conversation storage
      config.rs              # Config file operations
      secrets.rs             # Keychain integration
    llm/
      client.rs              # SerdesAI LLM client wrapper
      agent.rs               # Agent mode integration
    mcp/
      runtime.rs             # Global MCP runtime
      toolset.rs             # MCP toolset creation
      
  ui/
    mod.rs
    chat_view.rs             # Pure UI, emits UserEvents
    settings_view.rs         # Pure UI, emits UserEvents
    history_view.rs          # Pure UI, emits UserEvents
    profile_editor_view.rs
    mcp_add_view.rs
    mcp_configure_view.rs
    model_selector_view.rs
    components/              # Reusable UI components
      message_bubble.rs
      profile_row.rs
      mcp_row.rs
      tool_indicator.rs
```


## Service Dependency Graph

```
[UI Views]
  -> [Presenters]
      -> [ChatService] -----> [ConversationService] -----> [ConversationRepository]
      -> [ChatService] -----> [ContextStrategy] ----------> (uses ModelClient for compression only)
      -> [ChatService] -----> [McpService] --------------> [McpRuntime]
      -> [ChatService] -----> [ProfileService] ----------> [ConfigRepository]
      -> [ModelsRegistryService] ------------------------> [ModelsRegistryRepository]
      -> [McpRegistryService] --------------------------> [McpRegistryRepository]

[ConversationService] <---- [ProfileService] (profile_id reference only)
[ConversationService] <---- [ContextStrategy] (new ContextState via update_context_state)
[ModelsRegistryService] ---> [ProfileService] (base_url + model_id selection)
```

### Data Flow Direction

- UI → Presenters → Services for all user actions.
- ChatService is the only service that orchestrates ContextStrategy + MCP + Conversation persistence.
- ConversationService owns Conversation, Message, and ContextState persistence.
- ProfileService owns ModelProfile persistence.
- ModelsRegistryService owns Provider + ModelInfo normalization and filtering.
- McpService owns McpConfig persistence and runtime lifecycle.
- McpRegistryService owns McpSource discovery metadata (EnvVarSpec, config schemas).

---

## Event System

See [events.md](../requirements/events.md) for complete event definitions.

### Event Flow Example: User Sends Message

```
1. User clicks Send
   └─▶ ChatView emits: UserEvent::SendMessage { text: "Hello" }

2. EventBus broadcasts to all subscribers
   └─▶ Logged: "UserEvent::SendMessage received"

3. ChatPresenter handles UserEvent::SendMessage
   ├─▶ Validates input (not empty)
   ├─▶ Updates ChatView: clear_input(), add_user_bubble(), show_loading()
   └─▶ Calls ChatService.send_message(conversation_id, text)

4. ChatService streams response, emits events:
   ├─▶ ChatEvent::StreamStarted { conversation_id, model_id }
   ├─▶ ChatEvent::TextDelta { text: "Hi" }
   ├─▶ ChatEvent::TextDelta { text: " there!" }
   ├─▶ ChatEvent::ToolCallStarted { tool_name: "search" }
   ├─▶ ChatEvent::ToolCallCompleted { success: true }
   └─▶ ChatEvent::StreamCompleted { message_id }

5. ChatPresenter handles each ChatEvent:
   ├─▶ StreamStarted → (no-op, loading already shown)
   ├─▶ TextDelta → view.append_to_message(text)
   ├─▶ ToolCallStarted → view.show_tool_indicator(name)
   ├─▶ ToolCallCompleted → view.update_tool_indicator(success)
   └─▶ StreamCompleted → view.hide_loading(), view.enable_input()
```

### EventBus Implementation

```rust
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
    
    pub fn emit(&self, event: AppEvent) {
        tracing::debug!(event_type = %event.type_name(), ?event, "Event emitted");
        let _ = self.sender.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }
}

// Global access
static EVENT_BUS: OnceLock<Arc<EventBus>> = OnceLock::new();

pub fn emit(event: impl Into<AppEvent>) {
    if let Some(bus) = EVENT_BUS.get() {
        bus.emit(event.into());
    }
}

pub fn subscribe() -> broadcast::Receiver<AppEvent> {
    EVENT_BUS.get().expect("EventBus not initialized").subscribe()
}
```

---

## Presenter Pattern

### Presenter Responsibilities

1. **Subscribe** to relevant events from EventBus
2. **Handle** user events by calling services
3. **Transform** domain events into view updates
4. **Manage** UI state (loading, error, streaming)
5. **Update** views with new state

### ChatPresenter Example

```rust
pub struct ChatPresenter {
    // Dependencies
    chat_service: Arc<dyn ChatService>,
    conversation_service: Arc<dyn ConversationService>,
    
    // View reference (weak to avoid cycles)
    view: Weak<dyn ChatViewProtocol>,
    
    // State
    current_conversation_id: RwLock<Option<Uuid>>,
    is_streaming: AtomicBool,
    pending_message: RwLock<String>,
}

impl ChatPresenter {
    pub fn start(&self) {
        let this = Arc::clone(&self);
        spawn(async move {
            let mut rx = subscribe();
            while let Ok(event) = rx.recv().await {
                this.handle_event(event).await;
            }
        });
    }
    
    async fn handle_event(&self, event: AppEvent) {
        match event {
            // User actions
            AppEvent::User(UserEvent::SendMessage { text }) => {
                self.handle_send_message(text).await;
            }
            AppEvent::User(UserEvent::StopStreaming) => {
                self.handle_stop_streaming();
            }
            AppEvent::User(UserEvent::SelectConversation { id }) => {
                self.handle_select_conversation(id).await;
            }
            AppEvent::User(UserEvent::NewConversation) => {
                self.handle_new_conversation().await;
            }
            
            // Domain events
            AppEvent::Chat(ChatEvent::TextDelta { text }) => {
                if let Some(view) = self.view.upgrade() {
                    view.append_to_message(&text);
                }
            }
            AppEvent::Chat(ChatEvent::StreamCompleted { .. }) => {
                self.is_streaming.store(false, Ordering::SeqCst);
                if let Some(view) = self.view.upgrade() {
                    view.hide_loading();
                    view.enable_input();
                }
            }
            AppEvent::Chat(ChatEvent::StreamError { error, .. }) => {
                if let Some(view) = self.view.upgrade() {
                    view.show_error(&error);
                }
            }
            
            // MCP events that affect chat
            AppEvent::Mcp(McpEvent::Started { tools, .. }) => {
                if let Some(view) = self.view.upgrade() {
                    view.update_tool_count(tools.len());
                }
            }
            
            _ => {} // Ignore events we don't care about
        }
    }
    
    async fn handle_send_message(&self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        
        let conv_id = match *self.current_conversation_id.read() {
            Some(id) => id,
            None => {
                // Auto-create conversation
                match self.conversation_service.create().await {
                    Ok(conv) => {
                        *self.current_conversation_id.write() = Some(conv.id);
                        conv.id
                    }
                    Err(e) => {
                        emit(SystemEvent::Error {
                            source: "ChatPresenter".into(),
                            error: e.to_string(),
                            context: Some("Creating conversation".into()),
                        });
                        return;
                    }
                }
            }
        };
        
        // Update view
        if let Some(view) = self.view.upgrade() {
            view.clear_input();
            view.add_user_message(&text);
            view.show_loading();
        }
        
        // Start streaming (ChatService will emit ChatEvents)
        self.is_streaming.store(true, Ordering::SeqCst);
        if let Err(e) = self.chat_service.send_message(conv_id, text).await {
            emit(ChatEvent::StreamError {
                conversation_id: conv_id,
                error: e.to_string(),
                recoverable: true,
            });
        }
    }
}
```

### View Protocol

```rust
/// Protocol for ChatView - implemented by UI layer
pub trait ChatViewProtocol: Send + Sync {
    // Message display
    fn add_user_message(&self, text: &str);
    fn add_assistant_message(&self, text: &str);
    fn append_to_message(&self, text: &str);
    fn update_thinking(&self, text: &str);
    
    // Tool indicators
    fn show_tool_indicator(&self, name: &str);
    fn update_tool_indicator(&self, name: &str, success: bool);
    
    // Loading state
    fn show_loading(&self);
    fn hide_loading(&self);
    
    // Input
    fn clear_input(&self);
    fn enable_input(&self);
    fn disable_input(&self);
    
    // Errors
    fn show_error(&self, message: &str);
    
    // Tools count
    fn update_tool_count(&self, count: usize);
    
    // Scroll
    fn scroll_to_bottom(&self);
}
```

---

## Service Interfaces

### ConversationService

```rust
#[async_trait]
pub trait ConversationService: Send + Sync {
    /// Create a new conversation
    async fn create(&self) -> Result<Conversation>;
    
    /// Load conversation by ID
    async fn load(&self, id: Uuid) -> Result<Conversation>;
    
    /// List all conversations (metadata only, sorted by date)
    async fn list(&self) -> Result<Vec<ConversationSummary>>;
    
    /// Add user message
    async fn add_user_message(&self, conv_id: Uuid, content: String) -> Result<Message>;
    
    /// Add assistant message
    async fn add_assistant_message(
        &self,
        conv_id: Uuid,
        content: String,
        model_id: String,
        thinking: Option<String>,
    ) -> Result<Message>;
    
    /// Update conversation title
    async fn rename(&self, conv_id: Uuid, title: String) -> Result<()>;
    
    /// Delete conversation
    async fn delete(&self, conv_id: Uuid) -> Result<()>;
    
    /// Set active conversation (global state)
    async fn set_active(&self, conv_id: Uuid) -> Result<()>;
    
    /// Get active conversation ID
    fn get_active(&self) -> Option<Uuid>;
}
```

### ChatService

```rust
#[async_trait]
pub trait ChatService: Send + Sync {
    /// Send a message and stream response
    /// Emits ChatEvent::* as streaming progresses
    async fn send_message(&self, conversation_id: Uuid, content: String) -> Result<()>;
    
    /// Cancel ongoing streaming
    fn cancel(&self);
    
    /// Check if currently streaming
    fn is_streaming(&self) -> bool;
}
```

### ProfileService

```rust
#[async_trait]
pub trait ProfileService: Send + Sync {
    /// List all profiles
    async fn list(&self) -> Result<Vec<ModelProfile>>;
    
    /// Get profile by ID
    async fn get(&self, id: Uuid) -> Result<ModelProfile>;
    
    /// Get default profile
    async fn get_default(&self) -> Result<Option<ModelProfile>>;
    
    /// Set default profile
    async fn set_default(&self, id: Uuid) -> Result<()>;
    
    /// Create profile
    async fn create(&self, profile: ModelProfile) -> Result<()>;
    
    /// Update profile
    async fn update(&self, profile: ModelProfile) -> Result<()>;
    
    /// Delete profile
    async fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Test profile connection
    /// Emits ProfileEvent::TestStarted and ProfileEvent::TestCompleted
    async fn test_connection(&self, id: Uuid) -> Result<()>;
}
```

### McpService

```rust
#[async_trait]
pub trait McpService: Send + Sync {
    /// List all configured MCPs
    async fn list(&self) -> Result<Vec<McpConfig>>;
    
    /// Get MCP status
    fn get_status(&self, id: Uuid) -> McpStatus;
    
    /// Enable/disable MCP
    /// Emits McpEvent::Starting, McpEvent::Started or McpEvent::StartFailed
    async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<()>;
    
    /// Get tools from all enabled MCPs
    fn get_available_tools(&self) -> Vec<ToolDefinition>;
    
    /// Add new MCP
    async fn add(&self, config: McpConfig) -> Result<()>;
    
    /// Update MCP configuration
    async fn update(&self, config: McpConfig) -> Result<()>;
    
    /// Delete MCP
    async fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Restart MCP
    async fn restart(&self, id: Uuid) -> Result<()>;
}
```

---

## SerdesAI Agent Integration

### Current (Problematic)

```rust
// In chat_view.rs - tightly coupled
fn send_message(&self) {
    // 1. Extract text from UI
    // 2. Add to conversation
    // 3. Create LLM client
    // 4. Build messages
    // 5. Start streaming
    // 6. Handle events
    // 7. Update UI
    // ALL IN ONE 200-LINE METHOD
}
```

### Target (Event-Driven with Agent Mode)

```rust
// In services/chat.rs
impl ChatServiceImpl {
    pub async fn send_message(&self, conversation_id: Uuid, content: String) -> Result<()> {
        // 1. Save user message
        self.conversation_service.add_user_message(conversation_id, content.clone()).await?;
        
        // 2. Get conversation history
        let conversation = self.conversation_service.load(conversation_id).await?;
        let messages = self.build_llm_messages(&conversation);
        
        // 3. Get current profile
        let profile = self.profile_service.get_default().await?
            .ok_or(Error::NoDefaultProfile)?;
        
        // 4. Get MCP tools
        let tools = self.mcp_service.get_available_tools();
        
        // 5. Emit stream started
        emit(ChatEvent::StreamStarted {
            conversation_id,
            message_id: Uuid::new_v4(),
            model_id: profile.model_id.clone(),
        });
        
        // 6. Stream with Agent mode
        let mut stream = self.agent_client
            .stream_with_tools(&messages, &profile, tools)
            .await?;
        
        let mut full_content = String::new();
        let mut thinking_content = String::new();
        
        while let Some(event) = stream.next().await {
            match event? {
                AgentEvent::TextDelta { text } => {
                    full_content.push_str(&text);
                    emit(ChatEvent::TextDelta { text });
                }
                AgentEvent::ThinkingDelta { text } => {
                    thinking_content.push_str(&text);
                    emit(ChatEvent::ThinkingDelta { text });
                }
                AgentEvent::ToolCallStart { name, id } => {
                    emit(ChatEvent::ToolCallStarted { tool_call_id: id, tool_name: name });
                }
                AgentEvent::ToolCallComplete { name, id, success, result, duration_ms } => {
                    emit(ChatEvent::ToolCallCompleted {
                        tool_call_id: id,
                        tool_name: name,
                        success,
                        result,
                        duration_ms,
                    });
                }
                AgentEvent::Complete { usage } => {
                    // Save assistant message
                    let message = self.conversation_service.add_assistant_message(
                        conversation_id,
                        full_content.clone(),
                        profile.model_id.clone(),
                        if thinking_content.is_empty() { None } else { Some(thinking_content.clone()) },
                    ).await?;
                    
                    emit(ChatEvent::StreamCompleted {
                        conversation_id,
                        message_id: message.id,
                        total_tokens: usage.map(|u| u.total_tokens),
                    });
                }
                AgentEvent::Error { message } => {
                    emit(ChatEvent::StreamError {
                        conversation_id,
                        error: message,
                        recoverable: true,
                    });
                }
            }
        }
        
        Ok(())
    }
}
```

---

## Migration Strategy

### Phase 1: Event System (2 days)

1. Create `src/events/` module
2. Implement `EventBus` with broadcast channel
3. Define all event types (AppEvent hierarchy)
4. Add global `emit()` and `subscribe()` functions
5. Add event logging with tracing

### Phase 2: Extract Services (3-4 days)

1. Create `src/services/` module with traits
2. Implement `ConversationService` (wrap existing storage)
3. Implement `ProfileService` (wrap existing config)
4. Implement `ChatService` (move streaming logic here)
5. Refactor `McpService` to emit events
6. Each service emits domain events as operations progress

### Phase 3: Create Presenters (3-4 days)

1. Create `src/presentation/` module
2. Implement `ChatPresenter` (subscribe to events, update view)
3. Implement `SettingsPresenter`
4. Implement `HistoryPresenter`
5. Implement other presenters
6. Define view protocols (traits)

### Phase 4: Refactor UI Views (4-5 days)

1. Have views emit `UserEvent` instead of calling methods
2. Views implement presenter protocols
3. Remove business logic from views
4. Slim `chat_view.rs` from 980 to <500 lines
5. Slim `settings_view.rs` from 1191 to <500 lines
6. Extract reusable components

### Phase 5: Update Models (1-2 days)

1. Remove `profile_id` from `Conversation`
2. Add `model_id`, `cancelled`, `tool_calls` to `Message`
3. Update storage format to match requirements
4. Migrate existing conversations

---

## Testing Strategy

### Unit Tests (Services)

```rust
#[tokio::test]
async fn test_chat_service_emits_correct_events() {
    let (tx, mut rx) = broadcast::channel(16);
    let service = ChatServiceImpl::new(mock_deps(), tx);
    
    service.send_message(conv_id, "Hello".into()).await.unwrap();
    
    // Verify event sequence
    assert!(matches!(rx.recv().await?, ChatEvent::StreamStarted { .. }));
    // ... verify TextDelta events
    assert!(matches!(rx.recv().await?, ChatEvent::StreamCompleted { .. }));
}
```

### Integration Tests (Event Flow)

```rust
#[tokio::test]
async fn test_full_send_message_flow() {
    let app = TestApp::new().await;
    
    // Emit user event
    emit(UserEvent::SendMessage { text: "Hello".into() });
    
    // Wait for completion
    let events = app.collect_events_until(|e| matches!(e, ChatEvent::StreamCompleted { .. })).await;
    
    // Verify sequence
    assert!(events.iter().any(|e| matches!(e, ChatEvent::StreamStarted { .. })));
    assert!(events.iter().any(|e| matches!(e, ChatEvent::TextDelta { .. })));
    assert!(events.iter().any(|e| matches!(e, ChatEvent::StreamCompleted { .. })));
}
```

### Presenter Tests

```rust
#[tokio::test]
async fn test_chat_presenter_handles_send_message() {
    let mock_view = MockChatView::new();
    let mock_service = MockChatService::new();
    let presenter = ChatPresenter::new(mock_view.clone(), mock_service);
    
    presenter.handle_event(UserEvent::SendMessage { text: "Hello".into() }).await;
    
    assert!(mock_view.clear_input_called());
    assert!(mock_view.add_user_message_called_with("Hello"));
    assert!(mock_view.show_loading_called());
}
```

---

## Success Criteria

1. **Testability**: 80%+ code coverage on services and presenters
2. **Separation**: UI files under 500 lines each
3. **Debuggability**: All events logged, easy to trace issues
4. **Performance**: No regression in streaming latency
5. **Reliability**: No tokio runtime issues
6. **Maintainability**: New features don't require touching UI code

---

## Open Questions

1. Should EventBus use `tokio::sync::broadcast` or `async-broadcast` crate?
2. How to handle UI thread affinity for NSView updates?
3. Should presenters buffer events or process immediately?
4. How to manage presenter lifecycle with view lifecycle?
5. Should we implement event replay for debugging?
