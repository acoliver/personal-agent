# GPUI Migration Specification

**Plan ID:** PLAN-20250128-GPUI
**Author:** Architect
**Date:** 2025-01-28
**Status:** Draft (Revised 2025-01-29)

**Authoritative Integration Contracts:** See `appendix-integration-contracts.md` for exhaustive UserEvent/ViewCommand mappings.

---

## 1. Purpose

Migrate the PersonalAgent UI layer from objc2/AppKit views to GPUI components while:
- Preserving the existing NSStatusItem/menu bar integration
- Keeping the existing EventBus, Presenters, and Services architecture
- Maintaining visual parity with current UI (tabs, chat bubbles, settings)
- Adding user-configurable background transparency
- Keeping the existing icon (`assets/MenuBarIcon.imageset/icon-32.png`)

**NOT in scope:**
- Deleting old AppKit UI code (will coexist for comparison/rollback)
- Changing the presenter layer or event architecture
- Changing the service layer

---

## 2. Architectural Decisions

### Pattern: Hybrid Native + GPUI (following ExactoBar pattern)

```
┌───────────────────────────────────────────────────────────────┐
│                    NSStatusItem (native)                       │
│                    Click → toggle popup                        │
└───────────────────────────┬───────────────────────────────────┘
                            │
                            ▼
┌───────────────────────────────────────────────────────────────┐
│                    GPUI Application                            │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │              WindowKind::PopUp                           │  │
│  │  ┌─────────────────────────────────────────────────────┐│  │
│  │  │ MainPanel (Render trait)                            ││  │
│  │  │  ┌─────────┬─────────┬──────────┐                   ││  │
│  │  │  │  Chat   │ History │ Settings │   ← Tab bar       ││  │
│  │  │  └─────────┴─────────┴──────────┘                   ││  │
│  │  │  ┌─────────────────────────────────────────────────┐││  │
│  │  │  │                                                 │││  │
│  │  │  │   Active View Content                           │││  │
│  │  │  │   (ChatView | HistoryView | SettingsView)       │││  │
│  │  │  │                                                 │││  │
│  │  │  └─────────────────────────────────────────────────┘││  │
│  │  └─────────────────────────────────────────────────────┘│  │
│  └─────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

### Technology Stack

- **Menu bar icon:** NSStatusItem via objc2 (keep existing code)
- **Popup window:** GPUI `WindowKind::PopUp` with `WindowBackgroundAppearance::Blurred`
- **Components:** GPUI `div()` builder pattern, `Render` trait
- **Cross-runtime channels:** `flume` (runtime-agnostic)
- **Styling:** GPUI's `Hsla`, `px()`, theming system

---

## 3. CRITICAL: Runtime Bridge Architecture

### The Problem

- **GPUI** uses smol runtime internally
- **Presenters/Services** use tokio runtime
- We need non-blocking bidirectional communication

### The Solution: flume Channels + GPUI Notifier

```
        (smol/GPUI thread)                         (tokio runtime)
