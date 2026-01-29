# Phase 02: Wire EventBus to HistoryPresenter

**Phase ID**: P02
**Type**: Implementation
**Status**: Pending
**Prerequisites**: P01a completion marker exists with PASS verdict

## Objective

Connect `HistoryPresenter` to the event bus to receive and react to conversation lifecycle events. Per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation", the presenter must subscribe to `AppEvent` for conversation events and emit `ViewCommand` to refresh the history view.

## Event Mapping

### HistoryPresenter Subscriptions

Per `src/events/types.rs`, HistoryPresenter must handle:

| Event Type | Rust Enum | ViewCommand Response |
|------------|-----------|---------------------|
| User creates conversation | `AppEvent::User(UserEvent::NewConversation)` | Refresh conversation list |
| User selects conversation | `AppEvent::User(UserEvent::SelectConversation { id })` | Highlight selected item |
| User starts rename | `AppEvent::User(UserEvent::StartRenameConversation { id })` | Show rename UI |
| User confirms rename | `AppEvent::User(UserEvent::ConfirmRenameConversation { id, title })` | Update list item |
| User cancels rename | `AppEvent::User(UserEvent::CancelRenameConversation)` | Hide rename UI |
| Conversation created | `AppEvent::Conversation(ConversationEvent::Created { id, title })` | Append to list |
| Conversation loaded | `AppEvent::Conversation(ConversationEvent::Loaded { id })` | Highlight active |
| Conversation title updated | `AppEvent::Conversation(ConversationEvent::TitleUpdated { id, title })` | Update item |
| Conversation deleted | `AppEvent::Conversation(ConversationEvent::Deleted { id })` | Remove from list |
| Conversation activated | `AppEvent::Conversation(ConversationEvent::Activated { id })` | Highlight active |
| Conversation deactivated | `AppEvent::Conversation(ConversationEvent::Deactivated)` | Clear selection |
| List refreshed | `AppEvent::Conversation(ConversationEvent::ListRefreshed { count })` | Reload list |

## Implementation Requirements

### 1. Add EventBus Subscription

**File**: `src/presentation/history_presenter.rs`

```rust
use crate::events::{AppEvent, UserEvent, ConversationEvent};
use crate::events::bus::EventBus;

impl HistoryPresenter {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let presenter = Self {
            event_bus: event_bus.clone(),
            // ... existing fields
        };

        // Subscribe to relevant events
        presenter.subscribe_to_events();
        presenter
    }

    fn subscribe_to_events(&self) {
        // Subscribe to user conversation events
        self.event_bus.subscribe(
            |event| {
                matches!(event,
                    AppEvent::User(UserEvent::NewConversation) |
                    AppEvent::User(UserEvent::SelectConversation { .. }) |
                    AppEvent::User(UserEvent::StartRenameConversation { .. }) |
                    AppEvent::User(UserEvent::ConfirmRenameConversation { .. }) |
                    AppEvent::User(UserEvent::CancelRenameConversation)
                )
            },
            self.clone(),
        );

        // Subscribe to conversation lifecycle events
        self.event_bus.subscribe(
            |event| matches!(event, AppEvent::Conversation(_)),
            self.clone(),
        );
    }
}
```

### 2. Implement EventHandler Trait

```rust
impl EventHandler for HistoryPresenter {
    fn handle_event(&self, event: &AppEvent) {
        match event {
            AppEvent::User(UserEvent::NewConversation) => {
                self.on_new_conversation();
            }

            AppEvent::User(UserEvent::SelectConversation { id }) => {
                self.on_select_conversation(*id);
            }

            AppEvent::User(UserEvent::StartRenameConversation { id }) => {
                self.on_start_rename(*id);
            }

            AppEvent::User(UserEvent::ConfirmRenameConversation { id, title }) => {
                self.on_confirm_rename(*id, title);
            }

            AppEvent::User(UserEvent::CancelRenameConversation) => {
                self.on_cancel_rename();
            }

            AppEvent::Conversation(ConversationEvent::Created { id, title }) => {
                self.on_conversation_created(*id, title);
            }

            AppEvent::Conversation(ConversationEvent::Loaded { id }) => {
                self.on_conversation_loaded(*id);
            }

            AppEvent::Conversation(ConversationEvent::TitleUpdated { id, title }) => {
                self.on_title_updated(*id, title);
            }

            AppEvent::Conversation(ConversationEvent::Deleted { id }) => {
                self.on_conversation_deleted(*id);
            }

            AppEvent::Conversation(ConversationEvent::Activated { id }) => {
                self.on_conversation_activated(*id);
            }

            AppEvent::Conversation(ConversationEvent::Deactivated) => {
                self.on_conversation_deactivated();
            }

            AppEvent::Conversation(ConversationEvent::ListRefreshed { count }) => {
                self.on_list_refreshed(*count);
            }

            _ => {} // Ignore other events
        }
    }
}
```

### 3. Emit ViewCommands

Each event handler must emit appropriate `ViewCommand`:

```rust
impl HistoryPresenter {
    fn on_conversation_created(&self, id: Uuid, title: &str) {
        let cmd = ViewCommand::AppendConversationItem {
            id,
            title: title.to_string(),
        };
        self.emit_view_command(cmd);
    }

    fn on_list_refreshed(&self, count: usize) {
        let cmd = ViewCommand::ReloadConversationList {
            count,
        };
        self.emit_view_command(cmd);
    }

    fn on_conversation_deleted(&self, id: Uuid) {
        let cmd = ViewCommand::RemoveConversationItem { id };
        self.emit_view_command(cmd);
    }

    // ... other handlers
}
```

## Inputs

### Files to Read
- `src/presentation/history_presenter.rs` - Current HistoryPresenter implementation
- `src/events/types.rs` - Event enum definitions (ConversationEvent)
- `src/events/bus.rs` - EventBus API
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Architecture patterns
- `dev-docs/requirements/presentation.md` - HistoryPresenter requirements

### State Required
- EventBus is already implemented and running
- ViewCommand enum exists
- HistoryPresenter struct exists
- P01a passed (ChatPresenter is working reference)

## Outputs

### Files to Modify
- `src/presentation/history_presenter.rs` - Add event subscription and handlers

### Evidence Files
- `project-plans/wire-presenters/plan/.completed/P02.md` - Phase completion evidence

## Verification Commands

```bash
# Build check
cargo build --all-targets

# Placeholder detection
grep -rn "unimplemented!\|todo!" src/presentation/history_presenter.rs
grep -rn "placeholder\|not yet implemented" src/presentation/history_presenter.rs

# Verify event subscription
grep -c "subscribe_to_events\|handle_event" src/presentation/history_presenter.rs

# Verify ConversationEvent handlers
grep -c "AppEvent::Conversation" src/presentation/history_presenter.rs
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- `grep -rn "unimplemented!" src/presentation/history_presenter.rs` returns no matches
- `grep -rn "todo!" src/presentation/history_presenter.rs` returns no matches
- `grep -c "subscribe_to_events" src/presentation/history_presenter.rs` returns count >= 1
- `grep -c "AppEvent::Conversation" src/presentation/history_presenter.rs` returns count >= 5

### FAIL Conditions
- Build fails with compilation errors
- Any `unimplemented!()`, `todo!()`, or placeholder strings found
- Missing event subscription code
- Missing ConversationEvent handlers

## Related Requirements

- REQ-019.2: Event-driven architecture
- dev-docs/requirements/presentation.md: HistoryPresenter must react to ConversationEvent
- dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md: Presenters must not directly call services
