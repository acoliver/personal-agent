# Phase 05a: Event TDD Verification

## Phase ID

`PLAN-20250125-REFACTOR.P05A`

## Prerequisites

- Required: Phase 05 (Event TDD) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P05" src/events/*_test.rs | wc -l`
- Expected from previous phase:
  - `src/events/event_bus_test.rs` - EventBus unit tests
  - `src/events/types_test.rs` - Event type tests
  - `src/events/global_test.rs` - Global singleton tests
  - `src/events/integration_test.rs` - Integration tests
  - `src/events/mod.rs` - Test module declarations

## Purpose

Verify that comprehensive tests were written for the EventBus system. This phase:

1. Verifies all test files were created
2. Verifies tests compile (but FAIL - expected TDD behavior)
3. Verifies test coverage is comprehensive
4. Verifies tests follow GIVEN/WHEN/THEN pattern
5. Verifies tests fail against stubs (no premature implementation)
6. Creates verification report

**Note:** This is a VERIFICATION phase. No code is written. Only verification commands are run.

## Requirements Verified

This phase verifies the test implementations from Phase 05:

- **REQ-020.1**: EventBus creation tests written
- **REQ-020.2**: Event publishing tests written
- **REQ-020.3**: No subscribers error tests written
- **REQ-020.4**: Subscription tests written
- **REQ-020.5**: Global singleton tests written
- **REQ-020.6**: Event type tests written

## Verification Tasks

### Structural Verification

#### 1. Test File Existence Checks

```bash
# Verify all test files created
echo "=== Test File Existence Checks ==="

test -f src/events/event_bus_test.rs && echo "[OK] event_bus_test.rs exists" || echo " event_bus_test.rs missing"
test -f src/events/types_test.rs && echo "[OK] types_test.rs exists" || echo " types_test.rs missing"
test -f src/events/global_test.rs && echo "[OK] global_test.rs exists" || echo " global_test.rs missing"
test -f src/events/integration_test.rs && echo "[OK] integration_test.rs exists" || echo " integration_test.rs missing"

echo "Expected: All 4 test files exist"
```

#### 2. Test Module Declaration Verification

```bash
# Verify test modules declared in mod.rs
echo "=== Test Module Declaration Check ==="

grep "#\[cfg(test)\]" src/events/mod.rs | wc -l
echo "Expected: 4+ cfg(test) attributes"

grep "mod.*_test;" src/events/mod.rs
echo "Expected: 4 test module declarations"

# Check specific modules
echo ""
echo "Specific module checks:"
grep "mod event_bus_test;" src/events/mod.rs && echo "[OK] event_bus_test" || echo " event_bus_test missing"
grep "mod types_test;" src/events/mod.rs && echo "[OK] types_test" || echo " types_test missing"
grep "mod global_test;" src/events/mod.rs && echo "[OK] global_test" || echo " global_test missing"
grep "mod integration_test;" src/events/mod.rs && echo "[OK] integration_test" || echo " integration_test missing"
```

#### 3. Test Compilation Verification

```bash
# Verify tests compile
echo "=== Test Compilation Check ==="

cargo test --lib events --no-run 2>&1 | tee compile.log

if [ $? -eq 0 ]; then
    echo "[OK] Tests compile successfully"
else
    echo " Tests fail to compile"
    cat compile.log
    exit 1
fi

echo "Expected: Tests compile (implementation may be incomplete)"
```

### Marker Verification

#### 4. Plan Marker Verification

```bash
# Verify plan markers in test files
echo "=== Plan Marker Check ==="

grep -r "@plan:PLAN-20250125-REFACTOR.P05" src/events/*_test.rs | wc -l
echo "Expected: 20+ occurrences (all tests tagged)"

# List files with plan markers
echo ""
echo "Files with plan markers:"
for file in src/events/*_test.rs; do
    count=$(grep "@plan:PLAN-20250125-REFACTOR.P05" "$file" | wc -l)
    echo "$file: $count markers"
done
echo "Expected: Each file has 5+ markers"
```

#### 5. Requirement Marker Verification