┌────────────────────────────┐            ┌───────────────────────────┐
│ UI widgets / actions       │            │ EventBus (broadcast)      │
│                            │            │ Presenters                │
│  try_send(UserEvent) ──────┼──────────► │ recv_async(UserEvent)     │
│                            │            │   -> process              │
│  try_recv(ViewCommand)     │            │   -> send(ViewCommand)    │
│   -> apply to UI state     │ ◄──────────┼───+ notifier.notify()     │
│   -> cx.notify()           │            └───────────────────────────┘
└────────────────────────────┘
```

### Channel Types

| Channel | Direction | Type | Ownership |
|---------|-----------|------|-----------|
| UserEvent | GPUI → tokio | `flume::bounded(256)` | Sender in GPUI, Receiver in tokio forwarder |
| ViewCommand | tokio → GPUI | `flume::bounded(1024)` | Sender in ViewCommandSink (given to presenters), Receiver in GPUI |

### Key Constraint: Never Block Either Runtime

- **GPUI side:** Only use `try_send()` and `try_recv()` (non-blocking)
- **tokio side:** Use `recv_async().await` and `send_async().await`
- **Re-render trigger:** tokio calls `notifier.notify()` after sending ViewCommand

### Backpressure Strategy

| Channel | Overflow Behavior | Rationale |
|---------|-------------------|-----------|
| UserEvent (256) | **Drop + warn** | UI responsiveness > event delivery; user can retry |
| ViewCommand (1024) | **Drop + notify** | Presenters should coalesce; GPUI must catch up |

See `appendix-integration-contracts.md` Section C for detailed overflow handling code.

### GPUI Notifier Mechanism

The notifier is an `AtomicBool` flag that tokio sets and GPUI checks:

1. **tokio side:** After `ViewCommandSink.send()`, calls `notifier.notify()` which sets `needs_update = true`
2. **GPUI side:** In render loop, calls `notifier.check_and_clear()` to see if update needed
3. **If true:** GPUI drains `view_rx` and calls `cx.notify()` to re-render

See `appendix-integration-contracts.md` Section D for concrete `GpuiNotifierImpl` code.

---

## 4. Project Structure

```
src/
  ui_gpui/
    mod.rs                    # Module exports
    app.rs                    # GPUI Application setup, popup window creation
    tray.rs                   # NSStatusItem bridge (adapted from existing code)
    theme.rs                  # GPUI color system, user transparency setting
    state.rs                  # UI state container (owned by GPUI)
    bridge/
      mod.rs                  # Bridge module exports
      user_event_forwarder.rs # tokio task: flume → EventBus
      view_command_sink.rs    # Wrapper for presenters (flume + notifier)
      gpui_bridge.rs          # Main bridge struct with channels
    components/
      mod.rs                  # Component exports
      tab_bar.rs              # Tab navigation (Chat, History, Settings)
      message_bubble.rs       # Chat message bubbles
      input_bar.rs            # Text input + Send/Stop buttons
      conversation_list.rs    # Conversation dropdown/list
      toggle.rs               # Toggle switch component
      button.rs               # Standard button component
    views/
      mod.rs                  # View exports
      main_panel.rs           # Root panel with tab switching + ViewCommand drain
      chat_view.rs            # Chat messages + input
      history_view.rs         # Conversation history list
      settings_view.rs        # Settings with nested views
