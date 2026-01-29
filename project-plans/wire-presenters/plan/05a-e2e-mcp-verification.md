# Phase 05a: Verify E2E MCP Lifecycle Tests

**Phase ID**: P05a
**Type**: Verification
**Status**: Pending
**Prerequisites**: P05 completion marker exists

## Objective

Verify that E2E tests for SettingsPresenter MCP event wiring are complete and passing. Per dev-docs/COORDINATING.md, this is a SKEPTICAL AUDITOR phase - verify tests actually exercise real behavior, not just compile.

## Verification Protocol

Per dev-docs/COORDINATING.md, assume nothing works until you have EVIDENCE it works.

## Prerequisite Check

Before running verification, confirm:

```bash
# Does P05 completion marker exist?
ls project-plans/wire-presenters/plan/.completed/P05.md

# Does P04a have PASS verdict?
grep "^## Verdict: PASS" project-plans/wire-presenters/plan/.completed/P04A.md

# Does evidence show no placeholders in P04?
grep "unimplemented!" project-plans/wire-presenters/plan/.completed/P04A.md
# Expected: (no output)
```

If any check fails: DO NOT PROCEED. Remediate failed phase first.

## Structural Checks

### 1. Test File Updated

```bash
# Check that test file was updated
ls -lh tests/e2e_presenter_tests.rs
# Should exist and have grown from P04 (> 3KB)
```

Expected: Test file now has chat tests (P04) + MCP tests (P05)

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
# Expected: (no output)
```

**IF ANY GREP RETURNS MATCHES: STOP. VERDICT IS FAIL.**

## Semantic Verification

### 1. Test Function Count

```bash
# Count all test functions (chat + MCP)
grep -c "^    async fn test_" tests/e2e_presenter_tests.rs
# Expected: >= 11 (5 chat from P04 + 6 MCP from P05)

# Count MCP-specific tests
grep -c "test_mcp_" tests/e2e_presenter_tests.rs
# Expected: >= 6
```

What to verify:
- [ ] At least 6 MCP test functions exist
- [ ] Chat tests from P04 still exist
- [ ] Total test count >= 11

### 2. MCP Event Emission

```bash
# Check that tests emit MCP events
grep -c "McpEvent::Starting\|McpEvent::Started\|McpEvent::StartFailed" tests/e2e_presenter_tests.rs
# Expected: >= 3

grep -c "McpEvent::Unhealthy\|McpEvent::Recovered" tests/e2e_presenter_tests.rs
# Expected: >= 2

grep -c "McpEvent::ConfigSaved\|McpEvent::Deleted" tests/e2e_presenter_tests.rs
# Expected: >= 2

grep -c "McpEvent::ToolCalled\|McpEvent::ToolCompleted" tests/e2e_presenter_tests.rs
# Expected: >= 2
```

What to verify:
- [ ] Tests emit all MCP event types
- [ ] Events match real event types from `src/events/types.rs`
- [ ] Events have realistic data

### 3. ViewCommand Verification

```bash
# Check that tests verify MCP ViewCommands
grep -c "ViewCommand::UpdateMcpStatus\|ViewCommand::ShowMcpError\|ViewCommand::ShowMcpWarning" tests/e2e_presenter_tests.rs
# Expected: >= 5

grep -c "ViewCommand::ShowMcpConfigEditor\|ViewCommand::RemoveMcpItem" tests/e2e_presenter_tests.rs
# Expected: >= 2
```

What to verify:
- [ ] Tests assert on MCP-specific ViewCommands
- [ ] Assertions check specific fields (id, status, error messages)
- [ ] Assertions match expected ViewCommand variants

### 4. Test Coverage

Verify each scenario from P05 is tested:

```bash
# Test 1: Start success
grep -c "test_mcp_server_start_success\|McpStatus::Running" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 2: Start failure
grep -c "test_mcp_server_start_failure\|McpEvent::StartFailed" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 3: Unhealthy/recover
grep -c "test_mcp_unhealthy_then_recovers\|McpEvent::Unhealthy\|McpEvent::Recovered" tests/e2e_presenter_tests.rs
# Expected: >= 3

# Test 4: Config update
grep -c "test_mcp_configuration_update\|UserEvent::SaveMcpConfig" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 5: Deletion
grep -c "test_mcp_deletion_flow\|UserEvent::ConfirmDeleteMcp" tests/e2e_presenter_tests.rs
# Expected: >= 2

