# Appendix: Exhaustive Integration Contracts

**Plan ID:** PLAN-20250128-GPUI
**Source of Truth:** This appendix is synchronized with actual code enums.
**Last Sync:** 2025-01-29

---

## A. UserEvent Mapping (GPUI Emits -> EventBus)

Source file: `src/events/types.rs` lines 46-141

GPUI emits `UserEvent` variants via `GpuiBridge.emit()`. The forwarder wraps them in `AppEvent::User(event)` and publishes to EventBus.

### Complete UserEvent Variant Table

| # | Variant | Payload | GPUI Emits | Origin | Handler |
|---|---------|---------|------------|--------|---------|
| **Chat Actions** |||||
| 1 | `SendMessage` | `{ text: String }` | YES | Send button / Enter key | ChatPresenter |
| 2 | `StopStreaming` | (none) | YES | Stop button | ChatPresenter |
| 3 | `NewConversation` | (none) | YES | [+] button | ChatPresenter |
| 4 | `SelectConversation` | `{ id: Uuid }` | YES | History list click | HistoryPresenter |
| 5 | `ToggleThinking` | (none) | YES | [T] button | ChatPresenter |
| 6 | `StartRenameConversation` | `{ id: Uuid }` | YES | Title click | ChatPresenter |
| 7 | `ConfirmRenameConversation` | `{ id: Uuid, title: String }` | YES | Rename confirm | ChatPresenter |
| 8 | `CancelRenameConversation` | (none) | YES | Rename cancel / Esc | ChatPresenter |
| **Profile Actions** |||||
| 9 | `SelectProfile` | `{ id: Uuid }` | YES | Profile row click | SettingsPresenter |
| 10 | `CreateProfile` | (none) | YES | Add profile button | SettingsPresenter |
| 11 | `EditProfile` | `{ id: Uuid }` | YES | Edit button | SettingsPresenter |
| 12 | `SaveProfile` | `{ profile: ModelProfile }` | YES | Save button | SettingsPresenter |
| 13 | `DeleteProfile` | `{ id: Uuid }` | YES | Delete button | SettingsPresenter |
| 14 | `ConfirmDeleteProfile` | `{ id: Uuid }` | YES | Confirm dialog | SettingsPresenter |
| 15 | `TestProfileConnection` | `{ id: Uuid }` | YES | Test button | SettingsPresenter |
| **MCP Actions** |||||
| 16 | `ToggleMcp` | `{ id: Uuid, enabled: bool }` | YES | Toggle switch | SettingsPresenter |
| 17 | `AddMcp` | (none) | YES | Add MCP button | SettingsPresenter |
| 18 | `SearchMcpRegistry` | `{ query: String, source: McpRegistrySource }` | YES | Search field | McpAddPresenter |
| 19 | `SelectMcpFromRegistry` | `{ source: McpRegistrySource }` | YES | Registry result click | McpAddPresenter |
| 20 | `ConfigureMcp` | `{ id: Uuid }` | YES | Configure button | SettingsPresenter |
| 21 | `SaveMcpConfig` | `{ id: Uuid, config: McpConfig }` | YES | Save config button | McpConfigurePresenter |
| 22 | `DeleteMcp` | `{ id: Uuid }` | YES | Delete button | SettingsPresenter |
| 23 | `ConfirmDeleteMcp` | `{ id: Uuid }` | YES | Confirm dialog | SettingsPresenter |
| 24 | `StartMcpOAuth` | `{ id: Uuid, provider: String }` | YES | OAuth button | McpConfigurePresenter |
| **Model Selector Actions** |||||
| 25 | `OpenModelSelector` | (none) | YES | Model dropdown click | SettingsPresenter |
| 26 | `SearchModels` | `{ query: String }` | YES | Search field | ModelSelectorPresenter |
| 27 | `FilterModelsByProvider` | `{ provider_id: Option<String> }` | YES | Provider filter | ModelSelectorPresenter |
| 28 | `SelectModel` | `{ provider_id: String, model_id: String }` | YES | Model row click | ModelSelectorPresenter |
| **Navigation** |||||
| 29 | `Navigate` | `{ to: ViewId }` | YES | Tab click / nav button | All presenters |
| 30 | `NavigateBack` | (none) | YES | Back button | All presenters |

