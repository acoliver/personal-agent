# Phase 04: E2E Test - Chat Stream Events

**Phase ID**: P04
**Type**: Implementation
**Status**: Pending
**Prerequisites**: P03a completion marker exists with PASS verdict

## Objective

Create end-to-end integration tests that verify ChatPresenter correctly responds to chat stream events from the event bus. Per dev-docs/COORDINATING.md, this is an IMPLEMENTATION phase - test code must be complete, not stubbed.

## Test Scenarios

### Test 1: Message Send and Stream Completion

**Events to emit:**
1. `AppEvent::User(UserEvent::SendMessage { text: "Hello, world!" })`
2. `AppEvent::Chat(ChatEvent::StreamStarted { conversation_id, message_id, model_id: "synthetic" })`
3. `AppEvent::Chat(ChatEvent::TextDelta { text: "Hi" })`
4. `AppEvent::Chat(ChatEvent::TextDelta { text: " there" })`
5. `AppEvent::Chat(ChatEvent::TextDelta { text: "!" })`
6. `AppEvent::Chat(ChatEvent::StreamCompleted { conversation_id, message_id, total_tokens: Some(10) })`

**Expected ViewCommands:**
- `ViewCommand::ShowLoading { conversation_id, message_id }`
- `ViewCommand::AppendMessageContent { text: "Hi" }`
- `ViewCommand::AppendMessageContent { text: " there" }`
- `ViewCommand::AppendMessageContent { text: "!" }`
- `ViewCommand::HideLoading { conversation_id, message_id }`
- `ViewCommand::SaveMessage { conversation_id, message_id, total_tokens: Some(10) }`

### Test 2: Tool Call During Stream

**Events to emit:**
1. `AppEvent::User(UserEvent::SendMessage { text: "Search for info" })`
2. `AppEvent::Chat(ChatEvent::StreamStarted { ... })`
3. `AppEvent::Chat(ChatEvent::TextDelta { text: "Searching" })`
4. `AppEvent::Chat(ChatEvent::ToolCallStarted { tool_call_id: "tc1", tool_name: "exa.search" })`
5. `AppEvent::Chat(ChatEvent::ToolCallCompleted { tool_call_id: "tc1", tool_name: "exa.search", success: true, result: "Found 5 results", duration_ms: 150 })`
6. `AppEvent::Chat(ChatEvent::TextDelta { text: " done" })`
7. `AppEvent::Chat(ChatEvent::StreamCompleted { ... })`

**Expected ViewCommands:**
- `ViewCommand::ShowLoading { ... }`
- `ViewCommand::AppendMessageContent { text: "Searching" }`
- `ViewCommand::ShowToolCallStarted { tool_call_id: "tc1", tool_name: "exa.search" }`
- `ViewCommand::ShowToolCallCompleted { tool_call_id: "tc1", success: true, result: "Found 5 results" }`
- `ViewCommand::AppendMessageContent { text: " done" }`
- `ViewCommand::HideLoading { ... }`

### Test 3: Thinking Display

**Events to emit:**
1. `AppEvent::User(UserEvent::SendMessage { text: "Explain this" })`
2. `AppEvent::Chat(ChatEvent::StreamStarted { ... })`
3. `AppEvent::Chat(ChatEvent::ThinkingDelta { text: "Let me think" })`
4. `AppEvent::Chat(ChatEvent::ThinkingDelta { text: " about this" })`
5. `AppEvent::Chat(ChatEvent::TextDelta { text: "Here's the explanation" })`
6. `AppEvent::Chat(ChatEvent::StreamCompleted { ... })`

**Expected ViewCommands:**
- `ViewCommand::ShowLoading { ... }`
- `ViewCommand::AppendThinkingContent { text: "Let me think" }`
- `ViewCommand::AppendThinkingContent { text: " about this" }`
- `ViewCommand::AppendMessageContent { text: "Here's the explanation" }`
- `ViewCommand::HideLoading { ... }`

### Test 4: Stream Error and Recovery

**Events to emit:**
1. `AppEvent::User(UserEvent::SendMessage { text: "Test error" })`
2. `AppEvent::Chat(ChatEvent::StreamStarted { ... })`
3. `AppEvent::Chat(ChatEvent::StreamError { conversation_id, error: "Network timeout", recoverable: true })`

**Expected ViewCommands:**
- `ViewCommand::ShowLoading { ... }`
- `ViewCommand::ShowError { error: "Network timeout", recoverable: true }`

### Test 5: User Cancels Stream

**Events to emit:**
1. `AppEvent::User(UserEvent::SendMessage { text: "Long response" })`
2. `AppEvent::Chat(ChatEvent::StreamStarted { ... })`
3. `AppEvent::Chat(ChatEvent::TextDelta { text: "Partial" })`
4. `AppEvent::User(UserEvent::StopStreaming)`
5. `AppEvent::Chat(ChatEvent::StreamCancelled { conversation_id, message_id, partial_content: "Partial" })`

**Expected ViewCommands:**
- `ViewCommand::ShowLoading { ... }`
- `ViewCommand::AppendMessageContent { text: "Partial" }`
- `ViewCommand::ShowStreamCancelled { conversation_id, message_id }`

## Implementation Requirements

### Test File Structure

**File**: `tests/e2e_presenter_tests.rs` (or similar)