```bash
# Verify requirement markers in test files
echo "=== Requirement Marker Check ==="

grep -r "@requirement:REQ-020" src/events/*_test.rs | wc -l
echo "Expected: 20+ occurrences"

# Breakdown by requirement
echo ""
echo "Requirement breakdown:"
for req in REQ-020.1 REQ-020.2 REQ-020.3 REQ-020.4 REQ-020.5 REQ-020.6; do
    count=$(grep -r "@requirement:$req" src/events/*_test.rs | wc -l)
    echo "$req: $count markers"
done
echo "Expected: Each requirement has 3+ markers"
```

#### 6. Pseudocode Reference Verification

```bash
# Verify pseudocode references in test files
echo "=== Pseudocode Reference Check ==="

grep -r "@pseudocode" src/events/*_test.rs | wc -l
echo "Expected: 10+ occurrences"

# Show sample references
echo ""
echo "Sample pseudocode references:"
grep -r "@pseudocode" src/events/*_test.rs | head -5
echo "Expected: References to event-bus.md with line numbers"
```

### Test Coverage Verification

#### 7. Test Count Verification

```bash
# Count total tests
echo "=== Test Count Check ==="

grep -r "#\[test\]" src/events/*_test.rs | wc -l
echo "Expected: 15+ tests"

# Count test functions
grep -r "^fn test_" src/events/*_test.rs | wc -l
echo "Expected: 15+ test functions"

# Breakdown by file
echo ""
echo "Tests per file:"
for file in src/events/*_test.rs; do
    count=$(grep "^fn test_" "$file" | wc -l)
    echo "$(basename $file): $count tests"
done
echo "Expected: Each file has 3+ tests"
```

#### 8. GIVEN/WHEN/THEN Pattern Verification

```bash
# Verify behavioral test pattern
echo "=== GIVEN/WHEN/THEN Pattern Check ==="

grep -r "GIVEN:" src/events/*_test.rs | wc -l
echo "GIVEN comments: (expected 15+)"

grep -r "WHEN:" src/events/*_test.rs | wc -l
echo "WHEN comments: (expected 15+)"

grep -r "THEN:" src/events/*_test.rs | wc -l
echo "THEN comments: (expected 30+)"

# Verify all three in same test
echo ""
echo "Tests with full GIVEN/WHEN/THEN:"
for file in src/events/*_test.rs; do
    # Count tests with all three
    count=$(awk '/^fn test_/ {test=1} test && /GIVEN:/ && /WHEN:/ && /THEN:/ {print; test=0}' "$file" | wc -l)
    echo "$(basename $file): $count complete tests"
done
echo "Expected: Most tests have all three comments"
```

#### 9. Test Failure Verification (Critical for TDD)

```bash
# Verify tests FAIL (TDD requirement)
echo "=== Test Failure Check (TDD Verification) ==="

cargo test --lib events 2>&1 | tee test_run.log

# Check for test failures
if grep -q "test result: FAILED" test_run.log; then
    echo "[OK] Tests FAIL as expected (TDD phase correct)"
else
    echo " Tests PASS unexpectedly - possible premature implementation"
    exit 1
fi

# Count failing tests
grep -o "test .* FAILED" test_run.log | wc -l
echo "Expected: 10+ failing tests (all should fail against stubs)"

# Verify failures are due to unimplemented!(), not compilation
if grep -q "not implemented" test_run.log; then
    echo "[OK] Failures due to unimplemented!() (correct)"
else
    echo " Warning: Failures may not be from stubs"
fi
```

### Manual Verification Checklist

#### Test Content Review

**Review each test file and answer:**

##### src/events/event_bus_test.rs
- [ ] File exists
- [ ] test_event_bus_creation exists
- [ ] test_single_subscription exists
- [ ] test_multiple_subscriptions exists
- [ ] test_publish_to_single_subscriber exists
- [ ] test_publish_to_multiple_subscribers exists
- [ ] test_publish_no_subscribers_error exists
- [ ] test_publish_all_event_types exists
- [ ] Each test has GIVEN/WHEN/THEN comments
- [ ] Each test has plan marker
- [ ] Each test has requirement marker
- [ ] Tests will fail against stubs (unimplemented!)
- [ ] Tests verify single behavior each
- [ ] Assertions are specific (not just "runs without error")