**Total: 30 UserEvent variants, ALL emitted by GPUI**

### ViewId Enum (for Navigate)

| Variant | GPUI Tab/View |
|---------|---------------|
| `Chat` | Chat tab |
| `History` | History tab |
| `Settings` | Settings tab |
| `ProfileEditor { id: Option<Uuid> }` | Profile editor sheet |
| `McpAdd` | MCP add sheet |
| `McpConfigure { id: Uuid }` | MCP configure sheet |
| `ModelSelector` | Model selector popup |

---

## B. ViewCommand Mapping (Presenters Send -> GPUI Handles)

Source file: `src/presentation/view_command.rs` lines 19-261

Presenters send `ViewCommand` via `ViewCommandSink.send()`. GPUI receives via `GpuiBridge.drain_commands()` and applies to `UiState`.

### Complete ViewCommand Variant Table

| # | Variant | Payload | GPUI Handles | UI State Update | Test Coverage |
|---|---------|---------|--------------|-----------------|---------------|
| **Chat Commands** ||||||
| 1 | `ConversationCreated` | `{ id: Uuid, profile_id: Uuid }` | YES | Set `active_conversation_id`, clear messages | P04 |
| 2 | `MessageAppended` | `{ conversation_id: Uuid, role: MessageRole, content: String }` | YES | Append to `messages` list | P04 |
| 3 | `ShowThinking` | `{ conversation_id: Uuid }` | YES | Set `is_streaming = true`, show spinner | P04 |
| 4 | `HideThinking` | `{ conversation_id: Uuid }` | YES | Set `is_streaming = false`, hide spinner | P04 |
| 5 | `AppendStream` | `{ conversation_id: Uuid, chunk: String }` | YES | Append to `streaming_content` | P04 |
| 6 | `FinalizeStream` | `{ conversation_id: Uuid, tokens: u64 }` | YES | Move streaming to message, clear buffer | P04 |
| 7 | `StreamCancelled` | `{ conversation_id: Uuid, partial_content: String }` | YES | Show partial, mark cancelled | P04 |
| 8 | `StreamError` | `{ conversation_id: Uuid, error: String, recoverable: bool }` | YES | Show error in chat area | P04 |
| 9 | `AppendThinking` | `{ conversation_id: Uuid, content: String }` | YES | Append to `thinking_content` | P04 |
| 10 | `ShowToolCall` | `{ conversation_id: Uuid, tool_name: String, status: String }` | YES | Add to `tool_calls` list | P04 |
| 11 | `UpdateToolCall` | `{ conversation_id, tool_name, status, result: Option, duration: Option }` | YES | Update tool call entry | P04 |
| 12 | `MessageSaved` | `{ conversation_id: Uuid }` | YES | (no visible change, maybe log) | P04 |
| 13 | `ToggleThinkingVisibility` | (none) | YES | Toggle `thinking_visible` | P04 |
| 14 | `ConversationRenamed` | `{ id: Uuid, new_title: String }` | YES | Update title in dropdown | P04 |
| 15 | `ConversationCleared` | (none) | YES | Clear `messages`, `streaming_content` | P04 |
| 16 | `HistoryUpdated` | `{ count: Option<usize> }` | YES | Trigger history refresh | P04 |
| **History Commands** ||||||
| 17 | `ConversationListRefreshed` | `{ conversations: Vec<ConversationSummary> }` | YES | Replace `conversations` list | P04 |
| 18 | `ConversationActivated` | `{ id: Uuid }` | YES | Set `active_conversation_id`, load messages | P04 |
| 19 | `ConversationDeleted` | `{ id: Uuid }` | YES | Remove from `conversations` list | P04 |
| 20 | `ConversationTitleUpdated` | `{ id: Uuid, title: String }` | YES | Update title in list | P04 |
| **Settings Commands** ||||||
| 21 | `ShowSettings` | `{ profiles: Vec<ProfileSummary> }` | YES | Populate `profiles` list | P04 |
| 22 | `ShowNotification` | `{ message: String }` | YES | Set `notification = Some(message)` | P04 |
| 23 | `ProfileCreated` | `{ id: Uuid, name: String }` | YES | Add to `profiles` list | P04 |
| 24 | `ProfileUpdated` | `{ id: Uuid, name: String }` | YES | Update in `profiles` list | P04 |
| 25 | `ProfileDeleted` | `{ id: Uuid }` | YES | Remove from `profiles` list | P04 |
| 26 | `DefaultProfileChanged` | `{ profile_id: Option<Uuid> }` | YES | Update `is_default` flags | P04 |
| 27 | `ProfileTestStarted` | `{ id: Uuid }` | YES | Show test-in-progress indicator | P04 |
| 28 | `ProfileTestCompleted` | `{ id, success, response_time_ms: Option, error: Option }` | YES | Show test result | P04 |
| **MCP Commands** ||||||
| 29 | `McpServerStarted` | `{ id: Uuid, tool_count: usize }` | YES | Update MCP status to Running | P04 |
| 30 | `McpServerFailed` | `{ id: Uuid, error: String }` | YES | Update MCP status to Failed, show error | P04 |
| 31 | `McpToolsUpdated` | `{ tools: Vec<ToolInfo> }` | YES | Update available tools display | P04 |
| 32 | `McpStatusChanged` | `{ id: Uuid, status: McpStatus }` | YES | Update `mcp_statuses[id]` | P04 |
| 33 | `McpConfigSaved` | `{ id: Uuid }` | YES | Show save confirmation | P04 |
| 34 | `McpDeleted` | `{ id: Uuid }` | YES | Remove from MCP list | P04 |
| **Model Selector Commands** ||||||
| 35 | `ModelSearchResults` | `{ models: Vec<ModelInfo> }` | YES | Populate search results | P04 |
| 36 | `ModelSelected` | `{ provider_id: String, model_id: String }` | YES | Update selected model display | P04 |
| **Error Commands** ||||||
| 37 | `ShowError` | `{ title: String, message: String, severity: ErrorSeverity }` | YES | Set `error = Some(ErrorState{...})` | P04 |
| 38 | `ClearError` | (none) | YES | Set `error = None` | P04 |
| **Navigation Commands** ||||||
| 39 | `NavigateTo` | `{ view: ViewId }` | YES | Set `active_tab`, push nav stack | P04 |
| 40 | `NavigateBack` | (none) | YES | Pop nav stack | P04 |
| 41 | `ShowModal` | `{ modal: ModalId }` | YES | Set `modal = Some(ModalState{...})` | P04 |
| 42 | `DismissModal` | (none) | YES | Set `modal = None` | P04 |

