# Phase 04a: Verify E2E Chat Stream Tests

**Phase ID**: P04a
**Type**: Verification
**Status**: Pending
**Prerequisites**: P04 completion marker exists

## Objective

Verify that E2E tests for ChatPresenter event wiring are complete and passing. Per dev-docs/COORDINATING.md, this is a SKEPTICAL AUDITOR phase - verify tests actually exercise real behavior, not just compile.

## Verification Protocol

Per dev-docs/COORDINATING.md, assume nothing works until you have EVIDENCE it works.

## Prerequisite Check

Before running verification, confirm:

```bash
# Does P04 completion marker exist?
ls project-plans/wire-presenters/plan/.completed/P04.md

# Does P03a have PASS verdict?
grep "^## Verdict: PASS" project-plans/wire-presenters/plan/.completed/P03A.md

# Does evidence show no placeholders in P03?
grep "unimplemented!" project-plans/wire-presenters/plan/.completed/P03A.md
# Expected: (no output)
```

If any check fails: DO NOT PROCEED. Remediate failed phase first.

## Structural Checks

### 1. Test File Exists

```bash
# Check that test file was created
ls -lh tests/e2e_presenter_tests.rs
# Should exist and have reasonable size (> 1KB)
```

Expected: Test file exists with at least 5 test functions

### 2. Build Verification

```bash
cargo build --all-targets 2>&1 | tail -20
# Expected: exit code 0, "Finished" message
```

### 3. Test Compilation

```bash
cargo test --test e2e_presenter_tests --no-run 2>&1 | tail -20
# Expected: exit code 0, tests compile successfully
```

## Placeholder Detection (MANDATORY)

Run ALL of these commands and record EXACT output:

```bash
# Check 1: unimplemented! macro
grep -rn "unimplemented!" tests/e2e_presenter_tests.rs
# Expected: (no output)

# Check 2: todo! macro
grep -rn "todo!" tests/e2e_presenter_tests.rs
# Expected: (no output)

# Check 3: TODO/FIXME comments
grep -rn "// TODO\|// FIXME\|// HACK\|// STUB" tests/e2e_presenter_tests.rs
# Expected: (no output)

# Check 4: Placeholder strings
grep -rn "placeholder\|not yet implemented\|will be implemented" tests/e2e_presenter_tests.rs
# Expected: (no output)

# Check 5: Placeholder assertions
grep -rn "assert!(true)\|assert_eq!(true, true)" tests/e2e_presenter_tests.rs
# Expected: (no output) - these are meaningless assertions
```

**IF ANY GREP RETURNS MATCHES: STOP. VERDICT IS FAIL.**

## Semantic Verification

### 1. Test Function Count

```bash
# Count test functions
grep -c "^    async fn test_" tests/e2e_presenter_tests.rs
# Expected: >= 5
```

What to verify:
- [ ] At least 5 test functions exist
- [ ] Tests are named descriptively (e.g., `test_chat_send_and_stream_completion`)
- [ ] Each test is marked `#[tokio::test]` (for async)

### 2. Event Emission

```bash
# Check that tests emit events via EventBus
grep -c "event_bus.emit\|bus.emit" tests/e2e_presenter_tests.rs
# Expected: >= 20 (at least 4 events per test * 5 tests)
```

What to verify:
- [ ] Tests use `event_bus.emit(AppEvent::...)`
- [ ] Events match real event types from `src/events/types.rs`
- [ ] Events have realistic data (not empty strings, default UUIDs)

### 3. ViewCommand Verification

```bash
# Check that tests verify ViewCommands
grep -c "assert_eq.*ViewCommand\|assert.*ViewCommand" tests/e2e_presenter_tests.rs
# Expected: >= 20 (at least 4 assertions per test)
```

What to verify:
- [ ] Tests assert on received ViewCommands
- [ ] Assertions check specific fields (not just `assert!(cmd.is_some())`)
- [ ] Assertions match expected ViewCommand variants

### 4. Test Coverage

Verify each scenario from P04 is tested:

```bash
# Test 1: Send and stream completion
grep -c "test_chat_send_and_stream_completion\|StreamCompleted" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 2: Tool calls
grep -c "test_tool_call_during_stream\|ToolCallStarted\|ToolCallCompleted" tests/e2e_presenter_tests.rs
# Expected: >= 3

# Test 3: Thinking display
grep -c "test_thinking_display\|ThinkingDelta" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 4: Stream errors
grep -c "test_stream_error\|StreamError" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 5: User cancellation
grep -c "test_user_cancels_stream\|StreamCancelled\|StopStreaming" tests/e2e_presenter_tests.rs
# Expected: >= 3
```

Expected: All 5 scenarios are covered

### 5. Test Isolation

