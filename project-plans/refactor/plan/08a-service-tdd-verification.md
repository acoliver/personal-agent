# Phase 08a: Service Layer TDD Verification

## Phase ID

`PLAN-20250125-REFACTOR.P08A`

## Prerequisites

- Required: Phase 08 (Service TDD) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P08" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P08.md`
  - All 5 service test modules created
  - Tests compile and run (failures expected)
- Preflight verification: Phases 01-08 completed

## Purpose

Verify that all service layer tests are properly written and ready for implementation phase. This phase:

1. Confirms all test files exist and compile
2. Verifies test coverage meets requirements
3. Validates test quality (event emission, error paths)
4. Confirms tests fail against stubs (TDD red state)
5. Documents test results for Phase 09 implementation

**Note:** This is a VERIFICATION phase. No code changes expected. Tests should FAIL against stubs.

## Requirements Verified

### REQ-023.1: Service Unit Test Coverage
- Verify coverage >= 80% of public APIs
- Verify all error paths tested
- Verify event emission verified

### REQ-023.2: ConversationService Tests
- Verify create_conversation() tested
- Verify send_message() tested
- Verify get_conversation() tested
- Verify set_active() tested
- Verify event emission verified

### REQ-023.3: ChatService Tests
- Verify send_message_stream() tested
- Verify build_message_history() tested
- Verify thinking events verified
- Verify stream events verified

### REQ-023.4: McpService Tests
- Verify start_server() tested
- Verify stop_server() tested
- Verify call_tool() tested
- Verify tool registry updates verified

### REQ-023.5: ProfileService Tests
- Verify add_profile() tested
- Verify get_profile() tested
- Verify update_profile() tested
- Verify persistence verified

### REQ-023.6: SecretsService Tests
- Verify get_api_key() tested
- Verify set_api_key() tested
- Verify security verified (no plaintext leaks)

## Verification Tasks

### Structural Verification

```bash
# Verify all test files exist
echo "=== Checking test file existence ==="
test -f src/services/conversation_test.rs && echo "[OK] conversation_test.rs exists" || echo " conversation_test.rs missing"
test -f src/services/chat_test.rs && echo "[OK] chat_test.rs exists" || echo " chat_test.rs missing"
test -f src/services/mcp_test.rs && echo "[OK] mcp_test.rs exists" || echo " mcp_test.rs missing"
test -f src/services/profile_test.rs && echo "[OK] profile_test.rs exists" || echo " profile_test.rs missing"
test -f src/services/secrets_test.rs && echo "[OK] secrets_test.rs exists" || echo " secrets_test.rs missing"

# Verify test modules declared in mod.rs
echo ""
echo "=== Checking test module declarations ==="
grep "mod conversation_test;" src/services/mod.rs && echo "[OK] conversation_test declared" || echo " conversation_test not declared"
grep "mod chat_test;" src/services/mod.rs && echo "[OK] chat_test declared" || echo " chat_test not declared"
grep "mod mcp_test;" src/services/mod.rs && echo "[OK] mcp_test declared" || echo " mcp_test not declared"
grep "mod profile_test;" src/services/mod.rs && echo "[OK] profile_test declared" || echo " profile_test not declared"
grep "mod secrets_test;" src/services/mod.rs && echo "[OK] secrets_test declared" || echo " secrets_test not declared"
```

### Plan Marker Verification

```bash
# Verify plan markers in test files
echo ""
echo "=== Checking plan markers ==="
grep -r "@plan:PLAN-20250125-REFACTOR.P08" src/services/*_test.rs | wc -l
echo "Expected: 50+ occurrences"

# List files with plan markers
for file in src/services/*_test.rs; do
  count=$(grep "@plan:PLAN-20250125-REFACTOR.P08" "$file" | wc -l)
  echo "$(basename $file): $count markers"
done

# Verify requirement markers
echo ""
echo "=== Checking requirement markers ==="
grep -r "@requirement:REQ-023" src/services/*_test.rs | wc -l
echo "Expected: 30+ occurrences"

# List requirement markers by file
for file in src/services/*_test.rs; do
  count=$(grep "@requirement:REQ-023" "$file" | wc -l)
  echo "$(basename $file): $count requirement markers"
done
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

### Coverage Verification

```bash
# Generate coverage report
echo ""
echo "=== Generating coverage report ==="
cargo tarpaulin --lib --output-dir coverage --output Html 2>&1 | tee coverage.log

