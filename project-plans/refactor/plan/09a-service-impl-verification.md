# Phase 09a: Service Layer Implementation Verification

## Phase ID

`PLAN-20250125-REFACTOR.P09A`

## Prerequisites

- Required: Phase 09 (Service Implementation) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P09" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P09.md`
  - All service implementations completed
  - All tests passing
- Preflight verification: Phases 01-09 completed

## Purpose

Verify that all service layer implementations are correct and all tests pass. This phase:

1. Confirms all stub implementations replaced
2. Verifies all tests pass (TDD green state)
3. Validates test coverage meets requirements
4. Reviews event emission patterns
5. Verifies concurrent access safety
6. Documents implementation quality

**Note:** This is a VERIFICATION phase. No code changes expected unless critical bugs found.

## Requirements Verified

### REQ-024.1: ConversationService Implementation
- Verify create_conversation() works correctly
- Verify send_message() appends messages
- Verify event emission correct
- Verify thread safety with Arc<Mutex<T>>

### REQ-024.2: ChatService Implementation
- Verify send_message_stream() spawns task correctly
- Verify thinking events emitted in order
- Verify stream chunks emitted correctly
- Verify background task cleanup

### REQ-024.3: McpService Implementation
- Verify start_server() registers tools
- Verify call_tool() routes correctly
- Verify tool registry updates correctly
- Verify MCP lifecycle events emitted

### REQ-024.4: ProfileService Implementation
- Verify add_profile() validates input
- Verify persistence works correctly
- Verify event emission correct
- Verify profile retrieval works

### REQ-024.5: SecretsService Implementation
- Verify get_api_key() retrieves correctly
- Verify set_api_key() stores securely
- Verify no plaintext leaks
- Verify encryption/decryption works

## Verification Tasks

### Implementation Completeness Verification

```bash
# Verify unimplemented! removed from implementations
echo "=== Checking for unimplemented! stubs ==="
grep -rn "unimplemented!" src/services/*.rs | grep -v "test" | grep -v "// STUB" | wc -l
echo "Expected: 0 occurrences (all stubs replaced)"

# List any remaining stubs
if grep -rn "unimplemented!" src/services/*.rs | grep -v "test" | grep -v "// STUB"; then
  echo "ERROR: Found remaining unimplemented! calls"
  exit 1
else
  echo "PASS: All stubs replaced with implementations"
fi
```

### Thread Safety Verification

```bash
# Verify Arc<Mutex<T>> patterns for shared state
echo ""
echo "=== Checking thread safety patterns ==="
grep -rn "Arc::new(Mutex::new" src/services/*.rs | grep -v "test"
echo "Expected: 10+ Arc<Mutex<T>> initializations"

# Verify mutex lock scopes are minimal
echo ""
echo "=== Checking mutex lock scopes ==="
# Look for potential long-held locks (patterns to avoid)
grep -rn "\.lock()\.unwrap()" src/services/*.rs | while read line; do
  # Extract function name
  func=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")
  echo "Lock found in: $func"
done
echo "WARNING: Review locks manually for minimal scope"
```

### Event Emission Verification

```bash
# Verify event emission in all services
echo ""
echo "=== Checking event emission patterns ==="
grep -rn "event_tx.send" src/services/*.rs | grep -v "test" | wc -l
echo "Expected: 20+ event emissions"

# List event emissions by service
echo ""
echo "Event emissions by service:"
for service in conversation chat mcp profile secrets; do
  count=$(grep "event_tx.send" src/services/${service}.rs 2>/dev/null | grep -v "test" | wc -l)
  echo "$service: $count emissions"
done
```

### Plan Marker Verification

```bash
# Verify plan markers in implementations
echo ""
echo "=== Checking plan markers ==="
grep -rn "@plan:PLAN-20250125-REFACTOR.P09" src/services/*.rs | wc -l
echo "Expected: 30+ occurrences"

# List files with plan markers
for file in src/services/*.rs; do
  if [ "$file" != "${file%_test.rs}" ]; then
    continue  # Skip test files
  fi
  count=$(grep "@plan:PLAN-20250125-REFACTOR.P09" "$file" | wc -l)
  echo "$(basename $file): $count markers"
done
```

### Test Execution Verification