```

---

## 5. Integration Contracts

> **AUTHORITATIVE SOURCE:** See `appendix-integration-contracts.md` for the **exhaustive** mapping tables.
> This section provides a summary. The appendix contains all 30 UserEvent variants and all 42 ViewCommand variants.

### 5.1 UserEvent Emission (GPUI → EventBus)

The GPUI UI emits these **existing** `UserEvent` variants (from `src/events/types.rs`).

**Summary table (see appendix for complete list of 30 variants):**

| UI Action | View | UserEvent Variant |
|-----------|------|-------------------|
| Click Send / Press Enter | Chat | `UserEvent::SendMessage { text }` |
| Click Stop button | Chat | `UserEvent::StopStreaming` |
| Click [+] New Conversation | Chat | `UserEvent::NewConversation` |
| Click [T] Thinking toggle | Chat | `UserEvent::ToggleThinking` |
| Confirm rename in dropdown | Chat | `UserEvent::ConfirmRenameConversation { id, title }` |
| Click conversation in list | History | `UserEvent::SelectConversation { id }` |
| Click delete conversation | History | (emit delete, then confirm modal) |
| Select profile | Settings | `UserEvent::SelectProfile { id }` |
| Toggle MCP enabled | Settings | `UserEvent::ToggleMcp { id, enabled }` |
| Click Add MCP | Settings | `UserEvent::AddMcp` |
| Click Configure MCP | Settings | `UserEvent::ConfigureMcp { id }` |
| Search models | ModelSelector | `UserEvent::SearchModels { query }` |
| Select model | ModelSelector | `UserEvent::SelectModel { provider_id, model_id }` |
| Navigate to view | Any | `UserEvent::Navigate { to: ViewId }` |

**Event boundary:** GPUI **only** emits `UserEvent` variants. Other `AppEvent` variants (`ChatEvent`, `McpEvent`, `ProfileEvent`, `ConversationEvent`, `NavigationEvent`, `SystemEvent`) are emitted by services/presenters, not by GPUI.

### 5.2 ViewCommand Reception (Presenters → GPUI)

The GPUI UI handles these **existing** `ViewCommand` variants (from `src/presentation/view_command.rs`).

**Summary table (see appendix for complete list of 42 variants):**

#### Chat View Commands

| ViewCommand | UI State Update |
|-------------|-----------------|
| `ConversationCreated { id, profile_id }` | Set active conversation ID |
| `ConversationActivated { id }` | Load conversation, scroll to bottom |
| `MessageAppended { conversation_id, role, content }` | Append message to list |
| `ShowThinking { conversation_id }` | Show thinking spinner |
| `HideThinking { conversation_id }` | Hide thinking spinner |
| `AppendStream { conversation_id, chunk }` | Append to current streaming message |
| `AppendThinking { conversation_id, content }` | Append to thinking section |
| `FinalizeStream { conversation_id, tokens }` | Mark message complete, hide spinner |
| `StreamCancelled { conversation_id, partial_content }` | Show partial, mark cancelled |
| `StreamError { conversation_id, error, recoverable }` | Show error in chat |
| `ShowToolCall { conversation_id, tool_name, status }` | Show tool call indicator |
| `UpdateToolCall { ... }` | Update tool status/result |
| `ToggleThinkingVisibility` | Toggle thinking section collapse |
| `ConversationRenamed { id, new_title }` | Update title in dropdown |
| `ConversationCleared` | Clear chat view |

#### History View Commands

| ViewCommand | UI State Update |
|-------------|-----------------|
| `ConversationListRefreshed { conversations }` | Replace conversation list |
| `HistoryUpdated { count }` | Refresh list, update count badge |
| `ConversationDeleted { id }` | Remove from list |
| `ConversationTitleUpdated { id, title }` | Update title in list |

#### Settings View Commands

| ViewCommand | UI State Update |
|-------------|-----------------|
| `ShowSettings { profiles }` | Display profile list |
| `ProfileCreated { id, name }` | Add to profile list |
| `ProfileUpdated { id, name }` | Update in list |
| `ProfileDeleted { id }` | Remove from list |
| `DefaultProfileChanged { profile_id }` | Update default indicator |
| `McpStatusChanged { id, status }` | Update MCP status indicator |
| `McpServerStarted { id, tool_count }` | Show running, tool count |
| `McpServerFailed { id, error }` | Show error state |
| `McpConfigSaved { id }` | Show success notification |
| `McpDeleted { id }` | Remove from list |

#### Global Commands

| ViewCommand | UI State Update |
|-------------|-----------------|
| `ShowError { title, message, severity }` | Show error banner/toast |
| `ClearError` | Dismiss error |
| `ShowNotification { message }` | Show notification toast |
| `NavigateTo { view }` | Switch active view/tab |
| `NavigateBack` | Pop navigation stack |
| `ShowModal { modal }` | Display confirmation modal |
| `DismissModal` | Close modal |

---

## 6. Bridge Implementation

### 6.1 GpuiBridge (Main Bridge Struct)

```rust
// src/ui_gpui/bridge/gpui_bridge.rs

use flume::{Receiver, Sender};
use crate::events::types::UserEvent;
use crate::presentation::ViewCommand;

/// Bridge between GPUI (smol) and tokio runtimes
pub struct GpuiBridge {
    /// Send UserEvents from GPUI to tokio
    user_tx: Sender<UserEvent>,
    /// Receive ViewCommands from tokio presenters
    view_rx: Receiver<ViewCommand>,
}

impl GpuiBridge {
    pub fn new(user_tx: Sender<UserEvent>, view_rx: Receiver<ViewCommand>) -> Self {
        Self { user_tx, view_rx }
    }

