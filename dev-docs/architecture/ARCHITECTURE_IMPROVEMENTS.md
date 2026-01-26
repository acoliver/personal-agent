# Architecture Improvements Plan

## Current Architecture Problems

### 1. UI-Business Logic Coupling

The current code has business logic deeply embedded in UI view controllers:

```
src/ui/chat_view.rs (39KB!)
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
- Massive file sizes (chat_view.rs is 39KB)

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

### 3. Async Runtime Fragility

The previous MCP runtime issue where temporary tokio runtimes killed MCP clients demonstrates:
- No clear ownership of async operations
- Runtime lifecycle not managed properly
- Background tasks orphaned when views deallocate

### 4. Missing Abstraction Layers

No clear separation between:
- Data access (storage, config)
- Domain logic (conversations, profiles, MCPs)
- Presentation (UI views)
- Infrastructure (LLM clients, MCP clients)

---

## Target Architecture

### Layer Diagram

```
+-----------------------------------------------------------+
|                     UI Layer (Views)                      |
|  NSViewControllers, NSViews, UI Components                |
|  - Receives ViewModels, renders UI                        |
|  - Sends user actions to Presenters                       |
+-----------------------------------------------------------+
                           |
                           v
+-----------------------------------------------------------+
|                   Presentation Layer                      |
|  Presenters / View Models                                 |
|  - Transforms domain models to view models                |
|  - Handles UI state (loading, error, success)             |
|  - Coordinates between views and use cases                |
+-----------------------------------------------------------+
                           |
                           v
+-----------------------------------------------------------+
|                    Domain Layer                           |
|  Use Cases / Interactors / Services                       |
|  - ConversationService, ProfileService, McpService        |
|  - Business rules and validation                          |
|  - Pure Rust, no UI dependencies                          |
+-----------------------------------------------------------+
                           |
                           v
+-----------------------------------------------------------+
|                 Infrastructure Layer                      |
|  Repositories, Clients, Storage                           |
|  - ConversationRepository (file storage)                  |
|  - ConfigRepository (config.json)                         |
|  - LlmClient (SerdesAI)                                   |
|  - McpClient (SerdesAI MCP)                               |
+-----------------------------------------------------------+
```

### Module Structure

```
src/
  domain/
    mod.rs
    conversation.rs      # Conversation, Message models
    profile.rs           # ModelProfile, Parameters
    mcp.rs               # McpConfig, McpStatus
    
  services/
    mod.rs
    conversation_service.rs  # Create, load, save, switch conversations
    profile_service.rs       # CRUD profiles, set default
    mcp_service.rs           # MCP lifecycle, tool management
    chat_service.rs          # Send message, handle streaming
    
  repositories/
    mod.rs
    conversation_repo.rs     # File-based conversation storage
    config_repo.rs           # Config file operations
    secrets_repo.rs          # Secure credential storage
    
  infrastructure/
    mod.rs
    llm/
      client.rs              # SerdesAI LLM client wrapper
      streaming.rs           # Stream event handling
    mcp/
      runtime.rs             # Global MCP runtime
      toolset.rs             # MCP toolset creation
      
  presentation/
    mod.rs
    chat_presenter.rs        # Chat view logic
    settings_presenter.rs    # Settings view logic
    history_presenter.rs     # History view logic
    
  ui/
    mod.rs
    chat_view.rs             # Pure UI, delegates to presenter
    settings_view.rs         # Pure UI, delegates to presenter
    history_view.rs          # Pure UI, delegates to presenter
    components/              # Reusable UI components
      message_bubble.rs
      profile_row.rs
      mcp_row.rs
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

## Service Interfaces

### ConversationService

