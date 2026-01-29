# Phase 01: Preflight Verification

## Phase ID

`PLAN-20250127-REMEDIATE.P01`

## Prerequisites

- None (this is the first phase)

## Purpose

Verify all assumptions before writing any code:

1. SerdesAI crate is available and has expected API
2. Existing code we plan to use actually works
3. Dependencies are correctly configured
4. Test infrastructure is ready

## Verification Tasks

### 1. SerdesAI Dependency Verification

```bash
# Verify serdes-ai crate is available
cargo tree -p serdes-ai 2>&1 | head -10

# Verify serdes-ai-mcp crate is available (for MCP toolsets)
cargo tree -p serdes-ai-mcp 2>&1 | head -10

# Check Cargo.toml has the dependency
grep -A5 "serdes-ai" Cargo.toml
```

**Expected:** Both crates appear in dependency tree.

### 2. SerdesAI API Verification

```bash
# Verify AgentBuilder exists and can be imported
grep -rn "AgentBuilder\|Agent\|ModelConfig" src/llm/ | head -10

# Check existing working code in src/llm/client.rs
cat src/llm/client.rs | head -50
```

**Expected:** Find existing usage patterns we can follow.

### 3. Existing MCP Service Verification

```bash
# Verify MCP_SERVICE singleton exists
grep -rn "MCP_SERVICE\|OnceLock" src/mcp/ | head -10

# Verify MCP client can provide tools
grep -rn "list_tools\|get_tools\|McpToolset" src/mcp/ | head -10
```

**Expected:** Existing MCP service is functional.

### 4. Existing Settings/Profile Verification

```bash
# Verify Settings struct with profiles
grep -rn "struct Settings\|profiles:" src/config/ | head -10

# Verify ModelProfile struct
grep -rn "struct ModelProfile" src/models/ src/config/ | head -10

# Verify load_settings function
grep -rn "fn load_settings\|fn save_settings" src/config/ | head -10
```

**Expected:** Existing config loading works.

### 5. EventBus Verification

```bash
# Verify EventBus compiles and tests pass
cargo test events:: 2>&1 | tail -20

# Verify ChatEvent types exist
grep -rn "enum ChatEvent" src/events/ | head -5
```

**Expected:** EventBus tests pass, ChatEvent enum exists.

### 6. Current ChatService State

```bash
# Check for placeholder in current implementation
grep -rn "placeholder\|unimplemented!\|todo!" src/services/chat_impl.rs

# Show current implementation
cat src/services/chat_impl.rs | head -100
```

**Expected:** Find the placeholder code we need to replace.

### 7. Build Verification

```bash
# Full build must pass
cargo build --all-targets 2>&1 | tail -20
```

**Expected:** Build passes with 0 errors.

## Blocking Issues Checklist

If ANY of these are true, STOP and update the plan:

- [ ] serdes-ai crate not in Cargo.toml
- [ ] serdes-ai-mcp crate not available
- [ ] AgentBuilder API not as expected
- [ ] MCP_SERVICE singleton doesn't exist
- [ ] Settings/profiles not loadable
- [ ] EventBus tests fail
- [ ] Build fails

## Deliverables

1. Evidence file at `project-plans/remediate-refactor/plan/.completed/P01.md` with:
   - All command outputs
   - Confirmation each check passed
   - Any issues discovered

## Success Criteria

- All verification commands return expected results
- No blocking issues identified
- Build passes
- We have confirmed the APIs we plan to use exist

## Next Phase

If this phase passes, proceed to Phase 02: ChatService Implementation.
