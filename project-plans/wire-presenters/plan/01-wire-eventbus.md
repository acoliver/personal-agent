# Phase 01: Wire EventBus to ChatPresenter

**Phase ID**: P01
**Type**: Implementation
**Status**: Pending
**Prerequisites**: P00 (Preflight) must PASS

## Prerequisite Check

```bash
# Verify P00 preflight passed
ls project-plans/wire-presenters/plan/.completed/P00.md && grep "PASS" project-plans/wire-presenters/plan/.completed/P00.md
# Must show file exists AND contains "PASS"
```

If check fails: DO NOT PROCEED. Run P00 first.

## Objective

Connect `ChatPresenter` to the event bus to receive and react to chat-related events. Per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation", the presenter must subscribe to `AppEvent` and emit `ViewCommand` without calling services directly.

## Event Mapping

### ChatPresenter Subscriptions

Per `src/events/types.rs`, ChatPresenter must handle:

| Event Type | Rust Enum | ViewCommand Response |
|------------|-----------|---------------------|
| User sends message | `AppEvent::User(UserEvent::SendMessage { text })` | Show loading state |
| Stream starts | `AppEvent::Chat(ChatEvent::StreamStarted { ... })` | Update conversation title |
| Text delta | `AppEvent::Chat(ChatEvent::TextDelta { text })` | Append to message content |
| Thinking delta | `AppEvent::Chat(ChatEvent::ThinkingDelta { text })` | Append to thinking block |
| Tool call starts | `AppEvent::Chat(ChatEvent::ToolCallStarted { tool_call_id, tool_name })` | Show tool call UI |
| Tool call completes | `AppEvent::Chat(ChatEvent::ToolCallCompleted { ... })` | Update tool call status |
| Stream completes | `AppEvent::Chat(ChatEvent::StreamCompleted { ... })` | Hide loading, save message |
| Stream cancelled | `AppEvent::Chat(ChatEvent::StreamCancelled { ... })` | Show cancelled state |
| Stream error | `AppEvent::Chat(ChatEvent::StreamError { ... })` | Show error message |
| User stops streaming | `AppEvent::User(UserEvent::StopStreaming)` | Cancel current stream |
| User toggles thinking | `AppEvent::User(UserEvent::ToggleThinking)` | Toggle thinking visibility |
| New conversation | `AppEvent::User(UserEvent::NewConversation)` | Clear chat view |
| Rename conversation | `AppEvent::User(UserEvent::ConfirmRenameConversation { id, title })` | Update title in view |

## Implementation Requirements

### 1. Add EventBus Subscription

**File**: `src/presentation/chat_presenter.rs`

**IMPORTANT**: EventBus has NO filter-based subscription. Presenters subscribe to ALL events and filter in their async event loop.

```rust
use crate::events::{AppEvent, UserEvent, ChatEvent};
use crate::events::bus::EventBus;
use tokio::sync::broadcast;
use std::sync::Arc;

impl ChatPresenter {
    pub fn new(event_bus: Arc<EventBus>, /* other deps */) -> Self {
        let presenter = Self {
            // ... existing fields
        };

        // Get a receiver for all events
        let mut rx: broadcast::Receiver<AppEvent> = event_bus.subscribe();
        
        // Spawn event handling loop
        let presenter_clone = presenter.clone(); // If presenter is Clone
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => presenter_clone.handle_event(event),
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        // Log skipped events, continue
                        tracing::warn!("ChatPresenter lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Bus closed, exit loop
                        break;
                    }
                }
            }
        });
        
        presenter
    }

    fn handle_event(&self, event: AppEvent) {
        // Filter for events this presenter cares about
        match event {
            AppEvent::User(user_event) => self.handle_user_event(user_event),
            AppEvent::Chat(chat_event) => self.handle_chat_event(chat_event),
            AppEvent::Conversation(conv_event) => self.handle_conversation_event(conv_event),
            _ => {} // Ignore other event types
        }
    }
}
```

### 2. Implement EventHandler Trait

