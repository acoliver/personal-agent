# Presentation Layer Requirements

The Presentation Layer contains **Presenters** that coordinate between Views and Services via the EventBus. Presenters subscribe to events, call services, and update views.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                          Views (UI)                              │
│  • Pure rendering, no business logic                            │
│  • Emit UserEvents on user actions                              │
│  • Receive state updates from Presenters                        │
└──────────────────────────────┬──────────────────────────────────┘
                               │ UserEvent::*
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         EventBus                                 │
│  • tokio::sync::broadcast<AppEvent>                             │
│  • Multi-consumer event delivery                                │
└──────────────────────────────┬──────────────────────────────────┘
                               │ AppEvent::*
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Presenters                                │
│  • Subscribe to relevant events                                 │
│  • Call services to perform operations                          │
│  • Update view state                                            │
│  • Emit result events                                           │
└──────────────────────────────┬──────────────────────────────────┘
                               │ method calls
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Services                                 │
│  • Pure business logic                                          │
│  • Emit domain events (ChatEvent, McpEvent, etc.)               │
└─────────────────────────────────────────────────────────────────┘
```

---

## Presenter Responsibilities

| Responsibility | Description |
|----------------|-------------|
| Event Subscription | Subscribe to UserEvents and domain events relevant to managed views |
| Service Coordination | Call services in response to events |
| View Updates | Update view state/data via view interface |
| Error Handling | Convert service errors to user-friendly messages |
| State Management | Track UI state (loading, selected items, etc.) |
| Navigation | Emit NavigationEvents for view transitions |

---

## Presenter Registry

Each major view has a dedicated presenter:

| Presenter | Managed Views | Event Subscriptions |
|-----------|---------------|---------------------|
| `ChatPresenter` | ChatView | `UserEvent::SendMessage`, `UserEvent::StopStreaming`, `UserEvent::NewConversation`, `ChatEvent::*` |
| `HistoryPresenter` | HistoryView | `UserEvent::SelectConversation`, `UserEvent::DeleteConversation`, `ConversationEvent::*` |
| `SettingsPresenter` | SettingsView | `UserEvent::SelectProfile`, `UserEvent::ToggleMcp`, `ProfileEvent::*`, `McpEvent::*` |
| `ProfileEditorPresenter` | ProfileEditorView | `UserEvent::SaveProfile`, `UserEvent::TestProfileConnection`, `ProfileEvent::TestCompleted` |
| `McpAddPresenter` | McpAddView | `UserEvent::SearchMcpRegistry`, `UserEvent::SelectMcpFromRegistry` |
| `McpConfigurePresenter` | McpConfigureView | `UserEvent::SaveMcpConfig`, `UserEvent::StartMcpOAuth`, `McpEvent::ConfigSaved` |
| `ModelSelectorPresenter` | ModelSelectorView | `UserEvent::SearchModels`, `UserEvent::FilterModelsByProvider`, `UserEvent::SelectModel` |

---

## Presenter Interface

```rust
/// Base trait for all presenters
pub trait Presenter: Send + Sync {
    /// Start the presenter (subscribe to events, initialize state)
    fn start(&self);
    
    /// Stop the presenter (unsubscribe from events)
    fn stop(&self);
}

/// Presenter with associated view type
pub trait ViewPresenter<V>: Presenter {
    /// Get reference to the managed view
    fn view(&self) -> &V;
    
