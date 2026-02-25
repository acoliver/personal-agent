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

---

## Existing Code That Will Use Changes

These files/functions will call into or depend on the remediated event pipeline without themselves being replaced:

| File | Function/Symbol | Why It Uses Changes |
|------|----------------|---------------------|
| `src/main_gpui.rs` | `spawn_user_event_forwarder` call at line 414 | Forwarder receives `UserEvent` from bridge; must route to global `EventBus` — this call site stays, routing target is corrected |
| `src/main_gpui.rs` | `ChatPresenter::new(…, view_tx.clone())` at line 480 | Already wired to global `EventBus`; unchanged but relies on corrected forwarder |
| `src/main_gpui.rs` | `HistoryPresenter::new(…, view_tx.clone())` at line 486 | Already wired to global `EventBus`; unchanged |
| `src/main_gpui.rs` | tokio spawn/mpsc-to-flume forwarder (lines 461–468) | Bridges `view_tx` mpsc to flume `view_cmd_tx`; stays as-is once all presenters are on the same `view_tx` |
| `src/ui_gpui/views/main_panel.rs` | `MainPanel::render()` — `bridge.drain_commands()` at line 235 | Polling loop that reads `ViewCommand`s from the bridge; stays but must dispatch all variants, not just navigation |
| `src/ui_gpui/views/main_panel.rs` | `MainPanel::handle_command()` at line 171 | Central dispatch; stays but must be expanded to forward non-navigation commands to child views |
| `src/ui_gpui/bridge/gpui_bridge.rs` | `GpuiBridge::emit()` | Called by every view's `fn emit()`; unchanged, correct path |
| `src/ui_gpui/bridge/gpui_bridge.rs` | `GpuiBridge::drain_commands()` | Called in MainPanel render; unchanged |
| `src/ui_gpui/bridge/user_event_forwarder.rs` | `spawn_user_event_forwarder()` | Correct implementation; stays |
| `src/events/bus.rs` | `EventBus::publish()`, `EventBus::subscribe()` | Global bus; unchanged |
| `src/events/types.rs` | `UserEvent`, `AppEvent`, `ViewId` enums | Consumed by presenters and views; unchanged |
| `src/ui_gpui/views/chat_view.rs` | `ChatView::emit()`, `handle_enter()`, `handle_send_click()`, `handle_stop_click()`, `handle_new_conversation_click()` | Already emit via bridge correctly; unchanged |
| `src/ui_gpui/views/history_view.rs` | `HistoryView::emit()`, conversation select/delete click handlers | Already emit via bridge; stay |
| `src/presentation/chat_presenter.rs` | `ChatPresenter::start()` event loop | Subscribes to global `EventBus`; correct path; stays |
| `src/presentation/history_presenter.rs` | `HistoryPresenter::start()` event loop | Subscribes to global `EventBus`; correct path; stays |

---

## Existing Code To Be Replaced

These files/functions implement the broken or redundant paths that the remediation will eliminate:

