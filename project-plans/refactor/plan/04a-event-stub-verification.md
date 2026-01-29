# Phase 04a: Event Stub Verification

## Phase ID

`PLAN-20250125-REFACTOR.P04A`

## Prerequisites

- Required: Phase 04 (Event Stub) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P04" src/events/ | wc -l`
- Expected from previous phase:
  - `src/events/mod.rs` - Module exports
  - `src/events/event_bus.rs` - EventBus stub
  - `src/events/types.rs` - Event type enums
  - `src/events/error.rs` - EventBusError enum
  - `src/events/global.rs` - Global singleton stubs
  - `src/lib.rs` - Module declaration

## Purpose

Verify that the EventBus stub phase created all necessary files and code structure. This phase:

1. Verifies all files were created
2. Verifies code compiles (with stubs)
3. Verifies plan markers are present
4. Verifies requirement markers are present
5. Verifies stub implementations are present (not real implementation)
6. Creates verification report

**Note:** This is a VERIFICATION phase. No code is written. Only verification commands are run.

## Requirements Verified

This phase verifies the stub implementations from Phase 04:

- **REQ-019.1**: EventBus core structure defined (stub)
- **REQ-019.2**: Event type hierarchy defined (enums)
- **REQ-019.3**: Event publishing method signature defined (stub)
- **REQ-019.4**: Event subscription method signature defined (stub)
- **REQ-019.5**: Global singleton functions defined (stub)

## Verification Tasks

### Structural Verification

#### 1. File Existence Checks

```bash
# Verify all files created
echo "=== File Existence Checks ==="

test -f src/events/mod.rs && echo "[OK] mod.rs exists" || echo " mod.rs missing"
test -f src/events/event_bus.rs && echo "[OK] event_bus.rs exists" || echo " event_bus.rs missing"
test -f src/events/types.rs && echo "[OK] types.rs exists" || echo " types.rs missing"
test -f src/events/error.rs && echo "[OK] error.rs exists" || echo " error.rs missing"
test -f src/events/global.rs && echo "[OK] global.rs exists" || echo " global.rs missing"
test -f src/lib.rs && echo "[OK] lib.rs exists" || echo " lib.rs missing"

echo "Expected: All files exist"
```

#### 2. Module Declaration Verification

```bash
# Verify module is declared in lib.rs
echo "=== Module Declaration Check ==="

grep -n "pub mod events;" src/lib.rs
echo "Expected: Line with 'pub mod events;' found"

# Verify mod.rs exports required items
echo ""
echo "=== Module Exports Check ==="

grep -E "pub use (EventBus|AppEvent|EventBusError|emit|subscribe);" src/events/mod.rs
echo "Expected: 5 exports found"
```

#### 3. Compilation Verification

```bash
# Verify code compiles (stubs allowed)
echo "=== Compilation Check ==="

cargo build --lib 2>&1 | tee build.log

if [ $? -eq 0 ]; then
    echo "[OK] Build successful"
else
    echo " Build failed"
    exit 1
fi

echo "Expected: Build succeeds (with warnings about unused code OK)"
```

### Marker Verification

#### 4. Plan Marker Verification

```bash
# Verify plan markers present
echo "=== Plan Marker Check ==="

grep -r "@plan:PLAN-20250125-REFACTOR.P04" src/events/ | wc -l
echo "Expected: 15+ occurrences"

# List files with plan markers
echo ""
echo "Files with plan markers:"
grep -l "@plan:PLAN-20250125-REFACTOR.P04" src/events/*.rs
echo "Expected: All 5 files listed"
```

#### 5. Requirement Marker Verification

```bash
# Verify requirement markers present
echo "=== Requirement Marker Check ==="

grep -r "@requirement:REQ-019" src/events/ | wc -l
echo "Expected: 15+ occurrences"

# Breakdown by requirement
echo ""
echo "Requirement breakdown:"
echo "REQ-019.1:" $(grep -r "@requirement:REQ-019.1" src/events/ | wc -l)
echo "REQ-019.2:" $(grep -r "@requirement:REQ-019.2" src/events/ | wc -l)
echo "REQ-019.3:" $(grep -r "@requirement:REQ-019.3" src/events/ | wc -l)
echo "REQ-019.4:" $(grep -r "@requirement:REQ-019.4" src/events/ | wc -l)
echo "REQ-019.5:" $(grep -r "@requirement:REQ-019.5" src/events/ | wc -l)
echo "Expected: Each requirement has 2+ markers"
```

#### 6. Pseudocode Reference Verification

```bash
# Verify pseudocode references
echo "=== Pseudocode Reference Check ==="

grep -r "@pseudocode event-bus.md" src/events/ | wc -l
echo "Expected: 10+ occurrences"

# List pseudocode line references
echo ""
echo "Pseudocode line references:"
grep -r "@pseudocode" src/events/ | grep "event-bus.md"
echo "Expected: Line numbers like 'lines 10-12', 'lines 80-84', etc."
```

### Stub Implementation Verification

#### 7. Stub Method Verification