```rust
impl EventHandler for ChatPresenter {
    fn handle_event(&self, event: &AppEvent) {
        match event {
            AppEvent::User(UserEvent::SendMessage { text }) => {
                self.on_send_message(text);
            }

            AppEvent::Chat(ChatEvent::StreamStarted { conversation_id, message_id, model_id }) => {
                self.on_stream_started(*conversation_id, *message_id, model_id);
            }

            AppEvent::Chat(ChatEvent::TextDelta { text }) => {
                self.on_text_delta(text);
            }

            AppEvent::Chat(ChatEvent::ThinkingDelta { text }) => {
                self.on_thinking_delta(text);
            }

            AppEvent::Chat(ChatEvent::ToolCallStarted { tool_call_id, tool_name }) => {
                self.on_tool_call_started(tool_call_id, tool_name);
            }

            AppEvent::Chat(ChatEvent::ToolCallCompleted { tool_call_id, tool_name, success, result, duration_ms }) => {
                self.on_tool_call_completed(tool_call_id, tool_name, *success, result, *duration_ms);
            }

            AppEvent::Chat(ChatEvent::StreamCompleted { conversation_id, message_id, total_tokens }) => {
                self.on_stream_completed(*conversation_id, *message_id, *total_tokens);
            }

            AppEvent::Chat(ChatEvent::StreamCancelled { conversation_id, message_id, partial_content }) => {
                self.on_stream_cancelled(*conversation_id, *message_id, partial_content);
            }

            AppEvent::Chat(ChatEvent::StreamError { conversation_id, error, recoverable }) => {
                self.on_stream_error(*conversation_id, error, *recoverable);
            }

            _ => {} // Ignore other events
        }
    }
}
```

### 3. Emit ViewCommands

Each event handler must emit appropriate `ViewCommand`:

```rust
impl ChatPresenter {
    fn on_stream_started(&self, conversation_id: Uuid, message_id: Uuid, model_id: &str) {
        let cmd = ViewCommand::ShowLoading {
            conversation_id,
            message_id,
        };
        self.emit_view_command(cmd);
    }

    fn on_text_delta(&self, text: &str) {
        let cmd = ViewCommand::AppendMessageContent {
            text: text.to_string(),
        };
        self.emit_view_command(cmd);
    }

    // ... other handlers
}
```

## Inputs

### Files to Read
- `src/presentation/chat_presenter.rs` - Current ChatPresenter implementation
- `src/events/types.rs` - Event enum definitions
- `src/events/bus.rs` - EventBus API
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Architecture patterns
- `dev-docs/requirements/presentation.md` - ChatPresenter requirements

### State Required
- EventBus is already implemented and running
- ViewCommand enum exists in `src/presentation/view_command.rs`
- ChatPresenter struct exists (may need event_bus field)

## Outputs

### Files to Modify
- `src/presentation/chat_presenter.rs` - Add event subscription and handlers

### Evidence Files
- `project-plans/wire-presenters/plan/.completed/P01.md` - Phase completion evidence

## Verification Commands

```bash
# Build check
cargo build --all-targets

# Placeholder detection
grep -rn "unimplemented!\|todo!" src/presentation/chat_presenter.rs
grep -rn "placeholder\|not yet implemented" src/presentation/chat_presenter.rs

# Verify event subscription
grep -c "subscribe_to_events\|handle_event" src/presentation/chat_presenter.rs
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- `grep -rn "unimplemented!" src/presentation/chat_presenter.rs` returns no matches
- `grep -rn "todo!" src/presentation/chat_presenter.rs` returns no matches
- `grep -c "subscribe_to_events" src/presentation/chat_presenter.rs` returns count >= 1
- `grep -c "AppEvent::Chat" src/presentation/chat_presenter.rs` returns count >= 5

### FAIL Conditions
- Build fails with compilation errors
- Any `unimplemented!()`, `todo!()`, or placeholder strings found
- Missing event subscription code
- Event handlers don't emit ViewCommands

## Related Requirements

- REQ-019.2: Event-driven architecture
- dev-docs/requirements/presentation.md: ChatPresenter must react to UserEvent and ChatEvent
- dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md: Presenters must not directly call services
