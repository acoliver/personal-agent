# Presenter Event Bus Wiring Specification

**Plan ID**: PLAN-20250128-PRESENTERS
**Status**: Active
**Created**: 2025-01-28

## Overview

This specification defines how to wire presentation layer components to the event bus for real-time state updates. The implementation follows the event-driven architecture specified in `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md`.

## Project Context

### Technology Stack
- **Language**: Rust
- **Build Tool**: Cargo
- **Package Manager**: Cargo
- **Testing**: `cargo test --test e2e_presenter_tests`

### Key Architecture Patterns
1. **Event-Driven Architecture**: All state changes flow through `AppEvent` enum
2. **Presenter Pattern**: Presenters subscribe to events and emit `ViewCommand`
3. **No Direct Service Calls**: Presenters react to events, don't call services directly
4. **Rust Type Safety**: Events are Rust enums, not strings

## Event System

### Event Hierarchy

Per `src/events/types.rs`, the event system is organized as:

```rust
pub enum AppEvent {
    User(UserEvent),        // User-initiated actions from UI
    Chat(ChatEvent),        // Chat and streaming events
    Mcp(McpEvent),          // MCP server lifecycle events
    Profile(ProfileEvent),  // Profile and settings events
    Conversation(ConversationEvent),  // Conversation lifecycle
    Navigation(NavigationEvent),      // Navigation events
    System(SystemEvent),    // System-level events
}
```

### Key Event Types

#### UserEvent
Represents user-initiated actions from the UI:

```rust
pub enum UserEvent {
    // Chat actions
    SendMessage { text: String },
    StopStreaming,
    NewConversation,
    SelectConversation { id: Uuid },
    ToggleThinking,
    StartRenameConversation { id: Uuid },
    ConfirmRenameConversation { id: Uuid, title: String },
    CancelRenameConversation,

    // Profile actions
    SelectProfile { id: Uuid },
    CreateProfile,
    EditProfile { id: Uuid },
    SaveProfile { profile: ModelProfile },
    DeleteProfile { id: Uuid },
    ConfirmDeleteProfile { id: Uuid },
    TestProfileConnection { id: Uuid },

    // MCP actions
    ToggleMcp { id: Uuid, enabled: bool },
    AddMcp,
    SearchMcpRegistry { query: String, source: McpRegistrySource },
    SelectMcpFromRegistry { source: McpRegistrySource },
    ConfigureMcp { id: Uuid },
    SaveMcpConfig { id: Uuid, config: McpConfig },
    DeleteMcp { id: Uuid },
    ConfirmDeleteMcp { id: Uuid },
    StartMcpOAuth { id: Uuid, provider: String },

    // Model selector actions
    OpenModelSelector,
    SearchModels { query: String },
    FilterModelsByProvider { provider_id: Option<String> },
    SelectModel { provider_id: String, model_id: String },

    // Navigation
    Navigate { to: ViewId },
    NavigateBack,
}
```

#### ChatEvent
Represents chat and streaming events:

```rust
pub enum ChatEvent {
    StreamStarted { conversation_id: Uuid, message_id: Uuid, model_id: String },
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ToolCallStarted { tool_call_id: String, tool_name: String },
    ToolCallCompleted {
        tool_call_id: String,
        tool_name: String,
        success: bool,
        result: String,
        duration_ms: u64,
    },
    StreamCompleted {
        conversation_id: Uuid,
        message_id: Uuid,
        total_tokens: Option<u32>,
    },
    StreamCancelled {
        conversation_id: Uuid,
        message_id: Uuid,
        partial_content: String,
    },
    StreamError {
        conversation_id: Uuid,
        error: String,
        recoverable: bool,
    },
    MessageSaved { conversation_id: Uuid, message_id: Uuid },
}
```

#### McpEvent
Represents MCP server lifecycle events:

```rust
pub enum McpEvent {
    Starting { id: Uuid, name: String },
    Started {
        id: Uuid,
        name: String,
        tools: Vec<String>,
        tool_count: usize,
    },
    StartFailed { id: Uuid, name: String, error: String },
    Stopped { id: Uuid, name: String },
    Unhealthy { id: Uuid, name: String, error: String },
    Recovered { id: Uuid, name: String },
    Restarting { id: Uuid, name: String },
    ToolCalled {
        mcp_id: Uuid,
        tool_name: String,
        tool_call_id: String,
    },
    ToolCompleted {
        mcp_id: Uuid,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        duration_ms: u64,
    },
    ConfigSaved { id: Uuid },
    Deleted { id: Uuid, name: String },
}
```

## Presenter Contracts

### Common Presenter Pattern

All presenters follow this pattern:

```rust
use crate::events::{AppEvent, EventBus};
use crate::presentation::view_command::ViewCommand;

pub struct SomePresenter {
    event_bus: Arc<EventBus>,
    view_command_sender: Sender<ViewCommand>,
}

impl SomePresenter {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let presenter = Self {
            event_bus: event_bus.clone(),
            view_command_sender: /* create channel */,
        };

        presenter.subscribe_to_events();
        presenter
    }

    fn subscribe_to_events(&self) {
        // Subscribe to relevant AppEvent variants
        self.event_bus.subscribe(
            |event| matches!(event, AppEvent::SomeEventType(_)),
            self.clone(),
        );
    }

    fn emit_view_command(&self, cmd: ViewCommand) {
        // Send to view layer
    }
}

impl EventHandler for SomePresenter {
    fn handle_event(&self, event: &AppEvent) {
        match event {
            AppEvent::SomeEventType(variant) => {
                self.on_some_event(variant);
            }
            _ => {}
        }
    }
}
```

### ChatPresenter Contract

**File**: `src/presentation/chat_presenter.rs`

**Subscribes to**:
- `AppEvent::User(UserEvent::SendMessage { text })` → Show loading state
- `AppEvent::User(UserEvent::StopStreaming)` → Cancel current stream
- `AppEvent::User(UserEvent::ToggleThinking)` → Toggle thinking visibility
- `AppEvent::User(UserEvent::NewConversation)` → Clear chat view
- `AppEvent::Chat(ChatEvent::StreamStarted { ... })` → Update conversation title
- `AppEvent::Chat(ChatEvent::TextDelta { text })` → Append to message content
- `AppEvent::Chat(ChatEvent::ThinkingDelta { text })` → Append to thinking block
- `AppEvent::Chat(ChatEvent::ToolCallStarted { ... })` → Show tool call UI
- `AppEvent::Chat(ChatEvent::ToolCallCompleted { ... })` → Update tool call status
- `AppEvent::Chat(ChatEvent::StreamCompleted { ... })` → Hide loading, save message
- `AppEvent::Chat(ChatEvent::StreamCancelled { ... })` → Show cancelled state
- `AppEvent::Chat(ChatEvent::StreamError { ... })` → Show error message

**Emits ViewCommand**:
- `ShowLoading { conversation_id, message_id }`
- `HideLoading { conversation_id, message_id }`
- `AppendMessageContent { text }`
- `AppendThinkingContent { text }`
- `ShowToolCallStarted { tool_call_id, tool_name }`
- `ShowToolCallCompleted { tool_call_id, success, result }`
- `ShowError { error, recoverable }`
- `ShowStreamCancelled { conversation_id, message_id }`
- `ClearChatView`
- `SaveMessage { conversation_id, message_id, total_tokens }`

### HistoryPresenter Contract

**File**: `src/presentation/history_presenter.rs`

**Subscribes to**:
- `AppEvent::User(UserEvent::NewConversation)` → Refresh conversation list
- `AppEvent::User(UserEvent::SelectConversation { id })` → Highlight selected item
- `AppEvent::User(UserEvent::StartRenameConversation { id })` → Show rename UI
- `AppEvent::User(UserEvent::ConfirmRenameConversation { id, title })` → Update list item
- `AppEvent::User(UserEvent::CancelRenameConversation)` → Hide rename UI
- `AppEvent::Conversation(ConversationEvent::Created { id, title })` → Append to list
- `AppEvent::Conversation(ConversationEvent::Loaded { id })` → Highlight active
- `AppEvent::Conversation(ConversationEvent::TitleUpdated { id, title })` → Update item
- `AppEvent::Conversation(ConversationEvent::Deleted { id })` → Remove from list
- `AppEvent::Conversation(ConversationEvent::Activated { id })` → Highlight active
- `AppEvent::Conversation(ConversationEvent::Deactivated)` → Clear selection
- `AppEvent::Conversation(ConversationEvent::ListRefreshed { count })` → Reload list