```bash
# Verify all methods are stubs
echo "=== Stub Method Check ==="

grep -r "unimplemented!" src/events/*.rs | grep -v "tests" | wc -l
echo "Expected: 8+ unimplemented!() calls (all methods are stubs)"

# List stub methods
echo ""
echo "Stub methods found:"
grep -n "unimplemented!" src/events/*.rs | grep -v "tests"
echo "Expected: new(), publish(), subscribe(), subscriber_count(), emit(), etc."
```

#### 8. No Real Implementation Verification

```bash
# Verify NO real implementation yet
echo "=== No Real Implementation Check ==="

# Check for broadcast channel creation (should NOT exist yet)
if grep -q "broadcast::channel" src/events/event_bus.rs; then
    echo " Real implementation found (broadcast::channel)"
    exit 1
else
    echo "[OK] No broadcast::channel (correct for stub phase)"
fi

# Check for OnceLock usage (should NOT exist yet)
if grep -q "OnceLock" src/events/global.rs; then
    echo " Real implementation found (OnceLock)"
    exit 1
else
    echo "[OK] No OnceLock (correct for stub phase)"
fi

echo "Expected: No real implementation, only stubs"
```

### Type Definition Verification

#### 9. EventBus Struct Verification

```bash
# Verify EventBus struct exists
echo "=== EventBus Struct Check ==="

grep -A 10 "pub struct EventBus" src/events/event_bus.rs
echo "Expected: Struct definition found (fields may be empty or placeholder)"

# Verify EventBus methods exist
echo ""
echo "EventBus methods:"
grep -n "pub fn" src/events/event_bus.rs
echo "Expected: new(), publish(), subscribe(), subscriber_count()"
```

#### 10. Event Type Verification

```bash
# Verify event type enums exist
echo "=== Event Type Enums Check ==="

echo "AppEvent variants:"
grep -A 20 "pub enum AppEvent" src/events/types.rs | grep "^[[:space:]]*[A-Z]"
echo "Expected: User, Chat, Mcp, System variants"

echo ""
echo "UserEvent variants:"
grep -A 10 "pub enum UserEvent" src/events/types.rs | grep "^[[:space:]]*[A-Z]"
echo "Expected: SendMessage, CancelRequest, OpenSettings, OpenHistory, Quit"

echo ""
echo "ChatEvent variants:"
grep -A 10 "pub enum ChatEvent" src/events/types.rs | grep "^[[:space:]]*[A-Z]"
echo "Expected: ConversationStarted, MessageReceived, ThinkingStarted, ThinkingEnded, ResponseGenerated, Error"

echo ""
echo "McpEvent variants:"
grep -A 10 "pub enum McpEvent" src/events/types.rs | grep "^[[:space:]]*[A-Z]"
echo "Expected: ServerStarting, ServerStarted, ServerFailed, ServerStopped, ToolsUpdated, ToolCalled, ToolResult"

echo ""
echo "SystemEvent variants:"
grep -A 10 "pub enum SystemEvent" src/events/types.rs | grep "^[[:space:]]*[A-Z]"
echo "Expected: Shutdown, Error, ConfigChanged"
```

#### 11. Error Type Verification

```bash
# Verify EventBusError enum exists
echo "=== EventBusError Enum Check ==="

grep -A 10 "pub enum EventBusError" src/events/error.rs
echo "Expected: NoSubscribers, ChannelClosed variants"

# Verify thiserror derive
echo ""
grep "thiserror::Error" src/events/error.rs
echo "Expected: thiserror::Error derive found"
```

### Manual Verification Checklist

#### Code Review Checks

**Review each file and answer:**

##### src/events/mod.rs
- [ ] File exists
- [ ] Exports EventBus
- [ ] Exports AppEvent
- [ ] Exports EventBusError
- [ ] Exports emit function
- [ ] Exports subscribe function
- [ ] Plan marker present
- [ ] No implementation code (only exports)

##### src/events/event_bus.rs
- [ ] File exists
- [ ] EventBus struct defined
- [ ] new() method signature correct
- [ ] publish() method signature correct
- [ ] subscribe() method signature correct
- [ ] subscriber_count() method signature correct
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-019.1 marker
- [ ] Requirement REQ-019.3 marker
- [ ] Requirement REQ-019.4 marker
- [ ] Pseudocode reference present
- [ ] No real implementation (broadcast::channel)

##### src/events/types.rs
- [ ] File exists
- [ ] AppEvent enum with all variants
- [ ] UserEvent enum with all variants
- [ ] ChatEvent enum with all variants
- [ ] McpEvent enum with all variants
- [ ] SystemEvent enum with all variants
- [ ] All derive Debug and Clone
- [ ] Plan marker present
- [ ] Requirement REQ-019.2 marker
- [ ] Pseudocode reference present

##### src/events/error.rs
- [ ] File exists
- [ ] EventBusError enum defined
- [ ] NoSubscribers variant exists
- [ ] ChannelClosed variant exists
- [ ] Derives thiserror::Error and Debug
- [ ] Plan marker present
- [ ] Pseudocode reference present