    /// Emit a UserEvent (non-blocking)
    pub fn emit(&self, event: UserEvent) {
        if let Err(e) = self.user_tx.try_send(event) {
            tracing::warn!("Failed to emit UserEvent: {}", e);
        }
    }

    /// Drain all pending ViewCommands (non-blocking)
    pub fn drain_commands(&self) -> Vec<ViewCommand> {
        let mut commands = Vec::new();
        while let Ok(cmd) = self.view_rx.try_recv() {
            commands.push(cmd);
        }
        commands
    }
}
```

### 6.2 ViewCommandSink (Given to Presenters)

```rust
// src/ui_gpui/bridge/view_command_sink.rs

use flume::Sender;
use crate::presentation::ViewCommand;

/// Sink for presenters to send ViewCommands to GPUI
/// Includes notifier to wake GPUI after sending
pub struct ViewCommandSink {
    tx: Sender<ViewCommand>,
    notifier: GpuiNotifier,
}

impl ViewCommandSink {
    pub fn new(tx: Sender<ViewCommand>, notifier: GpuiNotifier) -> Self {
        Self { tx, notifier }
    }

    /// Send a ViewCommand and wake GPUI (non-blocking)
    pub fn send(&self, cmd: ViewCommand) {
        match self.tx.try_send(cmd) {
            Ok(()) => {
                self.notifier.notify();
            }
            Err(flume::TrySendError::Full(_)) => {
                tracing::warn!("ViewCommand channel full, dropping");
                self.notifier.notify(); // Still wake in case backlogged
            }
            Err(flume::TrySendError::Disconnected(_)) => {
                tracing::info!("GPUI disconnected, ignoring ViewCommand");
            }
        }
    }
}
```

### 6.3 UserEvent Forwarder (tokio task)

```rust
// src/ui_gpui/bridge/user_event_forwarder.rs

use std::sync::Arc;
use flume::Receiver;
use crate::events::{AppEvent, EventBus};
use crate::events::types::UserEvent;

/// Spawn tokio task that forwards UserEvents from flume to EventBus
pub fn spawn_user_event_forwarder(
    event_bus: Arc<EventBus>,
    user_rx: Receiver<UserEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(event) = user_rx.recv_async().await {
            let _ = event_bus.publish(AppEvent::User(event));
        }
        tracing::info!("UserEvent forwarder shutting down");
    })
}
```

---

## 7. UI State Management

### State Lives in GPUI (Single-Threaded)

```rust
// src/ui_gpui/state.rs

use uuid::Uuid;
use crate::presentation::view_command::{MessageRole, ConversationSummary, McpStatus};

/// UI state owned entirely by GPUI (no cross-thread mutation)
#[derive(Default)]
pub struct UiState {
    // === Active View ===
    pub active_tab: Tab,
    pub navigation_stack: Vec<ViewId>,

    // === Chat State ===
    pub active_conversation_id: Option<Uuid>,
    pub messages: Vec<UiMessage>,
    pub input_text: String,
    pub is_streaming: bool,
    pub streaming_content: String,
    pub thinking_content: String,
    pub thinking_visible: bool,
    pub tool_calls: Vec<ToolCallState>,

    // === History State ===
    pub conversations: Vec<ConversationSummary>,

    // === Settings State ===
    pub profiles: Vec<ProfileSummary>,
    pub mcp_statuses: HashMap<Uuid, McpStatus>,

    // === UI State ===
    pub error: Option<ErrorState>,
    pub notification: Option<String>,
    pub modal: Option<ModalState>,
}

