# Phase 04a: Final Verification

## Phase ID

`PLAN-20250127-REMEDIATE.P04A`

## Prerequisites

- Required: Phase 04 completed
- Evidence file exists: `project-plans/remediate-refactor/plan/.completed/P04.md`

## Purpose

Final verification that all requirements are met and the remediation is complete.

## Final Placeholder Check

**This is the FINAL check. These MUST all return empty.**

```bash
# Complete sweep
$ grep -rn "unimplemented!\|todo!" src/services/chat_impl.rs src/services/mcp_impl.rs
[PASTE OUTPUT - must be empty]

$ grep -rn "placeholder\|not yet implemented" src/services/chat_impl.rs src/services/mcp_impl.rs
[PASTE OUTPUT - must be empty]

$ grep -rn "// TODO\|// FIXME" src/services/chat_impl.rs src/services/mcp_impl.rs
[PASTE OUTPUT - must be empty]
```

## All Evidence Files Exist

```bash
$ ls -la project-plans/remediate-refactor/plan/.completed/
[PASTE OUTPUT]
# Expected: P01.md, P01A.md, P02.md, P02A.md, P03.md, P03A.md, P04.md exist
```

## All Phases Passed

```bash
$ grep "^## Verdict:" project-plans/remediate-refactor/plan/.completed/*.md
[PASTE OUTPUT]
# Expected: All show "Verdict: PASS"
```

## Build Passes

```bash
$ cargo build --all-targets 2>&1 | tail -5
[PASTE OUTPUT]
# Expected: "Finished" with 0 errors
```

## Tests Pass

```bash
$ cargo test 2>&1 | tail -10
[PASTE OUTPUT]
# Expected: "test result: ok" with 0 failures
```

## Requirements Verification Summary

| ID | Requirement | Verified | Evidence File |
|----|-------------|----------|---------------|
| REM-001 | ChatService.send_message calls SerdesAI Agent | | P02A.md |
| REM-002 | ChatService uses profile from ProfileService | | P02A.md |
| REM-003 | ChatService resolves API key correctly | | P02A.md |
| REM-004 | ChatService attaches MCP tools from McpService | | P03A.md |
| REM-005 | ChatService emits ChatEvent::TextDelta | | P02A.md |
| REM-006 | ChatService emits ChatEvent::StreamCompleted | | P02A.md |
| REM-007 | Tool calls work during streaming | | P03A.md |

All requirements must show verified in P02A.md and P03A.md evidence files.

## Final Verdict Criteria

**PASS requires ALL of the following:**

- [ ] All placeholder detection returns EMPTY
- [ ] All evidence files exist (P01.md through P04.md, P01A.md through P04A.md)
- [ ] All phases show "Verdict: PASS" (not conditional)
- [ ] `cargo build --all-targets` passes
- [ ] `cargo test` passes with 0 failures
- [ ] All requirements verified in evidence files
- [ ] Code actually implements what requirements specify (verified by reading evidence)

**If ANY checkbox is unchecked: VERDICT: FAIL**

## Final Deliverable

Create evidence file at `project-plans/remediate-refactor/plan/.completed/P04A.md`:

```markdown
# Phase 04A Final Verification Evidence

## FINAL VERDICT: [PASS|FAIL]

## Completion Timestamp
Completed: YYYY-MM-DD HH:MM

## Placeholder Detection (Final)

$ grep -rn "unimplemented!\|todo!" src/services/chat_impl.rs src/services/mcp_impl.rs
[output - must be empty]

$ grep -rn "placeholder\|not yet implemented" src/services/chat_impl.rs src/services/mcp_impl.rs
[output - must be empty]

## Evidence Files

$ ls -la project-plans/remediate-refactor/plan/.completed/
[output]

## Phase Verdicts

$ grep "^## Verdict:" project-plans/remediate-refactor/plan/.completed/*.md
[output - all must show PASS]

## Build Status

$ cargo build --all-targets 2>&1 | tail -5
[output]

## Test Status

$ cargo test 2>&1 | tail -10
[output]

## Requirements Summary

| ID | Description | Status | Evidence |
|----|-------------|--------|----------|
| REM-001 | ChatService calls SerdesAI Agent | [PASS/FAIL] | P02A.md:line |
| REM-002 | Uses profile from ProfileService | [PASS/FAIL] | P02A.md:line |
| REM-003 | Resolves API key | [PASS/FAIL] | P02A.md:line |
| REM-004 | Attaches MCP tools | [PASS/FAIL] | P03A.md:line |
| REM-005 | Emits TextDelta | [PASS/FAIL] | P02A.md:line |
| REM-006 | Emits StreamCompleted | [PASS/FAIL] | P02A.md:line |
| REM-007 | Tool calls work | [PASS/FAIL] | P03A.md:line |

## Completion Checklist

- [ ] All placeholder detection returns empty
- [ ] All evidence files exist
- [ ] All phases PASS
- [ ] Build passes
- [ ] Tests pass
- [ ] All requirements verified

## Final Verdict Justification

[Explain why PASS or FAIL based on all above evidence]

## What Was Accomplished

[Summary of what this remediation achieved:
- ChatService now calls SerdesAI Agent instead of returning placeholder
- MCP toolsets are wired to Agent
- Events flow through EventBus
- etc.]

## Remaining Work (if any)

[List any follow-up tasks not in scope for this remediation:
- UI not yet modified to emit UserEvents
- ProfileService still has some unimplemented stubs
- etc.]
```

## Plan Completion

If FINAL VERDICT is PASS:
- The remediation is complete
- ChatService is no longer hollow
- MCP integration works
- Events flow correctly

If FINAL VERDICT is FAIL:
- Identify which check failed
- Go back to the appropriate phase
- Fix and re-verify

**There is NO conditional pass. The plan either succeeded or it didn't.**
