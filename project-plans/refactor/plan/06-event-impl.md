# Phase 06: Event System Implementation

## Phase ID

`PLAN-20250125-REFACTOR.P06`

## Prerequisites

- Required: Phase 05a (Event TDD Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P05A" project-plans/`
- Expected files from previous phase:
  - All test files from Phase 05 verified
  - `project-plans/refactor/plan/.completed/P05A.md`
  - Tests FAILING against stubs (expected TDD behavior)
- Preflight verification: Phases 01-05a completed

## Purpose

Implement the EventBus functionality to make tests from Phase 05 PASS. This phase:

1. Implements EventBus core functionality
2. Implements global singleton functions
3. Implements error handling
4. Adds event logging
5. Makes ALL tests from Phase 05 pass

**Note:** This is an IMPLEMENTATION phase. Tests from Phase 05 MUST PASS after this phase.

## Requirements Implemented (Expanded)

### REQ-021.1: EventBus Core Implementation

**Full Text**: EventBus MUST use tokio::sync::broadcast for event distribution.

**Behavior**:
- GIVEN: EventBus::new(capacity) called
- WHEN: Channel is created
- THEN: broadcast::channel(capacity) is used
- AND: Sender stored in EventBus
- AND: Receiver kept to prevent channel closure

**Why This Matters**: tokio::sync::broadcast provides lock-free multi-producer multi-consumer pattern.

### REQ-021.2: Event Publishing Implementation

**Full Text**: EventBus::publish() MUST broadcast events using tx.send().

**Behavior**:
- GIVEN: EventBus with subscribers
- WHEN: publish(event) is called
- THEN: tx.send(event.clone()) is called
- AND: Returns Ok(subscriber_count) on success
- AND: Returns Err(EventBusError::NoSubscribers) if no receivers

**Why This Matters**: Actual broadcast implementation enables fan-out pattern.

### REQ-021.3: Event Subscription Implementation

**Full Text**: EventBus::subscribe() MUST return tx.subscribe().

**Behavior**:
- GIVEN: EventBus instance
- WHEN: subscribe() is called
- THEN: tx.subscribe() is called
- AND: Returned Receiver receives all future events

**Why This Matters**: Enables components to listen to events channel.

### REQ-021.4: Global Singleton Implementation

**Full Text**: Global event bus MUST use OnceLock<Arc<EventBus>> for lazy initialization.

**Behavior**:
- GIVEN: First call to get_or_init_event_bus()
- WHEN: GLOBAL_BUS.get() returns None
- THEN: Arc::new(EventBus::new(16)) is created
- AND: GLOBAL_BUS.set() is called
- AND: Arc is returned

**Why This Matters**: Thread-safe singleton without static initialization issues.

### REQ-021.5: Event Logging Implementation

**Full Text**: EventBus MUST log events using tracing crate.

**Behavior**:
- GIVEN: Event is published successfully
- WHEN: send() returns Ok(count)
- THEN: info!("Event emitted: {:?} ({} subscribers)", event, count) is called

**Why This Matters**: Debugging and observability of event flow.

## Implementation Tasks

### Files to Modify

- `src/events/event_bus.rs`
  - IMPLEMENT: EventBus::new() with broadcast::channel
  - IMPLEMENT: EventBus::publish() with tx.send()
  - IMPLEMENT: EventBus::subscribe() with tx.subscribe()
  - IMPLEMENT: EventBus::subscriber_count() with tx.receiver_count()
  - ADD: Event logging (tracing)
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 10-46
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P06`
  - Implements: `@requirement:REQ-021.1`, `@requirement:REQ-021.2`, `@requirement:REQ-021.3`, `@requirement:REQ-021.5`

- `src/events/global.rs`
  - IMPLEMENT: GLOBAL_BUS static with OnceLock<Arc<EventBus>>
  - IMPLEMENT: get_or_init_event_bus() helper
  - IMPLEMENT: init_event_bus() function
  - IMPLEMENT: emit() function calling EventBus::publish
  - IMPLEMENT: subscribe() function calling EventBus::subscribe
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 50-75, 150-156
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P06`
  - Implements: `@requirement:REQ-021.4`

- `src/events/error.rs`
  - VERIFY: EventBusError has all variants
  - VERIFY: thiserror::Error derive
  - VERIFY: Display implementation
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 160-162
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P06`

- `src/events/types.rs`
  - VERIFY: All event types defined
  - VERIFY: All derive traits present (Debug, Clone)
  - VERIFY: No changes needed (already complete from stub)
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 80-123
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P06`

### Implementation Details

#### EventBus::new() (lines 20-23)

```rust
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.1
/// @pseudocode event-bus.md lines 20-23
pub fn new(capacity: usize) -> Self {
    let (tx, rx) = broadcast::channel(capacity);
    EventBus { tx, _rx: rx }
}
```

#### EventBus::publish() (lines 30-38)

```rust
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.2
/// @requirement REQ-021.5
/// @pseudocode event-bus.md lines 30-38
pub fn publish(&self, event: AppEvent) -> Result<usize, EventBusError> {
    match self.tx.send(event.clone()) {
        Ok(count) => {
            info!("Event emitted: {:?} ({} subscribers)", event, count);
            Ok(count)
        },
        Err(_) => {
            warn!("Event dropped (no subscribers): {:?}", event);
            Err(EventBusError::NoSubscribers)
        }
    }
}
```

#### EventBus::subscribe() (lines 40-41)

```rust
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.3
/// @pseudocode event-bus.md lines 40-41
pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
    self.tx.subscribe()
}
```

#### EventBus::subscriber_count() (lines 45-46)

```rust
/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 45-46
pub fn subscriber_count(&self) -> usize {
    self.tx.receiver_count()
}
```

#### Global Singleton (lines 50-75, 150-156)

```rust
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 50-60
use std::sync::OnceLock;
use std::sync::Arc;

static GLOBAL_BUS: OnceLock<Arc<EventBus>> = OnceLock::new();

/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 150-156
fn get_or_init_event_bus() -> Arc<EventBus> {
    match GLOBAL_BUS.get() {
        Some(bus) => bus.clone(),
        None => {
            let bus = Arc::new(EventBus::new(16));
            let _ = GLOBAL_BUS.set(bus.clone()).expect("once lock initialized");
            bus
        }
    }
}

/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 55-60
pub fn init_event_bus() -> Arc<EventBus> {
    get_or_init_event_bus()
}

/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 65-69
pub fn emit(event: AppEvent) -> Result<(), EventBusError> {
    let bus = get_or_init_event_bus();
    match bus.publish(event) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 73-75
pub fn subscribe() -> broadcast::Receiver<AppEvent> {
    let bus = get_or_init_event_bus();
    bus.subscribe()
}
```

### Dependencies to Add

Verify dependencies in `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1.0", features = ["sync"] }
tracing = "0.1"
thiserror = "1.0"
```

## Pseudocode References

### Implementation Coverage Map

- Lines 10-12 (EventBus struct): Implement struct fields
- Lines 20-23 (new()): Implement with broadcast::channel
- Lines 30-38 (publish()): Implement with tx.send() and logging
- Lines 40-41 (subscribe()): Implement with tx.subscribe()
- Lines 45-46 (subscriber_count()): Implement with tx.receiver_count()
- Lines 50-60 (GLOBAL_BUS): Implement OnceLock static
- Lines 65-69 (emit()): Implement wrapper function
- Lines 73-75 (subscribe()): Implement wrapper function
- Lines 150-156 (get_or_init): Implement helper function
- Lines 130-141 (logging): Implement event logging

## Verification Commands

### Structural Verification

```bash
# Check implementation files modified
grep -r "@plan:PLAN-20250125-REFACTOR.P06" src/events/*.rs | grep -v test | wc -l
# Expected: 20+ occurrences (all functions implemented)

# Check requirement markers
grep -r "@requirement:REQ-021" src/events/*.rs | grep -v test | wc -l
# Expected: 10+ occurrences

# Verify no stubs remain
grep -r "unimplemented!" src/events/*.rs | grep -v test
# Expected: 0 matches (all implemented)

# Verify real implementation exists
grep -r "broadcast::channel" src/events/event_bus.rs
# Expected: Found (real implementation)
grep -r "OnceLock" src/events/global.rs
# Expected: Found (real implementation)
```

### Compilation Verification

```bash
# Verify code compiles
cargo build --lib 2>&1 | tee build.log

if [ $? -eq 0 ]; then
    echo "[OK] Build successful"
else
    echo " Build failed"
    cat build.log
    exit 1
fi

echo "Expected: Build succeeds without warnings"
```

### Test Execution (Critical: Must PASS)

```bash
# Run tests - MUST PASS
cargo test --lib events 2>&1 | tee test.log

# Check for test success
if grep -q "test result: OK" test.log; then
    echo "[OK] All tests PASS"
else
    echo " Tests FAILED"
    grep "FAILED" test.log
    exit 1
fi

# Count passing tests
grep -o "test .* OK" test.log | wc -l
echo "Expected: 15+ tests pass"

# Verify no tests fail
grep "FAILED" test.log
# Expected: No matches (all tests pass)
```

### Implementation Verification

```bash
# Verify broadcast channel usage
echo "=== Broadcast Channel Implementation Check ==="
grep -n "broadcast::channel" src/events/event_bus.rs
echo "Expected: Found in new() method"

# Verify send usage
echo ""
echo "=== Send Implementation Check ==="
grep -n "\.send(" src/events/event_bus.rs
echo "Expected: Found in publish() method"

# Verify subscribe usage
echo ""
echo "=== Subscribe Implementation Check ==="
grep -n "\.subscribe()" src/events/event_bus.rs src/events/global.rs
echo "Expected: Found in subscribe() methods"

# Verify OnceLock usage
echo ""
echo "=== OnceLock Implementation Check ==="
grep -n "OnceLock" src/events/global.rs
echo "Expected: Found for GLOBAL_BUS static"

# Verify logging
echo ""
echo "=== Logging Implementation Check ==="
grep -n "info!\|warn!\|debug!" src/events/event_bus.rs
echo "Expected: Found in publish() method"
```

### Manual Verification Checklist

Read each file and verify implementation:

#### src/events/event_bus.rs
- [ ] EventBus struct has tx: broadcast::Sender
- [ ] EventBus struct has _rx: broadcast::Receiver
- [ ] new() creates broadcast::channel
- [ ] publish() calls tx.send()
- [ ] publish() logs on success
- [ ] publish() logs warn on no subscribers
- [ ] publish() returns Ok(count) or Err(NoSubscribers)
- [ ] subscribe() calls tx.subscribe()
- [ ] subscriber_count() calls tx.receiver_count()
- [ ] All stubs (unimplemented!) removed
- [ ] Plan markers present
- [ ] Requirement markers present
- [ ] Pseudocode references present

#### src/events/global.rs
- [ ] GLOBAL_BUS defined as OnceLock<Arc<EventBus>>
- [ ] get_or_init_event_bus() checks GLOBAL_BUS.get()
- [ ] get_or_init_event_bus() creates EventBus if None
- [ ] get_or_init_event_bus() sets GLOBAL_BUS
- [ ] init_event_bus() calls get_or_init_event_bus()
- [ ] emit() calls bus.publish()
- [ ] emit() maps EventBusError appropriately
- [ ] subscribe() calls bus.subscribe()
- [ ] All stubs removed
- [ ] Plan markers present
- [ ] Requirement markers present
- [ ] Pseudocode references present

#### src/events/error.rs
- [ ] EventBusError enum exists
- [ ] NoSubscribers variant exists
- [ ] ChannelClosed variant exists (or removed if not used)
- [ ] thiserror::Error derive present
- [ ] Display implementation present
- [ ] No changes needed from stub phase

#### src/events/types.rs
- [ ] All event types defined
- [ ] All derive Debug and Clone
- [ ] No changes needed from stub phase
- [ ] Plan markers present

## Success Criteria

### Automated Checks

- All EventBus methods implemented (no stubs)
- Code compiles without errors
- ALL tests from Phase 05 PASS
- 0 tests failing
- broadcast::channel used in new()
- OnceLock used in global.rs
- Logging present in publish()
- Plan markers present in all implementations

### Manual Verification

- EventBus::new() creates real broadcast channel
- EventBus::publish() broadcasts to all subscribers
- EventBus::subscribe() returns working receiver
- Global singleton correctly initialized
- emit() and subscribe() work as expected
- Event logging occurs on publish
- Error handling works correctly

## Failure Recovery

If this phase fails:

### If tests fail

```bash
# Check which tests fail
cat test.log | grep FAILED

# Common issues:
# 1. broadcast::channel not created correctly
# 2. tx.send() signature wrong
# 3. subscribe() not returning receiver
# 4. OnceLock not working correctly
# 5. Logging causing errors

# Debug each failing test individually:
cargo test test_event_bus_creation -- --nocapture
cargo test test_publish_to_single_subscriber -- --nocapture
# etc.
```

### If compilation fails

```bash
# Check compilation errors
cat build.log

# Common fixes:
# 1. Missing imports (tokio::sync::broadcast, OnceLock, Arc)
# 2. Wrong function signatures
# 3. Missing derive traits
# 4. Type mismatches

# Add missing dependencies to Cargo.toml if needed
```

### If tests pass but implementation incorrect

```bash
# This shouldn't happen if tests from Phase 05 are good
# But verify manually:

# 1. Check broadcast channel actually created
grep -r "broadcast::channel" src/events/event_bus.rs

# 2. Check send() actually called
grep -r "\.send(" src/events/event_bus.rs

# 3. Check OnceLock actually used
grep -r "OnceLock" src/events/global.rs

# 4. Check logging present
grep -r "info!" src/events/event_bus.rs
```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P06.md`

Contents:

```markdown
Phase: P06
Completed: YYYY-MM-DD HH:MM
Files Modified:
  - src/events/event_bus.rs (implemented)
  - src/events/global.rs (implemented)
  - src/events/error.rs (verified)
  - src/events/types.rs (verified)
Files Created: 0 (implementation phase)
Tests Added: 0 (tests from P05 used)
Test Results:
  - Compilation: PASS
  - Test execution: PASS
  - Passing tests: N (all from P05)
  - Failing tests: 0
Implementation:
  - broadcast::channel: Used
  - OnceLock: Used
  - Logging: Implemented
  - All stubs: Removed
Verification:
  - Plan markers: Present
  - Requirement markers: Present
  - Pseudocode references: Present
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 06a: Event Implementation Verification
2. Verify all tests pass
3. Verify implementation is correct (not just making tests pass)
4. Verify no stubs remain
5. Then proceed to Phase 07 (next component)

## Important Notes

- This is an IMPLEMENTATION phase
- ALL tests from Phase 05 MUST PASS
- DO NOT modify tests - only implement functionality
- Remove ALL unimplemented!() stubs
- Use real tokio::sync::broadcast implementation
- Use real OnceLock for singleton
- Add proper logging with tracing
- Follow pseudocode exactly
- If tests don't pass, fix implementation not tests