#[derive(Clone)]
pub struct UiMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub tool_calls: Vec<ToolCallState>,
}
```

### ViewCommand Application

```rust
impl UiState {
    /// Apply a ViewCommand to update state
    pub fn apply(&mut self, cmd: ViewCommand) {
        match cmd {
            ViewCommand::AppendStream { chunk, .. } => {
                self.streaming_content.push_str(&chunk);
            }
            ViewCommand::FinalizeStream { .. } => {
                // Move streaming content to message
                self.messages.push(UiMessage {
                    role: MessageRole::Assistant,
                    content: std::mem::take(&mut self.streaming_content),
                    thinking: if self.thinking_content.is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut self.thinking_content))
                    },
                    ..Default::default()
                });
                self.is_streaming = false;
            }
            ViewCommand::ShowThinking { .. } => {
                self.is_streaming = true;
            }
            ViewCommand::HideThinking { .. } => {
                self.is_streaming = false;
            }
            // ... handle all ViewCommand variants
            _ => {}
        }
    }
}
```

---

## 8. GPUI View Pattern

### MainPanel with ViewCommand Drain

```rust
// src/ui_gpui/views/main_panel.rs

pub struct MainPanel {
    bridge: GpuiBridge,
    state: UiState,
}

impl MainPanel {
    /// Called on each render or when notifier wakes us
    fn drain_and_apply(&mut self, cx: &mut Context<Self>) {
        let commands = self.bridge.drain_commands();
        for cmd in commands {
            self.state.apply(cmd);
        }
        if !commands.is_empty() {
            cx.notify(); // Trigger re-render
        }
    }
}

impl Render for MainPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Drain ViewCommands first
        self.drain_and_apply(cx);

        // Render based on state
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(self.theme.background)
            .child(TabBar::new(self.state.active_tab))
            .child(self.render_active_view(cx))
    }
}
```

### Emitting UserEvents

```rust
impl MainPanel {
    fn on_send_clicked(&mut self, cx: &mut Context<Self>) {
        let text = std::mem::take(&mut self.state.input_text);
        if !text.trim().is_empty() {
            self.bridge.emit(UserEvent::SendMessage { text });
        }
        cx.notify();
    }

    fn on_stop_clicked(&mut self, cx: &mut Context<Self>) {
        self.bridge.emit(UserEvent::StopStreaming);
    }

    fn on_new_conversation(&mut self, cx: &mut Context<Self>) {
        self.bridge.emit(UserEvent::NewConversation);
    }
}
```

---

## 9. Initialization Sequence

```rust
// At app startup (in main.rs or dedicated init module)

pub async fn initialize_gpui_app(event_bus: Arc<EventBus>) -> Result<()> {
    // 1. Create channels
    let (user_tx, user_rx) = flume::bounded::<UserEvent>(256);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(1024);

    // 2. Spawn UserEvent forwarder (tokio task)
    spawn_user_event_forwarder(event_bus.clone(), user_rx);

    // 3. Create GPUI app and capture notifier
    let app = Application::new();
    let notifier = app.notifier(); // Capture thread-safe wake handle

    // 4. Create ViewCommandSink for presenters
    let view_sink = ViewCommandSink::new(view_tx, notifier.clone());

    // 5. Initialize presenters with ViewCommandSink
    let chat_presenter = ChatPresenter::new(
        event_bus.clone(),
        conversation_service.clone(),
        chat_service.clone(),
        view_sink.clone(),
    );
    // ... other presenters

    // 6. Start presenters
    chat_presenter.start().await?;

    // 7. Create GPUI bridge for UI
    let bridge = GpuiBridge::new(user_tx, view_rx);

    // 8. Run GPUI with bridge
    app.run(move |cx| {
        // Create window with MainPanel
        let panel = MainPanel::new(bridge, UiState::default());
        // ... window creation
    });

    Ok(())
}
```

---

## 10. Dependencies

### Cargo.toml additions

```toml
[dependencies]
# GPUI from Zed (pin to specific commit for stability)
gpui = { git = "https://github.com/zed-industries/zed", branch = "main", package = "gpui" }

