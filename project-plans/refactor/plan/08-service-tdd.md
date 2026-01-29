# Phase 08: Service Layer TDD Phase

## Phase ID

`PLAN-20250125-REFACTOR.P08`

## Prerequisites

- Required: Phase 07a (Service Stub Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P07A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P07A.md`
  - All service stubs implemented and compiling
  - All 5 services structurally defined
- Preflight verification: Phases 01-07a completed

## Purpose

Write comprehensive tests for all service layer components BEFORE implementing real functionality. This phase:

1. Creates unit tests for all 5 service modules
2. Defines test coverage for public APIs
3. Tests event emission patterns
4. Tests error handling paths
5. Verifies concurrent access patterns

**Note:** This is a TDD phase. Tests will FAIL against stub implementations. Phase 09 will implement real functionality to make tests pass.

## Requirements Implemented (Expanded)

### REQ-023.1: Service Unit Test Coverage

**Full Text**: All service modules MUST have comprehensive unit tests.

**Behavior**:
- GIVEN: Service is being tested
- WHEN: Unit tests run
- THEN: All public methods tested
- AND: All error paths tested
- AND: Event emission verified
- AND: Coverage >= 80%

**Why This Matters**: TDD ensures services work correctly before integration.

### REQ-023.2: ConversationService Tests

**Full Text**: ConversationService tests MUST cover conversation lifecycle.

**Behavior**:
- GIVEN: ConversationService instance
- WHEN: Tests run
- THEN: create_conversation() tested
- AND: send_message() tested
- AND: get_conversation() tested
- AND: set_active() tested
- AND: Event emission verified

**Why This Matters**: Conversation state is core to chat functionality.

### REQ-023.3: ChatService Tests

**Full Text**: ChatService tests MUST cover LLM integration flow.

**Behavior**:
- GIVEN: ChatService instance
- WHEN: Tests run
- THEN: send_message_stream() tested
- AND: build_message_history() tested
- AND: Thinking events verified
- AND: Stream events verified

**Why This Matters**: Chat flow coordination is critical for UX.

### REQ-023.4: McpService Tests

**Full Text**: McpService tests MUST cover MCP server lifecycle.

**Behavior**:
- GIVEN: McpService instance
- WHEN: Tests run
- THEN: start_server() tested
- AND: stop_server() tested
- AND: call_tool() tested
- AND: Tool registry updates verified

**Why This Matters**: MCP integration requires robust error handling.

### REQ-023.5: ProfileService Tests

**Full Text**: ProfileService tests MUST cover profile management.

**Behavior**:
- GIVEN: ProfileService instance
- WHEN: Tests run
- THEN: add_profile() tested
- AND: get_profile() tested
- AND: update_profile() tested
- AND: Persistence verified

**Why This Matters**: Profile management must be reliable.

### REQ-023.6: SecretsService Tests

**Full Text**: SecretsService tests MUST cover secure credential storage.

**Behavior**:
- GIVEN: SecretsService instance
- WHEN: Tests run
- THEN: get_api_key() tested
- AND: set_api_key() tested
- AND: Security verified (no plaintext leaks)

**Why This Matters**: Security bugs in credential storage are critical.

## Implementation Tasks

### Files to Create

- `src/services/conversation_test.rs`
  - ConversationService unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P08`
  - Implements: `@requirement:REQ-023.2`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 30-164
  - Test modules:
    - test_create_conversation()
    - test_send_message()
    - test_get_conversation()
    - test_set_active()
    - test_conversation_not_found()
    - test_event_emission()

- `src/services/chat_test.rs`
  - ChatService unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P08`
  - Implements: `@requirement:REQ-023.3`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 170-251
  - Test modules:
    - test_send_message_stream()
    - test_build_message_history()
    - test_thinking_events()
    - test_stream_events()
    - test_error_handling()

- `src/services/mcp_test.rs`
  - McpService unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P08`
  - Implements: `@requirement:REQ-023.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 260-360
  - Test modules:
    - test_start_server()
    - test_stop_server()
    - test_call_tool()
    - test_list_tools()
    - test_tool_registry()
    - test_mcp_error_handling()

- `src/services/profile_test.rs`
  - ProfileService unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P08`
  - Implements: `@requirement:REQ-023.5`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 370-426
  - Test modules:
    - test_add_profile()
    - test_get_profile()
    - test_update_profile()
    - test_list_profiles()
    - test_validation()
    - test_persistence()

- `src/services/secrets_test.rs`
  - SecretsService unit tests
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P08`
  - Implements: `@requirement:REQ-023.6`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 430-452
  - Test modules:
    - test_get_api_key()
    - test_set_api_key()
    - test_delete_api_key()
    - test_security_no_plaintext()
    - test_error_handling()

### Files to Modify

- `src/services/mod.rs`
  - ADD test module declarations:
    ```rust
    #[cfg(test)]
    mod conversation_test;
    #[cfg(test)]
    mod chat_test;
    #[cfg(test)]
    mod mcp_test;
    #[cfg(test)]
    mod profile_test;
    #[cfg(test)]
    mod secrets_test;
    ```
  - ADD comment: `@plan:PLAN-20250125-REFACTOR.P08`

### Test Structure Pattern

All test files follow this structure:

```rust
/// @plan PLAN-20250125-REFACTOR.P08
/// @requirement REQ-023.X
use super::*;
use tokio::test;
use uuid::Uuid;

#[cfg(test)]
mod tests {
    /// Test successful operation
    #[tokio::test]
    async fn test_success_case() {
        // Arrange
        let (event_tx, _) = broadcast::channel(100);
        let service = ServiceUnderTest::new(event_tx);

        // Act
        let result = service.method().await;

        // Assert
        assert!(result.is_ok());
        // Verify events emitted
        // Verify state changes
    }

    /// Test error case
    #[tokio::test]
    async fn test_error_case() {
        // Arrange
        let (event_tx, _) = broadcast::channel(100);
        let service = ServiceUnderTest::new(event_tx);

        // Act
        let result = service.method().await;

        // Assert
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::NotFound(_))));
    }

    /// Test event emission
    #[tokio::test]
    async fn test_event_emission() {
        // Arrange
        let (event_tx, mut event_rx) = broadcast::channel(100);
        let service = ServiceUnderTest::new(event_tx);

        // Act
        service.method().await;

        // Assert
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(event, ExpectedEvent::Variant { ... }));
    }

    /// Test concurrent access
    #[tokio::test]
    async fn test_concurrent_access() {
        // Arrange
        let (event_tx, _) = broadcast::channel(100);
        let service = Arc::new(ServiceUnderTest::new(event_tx));

        // Act
        let tasks = (0..10).map(|_| {
            let service = service.clone();
            tokio::spawn(async move {
                service.method().await
            })
        });

        // Assert
        let results = futures::future::join_all(tasks).await;
        assert!(results.into_iter().all(|r| r.is_ok()));
    }
}
```

## Pseudocode Test References

### ConversationService Tests (lines 30-164)
- test_create_conversation_emits_event()
  - Asserts ChatEvent::ConversationStarted emitted
  - Validates conversation ID returned
  - Validates profile_id stored correctly

- test_send_message_appends_to_history()
  - Asserts MessageReceived event emitted
  - Validates message content stored
  - Validates timestamp set

- test_get_conversation_returns_none_when_not_found()
  - Tests non-existent conversation ID
  - Asserts Option::None returned

- test_set_active_updates_active_id()
  - Asserts ActiveConversationChanged event emitted
  - Validates get_active() returns new ID

### ChatService Tests (lines 170-251)
- test_send_message_stream_spawns_task()
  - Asserts task spawned in runtime
  - Asserts ThinkingStarted event emitted
  - Asserts ThinkingEnded event emitted

- test_build_message_history()
  - Validates message order preserved
  - Validates user/assistant roles correct

- test_stream_chunks_emitted()
  - Asserts StreamChunk events emitted
  - Validates content accumulated

### McpService Tests (lines 260-360)
- test_start_server_registers_tools()
  - Asserts tools registered in tool registry
  - Asserts ServerStarted event emitted

- test_call_tool_routes_to_correct_mcp()
  - Validates tool name lookup in registry
  - Asserts ToolCalled event emitted
  - Asserts ToolResult event emitted

### ProfileService Tests (lines 370-426)
- test_add_profile_validates_input()
  - Asserts validation errors for invalid profiles
  - Asserts ProfileAdded event emitted on success

- test_update_profile_persists_changes()
  - Asserts storage.save() called
  - Validates updates reflected in get_profile()

### SecretsService Tests (lines 430-452)
- test_set_api_key_stores_securely()
  - Asserts key stored (not plaintext in memory)
  - Asserts get_api_key() retrieves correct key

- test_security_no_plaintext_leaks()
  - Validates no plaintext in debug output
  - Validates no plaintext in logs

## Verification Commands

### Structural Verification

```bash
# Check all test files created
test -f src/services/conversation_test.rs
test -f src/services/chat_test.rs
test -f src/services/mcp_test.rs
test -f src/services/profile_test.rs
test -f src/services/secrets_test.rs
echo "Expected: All 5 test files exist"

