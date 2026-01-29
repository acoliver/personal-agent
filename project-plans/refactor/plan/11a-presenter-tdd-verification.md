# Phase 11a: Presenter Layer TDD Verification

## Phase ID

`PLAN-20250125-REFACTOR.P11A`

## Prerequisites

- Required: Phase 11 (Presenter TDD) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P11" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P11.md`
  - All 4 presenter test modules created
  - Tests compile and run (failures expected)
- Preflight verification: Phases 01-11 completed

## Purpose

Verify that all presenter layer tests are properly written and ready for implementation phase. This phase:

1. Confirms all test files exist and compile
2. Verifies test coverage meets requirements
3. Validates test quality (event routing, ViewCommand emission)
4. Confirms tests fail against stubs (TDD red state)
5. Documents test results for Phase 12 implementation

**Note:** This is a VERIFICATION phase. No code changes expected. Tests should FAIL against stubs.

## Requirements Verified

### REQ-026.1: Presenter Unit Test Coverage
- Verify coverage >= 80% of presenter methods
- Verify all event handlers tested
- Verify ViewCommand emission verified

### REQ-026.2: ChatPresenter Tests
- Verify UserEvent routing tested
- Verify ChatEvent routing tested
- Verify ViewCommands verified
- Verify event loop tested

### REQ-026.3: McpPresenter Tests
- Verify UserEvent routing tested
- Verify McpEvent routing tested
- Verify ViewCommands verified

### REQ-026.4: SettingsPresenter Tests
- Verify UserEvent routing tested
- Verify profile management tested

### REQ-026.5: ErrorPresenter Tests
- Verify error routing tested
- Verify all error types tested

## Verification Tasks

### Structural Verification

```bash
# Verify all test files exist
echo "=== Checking test file existence ==="
test -f src/presentation/chat_test.rs && echo "[OK] chat_test.rs exists" || echo " chat_test.rs missing"
test -f src/presentation/mcp_test.rs && echo "[OK] mcp_test.rs exists" || echo " mcp_test.rs missing"
test -f src/presentation/settings_test.rs && echo "[OK] settings_test.rs exists" || echo " settings_test.rs missing"
test -f src/presentation/error_test.rs && echo "[OK] error_test.rs exists" || echo " error_test.rs missing"

# Verify test modules declared in mod.rs
echo ""
echo "=== Checking test module declarations ==="
grep "mod chat_test;" src/presentation/mod.rs && echo "[OK] chat_test declared" || echo " chat_test not declared"
grep "mod mcp_test;" src/presentation/mod.rs && echo "[OK] mcp_test declared" || echo " mcp_test not declared"
grep "mod settings_test;" src/presentation/mod.rs && echo "[OK] settings_test declared" || echo " settings_test not declared"
grep "mod error_test;" src/presentation/mod.rs && echo "[OK] error_test declared" || echo " error_test not declared"
```

### Plan Marker Verification

```bash
# Verify plan markers in test files
echo ""
echo "=== Checking plan markers ==="
grep -r "@plan:PLAN-20250125-REFACTOR.P11" src/presentation/*_test.rs | wc -l
echo "Expected: 40+ occurrences"

# List files with plan markers
for file in src/presentation/*_test.rs; do
  count=$(grep "@plan:PLAN-20250125-REFACTOR.P11" "$file" | wc -l)
  echo "$(basename $file): $count markers"
done

# Verify requirement markers
echo ""
echo "=== Checking requirement markers ==="
grep -r "@requirement:REQ-026" src/presentation/*_test.rs | wc -l
echo "Expected: 20+ occurrences"
```

### Test Compilation Verification

```bash
# Verify tests compile
echo ""
echo "=== Compiling tests ==="
cargo test --lib --no-run 2>&1 | tee compile_test.log

# Check for compilation errors
if grep -i "error:" compile_test.log; then
  echo " Compilation errors found"
  grep "error\[" compile_test.log
  exit 1
else
  echo "[OK] Tests compile successfully"
fi
```

### Test Execution Verification (EXPECTED FAILURES)

```bash
# Run tests (EXPECTED: Many failures)
echo ""
echo "=== Running tests (expected failures) ==="
cargo test --lib -- --test-threads=1 2>&1 | tee test_run.log

# Extract test summary
echo ""
echo "=== Test Summary ==="
grep -E "test result:" test_run.log | tail -1

# Count total tests
total_tests=$(grep -E "test [a-z_]+" test_run.log | wc -l)
echo "Total tests: $total_tests"

# Count passed tests
passed=$(grep -E "test [a-z_]+ \.\.\. ok" test_run.log | wc -l)
echo "Passed: $passed"

# Count failed tests
failed=$(grep -E "test [a-z_]+ \.\.\. FAILED" test_run.log | wc -l)
echo "Failed: $failed (EXPECTED against stubs)"

# Count ignored tests
ignored=$(grep -E "test [a-z_]+ \.\.\. ignored" test_run.log | wc -l)
echo "Ignored: $ignored"
```

### Manual Verification Checklist

#### src/presentation/chat_test.rs
```bash
echo ""
echo "=== Verifying chat_test.rs ==="
grep "test_new" src/presentation/chat_test.rs && echo "[OK] test_new exists" || echo " Missing"
grep "test_start_stop_lifecycle" src/presentation/chat_test.rs && echo "[OK] test_start_stop_lifecycle exists" || echo " Missing"
grep "test_handle_user_event" src/presentation/chat_test.rs && echo "[OK] test_handle_user_event exists" || echo " Missing"
grep "test_handle_chat_event" src/presentation/chat_test.rs && echo "[OK] test_handle_chat_event exists" || echo " Missing"
grep "view_rx.recv" src/presentation/chat_test.rs && echo "[OK] ViewCommand emission verified" || echo " ViewCommand verification missing"
```

