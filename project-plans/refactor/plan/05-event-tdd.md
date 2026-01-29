# Phase 05: Event System TDD Phase

## Phase ID

`PLAN-20250125-REFACTOR.P05`

## Prerequisites

- Required: Phase 04a (Event Stub Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P04A" project-plans/`
- Expected files from previous phase:
  - All stub files from Phase 04 verified
  - `project-plans/refactor/plan/.completed/P04A.md`
- Preflight verification: Phases 01-04a completed

## Purpose

Write comprehensive tests for the EventBus system BEFORE implementing functionality. This phase:

1. Creates test file for EventBus core functionality
2. Creates test file for event types
3. Creates test file for global singleton
4. Tests FAIL against stubs (expected TDD behavior)
5. Tests define expected behavior using GIVEN/WHEN/THEN

**Note:** This is a TDD phase. Tests are written and MUST FAIL. Implementation happens in Phase 06.

## Requirements Implemented (Expanded)

### REQ-020.1: EventBus Creation Test

**Full Text**: EventBus MUST be createable with specified channel capacity.

**Behavior**:
- GIVEN: No EventBus exists
- WHEN: EventBus::new(capacity) is called with capacity=16
- THEN: EventBus instance is returned
- AND: Channel capacity is 16

**Why This Matters**: Tests verify EventBus can be instantiated with correct configuration.

### REQ-020.2: Event Publishing Test

**Full Text**: EventBus MUST publish events to all subscribers.

**Behavior**:
- GIVEN: EventBus with 2 subscribers
- WHEN: publish(AppEvent::User(UserEvent::Quit)) is called
- THEN: Both subscribers receive the event
- AND: publish() returns Ok(2)

**Why This Matters**: Tests verify broadcast behavior works correctly.

### REQ-020.3: No Subscribers Error Test

**Full Text**: EventBus MUST return error when publishing with no subscribers.

**Behavior**:
- GIVEN: EventBus with 0 subscribers
- WHEN: publish(event) is called
- THEN: publish() returns Err(EventBusError::NoSubscribers)
- AND: Event is not delivered

**Why This Matters**: Tests verify error handling for edge case.

### REQ-020.4: Subscription Test

**Full Text**: EventBus MUST allow multiple subscribers to receive events.

**Behavior**:
- GIVEN: EventBus instance
- WHEN: subscribe() is called 3 times
- THEN: 3 unique Receivers are returned
- AND: All Receivers are independent
- AND: subscriber_count() returns 3

**Why This Matters**: Tests verify fan-out pattern works correctly.

### REQ-020.5: Global Singleton Test

**Full Text**: Global event bus MUST be lazy-initialized singleton.

**Behavior**:
- GIVEN: No global event bus initialized
- WHEN: emit() is called
- THEN: EventBus is created on first call
- AND: Subsequent emit() calls use same instance
- AND: Only one EventBus exists

**Why This Matters**: Tests verify singleton pattern prevents multiple instances.

### REQ-020.6: Event Type Test

**Full Text**: Event type hierarchy MUST support all domain events.

**Behavior**:
- GIVEN: Any domain component needs to emit an event
- WHEN: Component creates AppEvent with appropriate variant
- THEN: Event compiles and type-checks
- AND: Event can be cloned
- AND: Event can be debug-printed

**Why This Matters**: Tests verify type system covers all use cases.

## Implementation Tasks

### Files to Create

- `src/events/event_bus_test.rs`
  - Unit tests for EventBus core functionality
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P05`
  - Implements: `@requirement:REQ-020.1`, `@requirement:REQ-020.2`, `@requirement:REQ-020.3`, `@requirement:REQ-020.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 10-46

- `src/events/types_test.rs`
  - Unit tests for event type hierarchy
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P05`
  - Implements: `@requirement:REQ-020.6`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 80-123

- `src/events/global_test.rs`
  - Unit tests for global singleton
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P05`
  - Implements: `@requirement:REQ-020.5`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 50-75, 150-156

- `src/events/integration_test.rs`
  - Integration tests for end-to-end event flow
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P05`
  - Tests: Emit from one "component", receive in another

### Files to Modify

- `src/events/mod.rs`
  - ADD: `#[cfg(test)] mod event_bus_test;`
  - ADD: `#[cfg(test)] mod types_test;`
  - ADD: `#[cfg(test)] mod global_test;`
  - ADD: `#[cfg(test)] mod integration_test;`
  - ADD comment: `@plan:PLAN-20250125-REFACTOR.P05`

### Test Structure Template

Each test MUST follow this pattern:

```rust
/// EventBus creation test
///
/// GIVEN: No EventBus exists
/// WHEN: EventBus::new(capacity) is called with capacity=16
/// THEN: EventBus instance is returned
/// AND: Channel capacity is 16
///
/// @plan PLAN-20250125-REFACTOR.P05
/// @requirement REQ-020.1
#[test]
fn test_event_bus_creation() {
    // Given
    let capacity = 16;

    // When
    let bus = EventBus::new(capacity);

    // Then
    assert!(bus.subscriber_count() == 0, "New bus has no subscribers");
    // More assertions...
}
```

### Test Cases to Implement

#### event_bus_test.rs

```rust
// Test 1: EventBus creation
#[test]
fn test_event_bus_creation() {
    // GIVEN: No EventBus exists
    // WHEN: EventBus::new(16) is called
    // THEN: EventBus instance returned
    // AND: subscriber_count() returns 0
}

// Test 2: Single subscriber
#[test]
fn test_single_subscription() {
    // GIVEN: EventBus instance
    // WHEN: subscribe() is called once
    // THEN: Receiver returned
    // AND: subscriber_count() returns 1
}

// Test 3: Multiple subscribers
#[test]
fn test_multiple_subscriptions() {
    // GIVEN: EventBus instance
    // WHEN: subscribe() is called 3 times
    // THEN: 3 unique Receivers returned
    // AND: subscriber_count() returns 3
}

// Test 4: Publish to single subscriber
#[test]
fn test_publish_to_single_subscriber() {
    // GIVEN: EventBus with 1 subscriber
    // WHEN: publish(event) is called
    // THEN: Subscriber receives event
    // AND: publish() returns Ok(1)
}

// Test 5: Publish to multiple subscribers
#[test]
fn test_publish_to_multiple_subscribers() {
    // GIVEN: EventBus with 3 subscribers
    // WHEN: publish(event) is called
    // THEN: All 3 subscribers receive event
    // AND: publish() returns Ok(3)
}

// Test 6: Publish with no subscribers
#[test]
fn test_publish_no_subscribers_error() {
    // GIVEN: EventBus with 0 subscribers
    // WHEN: publish(event) is called
    // THEN: publish() returns Err(EventBusError::NoSubscribers)
    // AND: Event is not delivered anywhere
}

// Test 7: Event types can be published
#[test]
fn test_publish_all_event_types() {
    // GIVEN: EventBus with subscriber
    // WHEN: Each event type is published
    // THEN: All events received successfully
    // Test: UserEvent, ChatEvent, McpEvent, SystemEvent
}
```

#### types_test.rs

```rust
// Test 1: Event types can be created
#[test]
fn test_event_types_create() {
    // GIVEN: No events exist
    // WHEN: Each event variant is created
    // THEN: All events compile
    // Test: All UserEvent, ChatEvent, McpEvent, SystemEvent variants
}

// Test 2: Events can be cloned
#[test]
fn test_events_can_be_cloned() {
    // GIVEN: Event instance
    // WHEN: event.clone() is called
    // THEN: Identical event returned
}

// Test 3: Events can be debug-printed
#[test]
fn test_events_can_be_debugged() {
    // GIVEN: Event instance
    // WHEN: format!("{:?}", event) is called
    // THEN: String representation returned
}

// Test 4: Event hierarchy wraps correctly
#[test]
fn test_app_event_wrapping() {
    // GIVEN: Domain event (e.g., UserEvent::Quit)
    // WHEN: Wrapped in AppEvent::User()
    // THEN: AppEvent variant correct
    // AND: Can be pattern-matched
}
```

#### global_test.rs

```rust
// Test 1: Global singleton lazy initialization
#[test]
fn test_global_singleton_lazy_init() {
    // GIVEN: No global bus initialized
    // WHEN: emit() called for first time
    // THEN: EventBus created
    // AND: emit() succeeds
}

// Test 2: Global singleton returns same instance
#[test]
fn test_global_singleton_same_instance() {
    // GIVEN: Global bus initialized
    // WHEN: emit() called twice
    // THEN: Same EventBus instance used
    // AND: subscriber_count() accumulates
}

// Test 3: Global subscribe works
#[test]
fn test_global_subscribe() {
    // GIVEN: Global bus initialized
    // WHEN: subscribe() called
    // THEN: Receiver returned
    // AND: Receiver receives global events
}

// Test 4: Global emit with no subscribers
#[test]
fn test_global_emit_no_subscribers() {
    // GIVEN: Global bus with 0 subscribers
    // WHEN: emit(event) called
    // THEN: Returns Err(EventBusError::NoSubscribers)
}
```

#### integration_test.rs

```rust
// Test 1: End-to-end event flow
#[test]
fn test_end_to_end_event_flow() {
    // GIVEN: Global event bus
    // AND: Subscriber registered
    // WHEN: Component emits event via emit()
    // THEN: Subscriber receives event
    // AND: Event is correct type
}

// Test 2: Multiple components receive events
#[test]
fn test_multiple_components_receive_events() {
    // GIVEN: 3 "components" (subscribers)
    // WHEN: One component emits event
    // THEN: All 3 components receive event
}

// Test 3: Event ordering preserved
#[test]
fn test_event_ordering_preserved() {
    // GIVEN: Single subscriber
    // WHEN: 5 events emitted in sequence
    // THEN: Events received in same order
}
```

## Pseudocode References

### Test Coverage Map

- Lines 10-12 (EventBus struct): Test creation and basic operations
- Lines 20-23 (new()): test_event_bus_creation
- Lines 30-38 (publish()): test_publish_to_single_subscriber, test_publish_to_multiple_subscribers, test_publish_no_subscribers_error
- Lines 40-41 (subscribe()): test_single_subscription, test_multiple_subscriptions
- Lines 45-46 (subscriber_count()): Used in all subscriber tests
- Lines 50-60 (GLOBAL_BUS): test_global_singleton_lazy_init, test_global_singleton_same_instance
- Lines 65-69 (emit()): test_global_emit_no_subscribers, integration tests
- Lines 73-75 (subscribe()): test_global_subscribe
- Lines 80-123 (Event types): test_event_types_create, test_events_can_be_cloned, test_events_can_be_debugged

## Verification Commands

### Structural Verification

```bash
# Check test files created
test -f src/events/event_bus_test.rs
test -f src/events/types_test.rs
test -f src/events/global_test.rs
test -f src/events/integration_test.rs
echo "Expected: All 4 test files exist"

# Check plan markers in test files
grep -r "@plan:PLAN-20250125-REFACTOR.P05" src/events/*_test.rs | wc -l
# Expected: 20+ occurrences (all tests)

# Check requirement markers
grep -r "@requirement:REQ-020" src/events/*_test.rs | wc -l
# Expected: 20+ occurrences (all tests)

# Check test module declarations in mod.rs
grep "mod.*_test;" src/events/mod.rs
# Expected: 4 test modules declared
```

### Test Compilation (Expected: FAIL)

```bash
# Tests should compile but fail when run
cargo test --lib events 2>&1 | tee test.log

# Expected:
# - Compiles successfully
# - Tests FAIL with "not implemented" or panics
# - NOT compilation errors

if grep -q "test result: FAILED" test.log; then
    echo "[OK] Tests fail as expected (TDD phase)"
else
    echo " Tests passed unexpectedly - were stubs implemented?"
    exit 1
fi
```

### Test Coverage Check

```bash
# Count tests written
grep -r "^#\[test\]" src/events/*_test.rs | wc -l
# Expected: 15+ tests

# Count test functions
grep -r "^fn test_" src/events/*_test.rs | wc -l
# Expected: 15+ test functions

# Verify GIVEN/WHEN/THEN comments in tests
grep -r "GIVEN:" src/events/*_test.rs | wc -l
grep -r "WHEN:" src/events/*_test.rs | wc -l
grep -r "THEN:" src/events/*_test.rs | wc -l
# Expected: All three present in most tests
```

### Manual Verification Checklist

Read each test file and verify:

#### event_bus_test.rs
- [ ] test_event_bus_creation exists
- [ ] test_single_subscription exists
- [ ] test_multiple_subscriptions exists
- [ ] test_publish_to_single_subscriber exists
- [ ] test_publish_to_multiple_subscribers exists
- [ ] test_publish_no_subscribers_error exists
- [ ] test_publish_all_event_types exists
- [ ] All tests have GIVEN/WHEN/THEN comments
- [ ] All tests have plan markers
- [ ] All tests have requirement markers
- [ ] Tests will fail against stubs

#### types_test.rs
- [ ] test_event_types_create exists
- [ ] test_events_can_be_cloned exists
- [ ] test_events_can_be_debugged exists
- [ ] test_app_event_wrapping exists
- [ ] All event variants covered
- [ ] Tests verify Clone and Debug traits
- [ ] Plan markers present

#### global_test.rs
- [ ] test_global_singleton_lazy_init exists
- [ ] test_global_singleton_same_instance exists
- [ ] test_global_subscribe exists
- [ ] test_global_emit_no_subscribers exists
- [ ] Tests verify singleton behavior
- [ ] Plan markers present

#### integration_test.rs
- [ ] test_end_to_end_event_flow exists
- [ ] test_multiple_components_receive_events exists
- [ ] test_event_ordering_preserved exists
- [ ] Tests verify real-world usage
- [ ] Plan markers present

#### src/events/mod.rs
- [ ] All 4 test modules declared
- [ ] Test modules use #[cfg(test)]
- [ ] Plan marker comment added

## Success Criteria

- All 4 test files created
- 15+ tests written
- All tests compile
- All tests FAIL (expected TDD behavior)
- Tests follow GIVEN/WHEN/THEN pattern
- Plan markers present in all tests
- Requirement markers present in all tests
- No tests pass (if any pass, stubs too complete)

## Failure Recovery

If this phase fails:

### If tests don't compile

```bash
# Check compilation errors
cat test.log

# Fix issues:
# 1. Missing test module declarations in mod.rs
# 2. Syntax errors in tests
# 3. Missing imports
# 4. Type mismatches

# DO NOT modify stub implementation - fix tests only
```

### If tests pass unexpectedly

```bash
# This means stubs have too much implementation
# Check what's implemented
grep -r "broadcast::channel\|OnceLock" src/events/*.rs | grep -v test

# If found, revert to simpler stubs:
# - Remove real implementation
# - Use unimplemented!() instead
```

### If test coverage insufficient

```bash
# Count tests
grep -r "^fn test_" src/events/*_test.rs | wc -l

# If < 15, add more tests to cover:
# - Edge cases
# - Error conditions
# - Integration scenarios
```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P05.md`

Contents:

```markdown
Phase: P05
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/events/event_bus_test.rs (N lines, M tests)
  - src/events/types_test.rs (N lines, M tests)
  - src/events/global_test.rs (N lines, M tests)
  - src/events/integration_test.rs (N lines, M tests)
Files Modified:
  - src/events/mod.rs (+4 lines for test modules)
Tests Added: M total tests
Test Results:
  - Compilation: PASS
  - Test run: FAIL (expected for TDD)
  - Failing tests: N
Verification:
  - Plan markers: M found
  - Requirement markers: M found
  - GIVEN/WHEN/THEN: Present in all tests
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 05a: Event TDD Verification
2. Verify tests are comprehensive and fail correctly
3. Then proceed to Phase 06: Event Implementation (make tests pass)

## Important Notes

- This is a TDD phase - write ONLY tests
- Tests MUST FAIL (if they pass, something is wrong)
- DO NOT implement functionality to make tests pass
- DO NOT modify stub code from Phase 04
- Tests define the contract for Phase 06 implementation
- Use GIVEN/WHEN/THEN comments for clarity
- Each test should verify ONE behavior