# Extract coverage percentage
echo ""
echo "=== Coverage Summary ==="
grep -E "^\s*[0-9]+\.[0-9]+% coverage" coverage.log || echo "Coverage not available (may need cargo-tarpaulin install)"
```

### Manual Verification Checklist

#### src/services/conversation_test.rs
```bash
echo ""
echo "=== Verifying conversation_test.rs ==="
grep "test_create_conversation" src/services/conversation_test.rs && echo "[OK] test_create_conversation exists" || echo " Missing"
grep "test_send_message" src/services/conversation_test.rs && echo "[OK] test_send_message exists" || echo " Missing"
grep "test_get_conversation" src/services/conversation_test.rs && echo "[OK] test_get_conversation exists" || echo " Missing"
grep "test_set_active" src/services/conversation_test.rs && echo "[OK] test_set_active exists" || echo " Missing"
grep "test_conversation_not_found" src/services/conversation_test.rs && echo "[OK] test_not_found exists" || echo " Missing"
grep "test_event_emission" src/services/conversation_test.rs && echo "[OK] test_event_emission exists" || echo " Missing"
grep "event_rx.recv" src/services/conversation_test.rs && echo "[OK] Event emission verified" || echo " Event verification missing"
```

#### src/services/chat_test.rs
```bash
echo ""
echo "=== Verifying chat_test.rs ==="
grep "test_send_message_stream" src/services/chat_test.rs && echo "[OK] test_send_message_stream exists" || echo " Missing"
grep "test_build_message_history" src/services/chat_test.rs && echo "[OK] test_build_message_history exists" || echo " Missing"
grep "test_thinking_events" src/services/chat_test.rs && echo "[OK] test_thinking_events exists" || echo " Missing"
grep "test_stream_events" src/services/chat_test.rs && echo "[OK] test_stream_events exists" || echo " Missing"
grep "test_error_handling" src/services/chat_test.rs && echo "[OK] test_error_handling exists" || echo " Missing"
```

#### src/services/mcp_test.rs
```bash
echo ""
echo "=== Verifying mcp_test.rs ==="
grep "test_start_server" src/services/mcp_test.rs && echo "[OK] test_start_server exists" || echo " Missing"
grep "test_stop_server" src/services/mcp_test.rs && echo "[OK] test_stop_server exists" || echo " Missing"
grep "test_call_tool" src/services/mcp_test.rs && echo "[OK] test_call_tool exists" || echo " Missing"
grep "test_list_tools" src/services/mcp_test.rs && echo "[OK] test_list_tools exists" || echo " Missing"
grep "test_tool_registry" src/services/mcp_test.rs && echo "[OK] test_tool_registry exists" || echo " Missing"
```

#### src/services/profile_test.rs
```bash
echo ""
echo "=== Verifying profile_test.rs ==="
grep "test_add_profile" src/services/profile_test.rs && echo "[OK] test_add_profile exists" || echo " Missing"
grep "test_get_profile" src/services/profile_test.rs && echo "[OK] test_get_profile exists" || echo " Missing"
grep "test_update_profile" src/services/profile_test.rs && echo "[OK] test_update_profile exists" || echo " Missing"
grep "test_list_profiles" src/services/profile_test.rs && echo "[OK] test_list_profiles exists" || echo " Missing"
grep "test_validation" src/services/profile_test.rs && echo "[OK] test_validation exists" || echo " Missing"
```

#### src/services/secrets_test.rs
```bash
echo ""
echo "=== Verifying secrets_test.rs ==="
grep "test_get_api_key" src/services/secrets_test.rs && echo "[OK] test_get_api_key exists" || echo " Missing"
grep "test_set_api_key" src/services/secrets_test.rs && echo "[OK] test_set_api_key exists" || echo " Missing"
grep "test_delete_api_key" src/services/secrets_test.rs && echo "[OK] test_delete_api_key exists" || echo " Missing"
grep "test_security_no_plaintext" src/services/secrets_test.rs && echo "[OK] test_security exists" || echo " Missing"
```

### Detailed Test Quality Review

For each test file, verify:

1. **Test Structure**: Each test follows Arrange-Act-Assert pattern
2. **Event Verification**: Tests subscribe to event_tx and verify emissions
3. **Error Paths**: Tests include both success and failure cases
4. **Async Handling**: Tests use #[tokio::test] attribute
5. **Assertions**: Tests have clear, specific assertions
6. **Test Names**: Test names clearly describe what is being tested

```bash
# Check for async test attribute
echo ""
echo "=== Checking async test attributes ==="
grep "#\[tokio::test\]" src/services/*_test.rs | wc -l
echo "Expected: 30+ async tests"

# Check for event verification
echo ""
echo "=== Checking event verification patterns ==="
grep "event_rx.recv" src/services/*_test.rs | wc -l
echo "Expected: 15+ event verification patterns"

# Check for error assertions
echo ""
echo "=== Checking error path tests ==="
grep "assert.*result.is_err" src/services/*_test.rs | wc -l
echo "Expected: 10+ error path tests"
```

## Success Criteria

- All 5 test files created and compile
- Tests run successfully (even with failures)
- Test coverage >= 80% of public APIs
- All event emission paths tested
- All error paths tested
- Plan markers present in all test files
- Requirement markers traceable
- Test failures documented (expected against stubs)

## Expected Test Failures

**Document expected failures in Phase 08 (against stubs):**

```
conversation_test:
- test_create_conversation: FAIL (unimplemented!)
- test_send_message: FAIL (unimplemented!)
- test_get_conversation: FAIL (unimplemented!)
- test_set_active: FAIL (unimplemented!)

chat_test:
- test_send_message_stream: FAIL (unimplemented!)
- test_build_message_history: FAIL (unimplemented!)

mcp_test:
- test_start_server: FAIL (unimplemented!)
- test_call_tool: FAIL (unimplemented!)

profile_test:
- test_add_profile: FAIL (unimplemented!)
- test_update_profile: FAIL (unimplemented!)

secrets_test:
- test_get_api_key: FAIL (unimplemented!)
- test_set_api_key: FAIL (unimplemented!)
```

**These failures are EXPECTED and will be fixed in Phase 09.**

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
   - Proceed to Phase 09

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P08A.md`

Contents:

```markdown
Phase: P08A
Completed: YYYY-MM-DD HH:MM
Verification Results:
  Test Files: 5/5 created
  Test Modules: 5/5 declared
  Plan Markers: 50+ found
  Requirement Markers: 30+ found
  Compilation: PASS
  Tests Run: YES
  Total Tests: 50+
  Passed: X
  Failed: Y (EXPECTED against stubs)
  Coverage: Z%
Quality Checks:
  - Event emission verified: YES
  - Error paths tested: YES
  - Async handling: YES
  - AAA pattern: YES
Ready for Phase 09: YES
```

## Next Steps

After successful verification:

1. All tests should be in place and compiling
2. Test failures should be documented (expected against stubs)
3. Proceed to Phase 09: Service Implementation
4. Implement real functionality to make tests pass

## Important Notes

- This is a VERIFICATION phase - no code changes expected
- Test failures are EXPECTED and DOCUMENTED
- Coverage will improve after Phase 09 implementation
- Next phase will implement real functionality
- All tests should pass after Phase 09