##### src/events/global.rs
- [ ] File exists
- [ ] GLOBAL_BUS static defined (placeholder type)
- [ ] init_event_bus() function signature correct
- [ ] emit() function signature correct
- [ ] subscribe() function signature correct
- [ ] All functions use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-019.5 marker
- [ ] Pseudocode reference present
- [ ] No real implementation (OnceLock, Arc)

##### src/lib.rs
- [ ] pub mod events; line added
- [ ] Plan marker comment added
- [ ] No other changes (minimal modification)

## Success Criteria

### Automated Checks

- All 5 files created (mod.rs, event_bus.rs, types.rs, error.rs, global.rs)
- lib.rs modified to declare events module
- Code compiles successfully
- 15+ plan markers found
- 15+ requirement markers found
- 8+ stub methods (unimplemented!())
- No real implementation found

### Manual Verification

- All event types defined with correct variants
- EventBus struct with all methods
- EventBusError enum with all variants
- All methods are stubs
- Plan markers present in all files
- Pseudocode references present
- Module exports correct items

## Failure Recovery

If verification fails:

### If files are missing

```bash
# Identify missing files
test -f src/events/mod.rs || echo "mod.rs missing"
test -f src/events/event_bus.rs || echo "event_bus.rs missing"
test -f src/events/types.rs || echo "types.rs missing"
test -f src/events/error.rs || echo "error.rs missing"
test -f src/events/global.rs || echo "global.rs missing"

# Re-run Phase 04 to create missing files
```

### If compilation fails

```bash
# Check build errors
cat build.log

# Fix compilation errors:
# 1. Missing imports
# 2. Syntax errors
# 3. Type mismatches
# 4. Missing derives

# DO NOT add real implementation - fix only compilation issues
```

### If markers are missing

```bash
# Find files without plan markers
for file in src/events/*.rs; do
    if ! grep -q "@plan:PLAN-20250125-REFACTOR.P04" "$file"; then
        echo "Missing plan marker: $file"
    fi
done

# Add missing markers manually
```

### If real implementation found

```bash
# This is verification phase - should NOT have implementation
# If found, revert to stub:

# Check what implementation exists
grep -rn "broadcast::channel\|OnceLock\|Arc::new" src/events/

# Revert to Phase 04 stub approach
```

## Verification Report Template

After running all verification commands, create a report:

```markdown
# Event Stub Verification Report

**Date**: YYYY-MM-DD HH:MM
**Phase**: P04A
**Previous Phase**: P04 (Event Stub)

## File Creation Status

- [ ] src/events/mod.rs - Created
- [ ] src/events/event_bus.rs - Created
- [ ] src/events/types.rs - Created
- [ ] src/events/error.rs - Created
- [ ] src/events/global.rs - Created
- [ ] src/lib.rs - Modified

## Compilation Status

- [ ] cargo build --lib - PASS/FAIL
- Build time: X.XXs
- Warnings: N
- Errors: N

## Marker Status

- Plan markers: N (expected 15+)
- Requirement markers: N (expected 15+)
- Pseudocode references: N (expected 10+)

## Stub Implementation Status

- Stub methods: N (expected 8+)
- Real implementation found: YES/NO

## Type Definition Status

- [ ] EventBus struct defined
- [ ] AppEvent enum with all variants
- [ ] UserEvent enum with all variants
- [ ] ChatEvent enum with all variants
- [ ] McpEvent enum with all variants
- [ ] SystemEvent enum with all variants
- [ ] EventBusError enum defined

## Manual Verification Status

- [ ] All files reviewed
- [ ] All signatures correct
- [ ] All stubs confirmed
- [ ] No real implementation found

## Issues Found

[List any issues discovered during verification]

## Resolution

[Any issues resolved? If yes, how?]

## Recommendation

- [ ] Proceed to Phase 05 (Event TDD)
- [ ] Re-run Phase 04 (fix issues first)
- [ ] Blocked - requires [what?]
```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P04A.md`

Contents:

```markdown
Phase: P04A
Completed: YYYY-MM-DD HH:MM
Files Verified:
  - src/events/mod.rs
  - src/events/event_bus.rs
  - src/events/types.rs
  - src/events/error.rs
  - src/events/global.rs
  - src/lib.rs
Files Created: 0 (verification phase)
Files Modified: 0 (verification phase)
Tests Added: 0 (verification phase)
Verification Results:
  - File existence: PASS (6/6 files)
  - Compilation: PASS
  - Plan markers: PASS (N markers found)
  - Requirement markers: PASS (N markers found)
  - Stub methods: PASS (N stubs found)
  - No real implementation: PASS
Report: event-stub-verification-report.md
```

## Next Steps

After successful verification:

1. Verification report confirms all stubs are in place
2. Proceed to Phase 05: Event TDD (write tests)
3. Tests will fail against stubs (expected)
4. Phase 06 will implement to make tests pass

## Important Notes

- This is a VERIFICATION ONLY phase
- DO NOT write any code
- DO NOT fix any issues found (document them in report)
- IF issues found, decide: proceed with issues or revert to P04
- Stub methods with `unimplemented!()` are EXPECTED and CORRECT
- Real implementation should NOT be present
