# Phase 03a: MCP Integration Verification

## Phase ID

`PLAN-20250127-REMEDIATE.P03A`

## Prerequisites

- Required: Phase 03 completed
- Evidence file exists: `project-plans/remediate-refactor/plan/.completed/P03.md`

## Purpose

Verify that MCP integration:
1. Contains NO placeholders
2. get_toolsets() returns actual toolsets from running MCPs
3. ChatService attaches toolsets to Agent
4. Tests pass

## CRITICAL: Placeholder Detection (RUN FIRST)

**These checks are BLOCKING. If ANY return matches, VERDICT is FAIL.**

### For mcp_impl.rs:

```bash
# Check 1: unimplemented! macro
$ grep -rn "unimplemented!" src/services/mcp_impl.rs
[PASTE EXACT OUTPUT HERE]
# Expected: (no output)

# Check 2: todo! macro
$ grep -rn "todo!" src/services/mcp_impl.rs
[PASTE EXACT OUTPUT HERE]
# Expected: (no output)

# Check 3: placeholder strings
$ grep -rn "placeholder" src/services/mcp_impl.rs
[PASTE EXACT OUTPUT HERE]
# Expected: (no output)

# Check 4: TODO comments
$ grep -rn "// TODO\|// FIXME" src/services/mcp_impl.rs
[PASTE EXACT OUTPUT HERE]
# Expected: (no output)
```

### For chat_impl.rs (regression check):

```bash
# Must still be clean from Phase 02
$ grep -rn "unimplemented!\|todo!\|placeholder" src/services/chat_impl.rs
[PASTE EXACT OUTPUT HERE]
# Expected: (no output)
```

**IF ANY CHECK RETURNS MATCHES: STOP. VERDICT IS FAIL. DO NOT PROCEED.**

## Implementation Verification

### 1. get_toolsets() Implementation Check

```bash
# Show the implementation
$ grep -A30 "fn get_toolsets" src/services/mcp_impl.rs
[PASTE OUTPUT]
```

Verify the implementation:
- [ ] Does NOT just return `Vec::new()` unconditionally
- [ ] Actually accesses MCP_SERVICE or similar
- [ ] Wraps clients in McpToolset
- [ ] Returns Arc<dyn AbstractToolset>

### 2. ChatService Uses Toolsets Check

```bash
# Show where ChatService uses toolsets
$ grep -B5 -A5 "get_toolsets\|\.toolset(" src/services/chat_impl.rs
[PASTE OUTPUT]
```

Verify:
- [ ] Calls `mcp_service.get_toolsets()` or equivalent
- [ ] Passes toolsets to Agent builder via `.toolset()` or similar

## Build Verification

```bash
$ cargo build --all-targets 2>&1 | tail -10
[PASTE OUTPUT]
# Expected: "Finished" with 0 errors
```

## Test Verification

```bash
$ cargo test services::mcp 2>&1 | grep -E "^test|passed|failed|FAILED"
[PASTE OUTPUT]
# Expected: All tests pass

$ cargo test services::chat 2>&1 | grep -E "^test|passed|failed|FAILED"
[PASTE OUTPUT]
# Expected: All tests pass
```

## Semantic Verification

### 1. Is get_toolsets() a REAL implementation?

Read the code and answer:

- [ ] It accesses the MCP singleton or stored clients
- [ ] It filters for enabled MCPs
- [ ] It wraps clients in McpToolset
- [ ] It would return real toolsets if MCPs were running

Evidence (cite file:line):
```
[Paste code showing it's real, not hollow]
```

**RED FLAG - Hollow Implementation:**
```rust
// This is HOLLOW and should FAIL:
fn get_toolsets(&self) -> Vec<Arc<dyn AbstractToolset>> {
    Vec::new()  // Always returns empty!
}
```

**What REAL looks like:**
```rust
fn get_toolsets(&self) -> Vec<Arc<dyn AbstractToolset>> {
    let configs = self.list_enabled().unwrap_or_default();
    let mut toolsets = Vec::new();
    for config in configs {
        if let Some(client) = MCP_SERVICE.get_client(config.id) {
            toolsets.push(Arc::new(McpToolset::new(client)) as Arc<dyn AbstractToolset>);
        }
    }
    toolsets
}
```

### 2. Does ChatService actually wire toolsets?

Evidence (cite file:line):
```
[Paste code showing toolsets are attached to Agent]
```

## Behavioral Verification Questions

All must be YES for PASS:

1. **Does the code DO what REM-004 says?**
   - [ ] REM-004: ChatService attaches MCP tools from McpService
   - Evidence: [file:line showing toolset attachment]

2. **Does the code DO what REM-007 says?**
   - [ ] REM-007: Tool calls work during streaming (toolsets attached)
   - Evidence: [file:line showing toolsets passed to Agent]

3. **Is get_toolsets() a REAL implementation?**
   - [ ] Not hollow (doesn't just return Vec::new())
   - [ ] Actually accesses MCP infrastructure

**If ANY checkbox is NO: VERDICT: FAIL**

## Verdict Rules

- **PASS**: All placeholder checks return empty, implementations are real (not hollow), build passes, tests pass
- **FAIL**: Any check fails

**THERE IS NO "CONDITIONAL PASS". THERE IS NO "PARTIAL PASS".**

## Deliverables

Create evidence file at `project-plans/remediate-refactor/plan/.completed/P03A.md`:

```markdown
# Phase 03A Verification Evidence

## Verdict: [PASS|FAIL]

## Completion Timestamp
Completed: YYYY-MM-DD HH:MM

## Placeholder Detection Results

### mcp_impl.rs
$ grep -rn "unimplemented!" src/services/mcp_impl.rs
[output - must be empty]

$ grep -rn "todo!" src/services/mcp_impl.rs
[output - must be empty]

$ grep -rn "placeholder" src/services/mcp_impl.rs
[output - must be empty]

### chat_impl.rs (regression)
$ grep -rn "unimplemented!\|todo!\|placeholder" src/services/chat_impl.rs
[output - must be empty]

## Build and Test Results

$ cargo build --all-targets 2>&1 | tail -10
[output]

$ cargo test services::mcp 2>&1 | grep -E "^test|passed|failed"
[output]

$ cargo test services::chat 2>&1 | grep -E "^test|passed|failed"
[output]

## Implementation Evidence

### get_toolsets() Implementation
[paste the actual implementation code]
File: src/services/mcp_impl.rs
Lines: XX-YY
Assessment: [Real implementation / Hollow - explain]

### ChatService Toolset Wiring
[paste code showing toolsets attached to Agent]
File: src/services/chat_impl.rs
Lines: XX-YY

## Requirements Satisfied

- REM-004: [YES/NO] Evidence: [file:line]
- REM-007: [YES/NO] Evidence: [file:line]

## Blocking Issues
[List any issues that prevent PASS, or "None"]

## Verdict Justification
[Explain why PASS or FAIL based on above evidence]
```

## Next Phase

If this phase passes (PASS, not conditional), proceed to Phase 04: End-to-End Verification.