**Total: 42 ViewCommand variants, ALL handled by GPUI**

### Supporting Types

| Type | Variants/Fields | Source |
|------|-----------------|--------|
| `MessageRole` | `User`, `Assistant`, `System`, `Tool` | view_command.rs:266-272 |
| `McpStatus` | `Starting`, `Running`, `Stopped`, `Failed`, `Unhealthy` | view_command.rs:309-316 |
| `ErrorSeverity` | `Info`, `Warning`, `Error`, `Critical` | view_command.rs:356-362 |
| `ModalId` | `ConfirmDeleteConversation`, `ConfirmDeleteProfile`, `ConfirmDeleteMcp` | view_command.rs:346-351 |
| `ViewId` | `Chat`, `History`, `Settings`, `ProfileEditor`, `McpAdd`, `McpConfigure`, `ModelSelector` | view_command.rs:332-341 |

---

## C. Channel Configuration

### Backpressure Strategy

| Channel | Direction | Capacity | Overflow Behavior |
|---------|-----------|----------|-------------------|
| UserEvent | GPUI -> tokio | **bounded(256)** | **Drop + warn** (UI responsiveness > event delivery) |
| ViewCommand | tokio -> GPUI | **bounded(1024)** | **Drop + notify** (UI must catch up) |

### Rationale