#### src/presentation/mcp_test.rs
```bash
echo ""
echo "=== Verifying mcp_test.rs ==="
grep "test_new" src/presentation/mcp_test.rs && echo "[OK] test_new exists" || echo " Missing"
grep "test_start_stop_lifecycle" src/presentation/mcp_test.rs && echo "[OK] test_start_stop_lifecycle exists" || echo " Missing"
grep "test_handle_user_event" src/presentation/mcp_test.rs && echo "[OK] test_handle_user_event exists" || echo " Missing"
grep "test_handle_mcp_event" src/presentation/mcp_test.rs && echo "[OK] test_handle_mcp_event exists" || echo " Missing"
```

#### src/presentation/settings_test.rs
```bash
echo ""
echo "=== Verifying settings_test.rs ==="
grep "test_new" src/presentation/settings_test.rs && echo "[OK] test_new exists" || echo " Missing"
grep "test_start_stop_lifecycle" src/presentation/settings_test.rs && echo "[OK] test_start_stop_lifecycle exists" || echo " Missing"
grep "test_handle_user_event" src/presentation/settings_test.rs && echo "[OK] test_handle_user_event exists" || echo " Missing"
```

#### src/presentation/error_test.rs
```bash
echo ""
echo "=== Verifying error_test.rs ==="
grep "test_new" src/presentation/error_test.rs && echo "[OK] test_new exists" || echo " Missing"
grep "test_start_stop_lifecycle" src/presentation/error_test.rs && echo "[OK] test_start_stop_lifecycle exists" || echo " Missing"
grep "test_handle.*error" src/presentation/error_test.rs && echo "[OK] test_handle_error exists" || echo " Missing"
```

### Test Quality Verification

```bash
# Check for async test attribute
echo ""
echo "=== Checking async test attributes ==="
grep "#\[tokio::test\]" src/presentation/*_test.rs | wc -l
echo "Expected: 20+ async tests"

# Check for ViewCommand verification
echo ""
echo "=== Checking ViewCommand verification patterns ==="
grep "view_rx.recv" src/presentation/*_test.rs | wc -l
echo "Expected: 15+ ViewCommand verification patterns"

# Check for event routing tests
echo ""
echo "=== Checking event routing tests ==="
grep "handle_event" src/presentation/*_test.rs | wc -l
echo "Expected: 10+ event routing tests"
```

## Success Criteria

- All 4 test files created and compile
- Tests run successfully (even with failures)
- Test coverage >= 80% of presenter methods
- All event routing paths tested
- All ViewCommand emissions tested
- All error paths tested
- Plan markers present in all test files
- Requirement markers traceable

## Expected Test Failures

**Document expected failures in Phase 11 (against stubs):**

```
chat_test:
- test_start_stop_lifecycle: FAIL (unimplemented!)
- test_handle_user_event_send_message: FAIL (unimplemented!)
- test_handle_chat_event_conversation_started: FAIL (unimplemented!)
- test_handle_chat_event_message_received: FAIL (unimplemented!)

mcp_test:
- test_start_stop_lifecycle: FAIL (unimplemented!)
- test_handle_user_event_start_server: FAIL (unimplemented!)
- test_handle_mcp_event_server_started: FAIL (unimplemented!)

settings_test:
- test_start_stop_lifecycle: FAIL (unimplemented!)
- test_handle_user_event_open_settings: FAIL (unimplemented!)

error_test:
- test_start_stop_lifecycle: FAIL (unimplemented!)
- test_handle_system_error: FAIL (unimplemented!)
```

**These failures are EXPECTED and will be fixed in Phase 12.**

## Failure Recovery

If verification fails:

1. If tests don't compile:
   - Fix compilation errors in test files
   - Ensure all imports are correct
   - Verify test module declarations in mod.rs

2. If tests are missing:
   - Create missing test files
   - Add missing test functions
   - Verify coverage meets requirements

3. If tests pass (unexpected):
   - This is acceptable (some tests may pass against stubs)
   - Proceed to Phase 12

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P11A.md`

Contents:

```markdown
Phase: P11A
Completed: YYYY-MM-DD HH:MM
Verification Results:
  Test Files: 4/4 created
  Test Modules: 4/4 declared
  Plan Markers: 40+ found
  Requirement Markers: 20+ found
  Compilation: PASS
  Tests Run: YES
  Total Tests: 40+
  Passed: X
  Failed: Y (EXPECTED against stubs)
  Coverage: Z%
Quality Checks:
  - Event routing verified: YES
  - ViewCommand emission verified: YES
  - Error paths tested: YES
  - Async handling: YES
  - AAA pattern: YES
Ready for Phase 12: YES
```

## Next Steps

After successful verification:

1. All tests should be in place and compiling
2. Test failures should be documented (expected against stubs)
3. Proceed to Phase 12: Presenter Implementation
4. Implement real functionality to make tests pass

## Important Notes

- This is a VERIFICATION phase - no code changes expected
- Test failures are EXPECTED and DOCUMENTED
- Coverage will improve after Phase 12 implementation
- Next phase will implement real functionality
- All tests should pass after Phase 12
- Presenters are event-driven
- ViewCommands must be emitted correctly