```rust
//! End-to-end integration tests for presenter event wiring
//!
//! @plan PLAN-20250128-PRESENTERS.P04
//! @requirement REQ-019.2

use personal_agent::events::{AppEvent, UserEvent, ChatEvent};
use personal_agent::presentation::chat_presenter::ChatPresenter;
use personal_agent::events::bus::EventBus;
use personal_agent::presentation::view_command::ViewCommand;
use uuid::Uuid;

#[cfg(test)]
mod e2e_chat_tests {
    use super::*;

    #[tokio::test]
    async fn test_chat_send_and_stream_completion() {
        // Setup
        let event_bus = Arc::new(EventBus::new());
        let mut presenter = ChatPresenter::new(event_bus.clone());
        let mut receiver = presenter.subscribe_view_commands();

        let conversation_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();

        // Emit events
        event_bus.emit(AppEvent::User(UserEvent::SendMessage {
            text: "Hello, world!".to_string(),
        }));

        event_bus.emit(AppEvent::Chat(ChatEvent::StreamStarted {
            conversation_id,
            message_id,
            model_id: "synthetic".to_string(),
        }));

        event_bus.emit(AppEvent::Chat(ChatEvent::TextDelta {
            text: "Hi".to_string(),
        }));

        event_bus.emit(AppEvent::Chat(ChatEvent::TextDelta {
            text: " there".to_string(),
        }));

        event_bus.emit(AppEvent::Chat(ChatEvent::TextDelta {
            text: "!".to_string(),
        }));

        event_bus.emit(AppEvent::Chat(ChatEvent::StreamCompleted {
            conversation_id,
            message_id,
            total_tokens: Some(10),
        }));

        // Verify ViewCommands
        let commands = receiver.collect_view_commands(100).await;

        assert_eq!(commands[0], ViewCommand::ShowLoading {
            conversation_id,
            message_id,
        });

        assert_eq!(commands[1], ViewCommand::AppendMessageContent {
            text: "Hi".to_string(),
        });

        assert_eq!(commands[2], ViewCommand::AppendMessageContent {
            text: " there".to_string(),
        });

        assert_eq!(commands[3], ViewCommand::AppendMessageContent {
            text: "!".to_string(),
        });

        assert_eq!(commands[4], ViewCommand::HideLoading {
            conversation_id,
            message_id,
        });
    }

    #[tokio::test]
    async fn test_tool_call_during_stream() {
        // Similar structure for tool call scenario
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_thinking_display() {
        // Similar structure for thinking scenario
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_stream_error() {
        // Similar structure for error scenario
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_user_cancels_stream() {
        // Similar structure for cancellation scenario
        // NO unimplemented!() placeholders
    }
}
```

### Mock/Helper Requirements

```rust
/// Helper to collect ViewCommands from presenter
struct ViewCommandCollector {
    commands: Arc<Mutex<Vec<ViewCommand>>>,
}

impl ViewCommandCollector {
    fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn collect_view_commands(&self, expected_count: usize) -> Vec<ViewCommand> {
        // Wait for expected number of commands
        // Timeout after 5 seconds
        // Return collected commands
        // NO unimplemented!() placeholders
    }
}
```

## Inputs

### Files to Read
- `src/presentation/chat_presenter.rs` - ChatPresenter implementation (must be complete from P01)
- `src/events/types.rs` - Event enum definitions
- `src/events/bus.rs` - EventBus API (emit, subscribe methods)
- `src/presentation/view_command.rs` - ViewCommand enum definitions
- `dev-docs/COORDINATING.md` - Test requirements (no placeholders)

### State Required
- ChatPresenter is fully wired to EventBus (P01a passed)
- ViewCommand enum has all required variants
- EventBus supports test mode or can be instantiated in tests

## Outputs

### Files to Create
- `tests/e2e_presenter_tests.rs` - E2E test file with at least 5 test functions

### Evidence Files
- `project-plans/wire-presenters/plan/.completed/P04.md` - Phase completion evidence
- `evidence/PLAN-20250128-PRESENTERS/phase-04/test-output.txt` - Test run output (created in P04a)

## Verification Commands

```bash
# Build check
cargo build --all-targets

# Placeholder detection
grep -rn "unimplemented!\|todo!" tests/e2e_presenter_tests.rs
grep -rn "placeholder\|not yet implemented" tests/e2e_presenter_tests.rs

# Test compilation (don't run yet - that's P04a)
cargo test --test e2e_presenter_tests --no-run
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- Exit code 0 from `cargo test --test e2e_presenter_tests --no-run`
- `grep -rn "unimplemented!" tests/e2e_presenter_tests.rs` returns no matches
- `grep -rn "todo!" tests/e2e_presenter_tests.rs` returns no matches
- At least 5 test functions exist
- Each test has real assertions (not commented out)

### FAIL Conditions
- Build fails with compilation errors
- Any `unimplemented!()`, `todo!()`, or placeholder strings found
- Tests have placeholder assertions (e.g., `assert!(true) // TODO`)
- Missing test scenarios

## Related Requirements

- REQ-019.2: Event-driven architecture must be testable
- dev-docs/COORDINATING.md: ZERO TOLERANCE for placeholders in implementation phases
- dev-docs/goodtests.md: Tests must verify real behavior, not just compile
- ARCHITECTURE_IMPROVEMENTS.md: Presenters must be testable via EventBus