**Emits ViewCommand**:
- `AppendConversationItem { id, title }`
- `RemoveConversationItem { id }`
- `UpdateConversationItem { id, title }`
- `HighlightConversation { id }`
- `ClearConversationSelection`
- `ShowRenameDialog { id, current_title }`
- `HideRenameDialog`
- `ReloadConversationList { count }`

### SettingsPresenter Contract

**File**: `src/presentation/settings_presenter.rs`

**Subscribes to**:

*Profile Events*:
- `AppEvent::User(UserEvent::SelectProfile { id })` → Update active profile indicator
- `AppEvent::User(UserEvent::CreateProfile)` → Show profile editor
- `AppEvent::User(UserEvent::EditProfile { id })` → Show profile editor with data
- `AppEvent::User(UserEvent::SaveProfile { profile })` → Update profile list item
- `AppEvent::User(UserEvent::DeleteProfile { id })` → Show confirmation dialog
- `AppEvent::User(UserEvent::ConfirmDeleteProfile { id })` → Remove from list
- `AppEvent::User(UserEvent::TestProfileConnection { id })` → Show test status
- `AppEvent::Profile(ProfileEvent::Created { id, name })` → Append to list
- `AppEvent::Profile(ProfileEvent::Updated { id, name })` → Update list item
- `AppEvent::Profile(ProfileEvent::Deleted { id, name })` → Remove from list
- `AppEvent::Profile(ProfileEvent::DefaultChanged { profile_id })` → Update default indicator
- `AppEvent::Profile(ProfileEvent::TestStarted { id })` → Show loading spinner
- `AppEvent::Profile(ProfileEvent::TestCompleted { id, success, response_time_ms, error })` → Show test result

*MCP Events*:
- `AppEvent::User(UserEvent::ToggleMcp { id, enabled })` → Update toggle state
- `AppEvent::User(UserEvent::AddMcp)` → Show MCP registry search
- `AppEvent::User(UserEvent::ConfigureMcp { id })` → Show MCP config form
- `AppEvent::User(UserEvent::SaveMcpConfig { id, config })` → Update MCP item
- `AppEvent::User(UserEvent::DeleteMcp { id })` → Show confirmation
- `AppEvent::User(UserEvent::ConfirmDeleteMcp { id })` → Remove from list
- `AppEvent::Mcp(McpEvent::Starting { id, name })` → Show starting state
- `AppEvent::Mcp(McpEvent::Started { id, name, tools, tool_count })` → Update to running state
- `AppEvent::Mcp(McpEvent::StartFailed { id, name, error })` → Show error state
- `AppEvent::Mcp(McpEvent::Stopped { id, name })` → Update to stopped state
- `AppEvent::Mcp(McpEvent::Unhealthy { id, name, error })` → Show unhealthy warning
- `AppEvent::Mcp(McpEvent::Recovered { id, name })` → Update to healthy state
- `AppEvent::Mcp(McpEvent::ConfigSaved { id })` → Update config display
- `AppEvent::Mcp(McpEvent::Deleted { id, name })` → Remove from list

*System Events*:
- `AppEvent::System(SystemEvent::ConfigLoaded)` → Refresh settings view
- `AppEvent::System(SystemEvent::ConfigSaved)` → Show save confirmation
- `AppEvent::System(SystemEvent::ModelsRegistryRefreshed { provider_count, model_count })` → Update model counts

**Emits ViewCommand**:
- `UpdateMcpStatus { id, status, tool_count }`
- `ShowMcpError { id, error }`
- `ShowMcpWarning { id, warning }`
- `ShowMcpConfigEditor { id }`
- `RemoveMcpItem { id }`
- `UpdateMcpConfig { id, config }`
- `ShowConfigSavedConfirmation { message }`
- `ShowDeleteConfirmation { id, item_type }`
- `ShowDeletionSuccess { message }`
- `ShowMcpToolCallStarted { mcp_id, tool_name, tool_call_id }`
- `ShowMcpToolCallCompleted { tool_call_id, success, duration_ms }`