# Test 6: Tool calls
grep -c "test_mcp_tool_call_events\|McpEvent::ToolCalled" tests/e2e_presenter_tests.rs
# Expected: >= 2
```

Expected: All 6 MCP scenarios are covered

### 5. Test Isolation

```bash
# Check that tests create fresh instances
grep -c "SettingsPresenter::new\|EventBus::new" tests/e2e_presenter_tests.rs
# Expected: >= 6 (one per MCP test)
```

What to verify:
- [ ] Each MCP test creates its own SettingsPresenter instance
- [ ] Each MCP test creates its own EventBus instance
- [ ] No shared state between tests

### 6. Real Assertions

Pick ONE MCP test and read it completely:

```bash
# Find an MCP test function
sed -n '/async fn test_mcp_server_start_success/,/^    }/p' tests/e2e_presenter_tests.rs
```

What to verify:
- [ ] Test emits real MCP events (not mocked)
- [ ] Test waits for ViewCommands (with timeout or count)
- [ ] Test has specific assertions (e.g., `assert_eq!(commands[0], ViewCommand::UpdateMcpStatus { ... })`)
- [ ] Test doesn't use `assert!(true)` or similar meaningless assertions
- [ ] Test would fail if SettingsPresenter didn't handle MCP events

### 7. Comparison to Chat Tests

```bash
# Verify MCP tests follow same pattern as chat tests
grep -c "collect_view_commands\|subscribe_view_commands" tests/e2e_presenter_tests.rs
# Expected: >= 11 (one per test)
```

What to verify:
- [ ] MCP tests use same helper functions as chat tests
- [ ] MCP tests have similar structure (setup, emit, verify)
- [ ] MCP tests are consistent with chat tests

## Code Reading

Read at least ONE complete MCP test function and verify:

```bash
# Read one MCP test
sed -n '/async fn test_mcp_server_start_failure/,/^    }/p' tests/e2e_presenter_tests.rs
```

Verify:
- [ ] Emits `AppEvent::Mcp(McpEvent::StartFailed { ... })`
- [ ] Asserts on `ViewCommand::ShowMcpError` with error field
- [ ] Asserts on `ViewCommand::UpdateMcpStatus` with `McpStatus::Failed`
- [ ] Uses realistic error message (e.g., "Connection refused")
- [ ] Checks that status reflects failure

## Inputs

### Files to Read
- `tests/e2e_presenter_tests.rs` - Complete test file (focus on MCP tests)
- `project-plans/wire-presenters/plan/.completed/P05.md` - Phase completion evidence
- `src/events/types.rs` - Reference for McpEvent structure
- `src/presentation/view_command.rs` - Reference for ViewCommand MCP variants

### Evidence Required
- Exact grep outputs from all checks above
- Complete text of at least 2 MCP test functions
- Count of MCP test functions and assertions
- Verification that tests would fail if SettingsPresenter was broken

## Outputs

### Evidence File
Create: `project-plans/wire-presenters/plan/.completed/P05A.md`

Must contain:

```markdown
# Phase 05a Verification Results

## Verdict: [PASS|FAIL]

## Structural Checks
- Test file updated: [YES/NO with ls output]
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
- Total test count: [number]
- MCP test count: [number]
- MCP event emission: [count by event type]
- ViewCommand assertion count: [number]
- Scenario coverage: [list which MCP scenarios are covered]
- Test isolation: [YES/NO with evidence]

## Code Reading Example
[Paste one complete MCP test function here]
[Analyze: Does it test real behavior? Would it fail if SettingsPresenter was broken?]

## Evidence Summary
[Bullet points of what was verified]

## Blocking Issues (if FAIL)
[List exactly what must be fixed]
```

## Test Execution (Optional)

This verification phase is primarily about checking test code quality. However, you may OPTIONALLY run the tests:

```bash
# Create evidence directory
mkdir -p evidence/PLAN-20250128-PRESENTERS/phase-05a

# Run tests (capture output)
cargo test --test e2e_presenter_tests 2>&1 | tee evidence/PLAN-20250128-PRESENTERS/phase-05a/test-output.txt
```

**IMPORTANT:** Test execution is NOT required for PASS verdict. The requirement is that tests are WELL-WRITTEN, not that they all pass (they may fail due to incomplete presenter implementations). What matters is:
- Tests compile
- Tests have real assertions
- Tests would fail if implementation was removed

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- Exit code 0 from `cargo test --test e2e_presenter_tests --no-run`
- ALL placeholder grep commands return no matches
- At least 6 MCP test functions exist
- At least 15 MCP event emissions across all MCP tests
- At least 15 ViewCommand assertions across all MCP tests
- All 6 MCP scenarios from P05 are covered
- Tests create fresh instances (no shared state)
- At least one complete MCP test function read and verified
- Evidence file created with all grep outputs and test text

### FAIL Conditions
- Build or test compilation fails
- ANY placeholder found (including `assert!(true)`)
- Missing MCP test scenarios
- Tests don't emit real MCP events
- Tests don't have real assertions
- Tests share state (not isolated)
- Evidence file missing or incomplete

## Related Requirements

- COORDINATING.md: Binary outcomes only (PASS or FAIL, no conditional)
- COORDINATING.md: Tests must exercise real behavior, not just compile
- goodtests.md: Tests should fail if implementation is removed/broken
- P05: All 6 MCP scenarios must be tested
- P04: Chat tests should still exist and be intact

## Completion

If this phase passes, the entire plan is complete. All presenters are wired to the event bus and tested.
