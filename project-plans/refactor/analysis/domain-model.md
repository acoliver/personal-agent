# Domain Analysis: Event-Driven Architecture Refactor

**Plan ID:** PLAN-20250125-REFACTOR
**Author:** Software Architect
**Date:** 2025-01-25
**Revised:** 2025-01-27
**Status:** Approved

> **Revision Note:** Updated to align with the 5-layer event-driven architecture
> defined in `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` and
> `dev-docs/requirements/`. Replaced ServiceRegistry/AgentService/LlmService
> model with EventBus/Presenters/ChatService model.

## 1. Domain Entities

### 1.1 Core Entities

```
┌─────────────────────────────────────────────────────────────┐
│                        EventBus                             │
│  Central event distribution (tokio::sync::broadcast)        │
│  - emit(event: AppEvent)                                    │
│  - subscribe() → Receiver<AppEvent>                         │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                       Presenters                            │
│  Event handlers that coordinate Views ↔ Services            │
└─────────────────────────────────────────────────────────────┘
    ▲         ▲          ▲           ▲          ▲
    │         │          │           │          │
┌───┴────┐┌───┴────┐┌────┴─────┐┌────┴────┐┌───┴──────┐
│ Chat   ││History ││Settings  ││Profile  ││McpAdd    │ ...
│Prestr  ││Prestr  ││Prestr    ││EdPrestr ││Prestr    │
└────────┘└────────┘└──────────┘└─────────┘└──────────┘

┌─────────────────────────────────────────────────────────────┐
│                        Services                             │
│  Business logic, no UI dependencies                         │
└─────────────────────────────────────────────────────────────┘
    ▲         ▲          ▲           ▲          ▲
    │         │          │           │          │
┌───┴────┐┌───┴────┐┌────┴─────┐┌────┴────┐┌───┴──────┐
│ Chat   ││Conver- ││Profile   ││Mcp      ││AppSet-   │
│Service ││sation  ││Service   ││Service  ││tings     │
└────────┘└────────┘└──────────┘└─────────┘└──────────┘
```

### 1.2 Value Objects

**ModelProfile** (from existing code)
- LLM provider and model configuration
- API key reference, base URL, parameters
- System prompt, context limit, thinking settings

**McpConfig** (from existing code)
- MCP server configuration (command, args, env)
- Transport type (stdio, HTTP)
- Enabled/disabled state

**ConversationSummary**
- Lightweight metadata for conversation list display
- id, title, created_at, updated_at, message_count

**McpStatus**
```rust
pub enum McpStatus {
    Disabled,
    Starting,
    Running { tool_count: usize },
    Error { message: String },
    Stopping,
}
```

### 1.3 Event Types

The complete event hierarchy is defined in `dev-docs/requirements/events.md`.
Summary:

```rust
pub enum AppEvent {
    User(UserEvent),              // ~25 variants: SendMessage, ToggleMcp, SaveProfile, etc.
    Chat(ChatEvent),              // ~9 variants: StreamStarted, TextDelta, ToolCallStarted, etc.
    Mcp(McpEvent),                // ~12 variants: Starting, Started, StartFailed, ToolCalled, etc.
    Profile(ProfileEvent),        // ~7 variants: Created, Updated, Deleted, TestCompleted, etc.
    Conversation(ConversationEvent), // ~7 variants: Created, Loaded, Activated, etc.
    Navigation(NavigationEvent),  // ~5 variants: Navigating, Navigated, ModalPresented, etc.
    System(SystemEvent),          // ~13 variants: AppLaunched, HotkeyPressed, Error, etc.
}
```

## 2. Service Boundaries

### 2.1 ChatService Boundary

**Responsibilities:**
- Orchestrate LLM interactions via SerdesAI Agent
- Map SerdesAI events to ChatEvent variants
- Coordinate with ProfileService, ConversationService, McpService
- Handle cancellation

**Interface:**
```rust
#[async_trait]
pub trait ChatService: Send + Sync {
    async fn send_message(&self, conversation_id: Uuid, content: String) -> Result<()>;
    fn cancel(&self);
    fn is_streaming(&self) -> bool;
}
```

