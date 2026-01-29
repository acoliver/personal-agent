# Phase 11: Presenter Layer TDD Phase

## Phase ID

`PLAN-20250125-REFACTOR.P11`

## Prerequisites

- Required: Phase 10a (Presenter Stub Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P10A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P10A.md`
  - All presenter stubs implemented and compiling
  - All 4 presenters structurally defined
  - ViewCommand enum fully defined
- Preflight verification: Phases 01-10a completed

## Purpose

Write comprehensive tests for all presenter layer components BEFORE implementing real functionality. This phase:

1. Creates unit tests for all 4 presenter modules
2. Tests event routing and handling
3. Tests ViewCommand emission patterns
4. Tests error handling paths
5. Tests presenter lifecycle (start/stop)

**Note:** This is a TDD phase. Tests will FAIL against stub implementations. Phase 12 will implement real functionality to make tests pass.

## Requirements Implemented (Expanded)

### REQ-026.1: Presenter Unit Test Coverage

**Full Text**: All presenter modules MUST have comprehensive unit tests.

**Behavior**:
- GIVEN: Presenter is being tested
- WHEN: Unit tests run
- THEN: All public methods tested
- AND: All event handlers tested
- AND: ViewCommand emission verified
- AND: Coverage >= 80%

**Why This Matters**: TDD ensures presenters coordinate events correctly.

### REQ-026.2: ChatPresenter Tests

**Full Text**: ChatPresenter tests MUST cover chat event flow.

**Behavior**:
- GIVEN: ChatPresenter instance
- WHEN: Tests run
- THEN: UserEvent routing tested
- AND: ChatEvent routing tested
- AND: ViewCommands verified
- AND: Event loop tested

**Why This Matters**: ChatPresenter is primary UI coordinator.

### REQ-026.3: McpPresenter Tests

**Full Text**: McpPresenter tests MUST cover MCP management flow.

**Behavior**:
- GIVEN: McpPresenter instance
- WHEN: Tests run
- THEN: UserEvent routing tested
- AND: McpEvent routing tested
- AND: ViewCommands verified
- AND: Server management tested

**Why This Matters**: McpPresenter enables tool management.

### REQ-026.4: SettingsPresenter Tests

**Full Text**: SettingsPresenter tests MUST cover settings flow.

**Behavior**:
- GIVEN: SettingsPresenter instance
- WHEN: Tests run
- THEN: UserEvent routing tested
- AND: Profile management tested
- AND: ViewCommands verified

**Why This Matters**: SettingsPresenter enables configuration.

### REQ-026.5: ErrorPresenter Tests

**Full Text**: ErrorPresenter tests MUST cover error handling flow.

**Behavior**:
- GIVEN: ErrorPresenter instance
- WHEN: Tests run
- THEN: Error routing tested
- AND: ViewCommands verified
- AND: All error types tested

**Why This Matters**: ErrorPresenter ensures consistent error UX.

## Implementation Tasks

### Files to Create

- `src/presentation/chat_test.rs`
  - ChatPresenter unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P11`
  - Implements: `@requirement:REQ-026.2`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 20-240
  - Test modules:
    - test_new()
    - test_start_stop_lifecycle()
    - test_handle_user_event_send_message()
    - test_handle_chat_event_conversation_started()
    - test_handle_chat_event_message_received()
    - test_handle_chat_event_thinking_started()
    - test_handle_chat_event_stream_chunk()
    - test_handle_chat_event_error()
    - test_view_command_emission()

- `src/presentation/mcp_test.rs`
  - McpPresenter unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P11`
  - Implements: `@requirement:REQ-026.3`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 260-371
  - Test modules:
    - test_new()
    - test_start_stop_lifecycle()
    - test_handle_user_event_start_server()
    - test_handle_user_event_stop_server()
    - test_handle_mcp_event_server_started()
    - test_handle_mcp_event_tools_updated()
    - test_view_command_emission()

- `src/presentation/settings_test.rs`
  - SettingsPresenter unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P11`
  - Implements: `@requirement:REQ-026.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 380-444
  - Test modules:
    - test_new()
    - test_start_stop_lifecycle()
    - test_handle_user_event_open_settings()
    - test_handle_user_event_update_profile()
    - test_view_command_emission()

- `src/presentation/error_test.rs`
  - ErrorPresenter unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P11`
  - Implements: `@requirement:REQ-026.5`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 450-505
  - Test modules:
    - test_new()
    - test_start_stop_lifecycle()
    - test_handle_system_error()
    - test_handle_chat_error()
    - test_handle_mcp_error()
    - test_view_command_emission()

### Files to Modify

- `src/presentation/mod.rs`
  - ADD test module declarations:
    ```rust
    #[cfg(test)]
    mod chat_test;
    #[cfg(test)]
    mod mcp_test;
    #[cfg(test)]
    mod settings_test;
    #[cfg(test)]
    mod error_test;
    ```
  - ADD comment: `@plan:PLAN-20250125-REFACTOR.P11`

### Test Structure Pattern

All test files follow this structure:

```rust
/// @plan PLAN-20250125-REFACTOR.P11
/// @requirement REQ-026.X
use super::*;
use tokio::test;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    /// Test presenter creation
    #[tokio::test]
    async fn test_new() {
        // Arrange
        let (view_tx, mut view_rx) = broadcast::channel(100);
        let services = create_mock_services();
        let rx = create_mock_event_receiver();

        // Act
        let presenter = PresenterUnderTest::new(services, view_tx, rx);

        // Assert
        assert!(!presenter.is_running());
    }

    /// Test presenter lifecycle
    #[tokio::test]
    async fn test_start_stop_lifecycle() {
        // Arrange
        let presenter = create_presenter();

        // Act
        presenter.start().await.unwrap();
        assert!(presenter.is_running());

        presenter.stop().await.unwrap();
        assert!(!presenter.is_running());
    }

    /// Test event routing
    #[tokio::test]
    async fn test_handle_user_event() {
        // Arrange
        let (view_tx, mut view_rx) = broadcast::channel(100);
        let presenter = create_presenter();
        presenter.start().await.unwrap();

        // Act
        let event = AppEvent::User(UserEvent::SendMessage {
            conversation_id: Uuid::new_v4(),
            content: "test".to_string(),
        });
        presenter.handle_event(event).await;

        // Assert
        let view_cmd = view_rx.recv().await.unwrap();
        assert!(matches!(view_cmd, ViewCommand::...));
    }

    /// Test view command emission
    #[tokio::test]
    async fn test_view_command_emission() {
        // Arrange
        let (view_tx, mut view_rx) = broadcast::channel(100);
        let presenter = create_presenter();

        // Act
        trigger_event(&presenter).await;

        // Assert
        let view_cmd = view_rx.recv().await.unwrap();
        assert!(matches!(view_cmd, ViewCommand::MessageAppended { ... }));
    }

    /// Test error handling
    #[tokio::test]
    async fn test_error_handling() {
        // Arrange
        let presenter = create_presenter_with_failing_service();

        // Act
        let result = presenter.handle_error_event().await;

        // Assert
        assert!(result.is_ok());
        // Verify error ViewCommand emitted
    }
}
```

## Pseudocode Test References

### ChatPresenter Tests (lines 20-240)
- test_handle_user_event_send_message()
  - Validates UserEvent::SendMessage routed correctly
  - Verifies service call made
  - Verifies thinking events emitted

- test_handle_chat_event_conversation_started()
  - Validates ChatEvent::ConversationStarted handled
  - Verifies ViewCommand::ConversationCreated emitted

- test_handle_chat_event_message_received()
  - Validates ChatEvent::MessageReceived handled
  - Verifies ViewCommand::MessageAppended emitted

- test_handle_chat_event_stream_chunk()
  - Validates ChatEvent::StreamChunk handled
  - Verifies ViewCommand::AppendStream emitted

- test_handle_chat_event_error()
  - Validates ChatEvent::Error handled
  - Verifies ViewCommand::ShowError emitted

### McpPresenter Tests (lines 260-371)
- test_handle_user_event_start_server()
  - Validates UserEvent::StartMcpServer routed
  - Verifies McpService.start_server() called
  - Verifies events emitted

- test_handle_mcp_event_server_started()
  - Validates McpEvent::ServerStarted handled
  - Verifies ViewCommand::McpServerStarted emitted

- test_handle_mcp_event_tools_updated()
  - Validates McpEvent::ToolsUpdated handled
  - Verifies ViewCommand::McpToolsUpdated emitted

### SettingsPresenter Tests (lines 380-444)
- test_handle_user_event_open_settings()
  - Validates UserEvent::OpenSettings routed
  - Verifies ViewCommand::ShowSettings emitted

- test_handle_user_event_update_profile()
  - Validates UserEvent::UpdateProfile routed
  - Verifies service call made
  - Verifies notification emitted

### ErrorPresenter Tests (lines 450-505)
- test_handle_system_error()
  - Validates SystemEvent::Error handled
  - Verifies ViewCommand::ShowError with Critical severity

- test_handle_chat_error()
  - Validates ChatEvent::Error handled
  - Verifies ViewCommand::ShowError with Warning severity

- test_handle_mcp_error()
  - Validates McpEvent::ServerFailed handled
  - Verifies ViewCommand::ShowError emitted

## Verification Commands

### Structural Verification

```bash
# Check all test files created
test -f src/presentation/chat_test.rs
test -f src/presentation/mcp_test.rs
test -f src/presentation/settings_test.rs
test -f src/presentation/error_test.rs
echo "Expected: All 4 test files exist"

# Check test modules declared in mod.rs
grep "mod chat_test;" src/presentation/mod.rs
grep "mod mcp_test;" src/presentation/mod.rs
grep "mod settings_test;" src/presentation/mod.rs
grep "mod error_test;" src/presentation/mod.rs
echo "Expected: All test modules declared"

# Check plan markers in test files
grep -r "@plan:PLAN-20250125-REFACTOR.P11" src/presentation/*_test.rs | wc -l
# Expected: 40+ occurrences

# Check requirement markers
grep -r "@requirement:REQ-026" src/presentation/*_test.rs | wc -l
# Expected: 20+ occurrences
```

### Test Execution (EXPECTED TO FAIL in this phase)

```bash
# Run tests (EXPECTED: Many failures due to stubs)
cargo test --lib -- --test-threads=1 2>&1 | tee test_results.log

# Check test count
grep -E "test result:" test_results.log
# Expected: 40+ tests run

# Check failure count (EXPECTED in this phase)
grep -E "test result:.*FAILED" test_results.log
# Expected: Tests FAIL (stubs return unimplemented!)
```

### Manual Verification Checklist

Review each test file and verify:

#### src/presentation/chat_test.rs
- [ ] test_new() exists
- [ ] test_start_stop_lifecycle() exists
- [ ] test_handle_user_event_send_message() exists
- [ ] test_handle_chat_event_conversation_started() exists
- [ ] test_handle_chat_event_message_received() exists
- [ ] test_handle_chat_event_thinking_started() exists
- [ ] test_handle_chat_event_stream_chunk() exists
- [ ] test_handle_chat_event_error() exists
- [ ] ViewCommand emission verified
- [ ] Plan marker present

#### src/presentation/mcp_test.rs
- [ ] test_new() exists
- [ ] test_start_stop_lifecycle() exists
- [ ] test_handle_user_event_start_server() exists
- [ ] test_handle_mcp_event_server_started() exists
- [ ] test_handle_mcp_event_tools_updated() exists
- [ ] ViewCommand emission verified
- [ ] Plan marker present

#### src/presentation/settings_test.rs
- [ ] test_new() exists
- [ ] test_start_stop_lifecycle() exists
- [ ] test_handle_user_event_open_settings() exists
- [ ] test_handle_user_event_update_profile() exists
- [ ] ViewCommand emission verified
- [ ] Plan marker present

#### src/presentation/error_test.rs
- [ ] test_new() exists
- [ ] test_start_stop_lifecycle() exists
- [ ] test_handle_system_error() exists
- [ ] test_handle_chat_error() exists
- [ ] test_handle_mcp_error() exists
- [ ] ViewCommand emission verified
- [ ] Plan marker present

## Success Criteria

- All test files created (4 test modules)
- Tests compile successfully
- Tests run (even if failing against stubs)
- Test coverage >= 80% of presenter methods
- All event routing paths tested
- All ViewCommand emissions tested
- All error paths tested
- Plan markers present in all test files
- Requirement markers traceable

## Failure Recovery

If this phase fails:

1. Rollback commands:
   ```bash
   git checkout -- src/presentation/mod.rs
   rm -f src/presentation/*_test.rs
   ```

2. Files to revert:
   - src/presentation/chat_test.rs
   - src/presentation/mcp_test.rs
   - src/presentation/settings_test.rs
   - src/presentation/error_test.rs
   - src/presentation/mod.rs

3. Cannot proceed to Phase 11a until tests compile

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P11.md`

Contents:

```markdown
Phase: P11
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/presentation/chat_test.rs (N lines, X tests)
  - src/presentation/mcp_test.rs (N lines, X tests)
  - src/presentation/settings_test.rs (N lines, X tests)
  - src/presentation/error_test.rs (N lines, X tests)
Files Modified:
  - src/presentation/mod.rs (+5 lines)
Tests Added: 40+
Test Results: EXPECTED FAIL (stubs)
Coverage: X% (will improve in P12)
Verification:
  - cargo test --lib: Compiles, runs, fails (expected)
  - Plan markers: 40+ found
  - Requirement markers: 20+ found
  - Test coverage: >= 80% of presenter methods
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 11a: Presenter TDD Verification
2. Verify all tests are written and compile
3. Confirm tests fail against stubs (TDD red state)
4. Then proceed to Phase 12: Presenter Implementation (make tests pass)

## Important Notes

- This is a TDD phase - tests SHOULD FAIL against stubs
- Focus on test completeness, not passing tests
- Next phase will implement real functionality
- Test failures are expected and documented
- ViewCommand emission must be verified in tests
- Event routing must be tested
- Presenter lifecycle must be tested