| File | Function/Symbol | What Is Wrong | Replacement |
|------|----------------|---------------|-------------|
| `src/main_gpui.rs` lines 474–479 | Local `app_event_tx` broadcast channel creation | Creates a throwaway channel (`let (app_event_tx, _) = broadcast::channel(100)`) whose receiver is immediately dropped; events emitted to it are silently discarded | Remove; pass `event_bus_for_tokio.sender()` directly to all presenters that currently take `&broadcast::Sender<AppEvent>` |
| `src/main_gpui.rs` lines 493–498, 517–531 | `SettingsPresenter::new`, `ProfileEditorPresenter::new`, `McpAddPresenter::new`, `McpConfigurePresenter::new` wired to `&app_event_tx` | These four presenters subscribe to the dead local bus and never receive real `UserEvent`s | Re-wire each to subscribe to `event_bus_for_tokio` (global bus) |
| `src/main_gpui.rs` lines 475–479, 500–514 | Per-presenter `broadcast::Sender<ViewCommand>` channels (`settings_view_tx`, `model_selector_view_tx`, etc.) | Separate per-presenter broadcast channels for `ViewCommand` that are never connected to the `MainPanel`'s flume `view_cmd_rx`; only `model_selector_view_tx` has a forwarding task, rest are orphaned | Remove; pass a single `view_tx: mpsc::Sender<ViewCommand>` (which is already connected through the mpsc→flume bridge) to all presenters |
| `src/presentation/settings_presenter.rs` | `SettingsPresenter` constructor signature taking `&broadcast::Sender<AppEvent>` | Accepting local bus sender instead of global EventBus arc | Change to `Arc<EventBus>` and subscribe via `event_bus.subscribe()` |
| `src/presentation/profile_editor_presenter.rs` | Constructor taking `&broadcast::Sender<AppEvent>` | Same dead-bus issue | Same fix |
| `src/presentation/mcp_add_presenter.rs` | Constructor taking `&broadcast::Sender<AppEvent>` | Same | Same fix |
| `src/presentation/mcp_configure_presenter.rs` | Constructor taking `&broadcast::Sender<AppEvent>` | Same | Same fix |
| `src/presentation/model_selector_presenter.rs` | Constructor taking `broadcast::Sender<ViewCommand>` with separate forwarding task | Indirect path through extra channel and separate tokio spawn | Change to `mpsc::Sender<ViewCommand>` matching `ChatPresenter`/`HistoryPresenter` |
| `src/ui_gpui/views/main_panel.rs` `handle_command()` lines 171–192 | `_ => { tracing::debug!("Unhandled command") }` wildcard | All non-navigation `ViewCommand`s silently dropped; views never receive chat, history, settings updates | Add explicit arms forwarding each `ViewCommand` variant to the appropriate child view |
| `src/ui_gpui/navigation_channel.rs` | Entire `NavigationChannel` struct and `navigation_channel()` global | Out-of-band static-mutex navigation bypass that skips EventBus and presenters entirely; root cause of dead controls — view navigates but presenter never informed | Remove; route all navigation through `UserEvent::Navigate { to }` via bridge→EventBus→presenter→`ViewCommand::NavigateTo` |
| `src/ui_gpui/views/chat_view.rs` lines 387–435, 769–777 | `navigation_channel().request_navigate(...)` calls for History/Settings buttons | Bypass path; must become `self.emit(UserEvent::Navigate { to: ViewId::History })` etc. | Replace with `bridge.emit(UserEvent::Navigate { … })` |
| `src/ui_gpui/views/settings_view.rs` | All `navigation_channel().request_navigate(...)` calls | Same bypass; ~14 call sites | Replace with `UserEvent::Navigate` emissions through bridge |
| `src/ui_gpui/views/history_view.rs` | All `navigation_channel().request_navigate(...)` calls | Same bypass; ~4 call sites | Replace |
| `src/ui_gpui/views/profile_editor_view.rs` | All `navigation_channel().request_navigate(...)` calls | Same bypass; ~5 call sites | Replace |
| `src/ui_gpui/views/mcp_add_view.rs` | All `navigation_channel().request_navigate(...)` calls | Same bypass; ~3 call sites | Replace |
| `src/ui_gpui/views/mcp_configure_view.rs` | All `navigation_channel().request_navigate(...)` calls | Same bypass; ~4 call sites | Replace |
| `src/ui_gpui/views/model_selector_view.rs` | All `navigation_channel().request_navigate(...)` calls | Same bypass; ~4 call sites | Replace |
| `src/ui_gpui/views/main_panel.rs` | Background thread polling `navigation_channel().has_pending()` (lines 139–145) and `render()` polling (lines 212–225) | Polling loop for the bypass channel; remove once channel is gone | Remove |

---

## User Access Points

All button clicks, keyboard shortcuts, and view actions that trigger events in the active GPUI runtime:

### ChatView (`src/ui_gpui/views/chat_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Send button click | `UserEvent::SendMessage { text }` | bridge→forwarder→EventBus [OK] |
| Enter key (in text input) | `UserEvent::SendMessage { text }` | bridge→forwarder→EventBus [OK] |
| Stop button click | `UserEvent::StopStreaming` | bridge→forwarder→EventBus [OK] |
| New Conversation button | `UserEvent::NewConversation` | bridge→forwarder→EventBus [OK] |
| Toggle Thinking button | `UserEvent::ToggleThinking` | bridge→forwarder→EventBus [OK] |
| History button | `navigation_channel().request_navigate(History)` | **static bypass — broken** |
| Settings button | `navigation_channel().request_navigate(Settings)` | **static bypass — broken** |