**Emits:** ChatEvent::StreamStarted, TextDelta, ThinkingDelta, ToolCallStarted,
ToolCallCompleted, StreamCompleted, StreamCancelled, StreamError, MessageSaved

**Out of scope:**
- Profile management (ProfileService)
- Conversation persistence details (ConversationService)
- MCP lifecycle (McpService)
- UI rendering (ChatPresenter + ChatView)

### 2.2 ConversationService Boundary

**Responsibilities:**
- Conversation CRUD
- Message persistence (append-only .jsonl, metadata in .meta.json)
- Conversation listing with summaries

**Interface:**
```rust
#[async_trait]
pub trait ConversationService: Send + Sync {
    async fn create(&self) -> Result<Conversation>;
    async fn load(&self, id: Uuid) -> Result<Conversation>;
    async fn list(&self) -> Result<Vec<ConversationSummary>>;
    async fn add_user_message(&self, conv_id: Uuid, content: String) -> Result<Message>;
    async fn add_assistant_message(&self, conv_id: Uuid, content: String,
        model_id: String, thinking: Option<String>) -> Result<Message>;
    async fn rename(&self, conv_id: Uuid, title: String) -> Result<()>;
    async fn delete(&self, conv_id: Uuid) -> Result<()>;
    async fn set_active(&self, conv_id: Uuid) -> Result<()>;
    fn get_active(&self) -> Option<Uuid>;
}
```

**Emits:** ConversationEvent::Created, Loaded, TitleUpdated, Deleted, Activated,
Deactivated, ListRefreshed

**Out of scope:**
- Which conversation is "current" for the app (AppSettingsService)
- LLM interactions (ChatService)
- UI display (HistoryPresenter + HistoryView)

### 2.3 ProfileService Boundary

**Responsibilities:**
- Profile CRUD
- API key resolution via SecretsService
- Connection testing

**Interface:**
```rust
#[async_trait]
pub trait ProfileService: Send + Sync {
    async fn list(&self) -> Result<Vec<ModelProfile>>;
    async fn get(&self, id: Uuid) -> Result<ModelProfile>;
    async fn create(&self, profile: ModelProfile) -> Result<()>;
    async fn update(&self, profile: ModelProfile) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn test_connection(&self, id: Uuid) -> Result<()>;
}
```

**Emits:** ProfileEvent::Created, Updated, Deleted, TestStarted, TestCompleted,
ValidationFailed

**Out of scope:**
- Which profile is "default" (AppSettingsService)
- LLM communication (ChatService)
- API key storage internals (SecretsService)

### 2.4 McpService Boundary

**Responsibilities:**
- MCP server lifecycle (start, stop, restart)
- Tool management and toolset provision
- Status tracking
- Configuration CRUD

**Interface:**
```rust
#[async_trait]
pub trait McpService: Send + Sync {
    async fn list(&self) -> Result<Vec<McpConfig>>;
    fn get_status(&self, id: Uuid) -> McpStatus;
    async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<()>;
    fn get_available_tools(&self) -> Vec<ToolDefinition>;
    fn get_toolsets(&self) -> Vec<McpToolset>;
    async fn add(&self, config: McpConfig) -> Result<()>;
    async fn update(&self, config: McpConfig) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn restart(&self, id: Uuid) -> Result<()>;
}
```

**Emits:** McpEvent::Starting, Started, StartFailed, Stopped, Unhealthy,
Recovered, Restarting, ToolCalled, ToolCompleted, ConfigSaved, Deleted

**Out of scope:**
- Tool execution logic (delegated to MCP servers via SerdesAI)
- UI display (SettingsPresenter + SettingsView)

### 2.5 AppSettingsService Boundary

**Responsibilities:**
- Global app settings: default profile, current conversation, hotkey

**Interface:**
```rust
pub trait AppSettingsService: Send + Sync {
    fn get_default_profile_id(&self) -> Option<Uuid>;
    fn set_default_profile_id(&self, id: Uuid) -> Result<()>;
    fn clear_default_profile(&self) -> Result<()>;
    fn get_current_conversation_id(&self) -> Option<Uuid>;
    fn set_current_conversation_id(&self, id: Uuid) -> Result<()>;
    fn get_hotkey(&self) -> Option<HotkeyConfig>;
    fn set_hotkey(&self, hotkey: HotkeyConfig) -> Result<()>;
}
```