# Runtime-agnostic channels for tokio/smol bridge
flume = "0.11"
```

---

## 11. Formal Requirements

### REQ-GPUI-001: Popup Window
- [REQ-GPUI-001.1] GPUI popup opens on menu bar icon click
- [REQ-GPUI-001.2] Popup positioned below status item (same as NSPopover)
- [REQ-GPUI-001.3] Popup closes when clicking outside
- [REQ-GPUI-001.4] Background transparency configurable (0.1 to 1.0)

### REQ-GPUI-002: Tab Navigation
- [REQ-GPUI-002.1] Three tabs: Chat, History, Settings
- [REQ-GPUI-002.2] Tab state persists within session
- [REQ-GPUI-002.3] Keyboard shortcuts ⌘1, ⌘2, ⌘3 switch tabs

### REQ-GPUI-003: Chat View
- [REQ-GPUI-003.1] Display conversation messages (user right, assistant left)
- [REQ-GPUI-003.2] Model label above assistant messages
- [REQ-GPUI-003.3] Streaming cursor during response
- [REQ-GPUI-003.4] Input bar with Send/Stop buttons
- [REQ-GPUI-003.5] Conversation title dropdown
- [REQ-GPUI-003.6] Thinking toggle [T] button
- [REQ-GPUI-003.7] New conversation [+] button
- [REQ-GPUI-003.8] Collapsible thinking sections

### REQ-GPUI-004: History View
- [REQ-GPUI-004.1] List all conversations (newest first)
- [REQ-GPUI-004.2] Click to load conversation
- [REQ-GPUI-004.3] Delete conversation action

### REQ-GPUI-005: Settings View
- [REQ-GPUI-005.1] Profile management section
- [REQ-GPUI-005.2] MCP configuration section
- [REQ-GPUI-005.3] Appearance settings (transparency slider)
- [REQ-GPUI-005.4] Navigation to sub-views (profile editor, MCP configure)

### REQ-GPUI-006: Bridge Integration
- [REQ-GPUI-006.1] Use `flume` channels for cross-runtime communication
- [REQ-GPUI-006.2] GPUI emits UserEvents via `try_send()` (non-blocking)
- [REQ-GPUI-006.3] GPUI receives ViewCommands via `try_recv()` drain loop
- [REQ-GPUI-006.4] Presenters use ViewCommandSink with GPUI notifier
- [REQ-GPUI-006.5] Notifier wakes GPUI to drain ViewCommands
- [REQ-GPUI-006.6] `cx.notify()` triggers re-render after state update

### REQ-GPUI-007: Visual Parity
- [REQ-GPUI-007.1] Same color scheme as existing UI (Theme.rs values)
- [REQ-GPUI-007.2] Same 400x500 popup size
- [REQ-GPUI-007.3] Same message bubble styling (green user, gray assistant)
- [REQ-GPUI-007.4] Same font sizes and spacing

### REQ-GPUI-008: Menu Bar Icon
- [REQ-GPUI-008.1] Keep existing icon (assets/MenuBarIcon.imageset/icon-32.png)
- [REQ-GPUI-008.2] Keep existing NSStatusItem setup
- [REQ-GPUI-008.3] Click handler toggles GPUI popup

---

## 12. Success Criteria

- [ ] GPUI popup opens from menu bar click
- [ ] Tab navigation works (Chat, History, Settings)
- [ ] Chat view displays messages with correct styling
- [ ] Streaming text appears in real-time via ViewCommands
- [ ] UserEvents reach EventBus and trigger presenter logic
- [ ] History view shows conversation list
- [ ] Settings view has transparency slider
- [ ] Keyboard shortcuts work
- [ ] No blocking of either GPUI or tokio runtime
- [ ] Old AppKit UI code remains intact (not deleted)

---

## 13. Performance Requirements

- **Popup open time:** <100ms from click to visible
- **ViewCommand latency:** <50ms from presenter emit to UI update
- **Channel capacity:** UserEvent=256, ViewCommand=1024
- **Memory:** <50MB additional for GPUI layer
- **Binary size:** <20MB increase

---

## 14. References

- `research/exactobar/` - Reference implementation
- `dev-docs/requirements/ui/chat.md` - Chat view requirements
- `dev-docs/requirements/ui/history.md` - History view requirements
- `dev-docs/requirements/ui/settings.md` - Settings view requirements
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Architecture patterns
- `src/presentation/view_command.rs` - All ViewCommand variants
- `src/events/types.rs` - All UserEvent variants
- `src/presentation/*.rs` - Existing presenter implementations