```bash
# Run all tests
echo ""
echo "=== Running full test suite ==="
cargo test --lib -- --test-threads=1 2>&1 | tee test_verification.log

# Extract test summary
echo ""
echo "=== Test Summary ==="
grep -E "test result:" test_verification.log | tail -1

# Count total tests
total_tests=$(grep -E "test [a-z_]+" test_verification.log | wc -l)
echo "Total tests: $total_tests"

# Count passed tests
passed=$(grep -E "test [a-z_]+ \.\.\. ok" test_verification.log | wc -l)
echo "Passed: $passed"

# Count failed tests (should be 0)
failed=$(grep -E "test [a-z_]+ \.\.\. FAILED" test_verification.log | wc -l)
echo "Failed: $failed"

# Count ignored tests
ignored=$(grep -E "test [a-z_]+ \.\.\. ignored" test_verification.log | wc -l)
echo "Ignored: $ignored"

# Verify no failures
if [ "$failed" -gt 0 ]; then
  echo ""
  echo "ERROR: $failed tests failed!"
  echo "Failing tests:"
  grep -E "test [a-z_]+ \.\.\. FAILED" test_verification.log
  exit 1
else
  echo ""
  echo "PASS: All tests passed!"
fi
```

### Individual Service Test Verification

```bash
# Test each service module individually
echo ""
echo "=== Testing individual service modules ==="

echo "Testing conversation service..."
cargo test --lib conversation:: 2>&1 | grep -E "test result:"

echo "Testing chat service..."
cargo test --lib chat:: 2>&1 | grep -E "test result:"

echo "Testing mcp service..."
cargo test --lib mcp:: 2>&1 | grep -E "test result:"

echo "Testing profile service..."
cargo test --lib profile:: 2>&1 | grep -E "test result:"

echo "Testing secrets service..."
cargo test --lib secrets:: 2>&1 | grep -E "test result:"
```

### Coverage Verification

```bash
# Generate coverage report
echo ""
echo "=== Generating coverage report ==="
cargo tarpaulin --lib --output-dir coverage --output Html 2>&1 | tee coverage_verification.log

# Extract coverage percentage
echo ""
echo "=== Coverage Summary ==="
grep -E "^\s*[0-9]+\.[0-9]+% coverage" coverage_verification.log || echo "Coverage not available"

# Verify coverage meets threshold
coverage=$(grep -oP '\d+\.\d+(?=%)' coverage_verification.log | head -1)
if [ -n "$coverage" ]; then
  coverage_int=$(echo "$coverage * 100" | bc | cut -d'.' -f1)
  if [ "$coverage_int" -ge 80 ]; then
    echo "PASS: Coverage ${coverage}% meets >= 80% threshold"
  else
    echo "WARNING: Coverage ${coverage}% below 80% threshold"
  fi
fi
```

### Concurrent Access Verification

```bash
# Run tests with multiple threads to detect race conditions
echo ""
echo "=== Testing concurrent access ==="
cargo test --lib -- --test-threads=8 2>&1 | tee concurrent_test.log

# Check for any failures
if grep -q "FAILED" concurrent_test.log; then
  echo "WARNING: Some tests failed with multiple threads"
  echo "Possible race conditions detected"
  grep "FAILED" concurrent_test.log
else
  echo "PASS: All tests passed with concurrent execution"
fi
```

### Manual Verification Checklist

Review each implementation and verify:

#### src/services/conversation.rs
```bash
echo ""
echo "=== Verifying conversation.rs ==="
grep "pub async fn create_conversation" src/services/conversation.rs && echo "[OK] create_conversation implemented" || echo " Missing"
grep "pub async fn send_message" src/services/conversation.rs && echo "[OK] send_message implemented" || echo " Missing"
grep "pub fn get_conversation" src/services/conversation.rs && echo "[OK] get_conversation implemented" || echo " Missing"
grep "pub async fn set_active" src/services/conversation.rs && echo "[OK] set_active implemented" || echo " Missing"
grep "event_tx.send.*ConversationStarted" src/services/conversation.rs && echo "[OK] ConversationStarted event emitted" || echo " Missing"
grep "event_tx.send.*MessageReceived" src/services/conversation.rs && echo "[OK] MessageReceived event emitted" || echo " Missing"
grep "Arc::new(Mutex::new" src/services/conversation.rs && echo "[OK] Thread safety pattern used" || echo " Missing"
```