**Emits:** ProfileEvent::DefaultChanged, ConversationEvent::Activated/Deactivated,
SystemEvent::HotkeyChanged

### 2.6 SecretsService Boundary

**Responsibilities:**
- Secure credential storage and retrieval
- Encrypted file storage (future: macOS Keychain)

**Consumers:** ProfileService, McpService (ChatService gets keys via ProfileService)

### 2.7 ModelsRegistryService Boundary

**Responsibilities:**
- Fetch/cache models.dev API data
- Provider and model lookup
- 24h TTL with offline fallback

### 2.8 McpRegistryService Boundary

**Responsibilities:**
- Search Official MCP and Smithery registries
- Cache search results

## 3. Presenter Boundaries

Presenters subscribe to events, call services, and update views via protocols.

### 3.1 ChatPresenter

**Subscribes to:** UserEvent::{SendMessage, StopStreaming, NewConversation,
SelectConversation, ToggleThinking}, ChatEvent::*, McpEvent::{Started, Stopped}

**Calls:** ChatService, ConversationService, AppSettingsService

**Updates:** ChatView via ChatViewProtocol

### 3.2 HistoryPresenter

**Subscribes to:** UserEvent::{SelectConversation, StartRenameConversation,
ConfirmRenameConversation}, ConversationEvent::*

**Calls:** ConversationService, AppSettingsService

**Updates:** HistoryView via HistoryViewProtocol

### 3.3 SettingsPresenter

**Subscribes to:** UserEvent::{SelectProfile, ToggleMcp, DeleteProfile,
DeleteMcp}, ProfileEvent::*, McpEvent::*

**Calls:** ProfileService, McpService, AppSettingsService

**Updates:** SettingsView via SettingsViewProtocol

### 3.4 ProfileEditorPresenter

**Subscribes to:** UserEvent::{SaveProfile, TestProfileConnection, SelectModel},
ProfileEvent::TestCompleted

**Calls:** ProfileService

**Updates:** ProfileEditorView via ProfileEditorViewProtocol

### 3.5 McpAddPresenter

**Subscribes to:** UserEvent::{SearchMcpRegistry, SelectMcpFromRegistry}

**Calls:** McpRegistryService

**Updates:** McpAddView via McpAddViewProtocol

### 3.6 McpConfigurePresenter

**Subscribes to:** UserEvent::{SaveMcpConfig, StartMcpOAuth}, McpEvent::ConfigSaved

**Calls:** McpService, SecretsService

**Updates:** McpConfigureView via McpConfigureViewProtocol

### 3.7 ModelSelectorPresenter

**Subscribes to:** UserEvent::{SearchModels, FilterModelsByProvider, SelectModel}

**Calls:** ModelsRegistryService

**Updates:** ModelSelectorView via ModelSelectorViewProtocol

### 3.8 ErrorPresenter

**Subscribes to:** SystemEvent::Error, ChatEvent::StreamError, McpEvent::StartFailed

**Updates:** Global error display

## 4. Data Flows

### 4.1 User Sends Message

```
ChatView
  │ emit(UserEvent::SendMessage { text })
  ▼
EventBus → ChatPresenter
  │ validate input
  │ update view (clear input, show user bubble, show loading)
  │ call ChatService.send_message(conv_id, text)
  ▼
ChatService
  │ load conversation (ConversationService)
  │ get profile + API key (ProfileService → SecretsService)
  │ get toolsets (McpService)
  │ save user message (ConversationService)
  │ build SerdesAI Agent
  │ start streaming
  ▼
SerdesAI Agent → ChatService emits events:
  │ ChatEvent::StreamStarted
  │ ChatEvent::TextDelta (multiple)
  │ ChatEvent::ThinkingDelta (if thinking enabled)
  │ ChatEvent::ToolCallStarted / ToolCallCompleted (if tools used)
  │ ChatEvent::StreamCompleted
  ▼
EventBus → ChatPresenter
  │ TextDelta → view.append_to_message()
  │ ToolCallStarted → view.show_tool_indicator()
  │ StreamCompleted → view.hide_loading(), view.enable_input()
  ▼
ChatView renders updates
```

