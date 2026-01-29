# Architect Specification: Event-Driven Architecture Refactor

**Plan ID:** PLAN-20250125-REFACTOR
**Author:** Software Architect
**Date:** 2025-01-25
**Revised:** 2025-01-27
**Status:** Approved

> **Revision Note:** This specification was revised after architecture compliance review
> to align with the target event-driven architecture defined in
> `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` and `dev-docs/requirements/`.
> The original draft described a service consolidation; this revision describes the
> full 5-layer architecture transformation.

## 1. Overview

This specification defines a comprehensive refactoring of the PersonalAgent Rust
application from its current tightly-coupled UI-centric design to a **5-layer
event-driven architecture** with an EventBus, Presenters, and cleanly separated
Services. The refactor addresses:

- Business logic embedded in UI views (chat_view.rs is 980 lines)
- Scattered state management (RefCell, OnceLock, config files)
- No event-driven coordination (direct method calls everywhere)
- Missing abstraction layers between data access, domain logic, presentation, and rendering

## 2. Current State Analysis

### 2.1 Identified Issues

**UI-Business Logic Coupling:**
- `chat_view.rs` (980 lines) contains message persistence, streaming, conversation management, profile loading, MCP tool fetching, and LLM client creation
- `settings_view.rs` (1191 lines) mixes profile CRUD, MCP management, and UI rendering
- Impossible to unit test business logic without mocking NSViews

**Scattered State Management:**
- `RefCell<Option<T>>` fields in view controllers
- Global singletons (`MCP_SERVICE`)
- Config files on disk
- In-memory conversation objects
- Race conditions between UI thread and async operations

**No Event-Driven Architecture:**
- Views call services directly
- Services call callbacks directly
- No central place to log/debug application flow
- Components tightly coupled

**Service Fragmentation:**
- MCP service split across `mcp/service.rs` (singleton) and `mcp/manager.rs` (lifecycle)
- LLM client mixing concerns in `llm/client.rs` (SerdesAI bridge + message handling)
- Configuration mixes file I/O with data modeling and validation

### 2.2 Existing Assets

**Reusable Components:**
- Global runtime pattern in `agent/runtime.rs` (well-implemented)
- Secrets management in `mcp/secrets.rs` (solid foundation)
- Registry cache with HTTP fetching in `registry/` (good pattern)
- Configuration storage with secure permissions in `config/settings.rs`
- Conversation storage in `storage/conversations.rs`

## 3. Target Architecture

### 3.1 Five-Layer Architecture with EventBus

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
│  tokio::sync::broadcast<AppEvent>                           │
│  - Central nervous system for the app                       │
│  - Routes events to subscribers                             │
│  - Enables logging, debugging, replay                       │
│  - Decouples producers from consumers                       │
└─────────────────────────┬───────────────────────────────────┘
                          │ subscribe + dispatch
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                  Presentation Layer (Presenters)            │
│  ChatPresenter, SettingsPresenter, HistoryPresenter, etc.   │
│  - Subscribe to events they care about                      │
│  - Transform domain models to view state                    │
│  - Handle UI state (loading, error, success)                │
│  - Call services and emit result events                     │
│  - Update views via view protocols (main thread)            │
└─────────────────────────┬───────────────────────────────────┘
                          │ calls services, emit(DomainEvent)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Domain Layer (Services)                  │
│  ChatService, ConversationService, ProfileService, etc.     │
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
│  - LlmClient (SerdesAI Agent)                               │
│  - McpRuntime (SerdesAI MCP)                                │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Event System

The EventBus is the central nervous system. All user actions, domain events,
and system events flow through it using `tokio::sync::broadcast`.

```rust
pub struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self;
    pub fn emit(&self, event: AppEvent);
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent>;
}

// Global access
pub fn emit(event: impl Into<AppEvent>);
pub fn subscribe() -> broadcast::Receiver<AppEvent>;
```

**Event Hierarchy** (see `dev-docs/requirements/events.md` for complete definitions):

```rust
pub enum AppEvent {
    User(UserEvent),           // UI-initiated actions
    Chat(ChatEvent),           // Streaming and message events
    Mcp(McpEvent),             // MCP server lifecycle
    Profile(ProfileEvent),     // Profile CRUD events
    Conversation(ConversationEvent), // Conversation lifecycle
    Navigation(NavigationEvent),     // View transitions
    System(SystemEvent),       // Application-level events
}
```

### 3.3 Presenter Pattern

Presenters subscribe to events, call services, and update views via protocols.
Each major view has a dedicated presenter:

| Presenter | View | Key Events |
|-----------|------|------------|
| ChatPresenter | ChatView | SendMessage, StopStreaming, ChatEvent::* |
| HistoryPresenter | HistoryView | SelectConversation, ConversationEvent::* |
| SettingsPresenter | SettingsView | SelectProfile, ToggleMcp, ProfileEvent::*, McpEvent::* |
| ProfileEditorPresenter | ProfileEditorView | SaveProfile, TestConnection |
| McpAddPresenter | McpAddView | SearchMcpRegistry, SelectMcpFromRegistry |
| McpConfigurePresenter | McpConfigureView | SaveMcpConfig, StartMcpOAuth |
| ModelSelectorPresenter | ModelSelectorView | SearchModels, SelectModel |
| ErrorPresenter | (global) | SystemEvent::Error, ChatEvent::StreamError, McpEvent::StartFailed |

See `dev-docs/requirements/presentation.md` for full presenter specifications.

### 3.4 View Protocols

Views implement protocols that presenters use to push state updates.
All view updates must happen on the main thread (macOS NSView requirement).

```rust
pub trait ChatViewProtocol: Send + Sync {
    fn add_user_message(&self, text: &str);
    fn append_to_message(&self, text: &str);
    fn show_loading(&self);
    fn hide_loading(&self);
    fn clear_input(&self);
    fn enable_input(&self);
    fn show_error(&self, message: &str);
    fn update_tool_count(&self, count: usize);
    fn scroll_to_bottom(&self);
}
```

## 4. Service Specifications

Services are pure Rust with no UI dependencies. They emit domain events via the
EventBus as operations progress.

### 4.1 ChatService

**Responsibilities:**
- Orchestrate LLM interactions via SerdesAI Agent
- Map SerdesAI agent events to clean ChatEvent stream events
- Coordinate with ProfileService, ConversationService, and McpService
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

**Collaborators:** ProfileService (model config + API key), ConversationService (history, persistence), McpService (toolsets)

See `dev-docs/requirements/services/chat.md` and `dev-docs/architecture/chat-flow.md`.

### 4.2 ConversationService

**Responsibilities:**
- Conversation CRUD (create, load, list, rename, delete)
- Message persistence (append user/assistant messages)
- Active conversation tracking (delegates to AppSettingsService)

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

See `dev-docs/requirements/services/conversation.md`.

### 4.3 ProfileService

**Responsibilities:**
- Profile CRUD (create, update, delete, list, get)
- API key resolution via SecretsService
- Connection testing (emits ProfileEvent::TestStarted/TestCompleted)

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

**Note:** ProfileService does NOT manage which profile is "default". That is
AppSettingsService's responsibility.

See `dev-docs/requirements/services/profile.md`.

### 4.4 McpService

**Responsibilities:**
- MCP server lifecycle (start, stop, restart)
- Tool management (get available tools, provide toolsets to ChatService)
- Status tracking (running, error, disabled)
- Configuration CRUD (add, update, delete)

**Interface:**
```rust
#[async_trait]
pub trait McpService: Send + Sync {
    async fn list(&self) -> Result<Vec<McpConfig>>;
    fn get_status(&self, id: Uuid) -> McpStatus;
    async fn set_enabled(&self, id: Uuid, enabled: bool) -> Result<()>;
    fn get_available_tools(&self) -> Vec<ToolDefinition>;
    async fn add(&self, config: McpConfig) -> Result<()>;
    async fn update(&self, config: McpConfig) -> Result<()>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn restart(&self, id: Uuid) -> Result<()>;
}
```

**Consolidates:** `mcp/service.rs` (singleton + tool routing) and `mcp/manager.rs` (lifecycle)

See `dev-docs/requirements/services/mcp.md`.

### 4.5 AppSettingsService

**Responsibilities:**
- Global app settings: default profile, current conversation, hotkey
- Stored in `settings.json` (separate from per-profile/per-mcp config)

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

See `dev-docs/requirements/services/app-settings.md`.

### 4.6 SecretsService

**Responsibilities:**
- Secure credential storage (API keys, OAuth tokens)
- Encrypted file storage (future: macOS Keychain)
- Keyfile path resolution

See `dev-docs/requirements/services/secrets.md`. Existing implementation is solid.

### 4.7 ModelsRegistryService

**Responsibilities:**
- Fetch and cache model info from models.dev API
- Provider/model lookup
- 24h TTL cache with offline fallback

See `dev-docs/requirements/services/models-registry.md`.

### 4.8 McpRegistryService

**Responsibilities:**
- Search Official MCP registry and Smithery registry
- Cache search results

See `dev-docs/requirements/services/mcp-registry.md`.

### 4.9 Context Management

Context compression is **delegated to SerdesAI's HistoryProcessor** (`TruncateByTokens`),
configured per-profile with the model's context limit. No custom ContextService is needed.

See `dev-docs/requirements/services/context.md` (superseded) and
`dev-docs/architecture/chat-flow.md` for the integration pattern.

## 5. Cross-Cutting Concerns

### 5.1 Error Handling

Standardize on `ServiceError` enum across all services:

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

### 5.2 Logging and Observability

- Structured logging with `tracing` crate
- EventBus logs all events with type names
- Service-level spans for operations