- **UserEvent bounded(256):** User can't physically generate more than ~10 events/second. 256 provides headroom for bursts. If somehow full, drop and warn (indicates severe lag).
- **ViewCommand bounded(1024):** Streaming can generate many deltas. 1024 provides buffer. If full, presenter should coalesce (e.g., combine multiple `AppendStream` into one). GPUI notified regardless to catch up.

### Channel Overflow Handling

```rust
// In ViewCommandSink::send()
match self.tx.try_send(cmd) {
    Ok(()) => self.notifier.notify(),
    Err(TrySendError::Full(cmd)) => {
        tracing::warn!("ViewCommand channel full, dropping: {:?}", cmd);
        self.notifier.notify(); // Still wake GPUI to drain backlog
    }
    Err(TrySendError::Disconnected(_)) => {
        // GPUI closed, presenter should stop
        tracing::info!("GPUI disconnected");
    }
}
```

---

## D. GPUI Notifier Specification

### What is the Notifier?

The notifier is a **thread-safe handle** that allows tokio tasks to wake the GPUI event loop.

### GPUI API Binding

GPUI provides `cx.background_executor()` and window handles that can schedule work on the main thread. The exact mechanism:

```rust
// During GPUI app initialization
let app = Application::new();

// Option 1: Use cx.spawn() to schedule work
// Option 2: Use a waker pattern

// For our bridge, we use a simple pattern:
// 1. Store a channel sender that GPUI polls
// 2. Store a "needs_update" atomic flag
// 3. GPUI checks flag on each frame/tick
```

### Concrete Implementation

```rust
// src/ui_gpui/bridge/notifier.rs

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Thread-safe notifier for waking GPUI from tokio
#[derive(Clone)]
pub struct GpuiNotifierImpl {
    needs_update: Arc<AtomicBool>,
}

impl GpuiNotifierImpl {
    pub fn new() -> Self {
        Self {
            needs_update: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check and clear the update flag (called by GPUI render loop)
    pub fn check_and_clear(&self) -> bool {
        self.needs_update.swap(false, Ordering::SeqCst)
    }
}

impl GpuiNotifier for GpuiNotifierImpl {
    fn notify(&self) {
        self.needs_update.store(true, Ordering::SeqCst);
        // Note: GPUI will pick this up on next frame
        // For immediate wakeup, we'd need cx.notify() inside GPUI context
    }
}
```

### GPUI Integration Point

```rust
// In MainPanel::render() or update()
impl MainPanel {
    fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        // Check atomic flag
        if self.notifier.check_and_clear() || self.bridge.has_pending_commands() {
            // Drain and apply
            let commands = self.bridge.drain_commands();
            for cmd in commands {
                self.state.apply(cmd);
            }
            cx.notify(); // Request re-render
        }
    }
}
```

### Lifecycle

1. **Creation:** `GpuiNotifierImpl::new()` called during app init
2. **Distribution:** Clone given to `ViewCommandSink` for each presenter
3. **Usage:** Presenters call `sink.send(cmd)` which calls `notifier.notify()`
4. **Consumption:** GPUI render loop calls `notifier.check_and_clear()`
5. **Cleanup:** Dropped when GPUI app closes (Arc ensures last reference cleans up)

---

## E. End-to-End Behavioral Test

### Test: Full Round-Trip Flow