##### src/events/types_test.rs
- [ ] File exists
- [ ] test_event_types_create exists
- [ ] test_events_can_be_cloned exists
- [ ] test_events_can_be_debugged exists
- [ ] test_app_event_wrapping exists
- [ ] All event variants covered
- [ ] Tests verify Clone trait works
- [ ] Tests verify Debug trait works
- [ ] Tests verify type hierarchy
- [ ] Plan markers present
- [ ] Tests will fail (or pass if Clone/Debug derived)

##### src/events/global_test.rs
- [ ] File exists
- [ ] test_global_singleton_lazy_init exists
- [ ] test_global_singleton_same_instance exists
- [ ] test_global_subscribe exists
- [ ] test_global_emit_no_subscribers exists
- [ ] Tests verify singleton pattern
- [ ] Tests verify lazy initialization
- [ ] Tests verify global emit/subscribe
- [ ] Plan markers present
- [ ] Tests will fail against stubs

##### src/events/integration_test.rs
- [ ] File exists
- [ ] test_end_to_end_event_flow exists
- [ ] test_multiple_components_receive_events exists
- [ ] test_event_ordering_preserved exists
- [ ] Tests verify real-world scenarios
- [ ] Tests use global emit/subscribe
- [ ] Tests verify complete event lifecycle
- [ ] Plan markers present
- [ ] Tests will fail against stubs

##### src/events/mod.rs
- [ ] All 4 test modules declared
- [ ] Test modules use #[cfg(test)]
- [ ] Plan marker comment added

### Coverage Analysis

#### 10. Behavioral Coverage Verification

```bash
# Verify all behaviors from pseudocode are tested
echo "=== Behavioral Coverage Check ==="

echo "EventBus creation (lines 20-23):"
grep -r "EventBus::new" src/events/*_test.rs | wc -l
echo "Expected: 2+ tests"

echo ""
echo "EventBus publish (lines 30-38):"
grep -r "publish(" src/events/*_test.rs | wc -l
echo "Expected: 4+ tests"

echo ""
echo "EventBus subscribe (lines 40-41):"
grep -r "subscribe(" src/events/*_test.rs | wc -l
echo "Expected: 3+ tests"

echo ""
echo "EventBus subscriber_count (lines 45-46):"
grep -r "subscriber_count(" src/events/*_test.rs | wc -l
echo "Expected: 5+ tests (used in multiple tests)"

echo ""
echo "Global singleton (lines 50-75, 150-156):"
grep -r "emit(" src/events/*_test.rs | wc -l
echo "Expected: 3+ tests"

echo ""
echo "Event types (lines 80-123):"
grep -r "AppEvent::" src/events/*_test.rs | wc -l
echo "Expected: 10+ references (all variants tested)"
```

#### 11. Error Path Coverage

```bash
# Verify error cases are tested
echo "=== Error Path Coverage Check ==="

grep -r "NoSubscribers" src/events/*_test.rs | wc -l
echo "Expected: 2+ tests for no subscribers error"

grep -r "unwrap_err\|expect_err" src/events/*_test.rs | wc -l
echo "Expected: 3+ assertions on error returns"
```

## Success Criteria

### Automated Checks

- All 4 test files created
- 15+ tests written
- Tests compile successfully
- Tests FAIL when run (expected TDD behavior)
- 20+ plan markers found
- 20+ requirement markers found
- GIVEN/WHEN/THEN pattern in most tests
- Failures due to unimplemented!() (not compilation errors)

### Manual Verification

- All EventBus methods tested
- All event types tested
- All error paths tested
- Global singleton behavior tested
- Integration scenarios tested
- Tests follow behavioral pattern
- No premature implementation

## Failure Recovery

If verification fails:

### If test files missing

```bash
# Identify missing files
ls -la src/events/*_test.rs

# Re-run Phase 05 to create missing tests
```

### If tests don't compile

```bash
# Check compilation errors
cat compile.log

# Common fixes:
# 1. Missing test module declarations in mod.rs
# 2. Syntax errors in tests
# 3. Missing imports
# 4. Wrong function signatures in tests (must match stubs)

# DO NOT modify stub implementation - fix tests only
```

### If tests pass unexpectedly