```bash
# Check that tests create fresh instances
grep -c "ChatPresenter::new\|EventBus::new" tests/e2e_presenter_tests.rs
# Expected: >= 5 (one per test)
```

What to verify:
- [ ] Each test creates its own ChatPresenter instance
- [ ] Each test creates its own EventBus instance
- [ ] No shared state between tests

### 6. Real Assertions

Pick ONE test and read it completely:

```bash
# Find a test function
sed -n '/async fn test_chat_send_and_stream_completion/,/^    }/p' tests/e2e_presenter_tests.rs
```

What to verify:
- [ ] Test emits real events (not mocked)
- [ ] Test waits for ViewCommands (with timeout or count)
- [ ] Test has specific assertions (e.g., `assert_eq!(commands[0], ViewCommand::ShowLoading { ... })`)
- [ ] Test doesn't use `assert!(true)` or similar meaningless assertions
- [ ] Test would fail if ChatPresenter didn't handle events

## Test Execution (Dry Run)

**DO NOT RUN YET - This is verification of test code, not running tests**

But verify the tests CAN run:

```bash
# Check tests are in the right module
grep -c "^#\[cfg(test)\]\|mod e2e" tests/e2e_presenter_tests.rs
# Expected: >= 1
```

## Code Reading

Read at least ONE complete test function and verify:

```bash
# Read one test
sed -n '/async fn test_tool_call_during_stream/,/^    }/p' tests/e2e_presenter_tests.rs
```

Verify:
- [ ] Emits `AppEvent::Chat(ChatEvent::ToolCallStarted { ... })`
- [ ] Emits `AppEvent::Chat(ChatEvent::ToolCallCompleted { ... })`
- [ ] Asserts on `ViewCommand::ShowToolCallStarted` and `ViewCommand::ShowToolCallCompleted`
- [ ] Uses realistic tool names (e.g., "exa.search")
- [ ] Checks success flag and result field

## Inputs

### Files to Read
- `tests/e2e_presenter_tests.rs` - Complete test file (read all)
- `project-plans/wire-presenters/plan/.completed/P04.md` - Phase completion evidence
- `src/events/types.rs` - Reference for event structure
- `src/presentation/view_command.rs` - Reference for ViewCommand structure

### Evidence Required
- Exact grep outputs from all checks above
- Complete text of at least 2 test functions
- Count of test functions and assertions
- Verification that tests would fail if implementation was removed

## Outputs

### Evidence File
Create: `project-plans/wire-presenters/plan/.completed/P04A.md`

Must contain:

```markdown
# Phase 04a Verification Results

## Verdict: [PASS|FAIL]

## Structural Checks
- Test file exists: [YES/NO with ls output]
- Build: [PASS/FAIL with output]
- Test compilation: [PASS/FAIL with output]

## Placeholder Detection
Command: grep -rn "unimplemented!" tests/e2e_presenter_tests.rs
Output: [exact output or "no matches"]

Command: grep -rn "todo!" tests/e2e_presenter_tests.rs
Output: [exact output or "no matches"]

Command: grep -rn "placeholder" tests/e2e_presenter_tests.rs
Output: [exact output or "no matches"]

Command: grep -rn "assert!(true)" tests/e2e_presenter_tests.rs
Output: [exact output or "no matches"]

## Semantic Verification
- Test function count: [number]
- Event emission count: [number]
- ViewCommand assertion count: [number]
- Scenario coverage: [list which scenarios are covered]
- Test isolation: [YES/NO with evidence]

## Code Reading Example
[Paste one complete test function here]
[Analyze: Does it test real behavior? Would it fail if ChatPresenter was broken?]

## Evidence Summary
[Bullet points of what was verified]

## Blocking Issues (if FAIL)
[List exactly what must be fixed]
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- Exit code 0 from `cargo test --test e2e_presenter_tests --no-run`
- ALL placeholder grep commands return no matches
- At least 5 test functions exist
- At least 20 event emissions across all tests
- At least 20 ViewCommand assertions across all tests
- All 5 scenarios from P04 are covered
- Tests create fresh instances (no shared state)
- At least one complete test function read and verified
- Evidence file created with all grep outputs and test text

### FAIL Conditions
- Build or test compilation fails
- ANY placeholder found (including `assert!(true)`)
- Missing test scenarios
- Tests don't emit real events
- Tests don't have real assertions
- Tests share state (not isolated)
- Evidence file missing or incomplete

## Related Requirements

- COORDINATING.md: Binary outcomes only (PASS or FAIL, no conditional)
- COORDINATING.md: Tests must exercise real behavior, not just compile
- goodtests.md: Tests should fail if implementation is removed/broken
- P04: All 5 scenarios must be tested

## Next Phase (if PASS)

P05: E2E Test - MCP Lifecycle Events