```rust
pub trait ConversationService: Send + Sync {
    /// Create a new conversation with default title
    fn create_conversation(&self, profile_id: Uuid) -> Result<Conversation>;
    
    /// Load conversation by ID
    fn load_conversation(&self, id: Uuid) -> Result<Conversation>;
    
    /// List all conversations (metadata only)
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>>;
    
    /// Add user message and save
    fn add_user_message(&self, conv_id: Uuid, content: String) -> Result<()>;
    
    /// Add assistant message and save
    fn add_assistant_message(&self, conv_id: Uuid, content: String, thinking: Option<String>) -> Result<()>;
    
    /// Update conversation title
    fn rename_conversation(&self, conv_id: Uuid, title: String) -> Result<()>;
    
    /// Delete conversation
    fn delete_conversation(&self, conv_id: Uuid) -> Result<()>;
}
```

### ChatService

```rust
pub trait ChatService: Send + Sync {
    /// Send a message and get streaming response
    async fn send_message(
        &self,
        conversation_id: Uuid,
        content: String,
        on_event: impl Fn(ChatEvent) + Send + 'static,
    ) -> Result<()>;
    
    /// Cancel ongoing streaming
    fn cancel_streaming(&self);
}

pub enum ChatEvent {
    UserMessageSaved,
    StreamingStarted,
    TextDelta(String),
    ThinkingDelta(String),
    ToolCallStart { name: String, id: String },
    ToolCallComplete { name: String, success: bool },
    StreamingComplete,
    AssistantMessageSaved,
    Error(String),
}
```

### ProfileService

```rust
pub trait ProfileService: Send + Sync {
    /// Get all profiles
    fn list_profiles(&self) -> Result<Vec<ModelProfile>>;
    
    /// Get default profile
    fn get_default_profile(&self) -> Result<Option<ModelProfile>>;
    
    /// Set default profile
    fn set_default_profile(&self, id: Uuid) -> Result<()>;
    
    /// Create profile
    fn create_profile(&self, profile: ModelProfile) -> Result<()>;
    
    /// Update profile
    fn update_profile(&self, profile: ModelProfile) -> Result<()>;
    
    /// Delete profile
    fn delete_profile(&self, id: Uuid) -> Result<()>;
}
```

### McpService

```rust
pub trait McpService: Send + Sync {
    /// Get all configured MCPs
    fn list_mcps(&self) -> Result<Vec<McpConfig>>;
    
    /// Get MCP status
    fn get_status(&self, id: Uuid) -> McpStatus;
    
    /// Enable/disable MCP
    fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<()>;
    
    /// Get tools from all enabled MCPs
    fn get_available_tools(&self) -> Vec<ToolDefinition>;
    
    /// Add new MCP
    fn add_mcp(&self, config: McpConfig) -> Result<()>;
    
    /// Update MCP
    fn update_mcp(&self, config: McpConfig) -> Result<()>;
    
    /// Delete MCP
    fn delete_mcp(&self, id: Uuid) -> Result<()>;
}
```

---

## Presenter Pattern

### ChatPresenter Example

```rust
pub struct ChatPresenter {
    conversation_service: Arc<dyn ConversationService>,
    chat_service: Arc<dyn ChatService>,
    profile_service: Arc<dyn ProfileService>,
    
    // State
    current_conversation: RwLock<Option<Uuid>>,
    messages: RwLock<Vec<MessageViewModel>>,
    is_streaming: AtomicBool,
}

impl ChatPresenter {
    /// Called when user sends a message
    pub fn send_message(&self, content: String, view: Weak<dyn ChatView>) {
        let conv_id = match *self.current_conversation.read() {
            Some(id) => id,
            None => {
                if let Some(v) = view.upgrade() {
                    v.show_error("No conversation selected");
                }
                return;
            }
        };
        
        // Update UI immediately
        self.add_message_to_ui(MessageViewModel::user(content.clone()), &view);
        
        // Start streaming
        let presenter = self.clone();
        let view_weak = view.clone();
        
        spawn_in_agent_runtime(async move {
            let result = presenter.chat_service.send_message(
                conv_id,
                content,
                move |event| {
                    if let Some(v) = view_weak.upgrade() {
                        presenter.handle_chat_event(event, &v);
                    }
                },
            ).await;
            
            if let Err(e) = result {
                if let Some(v) = view_weak.upgrade() {
                    v.show_error(&e.to_string());
                }
            }
        });
    }
    
    fn handle_chat_event(&self, event: ChatEvent, view: &dyn ChatView) {
        match event {
            ChatEvent::TextDelta(text) => {
                self.append_to_current_message(&text);
                view.update_last_message(self.get_current_message_text());
            }
            ChatEvent::ThinkingDelta(text) => {
                self.append_to_thinking(&text);
                view.update_thinking(self.get_thinking_text());
            }
            ChatEvent::StreamingComplete => {
                self.is_streaming.store(false, Ordering::SeqCst);
                view.streaming_complete();
            }
            ChatEvent::Error(msg) => {
                view.show_error(&msg);
            }
            _ => {}
        }
    }
}
```