### 4.2 User Toggles MCP

```
SettingsView
  │ emit(UserEvent::ToggleMcp { id, enabled: true })
  ▼
EventBus → SettingsPresenter
  │ call McpService.set_enabled(id, true)
  ▼
McpService
  │ emit(McpEvent::Starting { id, name })
  │ spawn MCP process
  │ emit(McpEvent::Started { id, tools }) OR emit(McpEvent::StartFailed { error })
  ▼
EventBus → SettingsPresenter
  │ Starting → view.show_mcp_loading(id)
  │ Started → view.show_mcp_active(id, tool_count)
  │ StartFailed → view.show_mcp_error(id, error)
  ▼
EventBus → ChatPresenter (also subscribes to McpEvent::Started)
  │ update available tools display
```

### 4.3 Service Initialization Flow

```
main.rs
  ▼
EventBus::new(256)   ← no dependencies
  ▼
SecretsService::new()  ← no service dependencies
  ▼
AppSettingsService::new()  ← file system only
  ▼
ConversationService::new()  ← file system only
  ▼
ProfileService::new(secrets)  ← depends on SecretsService
  ▼
McpService::new(secrets)  ← depends on SecretsService
  ▼
ModelsRegistryService::new()  ← HTTP client only
  ▼
McpRegistryService::new()  ← HTTP client only
  ▼
ChatService::new(profile, conversation, mcp)  ← depends on 3 services
  ▼
Start all presenters (subscribe to EventBus)
  ▼
Initialize UI (views reference presenters)
```

## 5. State Management

### 5.1 Presenter State

Each presenter manages its own UI-related state:

```rust
pub struct ChatPresenterState {
    current_conversation_id: Option<Uuid>,
    is_streaming: bool,
    stream_handle: Option<StreamHandle>,
    show_thinking: bool,
}

pub struct SettingsPresenterState {
    selected_profile_id: Option<Uuid>,
    mcp_statuses: HashMap<Uuid, McpStatus>,
}
```

### 5.2 Service State

Services manage domain state:

```rust
// ChatService - mostly stateless, holds references
pub struct ChatServiceImpl {
    profile_service: Arc<dyn ProfileService>,
    conversation_service: Arc<dyn ConversationService>,
    mcp_service: Arc<dyn McpService>,
    active_stream: Mutex<Option<StreamHandle>>,
}

// McpService - manages running MCP connections
pub struct McpServiceImpl {
    connections: Mutex<HashMap<Uuid, McpConnection>>,
    configs: Mutex<HashMap<Uuid, McpConfig>>,
    secrets: Arc<dyn SecretsService>,
}

// ConversationService - wraps file storage
pub struct ConversationServiceImpl {
    storage_path: PathBuf,
    active_id: Mutex<Option<Uuid>>,
}
```

### 5.3 Concurrency Control

- **Read-heavy operations:** `Arc<Service>` for shared read-only access
- **Write operations:** `Arc<Mutex<T>>` for state mutations
- **Async operations:** All service methods `async fn`, tasks in global runtime
- **UI updates:** Dispatched to main thread via `dispatch_async_main()`

## 6. Error Handling

### 6.1 Error Categories

```rust
pub enum ServiceError {
    Initialization(String),
    DependencyMissing(String),
    Request(String),
    Timeout(Duration),
    RateLimit,
    NotFound(String),
    InvalidInput(String),
    Conflict(String),
    Network(String),
    Auth(String),
    Parse(String),
    Shutdown(String),
    AlreadyRunning,
}
```

### 6.2 Error Flow

```
External Error (SerdesAI, HTTP, MCP process)
  ↓
Service maps to ServiceError with context
  ↓
Service emits error event (ChatEvent::StreamError, McpEvent::StartFailed, etc.)
  ↓
EventBus delivers to relevant presenters
  ↓
Presenter converts to user-friendly message
  ↓
View displays error
  ↓
ErrorPresenter also captures for global error handling
```

## 7. Domain Rules