## E2E Testing Strategy

### Test Structure

All E2E tests follow this pattern:

```rust
#[tokio::test]
async fn test_specific_scenario() {
    // 1. Setup: Create EventBus, Presenter, and ViewCommand collector
    let event_bus = Arc::new(EventBus::new());
    let presenter = ChatPresenter::new(event_bus.clone());
    let mut receiver = presenter.subscribe_view_commands();

    // 2. Emit events
    event_bus.emit(AppEvent::User(UserEvent::SendMessage {
        text: "Hello".to_string(),
    }));

    event_bus.emit(AppEvent::Chat(ChatEvent::StreamStarted {
        conversation_id: Uuid::new_v4(),
        message_id: Uuid::new_v4(),
        model_id: "synthetic".to_string(),
    }));

    // 3. Wait for ViewCommands
    let commands = receiver.collect_view_commands(expected_count).await;

    // 4. Assert on specific ViewCommands
    assert_eq!(commands[0], ViewCommand::ShowLoading { ... });
    assert_eq!(commands[1], ViewCommand::AppendMessageContent { ... });
}
```

### Test Scenarios

#### Chat Tests (5 scenarios)
1. **Send and Stream Completion**: Full message flow from send to completion
2. **Tool Call During Stream**: Tool call started/completed events
3. **Thinking Display**: Thinking delta events
4. **Stream Error**: Error handling and recovery
5. **User Cancels Stream**: Cancellation flow

#### MCP Tests (6 scenarios)
1. **MCP Server Start Success**: Starting → Started flow
2. **MCP Server Start Failure**: Starting → StartFailed flow
3. **MCP Unhealthy Then Recovers**: Unhealthy → Recovered flow
4. **MCP Configuration Update**: Configure → Save flow
5. **MCP Deletion Flow**: Delete → Confirm → Deleted flow
6. **MCP Tool Call Events**: ToolCalled → ToolCompleted flow

## Configuration

### Test Profiles

Per dev-docs/architecture/SYSTEM_OVERVIEW.md, the project uses LLM profiles for testing:

- **Synthetic profile**: `~/.llxprt/profiles/synthetic.json`
- **API key file**: `~/.synthetic_key`

Setup:
```bash
# Create profile directory
mkdir -p ~/.llxprt/profiles

# Configure synthetic profile
cat > ~/.llxprt/profiles/synthetic.json << EOF
{
  "name": "synthetic",
  "provider": "anthropic",
  "model": "claude-sonnet",
  "api_key_file": "~/.synthetic_key"
}
EOF

# Set API key
echo "sk-ant-key" > ~/.synthetic_key
```

### MCP Servers

**Exa MCP**:
- No API key required
- Works out of the box
- Used for search tool testing

## Build and Test Commands

```bash
# Build all targets
cargo build --all-targets

# Run all presenter E2E tests
cargo test --test e2e_presenter_tests

# Run specific test
cargo test --test e2e_presenter_tests test_chat_send_and_stream_completion

# Run with output
cargo test --test e2e_presenter_tests -- --nocapture

# Placeholder detection
grep -rn "unimplemented!\|todo!" src/presentation/ tests/
```

## References

### Architecture Documentation
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Event system architecture
- `dev-docs/architecture/SYSTEM_OVERVIEW.md` - System overview
- `dev-docs/requirements/presentation.md` - Presenter requirements

### Implementation Files
- `src/events/types.rs` - Event enum definitions
- `src/events/bus.rs` - EventBus implementation
- `src/presentation/chat_presenter.rs` - ChatPresenter
- `src/presentation/history_presenter.rs` - HistoryPresenter
- `src/presentation/settings_presenter.rs` - SettingsPresenter
- `src/presentation/view_command.rs` - ViewCommand definitions

### Coordination
- `dev-docs/COORDINATING.md` - Multi-phase execution protocol