```rust
// tests/gpui_bridge_e2e_test.rs

/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006
/// @scenario End-to-end: UserEvent -> EventBus -> Presenter -> ViewCommand -> GPUI state
#[tokio::test]
async fn test_e2e_send_message_flow() {
    // 1. Setup
    let event_bus = Arc::new(EventBus::new(16));
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(16);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(16);
    let notifier = MockNotifier::new();

    // Create bridge (GPUI side)
    let bridge = GpuiBridge::new(user_tx.clone(), view_rx);
    
    // Create sink (presenter side)
    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    // Spawn forwarder
    let _forwarder = spawn_user_event_forwarder(event_bus.clone(), user_rx);

    // Subscribe to EventBus (simulating presenter)
    let mut bus_rx = event_bus.subscribe();

    // 2. GPUI emits UserEvent
    bridge.emit(UserEvent::SendMessage { text: "Hello".to_string() });

    // 3. Verify EventBus received it
    tokio::time::sleep(Duration::from_millis(50)).await;
    let received = bus_rx.try_recv();
    assert!(received.is_ok());
    assert!(matches!(
        received.unwrap(),
        AppEvent::User(UserEvent::SendMessage { text }) if text == "Hello"
    ));

    // 4. Simulate presenter sending ViewCommand
    sink.send(ViewCommand::ShowThinking { 
        conversation_id: Uuid::new_v4() 
    });

    // 5. Verify notifier was called
    assert!(notifier.was_notified());

    // 6. GPUI drains commands
    let commands = bridge.drain_commands();
    assert_eq!(commands.len(), 1);
    assert!(matches!(commands[0], ViewCommand::ShowThinking { .. }));

    // 7. Simulate state application
    let mut state = UiState::default();
    for cmd in commands {
        state.apply(cmd);
    }
    assert!(state.is_streaming);
}
```

### Test: Channel Overflow Behavior

```rust
/// @plan PLAN-20250128-GPUI.P05
/// @scenario ViewCommand channel overflow triggers notify but doesn't block
#[tokio::test]
async fn test_view_command_overflow_behavior() {
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(2); // Tiny buffer
    let notifier = MockNotifier::new();
    let sink = ViewCommandSink::new(view_tx, notifier.clone());

    // Fill channel
    sink.send(ViewCommand::ClearError);
    sink.send(ViewCommand::ClearError);
    
    // Next send should overflow but not block
    let start = std::time::Instant::now();
    sink.send(ViewCommand::ClearError); // This one will be dropped
    let elapsed = start.elapsed();

    // Should not block
    assert!(elapsed.as_millis() < 10);
    
    // Notifier should still be called (3 times)
    assert_eq!(notifier.notify_count(), 3);

    // Only 2 commands in channel
    let bridge = GpuiBridge::new(
        flume::bounded::<UserEvent>(1).0,
        view_rx
    );
    let commands = bridge.drain_commands();
    assert_eq!(commands.len(), 2);
}
```

---

## F. Synchronization Test

This test ensures the mapping tables stay synchronized with actual code:

```rust
// tests/integration_contract_sync_test.rs

/// Verify UserEvent variant count matches documentation
/// If this test fails, update appendix-integration-contracts.md
#[test]
fn test_user_event_variant_count() {
    // Count variants by attempting to match exhaustively
    // This will fail to compile if variants are added/removed
    fn count_variants(event: UserEvent) -> usize {
        match event {
            UserEvent::SendMessage { .. } => 1,
            UserEvent::StopStreaming => 2,
            UserEvent::NewConversation => 3,
            UserEvent::SelectConversation { .. } => 4,
            UserEvent::ToggleThinking => 5,
            UserEvent::StartRenameConversation { .. } => 6,
            UserEvent::ConfirmRenameConversation { .. } => 7,
            UserEvent::CancelRenameConversation => 8,
            UserEvent::SelectProfile { .. } => 9,
            UserEvent::CreateProfile => 10,
            UserEvent::EditProfile { .. } => 11,
            UserEvent::SaveProfile { .. } => 12,
            UserEvent::DeleteProfile { .. } => 13,
            UserEvent::ConfirmDeleteProfile { .. } => 14,
            UserEvent::TestProfileConnection { .. } => 15,
            UserEvent::ToggleMcp { .. } => 16,
            UserEvent::AddMcp => 17,
            UserEvent::SearchMcpRegistry { .. } => 18,
            UserEvent::SelectMcpFromRegistry { .. } => 19,
            UserEvent::ConfigureMcp { .. } => 20,
            UserEvent::SaveMcpConfig { .. } => 21,
            UserEvent::DeleteMcp { .. } => 22,
            UserEvent::ConfirmDeleteMcp { .. } => 23,
            UserEvent::StartMcpOAuth { .. } => 24,
            UserEvent::OpenModelSelector => 25,
            UserEvent::SearchModels { .. } => 26,
            UserEvent::FilterModelsByProvider { .. } => 27,
            UserEvent::SelectModel { .. } => 28,
            UserEvent::Navigate { .. } => 29,
            UserEvent::NavigateBack => 30,
        }
    }
    
    // Document says 30 variants
    assert_eq!(count_variants(UserEvent::NavigateBack), 30);
}

/// Verify ViewCommand variant count matches documentation
#[test]
fn test_view_command_variant_count() {
    fn count_variants(cmd: ViewCommand) -> usize {
        match cmd {
            ViewCommand::ConversationCreated { .. } => 1,
            ViewCommand::MessageAppended { .. } => 2,
            ViewCommand::ShowThinking { .. } => 3,
            ViewCommand::HideThinking { .. } => 4,
            ViewCommand::AppendStream { .. } => 5,
            ViewCommand::FinalizeStream { .. } => 6,
            ViewCommand::StreamCancelled { .. } => 7,
            ViewCommand::StreamError { .. } => 8,
            ViewCommand::AppendThinking { .. } => 9,
            ViewCommand::ShowToolCall { .. } => 10,
            ViewCommand::UpdateToolCall { .. } => 11,
            ViewCommand::MessageSaved { .. } => 12,
            ViewCommand::ToggleThinkingVisibility => 13,
            ViewCommand::ConversationRenamed { .. } => 14,
            ViewCommand::ConversationCleared => 15,
            ViewCommand::HistoryUpdated { .. } => 16,
            ViewCommand::ConversationListRefreshed { .. } => 17,
            ViewCommand::ConversationActivated { .. } => 18,
            ViewCommand::ConversationDeleted { .. } => 19,
            ViewCommand::ConversationTitleUpdated { .. } => 20,
            ViewCommand::ShowSettings { .. } => 21,
            ViewCommand::ShowNotification { .. } => 22,
            ViewCommand::ProfileCreated { .. } => 23,
            ViewCommand::ProfileUpdated { .. } => 24,
            ViewCommand::ProfileDeleted { .. } => 25,
            ViewCommand::DefaultProfileChanged { .. } => 26,
            ViewCommand::ProfileTestStarted { .. } => 27,
            ViewCommand::ProfileTestCompleted { .. } => 28,
            ViewCommand::McpServerStarted { .. } => 29,
            ViewCommand::McpServerFailed { .. } => 30,
            ViewCommand::McpToolsUpdated { .. } => 31,
            ViewCommand::McpStatusChanged { .. } => 32,
            ViewCommand::McpConfigSaved { .. } => 33,
            ViewCommand::McpDeleted { .. } => 34,
            ViewCommand::ModelSearchResults { .. } => 35,
            ViewCommand::ModelSelected { .. } => 36,
            ViewCommand::ShowError { .. } => 37,
            ViewCommand::ClearError => 38,
            ViewCommand::NavigateTo { .. } => 39,
            ViewCommand::NavigateBack => 40,
            ViewCommand::ShowModal { .. } => 41,
            ViewCommand::DismissModal => 42,
        }
    }
    
    // Document says 42 variants
    assert_eq!(count_variants(ViewCommand::DismissModal), 42);
}
```

---

## G. Revision History

| Date | Change |
|------|--------|
| 2025-01-29 | Initial creation with exhaustive variant tables |
| | Added backpressure strategy |
| | Added notifier specification |
| | Added E2E behavioral test |
| | Added synchronization test |