### 7.1 Profile Rules

1. Profile is **global**, not per-conversation
2. Default profile is managed by AppSettingsService, not ProfileService
3. Each assistant message stores its `model_id` for historical record
4. Changing profile affects all conversations (existing and new)
5. API keys resolved through ProfileService → SecretsService (ChatService never calls SecretsService directly)

### 7.2 MCP Rules

1. MCPs are shared across all conversations
2. MCP lifecycle is centralized in McpService
3. Individual MCP failure does not block other MCPs
4. Tool names must be unique across active MCPs
5. Toolsets provided to ChatService via `get_toolsets()`

### 7.3 Conversation Rules

1. Messages stored in append-only `.jsonl` format
2. Metadata in `.meta.json` (small, rewritable)
3. Conversations do NOT store `profile_id`
4. User messages persisted before agent starts
5. Assistant messages persisted after stream completes
6. Cancelled streams persist partial content with marker

### 7.4 Context Management

1. Delegated to SerdesAI HistoryProcessor (`TruncateByTokens`)
2. Configured with profile's context limit
3. `keep_first(true)` preserves system prompt
4. No custom ContextService needed

### 7.5 Event Rules

1. All user actions flow through EventBus as UserEvents
2. Services emit domain events as operations progress
3. Presenters subscribe to events, never call each other
4. Events are `Clone` (broadcast channel requirement)
5. Large payloads use IDs, not full content
6. Channel capacity: 256 events

## 8. Storage Layout

```
~/Library/Application Support/PersonalAgent/
├── settings.json                      # AppSettingsService
├── conversations/
│   ├── {timestamp}{random}.jsonl      # ConversationService (messages)
│   └── {timestamp}{random}.meta.json  # ConversationService (metadata)
├── profiles/
│   └── {uuid}.json                    # ProfileService
├── mcps/
│   └── {uuid}.json                    # McpService
├── secrets/
│   └── {type}_{uuid}_{name}.enc       # SecretsService
└── cache/
    ├── models-registry.json           # ModelsRegistryService (24h TTL)
    └── mcp-registry-*.json            # McpRegistryService
```

## 9. Module Structure

```
src/
  events/
    mod.rs
    bus.rs                   # EventBus implementation
    types.rs                 # AppEvent, UserEvent, ChatEvent, etc.

  presentation/
    mod.rs
    chat_presenter.rs
    history_presenter.rs
    settings_presenter.rs
    profile_editor_presenter.rs
    mcp_add_presenter.rs
    mcp_configure_presenter.rs
    model_selector_presenter.rs
    error_presenter.rs

  services/
    mod.rs
    chat.rs                  # ChatService (SerdesAI Agent orchestration)
    conversation.rs          # ConversationService
    profile.rs               # ProfileService
    mcp.rs                   # McpService (consolidated)
    app_settings.rs          # AppSettingsService
    secrets.rs               # SecretsService
    models_registry.rs       # ModelsRegistryService
    mcp_registry.rs          # McpRegistryService

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
      secrets.rs             # Encrypted file storage
    llm/
      client.rs              # SerdesAI LLM client wrapper
      agent.rs               # Agent mode integration
    mcp/
      runtime.rs             # Global MCP runtime
      toolset.rs             # MCP toolset creation

  ui/
    mod.rs
    chat_view.rs             # Pure UI, emits UserEvents (<500 lines)
    settings_view.rs         # Pure UI, emits UserEvents (<500 lines)
    history_view.rs
    profile_editor_view.rs
    mcp_add_view.rs
    mcp_configure_view.rs
    model_selector_view.rs
    components/
      message_bubble.rs
      profile_row.rs
      mcp_row.rs
      tool_indicator.rs
```

## 10. References

- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` — Target architecture
- `dev-docs/architecture/chat-flow.md` — Chat data flow
- `dev-docs/requirements/events.md` — Complete event hierarchy
- `dev-docs/requirements/presentation.md` — Presenter specifications
- `dev-docs/requirements/services/README.md` — Service catalog and dependencies
- `dev-docs/requirements/services/*.md` — Individual service requirements
- `dev-docs/requirements/application.md` — Application-level requirements