### 5.3 Threading and Async

- All service methods are `async fn`
- Services spawn tasks in the global runtime (`agent/runtime.rs`)
- `Arc<Mutex<T>>` for shared mutable state, `Arc<T>` for read-only
- View updates dispatched to main thread (macOS requirement)

## 6. Service Dependencies

```
ChatService
    ├── ProfileService (model config with resolved API key)
    ├── ConversationService (message history, persistence)
    └── McpService (toolsets for Agent)

ProfileService
    └── SecretsService (API key storage, resolution)

McpService
    └── SecretsService (env var secrets)

AppSettingsService
    └── (file system storage only)

ConversationService
    └── (file system storage only)

SecretsService
    └── (encrypted file storage)

ModelsRegistryService
    └── (HTTP client, local cache)

McpRegistryService
    └── (HTTP client for registry APIs)
```

**Key design:** ChatService does NOT call SecretsService directly. API keys
are resolved through ProfileService. The "default profile" is owned by
AppSettingsService, not ProfileService.

## 7. Integration Points

### 7.1 UI Layer Migration

Views must be refactored to:
1. **Emit UserEvents** instead of calling services directly
2. **Implement view protocols** for presenter-driven updates
3. **Contain no business logic** — pure rendering
4. **Be <500 lines each** (down from 980/1191)

### 7.2 Storage Layer

No changes to underlying storage. Services wrap existing storage:
- `storage/conversations.rs` → wrapped by ConversationService
- `config/settings.rs` → wrapped by AppSettingsService, ProfileService

### 7.3 SerdesAI Integration

ChatService builds and runs a SerdesAI Agent with:
- ModelConfig from ProfileService (including resolved API key)
- HistoryProcessor for context compression
- McpToolsets from McpService
- Streaming events mapped to ChatEvent variants

See `dev-docs/architecture/chat-flow.md` for the complete data flow.

## 8. Success Criteria

### 8.1 Functional Requirements

- [ ] EventBus distributes all events (User, Chat, Mcp, Profile, Conversation, Navigation, System)
- [ ] All 8 presenters subscribe to events and update views
- [ ] All services implement their specified interfaces
- [ ] ChatService orchestrates LLM via SerdesAI Agent with tool support
- [ ] Views emit UserEvents, contain no business logic, are <500 lines
- [ ] Event flows work end-to-end (see `dev-docs/requirements/events.md` examples)

### 8.2 Non-Functional Requirements

- [ ] No regression in startup time or streaming latency
- [ ] All existing tests pass
- [ ] New tests achieve 80%+ coverage of services and presenters
- [ ] Clippy passes with no warnings
- [ ] No `unwrap()` or `expect()` in production code paths

### 8.3 Code Quality Requirements

- [ ] All public APIs documented with rustdoc
- [ ] Module-level documentation updated
- [ ] Error handling consistent across services (ServiceError)
- [ ] All events logged via EventBus for debuggability

## 9. Migration Strategy

The plan follows a **3-phase pattern** for each major component (Stub → TDD → Implementation):

| Phase | Component | Description |
|-------|-----------|-------------|
| 01–03 | Preflight & Analysis | Verify dependencies, analyze domain, write pseudocode |
| 04–06 | Event System | EventBus + all event types (Stub → TDD → Impl) |
| 07–09 | Service Layer | All services (Stub → TDD → Impl) |
| 10–12 | Presenter Layer | All 8 presenters (Stub → TDD → Impl) |
| 13 | UI Integration | Refactor views to emit events, implement protocols |
| 14 | Data Migration | Storage format updates, backwards compatibility |
| 15 | Deprecation | Remove legacy code paths |
| 16 | E2E Testing | Full event flow verification |

Each phase has a verification sub-phase (e.g., 04a, 05a) to confirm correctness.

See `project-plans/refactor/plan/00-overview.md` for the complete phase plan.

## 10. Risk Assessment

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| SerdesAI toolset API not ready | High | Medium | Build stub, integrate later |
| MCP service regression | High | Low | Comprehensive integration tests |
| UI thread issues (main thread dispatch) | Medium | Medium | View protocol pattern |
| Performance degradation from event overhead | Low | Low | Benchmarking at each phase |
| Breaking changes for UI | Medium | Medium | Gradual migration with feature flags |

## 11. References

- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` — Target architecture (canonical)
- `dev-docs/architecture/chat-flow.md` — Chat data flow
- `dev-docs/requirements/events.md` — Complete event hierarchy
- `dev-docs/requirements/presentation.md` — Presenter specifications
- `dev-docs/requirements/services/README.md` — Service catalog and dependencies
- `dev-docs/requirements/services/*.md` — Individual service requirements
- `dev-docs/requirements/application.md` — Application-level requirements
- `project-plans/refactor/plan/00-overview.md` — Phase plan
- `project-plans/refactor/analysis/pseudocode/` — Implementation pseudocode