### View Protocol

```rust
/// Protocol for ChatView - implemented by UI layer
pub trait ChatView: Send + Sync {
    fn add_message(&self, message: MessageViewModel);
    fn update_last_message(&self, text: &str);
    fn update_thinking(&self, text: &str);
    fn streaming_complete(&self);
    fn show_error(&self, message: &str);
    fn clear_input(&self);
    fn scroll_to_bottom(&self);
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

### Target (Using Agent Mode)

```rust
// In infrastructure/llm/agent_client.rs
pub struct AgentClient {
    runtime: &'static Runtime,
    agent: RwLock<Option<Agent>>,
}

impl AgentClient {
    /// Initialize or reinitialize agent with current profile and MCPs
    pub async fn initialize(
        &self,
        profile: &ModelProfile,
        mcp_toolsets: Vec<McpToolset>,
    ) -> Result<()> {
        let agent = build_agent(profile, mcp_toolsets).await?;
        *self.agent.write() = Some(agent);
        Ok(())
    }
    
    /// Stream a response
    pub async fn stream_response(
        &self,
        messages: &[LlmMessage],
        on_event: impl Fn(StreamEvent) + Send,
    ) -> Result<()> {
        let agent = self.agent.read();
        let agent = agent.as_ref().ok_or(Error::NotInitialized)?;
        
        let mut stream = agent.run_stream(messages).await?;
        
        while let Some(event) = stream.next().await {
            match event? {
                AgentStreamEvent::TextDelta { text } => {
                    on_event(StreamEvent::TextDelta(text));
                }
                AgentStreamEvent::ThinkingDelta { text } => {
                    on_event(StreamEvent::ThinkingDelta(text));
                }
                AgentStreamEvent::ToolExecuted { tool_name, success, .. } => {
                    on_event(StreamEvent::ToolComplete { name: tool_name, success });
                }
                AgentStreamEvent::RunComplete { .. } => {
                    on_event(StreamEvent::Complete);
                }
                AgentStreamEvent::Error { message } => {
                    on_event(StreamEvent::Error(message));
                }
                _ => {}
            }
        }
        
        Ok(())
    }
}
```

---

## Migration Strategy

### Phase 1: Extract Domain Models (1-2 days)

1. Create `src/domain/` module
2. Move `Conversation`, `Message` models
3. Move `ModelProfile`, `ModelParameters` models
4. Move `McpConfig`, `McpStatus` types
5. Update imports throughout codebase

### Phase 2: Extract Repositories (1-2 days)

1. Create `src/repositories/` module
2. Extract `ConversationRepository` from `storage.rs`
3. Extract `ConfigRepository` from `config.rs`
4. Extract `SecretsRepository` from `mcp/secrets.rs`
5. Define repository traits

### Phase 3: Extract Services (2-3 days)

1. Create `src/services/` module
2. Implement `ConversationService`
3. Implement `ProfileService`
4. Implement `ChatService` (wraps LLM client)
5. Refactor `McpService` to use new pattern

### Phase 4: Create Presenters (2-3 days)

1. Create `src/presentation/` module
2. Implement `ChatPresenter`
3. Implement `SettingsPresenter`
4. Implement `HistoryPresenter`
5. Define view protocols

### Phase 5: Refactor UI Views (3-4 days)

1. Slim down `chat_view.rs` to pure UI
2. Delegate business logic to `ChatPresenter`
3. Repeat for `settings_view.rs`
4. Repeat for `history_view.rs`
5. Extract reusable components

### Phase 6: Agent Mode Migration (2-3 days)

1. Implement `AgentClient` using SerdesAI Agent
2. Update `ChatService` to use `AgentClient`
3. Remove manual tool execution loop
4. Update MCP toolset integration

---

## Testing Strategy

### Unit Tests (Services, Repositories)

```rust
#[test]
fn test_conversation_service_creates_with_default_title() {
    let repo = MockConversationRepo::new();
    let service = ConversationServiceImpl::new(Arc::new(repo));
    
    let conv = service.create_conversation(Uuid::new_v4()).unwrap();
    
    assert!(conv.title.unwrap().starts_with("New "));
}