    /// Update view with new state (called on main thread)
    fn update_view(&self, state: impl Into<ViewState>);
}
```

---

## Event Handling Pattern

### Event Loop Structure

```rust
impl ChatPresenter {
    pub fn start(&self) {
        let event_bus = self.event_bus.clone();
        let mut receiver = event_bus.subscribe();
        
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => self.handle_event(event).await,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("ChatPresenter lagged {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
    
    async fn handle_event(&self, event: AppEvent) {
        match event {
            AppEvent::User(UserEvent::SendMessage { content }) => {
                self.handle_send_message(content).await;
            }
            AppEvent::User(UserEvent::StopStreaming) => {
                self.handle_stop_streaming().await;
            }
            AppEvent::Chat(ChatEvent::TextDelta { text }) => {
                self.view.append_to_assistant(&text);
            }
            AppEvent::Chat(ChatEvent::StreamCompleted { .. }) => {
                self.view.set_streaming(false);
                self.view.finalize_assistant_message();
            }
            // ... other events
            _ => {} // Ignore unrelated events
        }
    }
}
```

### Main Thread Updates

Views must be updated on the main thread (NSView requirement). Use a dispatch mechanism:

```rust
impl ChatPresenter {
    fn update_view_on_main<F>(&self, f: F)
    where
        F: FnOnce(&ChatView) + Send + 'static,
    {
        let view = self.view.clone();
        dispatch_async_main(move || {
            f(&view);
        });
    }
    
    async fn handle_text_delta(&self, text: String) {
        self.update_view_on_main(move |view| {
            view.append_to_assistant(&text);
        });
    }
}
```

---

## ChatPresenter

### Responsibilities

- Handle message sending workflow
- Manage streaming state
- Handle cancellation
- Update conversation list on changes

### Event Handlers

```rust
impl ChatPresenter {
    async fn handle_send_message(&self, content: String) {
        // 1. Validate input
        if content.trim().is_empty() {
            return;
        }
        
        // 2. Get current conversation (or create new)
        let conversation_id = self.ensure_conversation().await?;
        
        // 3. Update UI immediately
        self.update_view_on_main(|view| {
            view.add_user_message(&content);
            view.clear_input();
            view.set_streaming(true);
        });
        
        // 4. Start streaming via service
        let handle = self.chat_service.send_message(
            conversation_id,
            &content,
        )?;
        
        self.stream_handle = Some(handle);
        
        // Note: ChatService emits ChatEvent::* which this presenter
        // handles in the event loop to update the view
    }
    
    async fn handle_stop_streaming(&self) {
        if let Some(handle) = &self.stream_handle {
            self.chat_service.cancel(handle)?;
            // ChatEvent::StreamCancelled will be emitted
        }
    }
    
    async fn handle_new_conversation(&self) {
        let conversation = self.conversation_service.create()?;
        self.app_settings_service.set_current_conversation_id(conversation.id)?;
        
        self.update_view_on_main(|view| {
            view.clear_messages();
            view.set_conversation_title(&conversation.metadata.title.unwrap_or_default());
        });
    }
}
```

---

## SettingsPresenter

### Responsibilities

- Manage profile list and selection
- Coordinate MCP enable/disable
- Handle profile deletion with confirmation

### Event Handlers

```rust
impl SettingsPresenter {
    async fn handle_select_profile(&self, profile_id: Uuid) {
        // Update app settings
        self.app_settings_service.set_default_profile_id(profile_id)?;
        
        // ProfileEvent::DefaultChanged will be emitted
        // View updates happen in that handler
    }
    
    async fn handle_toggle_mcp(&self, mcp_id: Uuid, enabled: bool) {
        if enabled {
            self.mcp_service.start(mcp_id).await?;
        } else {
            self.mcp_service.stop(mcp_id).await?;
        }
        // McpEvent::Started or McpEvent::Stopped will be emitted
    }
    
    fn handle_profile_default_changed(&self, profile_id: Option<Uuid>) {
        self.update_view_on_main(move |view| {
            view.set_selected_profile(profile_id);
        });
    }
    
    fn handle_mcp_started(&self, mcp_id: Uuid, name: String, tool_count: usize) {
        self.update_view_on_main(move |view| {
            view.set_mcp_status(mcp_id, McpUiState::Running { tool_count });
        });
    }
}
```

---

## ProfileEditorPresenter

### Responsibilities

- Manage profile creation/editing form
- Handle connection testing
- Coordinate with secrets service for API keys

### Event Handlers

```rust
impl ProfileEditorPresenter {
    async fn handle_save_profile(&self, profile_data: ProfileFormData) {
        self.update_view_on_main(|view| view.set_saving(true));
        
        let result = if let Some(id) = self.editing_profile_id {
            self.profile_service.update(id, &profile_data.into()).await
        } else {
            self.profile_service.create(&profile_data.into()).await
        };
        
        match result {
            Ok(profile) => {
                // ProfileEvent::Created or Updated will be emitted
                self.event_bus.emit(NavigationEvent::Back.into());
            }
            Err(e) => {
                self.update_view_on_main(move |view| {
                    view.set_saving(false);
                    view.show_error(&e.to_string());
                });
            }
        }
    }
    
    async fn handle_test_connection(&self, profile_id: Uuid) {
        self.update_view_on_main(|view| view.set_testing(true));
        
        // ProfileService emits ProfileEvent::TestStarted and TestCompleted
        self.profile_service.test_connection(profile_id).await?;
    }
    
    fn handle_test_completed(&self, success: bool, response_time_ms: Option<u64>, error: Option<String>) {
        self.update_view_on_main(move |view| {
            view.set_testing(false);
            if success {
                view.show_test_success(response_time_ms.unwrap_or(0));
            } else {
                view.show_test_failure(&error.unwrap_or_default());
            }
        });
    }
}
```

---

## Navigation Pattern

Presenters emit `NavigationEvent` to trigger view transitions:

```rust
pub enum NavigationEvent {
    /// Navigate to a specific view
    NavigateTo { view: ViewId, context: Option<NavigationContext> },
    
    /// Go back to previous view
    Back,
    
    /// Show modal dialog
    ShowModal { modal: ModalId, context: Option<ModalContext> },
    
    /// Dismiss modal
    DismissModal,
}

pub enum ViewId {
    Chat,
    History,
    Settings,
    ProfileEditor,
    McpAdd,
    McpConfigure,
    ModelSelector,
}

pub struct NavigationContext {
    /// Data passed to target view
    pub data: serde_json::Value,
}
```

### Navigation Handler

The app-level navigation handler subscribes to `NavigationEvent`:

```rust
impl NavigationHandler {
    fn handle_navigation_event(&self, event: NavigationEvent) {
        match event {
            NavigationEvent::NavigateTo { view, context } => {
                self.push_view(view, context);
            }
            NavigationEvent::Back => {
                self.pop_view();
            }
            NavigationEvent::ShowModal { modal, context } => {
                self.present_modal(modal, context);
            }
            NavigationEvent::DismissModal => {
                self.dismiss_modal();
            }
        }
    }
}
```

---

## State Management

### Presenter State

Each presenter manages its own state:

```rust
pub struct ChatPresenterState {
    /// Current conversation ID
    current_conversation_id: Option<Uuid>,
    
    /// Whether currently streaming
    is_streaming: bool,
    
    /// Active stream handle
    stream_handle: Option<StreamHandle>,
    
    /// Thinking visibility toggle
    show_thinking: bool,
}

impl ChatPresenter {
    fn state(&self) -> RwLockReadGuard<ChatPresenterState> {
        self.state.read().unwrap()
    }
    
    fn state_mut(&self) -> RwLockWriteGuard<ChatPresenterState> {
        self.state.write().unwrap()
    }
}
```

### View State

Views receive state updates, not raw events:

```rust
pub struct ChatViewState {
    pub messages: Vec<MessageViewModel>,
    pub is_streaming: bool,
    pub show_thinking: bool,
    pub conversation_title: String,
    pub can_send: bool,
}

pub struct MessageViewModel {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub model_id: Option<String>,
    pub is_cancelled: bool,
    pub tool_calls: Vec<ToolCallViewModel>,
}
```

---

## Error Handling

### Service Error to User Message

```rust
fn user_friendly_error(error: &ServiceError) -> String {
    match error {
        ServiceError::NetworkError(_) => "Network connection failed. Please check your internet connection.".to_string(),
        ServiceError::AuthenticationError(_) => "Authentication failed. Please check your API key.".to_string(),
        ServiceError::RateLimitError(_) => "Rate limit exceeded. Please wait a moment and try again.".to_string(),
        ServiceError::ModelError(msg) => format!("Model error: {}", msg),
        ServiceError::NotFound(_) => "The requested item was not found.".to_string(),
        _ => "An unexpected error occurred.".to_string(),
    }
}
```

### Error Display Pattern

```rust
impl ChatPresenter {
    async fn handle_stream_error(&self, error: String, recoverable: bool) {
        self.update_view_on_main(move |view| {
            view.set_streaming(false);
            view.show_error(&error, recoverable);
        });
        
        if !recoverable {
            self.event_bus.emit(SystemEvent::Error {
                message: error,
                recoverable: false,
            }.into());
        }
    }
}
```

---

## Testing Presenters

### Mock Dependencies

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_presenter() -> ChatPresenter {
        let event_bus = Arc::new(EventBus::new());
        let chat_service = Arc::new(MockChatService::new());
        let conversation_service = Arc::new(MockConversationService::new());
        let app_settings_service = Arc::new(MockAppSettingsService::new());
        let view = Arc::new(MockChatView::new());
        
        ChatPresenter::new(
            event_bus,
            chat_service,
            conversation_service,
            app_settings_service,
            view,
        )
    }
}
```

### Event-Driven Tests

```rust
#[tokio::test]
async fn test_send_message_updates_view() {
    let presenter = create_test_presenter();
    let view = presenter.view.as_mock();
    
    // Emit event
    presenter.event_bus.emit(UserEvent::SendMessage {
        content: "Hello".to_string(),
    }.into());
    
    // Wait for async processing
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Verify view was updated
    assert!(view.user_message_added());
    assert!(view.input_cleared());
    assert!(view.is_streaming());
}
```

---

## Test Requirements

| ID | Test |
|----|------|
| PR-T1 | Presenter subscribes to events on start() |
| PR-T2 | Presenter unsubscribes on stop() |
| PR-T3 | UserEvent triggers service call |
| PR-T4 | Service domain event updates view |
| PR-T5 | Error events show user-friendly message |
| PR-T6 | NavigationEvent triggers view transition |
| PR-T7 | View updates happen on main thread |
| PR-T8 | Presenter state is thread-safe |
| PR-T9 | Lagged events are logged and recovered |
| PR-T10 | Presenter handles closed channel gracefully |