```bash
# This means stubs have too much implementation
# Or tests don't actually verify behavior

# Check what's implemented
grep -r "broadcast::channel\|OnceLock" src/events/*.rs | grep -v test

# Check if tests have real assertions
grep -r "assert" src/events/*_test.rs | wc -l
# Expected: 30+ assertions

# If tests passing, review:
# 1. Are assertions present?
# 2. Do assertions verify real behavior?
# 3. Are stubs returning real values instead of panicking?
```

### If test coverage insufficient

```bash
# Analyze coverage
grep -r "^fn test_" src/events/*_test.rs | wc -l

# If < 15 tests, identify gaps:
# - Which EventBus methods untested?
# - Which event types untested?
# - Which error cases untested?
# - Which integration scenarios missing?

# Add missing tests in Phase 05 (re-run)
```

## Verification Report Template

After running all verification commands, create a report:

```markdown
# Event TDD Verification Report

**Date**: YYYY-MM-DD HH:MM
**Phase**: P05A
**Previous Phase**: P05 (Event TDD)

## Test File Status

- [ ] src/events/event_bus_test.rs - Created (N tests)
- [ ] src/events/types_test.rs - Created (N tests)
- [ ] src/events/global_test.rs - Created (N tests)
- [ ] src/events/integration_test.rs - Created (N tests)
- [ ] src/events/mod.rs - Modified (test modules declared)

## Test Compilation Status

- [ ] Tests compile - PASS/FAIL
- Compile time: X.XXs
- Warnings: N

## Test Execution Status

- [ ] Tests run - PASS/FAIL
- [ ] Tests FAIL (expected for TDD) - YES/NO
- Failing tests: N
- Passing tests: N (should be 0 or minimal)

## Coverage Analysis

- Total tests: N
- EventBus tests: N
- Event type tests: N
- Global singleton tests: N
- Integration tests: N

## Behavioral Coverage

- [ ] EventBus creation tested
- [ ] Event publishing tested
- [ ] Event subscription tested
- [ ] Error paths tested
- [ ] Global singleton tested
- [ ] All event types tested
- [ ] Integration scenarios tested

## Test Quality

- GIVEN/WHEN/THEN pattern: N% of tests
- Plan markers: N found
- Requirement markers: N found
- Pseudocode references: N found

## Issues Found

[List any issues discovered during verification]

## Resolution

[Any issues resolved? If yes, how?]

## TDD Compliance

- [ ] Tests fail against stubs (correct TDD)
- [ ] No premature implementation found
- [ ] Tests define clear contract
- [ ] Ready for implementation phase

## Recommendation

- [ ] Proceed to Phase 06 (Event Implementation)
- [ ] Re-run Phase 05 (add more tests)
- [ ] Fix specific issues first: [what?]
```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P05A.md`

Contents:

```markdown
Phase: P05A
Completed: YYYY-MM-DD HH:MM
Files Verified:
  - src/events/event_bus_test.rs (N tests)
  - src/events/types_test.rs (N tests)
  - src/events/global_test.rs (N tests)
  - src/events/integration_test.rs (N tests)
  - src/events/mod.rs
Files Created: 0 (verification phase)
Files Modified: 0 (verification phase)
Tests Added: 0 (verification phase)
Tests Verified: N total tests
Verification Results:
  - Test files: PASS (4/4)
  - Compilation: PASS
  - Test execution: FAIL (expected TDD)
  - Test count: N (expected 15+)
  - GIVEN/WHEN/THEN: PASS
  - Plan markers: PASS (N found)
  - Requirement markers: PASS (N found)
  - TDD compliance: PASS (tests fail correctly)
Report: event-tdd-verification-report.md
```

## Next Steps

After successful verification:

1. Tests verified as comprehensive
2. Tests verified as failing (correct TDD behavior)
3. Proceed to Phase 06: Event Implementation
4. Implement EventBus to make tests pass

## Important Notes

- This is a VERIFICATION ONLY phase
- DO NOT write any code
- DO NOT fix any issues found (document in report)
- Tests MUST FAIL - if they pass, something is wrong
- IF issues found, decide: proceed or re-run P05
- TDD requires tests fail before implementation