### SettingsView (`src/ui_gpui/views/settings_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Back button | `navigation_channel().request_navigate(Chat)` | **static bypass** |
| [+] Add Profile button | `navigation_channel().request_navigate(ModelSelector)` | **static bypass** |
| Select Profile click | `UserEvent::SelectProfile { id }` | bridge→forwarder→EventBus (but SettingsPresenter not subscribed to global bus) |
| Delete Profile click | `UserEvent::DeleteProfile { id }` | same broken path |
| Edit Profile click | `UserEvent::EditProfile { id }` + `navigation_channel(...)` | mixed: event on broken path, nav on bypass |
| Toggle MCP click | `UserEvent::ToggleMcp { id, enabled }` | bridge→forwarder→EventBus (SettingsPresenter not subscribed) |
| Delete MCP click | `UserEvent::DeleteMcp { id }` | same broken path |
| Configure MCP click | `UserEvent::ConfigureMcp { id }` + `navigation_channel(...)` | mixed |
| [+] Add MCP button | `navigation_channel().request_navigate(McpAdd)` | **static bypass** |
| Refresh Models button | `UserEvent::RefreshModelsRegistry` | bridge→EventBus (ModelSelectorPresenter not subscribed) |

### HistoryView (`src/ui_gpui/views/history_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Back button | `navigation_channel().request_navigate(Chat)` | **static bypass** |
| Conversation click | `UserEvent::SelectConversation { id }` + navigate | event correct; nav bypass |
| Delete conversation | `UserEvent::DeleteConversation { id }` | bridge→EventBus [OK] |
| Refresh | `UserEvent::RefreshHistory` | bridge→EventBus [OK] |

### ProfileEditorView (`src/ui_gpui/views/profile_editor_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Cancel button | `navigation_channel().request_navigate(Settings)` | **static bypass** |
| Save button | `UserEvent::SaveProfileEditor` | bridge→EventBus, but ProfileEditorPresenter on dead bus |
| Browse Keyfile button | `UserEvent::BrowseKeyfile` | same broken path |

### McpAddView (`src/ui_gpui/views/mcp_add_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Cancel button | `navigation_channel().request_navigate(Settings)` | **static bypass** |
| Next button | `UserEvent::McpAddNext` | bridge→EventBus, but McpAddPresenter on dead bus |

### McpConfigureView (`src/ui_gpui/views/mcp_configure_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Cancel button | `navigation_channel().request_navigate(Settings)` | **static bypass** |
| Save button | `UserEvent::SaveMcp` | bridge→EventBus, but McpConfigurePresenter on dead bus |

### ModelSelectorView (`src/ui_gpui/views/model_selector_view.rs`)

| UI Element | Event Emitted | Current Path |
|-----------|--------------|--------------|
| Select model click | `UserEvent::SelectModel { … }` | bridge→EventBus, but ModelSelectorPresenter on dead bus |
| Cancel/Back | `navigation_channel().request_navigate(Settings)` | **static bypass** |

### MainPanel keyboard shortcuts (`src/ui_gpui/views/main_panel.rs`)

| Shortcut | Action | Current Path |
|---------|--------|--------------|
| Ctrl+H | Navigate to History | `navigation_channel()` — bypass |
| Ctrl+S | Navigate to Settings | `navigation_channel()` — bypass |
| Ctrl+N | Navigate to Chat | `navigation_channel()` — bypass |
| Cmd+W | Navigate back | `navigation.navigate_back()` direct — bypass |
| Escape | Navigate back | `navigation.navigate_back()` direct — bypass |

---

## Out-of-Scope

The following are explicitly **not** addressed in this remediation:

1. **Backward-compatibility shims or migration system** — No adapter layers for old event formats; the remediation operates only on the active GPUI runtime code paths.

2. **Non-GPUI UI backends** — The `src/ui_tui/` module and any terminal or CLI rendering paths are unaffected and not modified.

3. **Service layer logic** — No changes to `src/services/` business logic implementations (`ChatServiceImpl`, `McpServiceImpl`, `ProfileServiceImpl`, etc.).

4. **EventBus capacity tuning** — The 256-event broadcast capacity is sufficient and not changed.

5. **New presenter features** — This remediation only fixes wiring; it does not add new presenter capabilities (e.g., search debouncing, pagination, OAuth flows).

6. **Hotkey reconfiguration** — `SystemEvent::HotkeyChanged` and hotkey settings UI are not part of this remediation.

7. **Tray/popup window management** — The `SystemTray::toggle_popup()` and `open_popup()`/`close_popup()` lifecycle in `src/main_gpui.rs` are not modified.

8. **Test infrastructure changes** — Existing tests in `tests/gpui_bridge_tests.rs`, `tests/gpui_chat_view_tests.rs`, etc. are not refactored; new tests added only for fixed paths.

9. **macOS-specific objc2 integration** — NSStatusBar, NSEvent polling, and tray icon rendering are out of scope.

10. **Migration of `src/events/types.rs` placeholder types** — `ModelProfile`, `McpConfig`, `McpRegistrySource`, `HotkeyConfig` remain as-is.