# Check test modules declared in mod.rs
grep "mod conversation_test;" src/services/mod.rs
grep "mod chat_test;" src/services/mod.rs
grep "mod mcp_test;" src/services/mod.rs
grep "mod profile_test;" src/services/mod.rs
grep "mod secrets_test;" src/services/mod.rs
echo "Expected: All test modules declared"

# Check plan markers in test files
grep -r "@plan:PLAN-20250125-REFACTOR.P08" src/services/*_test.rs | wc -l
# Expected: 50+ occurrences (all tests, fixtures, helpers)

# Check requirement markers
grep -r "@requirement:REQ-023" src/services/*_test.rs | wc -l
# Expected: 30+ occurrences
```

### Test Execution (EXPECTED TO FAIL in this phase)

```bash
# Run tests (EXPECTED: Many failures due to stubs)
cargo test --lib -- --test-threads=1 2>&1 | tee test_results.log

# Check test count
grep -E "test result:" test_results.log
# Expected: 50+ tests run

# Check failure count (EXPECTED in this phase)
grep -E "test result:.*FAILED" test_results.log
# Expected: Tests FAIL (stubs return unimplemented!)
```

### Coverage Verification

```bash
# Install tarpaulin if not present
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --lib --output-dir coverage --output Html

# Check coverage (will be low in this phase due to stubs)
# Expected: Coverage will increase after Phase 09 implementation
```

### Manual Verification Checklist

Review each test file and verify:

#### src/services/conversation_test.rs
- [ ] test_create_conversation() exists
- [ ] test_send_message() exists
- [ ] test_get_conversation() exists
- [ ] test_set_active() exists
- [ ] test_conversation_not_found() exists
- [ ] test_event_emission() exists
- [ ] Event emission verified via event_rx
- [ ] Error cases tested
- [ ] Plan marker present
- [ ] Requirement REQ-023.2 marker

#### src/services/chat_test.rs
- [ ] test_send_message_stream() exists
- [ ] test_build_message_history() exists
- [ ] test_thinking_events() exists
- [ ] test_stream_events() exists
- [ ] test_error_handling() exists
- [ ] Async task spawning verified
- [ ] Event emission verified
- [ ] Plan marker present
- [ ] Requirement REQ-023.3 marker

#### src/services/mcp_test.rs
- [ ] test_start_server() exists
- [ ] test_stop_server() exists
- [ ] test_call_tool() exists
- [ ] test_list_tools() exists
- [ ] test_tool_registry() exists
- [ ] test_mcp_error_handling() exists
- [ ] Tool registry updates verified
- [ ] Plan marker present
- [ ] Requirement REQ-023.4 marker

#### src/services/profile_test.rs
- [ ] test_add_profile() exists
- [ ] test_get_profile() exists
- [ ] test_update_profile() exists
- [ ] test_list_profiles() exists
- [ ] test_validation() exists
- [ ] test_persistence() exists
- [ ] Plan marker present
- [ ] Requirement REQ-023.5 marker

#### src/services/secrets_test.rs
- [ ] test_get_api_key() exists
- [ ] test_set_api_key() exists
- [ ] test_delete_api_key() exists
- [ ] test_security_no_plaintext() exists
- [ ] test_error_handling() exists
- [ ] Security assertions present
- [ ] Plan marker present
- [ ] Requirement REQ-023.6 marker

## Success Criteria

- All test files created (5 test modules)
- Tests compile successfully
- Tests run (even if failing against stubs)
- Test coverage >= 80% of public APIs
- All event emission paths tested
- All error paths tested
- Concurrent access tests present
- Plan markers present in all test files
- Requirement markers traceable

## Failure Recovery

If this phase fails:

1. Rollback commands:
   ```bash
   git checkout -- src/services/mod.rs
   rm -f src/services/*_test.rs
   ```

2. Files to revert:
   - src/services/conversation_test.rs
   - src/services/chat_test.rs
   - src/services/mcp_test.rs
   - src/services/profile_test.rs
   - src/services/secrets_test.rs
   - src/services/mod.rs

3. Cannot proceed to Phase 08a until tests compile

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P08.md`

Contents:

```markdown
Phase: P08
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/services/conversation_test.rs (N lines, X tests)
  - src/services/chat_test.rs (N lines, X tests)
  - src/services/mcp_test.rs (N lines, X tests)
  - src/services/profile_test.rs (N lines, X tests)
  - src/services/secrets_test.rs (N lines, X tests)
Files Modified:
  - src/services/mod.rs (+6 lines)
Tests Added: 50+
Test Results: EXPECTED FAIL (stubs)
Coverage: X% (will improve in P09)
Verification:
  - cargo test --lib: Compiles, runs, fails (expected)
  - Plan markers: 50+ found
  - Requirement markers: 30+ found
  - Test coverage: >= 80% of public APIs
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 08a: Service TDD Verification
2. Verify all tests are written and compile
3. Confirm tests fail against stubs (TDD red state)
4. Then proceed to Phase 09: Service Implementation (make tests pass)

## Important Notes

- This is a TDD phase - tests SHOULD FAIL against stubs
- Focus on test completeness, not passing tests
- Next phase will implement real functionality
- Test failures are expected and documented
- Event emission must be verified in tests
- Concurrent access must be tested