#[test]
fn test_chat_service_saves_user_message() {
    let conv_repo = MockConversationRepo::new();
    let llm_client = MockLlmClient::new();
    let service = ChatServiceImpl::new(Arc::new(conv_repo), Arc::new(llm_client));
    
    let conv_id = Uuid::new_v4();
    conv_repo.save(Conversation::new(conv_id));
    
    service.add_user_message(conv_id, "Hello".to_string()).unwrap();
    
    let conv = conv_repo.load(conv_id).unwrap();
    assert_eq!(conv.messages.len(), 1);
    assert_eq!(conv.messages[0].content, "Hello");
}
```

### Integration Tests (End-to-End)

```rust
#[tokio::test]
async fn test_full_chat_flow() {
    let temp_dir = tempdir().unwrap();
    let services = create_test_services(temp_dir.path());
    
    // Create conversation
    let conv = services.conversation.create_conversation(profile_id).unwrap();
    
    // Send message and collect events
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    
    services.chat.send_message(
        conv.id,
        "Hello".to_string(),
        move |event| events_clone.lock().push(event),
    ).await.unwrap();
    
    // Verify events
    let events = events.lock();
    assert!(events.iter().any(|e| matches!(e, ChatEvent::UserMessageSaved)));
    assert!(events.iter().any(|e| matches!(e, ChatEvent::StreamingComplete)));
    assert!(events.iter().any(|e| matches!(e, ChatEvent::AssistantMessageSaved)));
    
    // Verify persistence
    let loaded = services.conversation.load_conversation(conv.id).unwrap();
    assert_eq!(loaded.messages.len(), 2);
}
```

### UI Tests (With Mocks)

```rust
#[test]
fn test_chat_view_displays_messages() {
    let presenter = MockChatPresenter::new();
    presenter.set_messages(vec![
        MessageViewModel::user("Hello"),
        MessageViewModel::assistant("Hi there!"),
    ]);
    
    let view = ChatViewController::new(presenter);
    
    // Verify UI state
    assert_eq!(view.message_count(), 2);
}
```

---

## Success Criteria

1. **Testability**: 80%+ code coverage on services and repositories
2. **Separation**: UI files under 500 lines each
3. **Performance**: No regression in streaming latency
4. **Reliability**: No tokio runtime issues
5. **Maintainability**: New features don't require touching UI code

---

## Open Questions

1. Should we use `async_trait` for service traits or return boxed futures?
2. How to handle UI thread affinity for NSView updates?
3. Should presenters be `Send + Sync` or UI-thread only?
4. How to manage presenter lifecycle with view lifecycle?
5. Should we use dependency injection framework or manual wiring?