#### src/services/chat.rs
```bash
echo ""
echo "=== Verifying chat.rs ==="
grep "pub async fn send_message_stream" src/services/chat.rs && echo "[OK] send_message_stream implemented" || echo " Missing"
grep "spawn(" src/services/chat.rs && echo "[OK] Background task spawned" || echo " Missing"
grep "ThinkingStarted" src/services/chat.rs && echo "[OK] ThinkingStarted event emitted" || echo " Missing"
grep "StreamChunk" src/services/chat.rs && echo "[OK] StreamChunk event emitted" || echo " Missing"
grep "ResponseGenerated" src/services/chat.rs && echo "[OK] ResponseGenerated event emitted" || echo " Missing"
grep "ThinkingEnded" src/services/chat.rs && echo "[OK] ThinkingEnded event emitted" || echo " Missing"
```

#### src/services/mcp.rs
```bash
echo ""
echo "=== Verifying mcp.rs ==="
grep "pub async fn start_server" src/services/mcp.rs && echo "[OK] start_server implemented" || echo " Missing"
grep "pub async fn call_tool" src/services/mcp.rs && echo "[OK] call_tool implemented" || echo " Missing"
grep "ServerStarted" src/services/mcp.rs && echo "[OK] ServerStarted event emitted" || echo " Missing"
grep "ToolCalled" src/services/mcp.rs && echo "[OK] ToolCalled event emitted" || echo " Missing"
grep "ToolResult" src/services/mcp.rs && echo "[OK] ToolResult event emitted" || echo " Missing"
```

#### src/services/profile.rs
```bash
echo ""
echo "=== Verifying profile.rs ==="
grep "pub async fn add_profile" src/services/profile.rs && echo "[OK] add_profile implemented" || echo " Missing"
grep "pub fn get_profile" src/services/profile.rs && echo "[OK] get_profile implemented" || echo " Missing"
grep "pub async fn update_profile" src/services/profile.rs && echo "[OK] update_profile implemented" || echo " Missing"
grep "ProfileAdded" src/services/profile.rs && echo "[OK] ProfileAdded event emitted" || echo " Missing"
grep "Arc::new(Mutex::new" src/services/profile.rs && echo "[OK] Thread safety pattern used" || echo " Missing"
```

#### src/services/secrets.rs
```bash
echo ""
echo "=== Verifying secrets.rs ==="
grep "pub async fn get_api_key" src/services/secrets.rs && echo "[OK] get_api_key implemented" || echo " Missing"
grep "pub async fn set_api_key" src/services/secrets.rs && echo "[OK] set_api_key implemented" || echo " Missing"
echo "[OK] Manual security review required (no plaintext leaks)"
```

## Success Criteria

- All stub implementations replaced
- All tests pass (0 failures)
- No unimplemented!() calls in production code
- Event emission verified in all services
- Thread safety patterns correctly applied
- Test coverage >= 80%
- Concurrent access tests pass
- Plan markers present in all implementations

## Failure Recovery

If verification fails:

1. If tests fail:
   - Identify failing tests
   - Review implementation for bugs
   - Fix bugs in Phase 09 (update completion marker)
   - Re-run verification

2. If coverage too low:
   - Identify missing test coverage
   - Add tests in Phase 08 (update completion marker)
   - Re-run verification

3. If thread safety issues:
   - Review Arc<Mutex<T>> patterns
   - Fix lock scopes
   - Re-run verification

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P09A.md`

Contents:

```markdown
Phase: P09A
Completed: YYYY-MM-DD HH:MM
Verification Results:
  Stubs Replaced: YES
  Unimplemented calls: 0
  Thread Safety: PASS
  Event Emissions: 20+
  Plan Markers: 30+ found
Test Results:
  Total Tests: 50+
  Passed: 50+
  Failed: 0
  Ignored: 0
  Concurrent Tests: PASS
Coverage: >= 80%
Quality Checks:
  - ConversationService: PASS
  - ChatService: PASS
  - McpService: PASS
  - ProfileService: PASS
  - SecretsService: PASS
Ready for Phase 10: YES
```

## Next Steps

After successful verification:

1. All service implementations complete
2. All tests passing
3. Coverage meets requirements
4. Proceed to Phase 10: Presenter Layer Stub Phase
5. Begin Presenter layer development

## Important Notes

- This is a VERIFICATION phase - no code changes expected
- If bugs found, return to Phase 09 for fixes
- Ensure all tests pass before proceeding
- Coverage threshold is 80%
- Thread safety is critical for services
- Event emission must be complete
