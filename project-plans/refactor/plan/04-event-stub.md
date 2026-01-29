# Phase 04: Event System Stub Phase

## Phase ID

`PLAN-20250125-REFACTOR.P04`

## Prerequisites

- Required: Phase 03a (Pseudocode Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P03A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/03a-pseudocode-verification.md`
  - `project-plans/refactor/analysis/pseudocode/event-bus.md`
- Preflight verification: Phases 01, 01a, 02, 02a, 03, 03a completed

## Purpose

Create minimal stub implementations for the EventBus system. This phase:

1. Creates the EventBus module structure
2. Defines the EventBus struct with placeholder methods
3. Defines all event type enums
4. Sets up module exports
5. Ensures code compiles (with stub implementations)

**Note:** This is a STUB phase. Methods will return `unimplemented!()` or placeholder values. Tests will be written in Phase 05.

## Requirements Implemented (Expanded)

### REQ-019.1: EventBus Core Structure

**Full Text**: The application MUST provide a centralized EventBus using tokio::sync::broadcast for event distribution.

**Behavior**:
- GIVEN: Application is starting
- WHEN: EventBus::new() is called
- THEN: An EventBus instance is created with a broadcast channel of specified capacity

**Why This Matters**: Centralized event distribution prevents tight coupling between components.

### REQ-019.2: Event Type Hierarchy

**Full Text**: EventBus MUST support a typed event hierarchy (UserEvent, ChatEvent, McpEvent, SystemEvent).

**Behavior**:
- GIVEN: Component wants to emit a domain-specific event
- WHEN: Component creates an AppEvent variant
- THEN: Event is properly typed and categorized

**Why This Matters**: Type safety prevents accidental event misuse and enables pattern matching.

### REQ-019.3: Event Publishing

**Full Text**: EventBus MUST provide a publish() method to broadcast events to all subscribers.

**Behavior**:
- GIVEN: EventBus instance exists
- WHEN: publish(event) is called
- THEN: Event is broadcast to all active subscribers
- AND: Returns count of subscribers who received the event

**Why This Matters**: Enables loose coupling - emitters don't need to know who is listening.

### REQ-019.4: Event Subscription

**Full Text**: EventBus MUST provide a subscribe() method returning a broadcast::Receiver.

**Behavior**:
- GIVEN: Component wants to receive events
- WHEN: subscribe() is called
- THEN: A Receiver is returned that will receive all future events
- AND: Multiple subscribers can exist simultaneously

**Why This Matters**: Any component can listen to events without registration complexity.

### REQ-019.5: Global Event Bus Singleton

**Full Text**: EventBus MUST be accessible globally via a singleton pattern using OnceLock.

**Behavior**:
- GIVEN: Any code needs to emit an event
- WHEN: emit(event) is called
- THEN: EventBus is lazily initialized on first use
- AND: Same instance is reused for all subsequent calls

**Why This Matters**: Global access without manual dependency injection.

## Implementation Tasks

### Files to Create

- `src/events/mod.rs`
  - Module declaration file
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P04`
  - Exports: EventBus, AppEvent, EventBusError, emit, subscribe
  - Implements: `@requirement:REQ-019.5`

- `src/events/event_bus.rs`
  - EventBus struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P04`
  - Implements: `@requirement:REQ-019.1`, `@requirement:REQ-019.3`, `@requirement:REQ-019.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 10-46

- `src/events/types.rs`
  - Event type enums (AppEvent, UserEvent, ChatEvent, McpEvent, SystemEvent)
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P04`
  - Implements: `@requirement:REQ-019.2`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 80-123

- `src/events/error.rs`
  - EventBusError enum
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P04`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 160-162

- `src/events/global.rs`
  - Global singleton functions (emit, subscribe, init_event_bus)
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P04`
  - Implements: `@requirement:REQ-019.5`
  - Reference: `project-plans/refactor/analysis/pseudocode/event-bus.md` lines 50-75, 150-156

### Files to Modify

- `src/lib.rs`
  - ADD line: `pub mod events;`
  - ADD comment: `@plan:PLAN-20250125-REFACTOR.P04`
  - Makes events module available to crate

- `src/main.rs`
  - No changes in this phase (integration happens in Phase 07)
  - Note: Module must compile but not yet integrated

### Required Code Markers

Every struct/enum/function created in this phase MUST include:

```rust
/// EventBus stub implementation
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.1
/// @pseudocode event-bus.md lines 10-12
pub struct EventBus {
    // Stub fields
}
```

### Stub Implementation Guidelines

**In this phase, ALL methods should use stub implementations:**

```rust
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.1
impl EventBus {
    pub fn new(capacity: usize) -> Self {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn publish(&self, event: AppEvent) -> Result<usize, EventBusError> {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn subscriber_count(&self) -> usize {
        // STUB: Return placeholder
        unimplemented!()
    }
}
```

## Pseudocode References

### EventBus Core (event_bus.rs)
- Lines 10-12: EventBus struct definition
- Lines 20-23: new() constructor
- Lines 30-38: publish() method
- Lines 40-41: subscribe() method
- Lines 45-46: subscriber_count() method

### Global Singleton (global.rs)
- Lines 50-60: GLOBAL_BUS static and init_event_bus()
- Lines 65-69: emit() function
- Lines 73-75: subscribe() function
- Lines 150-156: get_or_init_event_bus() helper

### Event Types (types.rs)
- Lines 80-84: AppEvent enum
- Lines 90-95: UserEvent enum
- Lines 100-106: ChatEvent enum
- Lines 110-117: McpEvent enum
- Lines 120-123: SystemEvent enum

### Error Types (error.rs)
- Lines 160-162: EventBusError enum

## Verification Commands

### Structural Verification

```bash
# Check module file exists
test -f src/events/mod.rs
echo "Expected: File exists"

# Check all source files created
test -f src/events/event_bus.rs
test -f src/events/types.rs
test -f src/events/error.rs
test -f src/events/global.rs
echo "Expected: All files exist"

# Check plan markers in events module
grep -r "@plan:PLAN-20250125-REFACTOR.P04" src/events/ | wc -l
# Expected: 10+ occurrences (all structs, enums, functions)

# Check requirement markers
grep -r "@requirement:REQ-019" src/events/ | wc -l
# Expected: 10+ occurrences (all components tagged)

# Check module is exported in lib.rs
grep "pub mod events;" src/lib.rs
# Expected: Line found

# Verify code compiles (STUB compilation allowed)
cargo build --lib 2>&1 | tee build.log
# Expected: Compiles successfully (may have warnings about unused code)
```

### Stub Detection (EXPECTED in this phase)

```bash
# Verify stub methods exist (unimplemented! is OK in this phase)
grep -r "unimplemented!" src/events/*.rs | grep -v "tests"
# Expected: 5+ matches (all methods are stubs)

# Verify no real implementation exists yet
grep -r "broadcast::channel" src/events/*.rs | grep -v "tests"
# Expected: 0 matches (real implementation in Phase 06)
```

### Manual Verification Checklist

Read each file and verify:

#### src/events/mod.rs
- [ ] Module exports EventBus
- [ ] Module exports AppEvent
- [ ] Module exports EventBusError
- [ ] Module exports emit function
- [ ] Module exports subscribe function
- [ ] Plan marker present

#### src/events/event_bus.rs
- [ ] EventBus struct defined
- [ ] new() method defined (stub)
- [ ] publish() method defined (stub)
- [ ] subscribe() method defined (stub)
- [ ] subscriber_count() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement markers present

#### src/events/types.rs
- [ ] AppEvent enum defined with all variants
- [ ] UserEvent enum defined with all variants
- [ ] ChatEvent enum defined with all variants
- [ ] McpEvent enum defined with all variants
- [ ] SystemEvent enum defined with all variants
- [ ] All events derive Debug and Clone
- [ ] Plan marker present

#### src/events/error.rs
- [ ] EventBusError enum defined
- [ ] NoSubscribers variant exists
- [ ] ChannelClosed variant exists
- [ ] Error derives thiserror::Error
- [ ] Plan marker present

#### src/events/global.rs
- [ ] GLOBAL_BUS static defined
- [ ] init_event_bus() function defined (stub)
- [ ] emit() function defined (stub)
- [ ] subscribe() function defined (stub)
- [ ] All functions use unimplemented!()
- [ ] Plan marker present

#### src/lib.rs
- [ ] pub mod events; line added
- [ ] Plan marker comment added

## Success Criteria

- All event module files created
- Code compiles successfully
- All methods are stubs (unimplemented!())
- All structs and enums defined
- Plan markers present in all files
- Requirement markers traceable
- Module exported in lib.rs

## Failure Recovery

If this phase fails:

1. Rollback commands:
   ```bash
   git checkout -- src/events/ src/lib.rs
   rm -rf src/events/
   ```

2. Files to revert:
   - src/events/mod.rs
   - src/events/event_bus.rs
   - src/events/types.rs
   - src/events/error.rs
   - src/events/global.rs
   - src/lib.rs

3. Cannot proceed to Phase 04a until stub structure compiles

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P04.md`

Contents:

```markdown
Phase: P04
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/events/mod.rs (N lines)
  - src/events/event_bus.rs (N lines)
  - src/events/types.rs (N lines)
  - src/events/error.rs (N lines)
  - src/events/global.rs (N lines)
Files Modified:
  - src/lib.rs (+1 line)
Tests Added: 0 (stub phase)
Verification:
  - cargo build --lib: PASS
  - Plan markers: 15+ found
  - Requirement markers: 15+ found
  - All methods: Stub (unimplemented!)
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 04a: Event Stub Verification
2. Verify stub structure compiles and all types are defined
3. Then proceed to Phase 05: Event TDD (write tests)

## Important Notes

- This is a STUB phase - no real implementation
- All methods should use `unimplemented!()`
- Compilation is required, but functionality is not
- Next phase will write tests that fail against these stubs
- Phase 06 will implement real functionality to make tests pass
