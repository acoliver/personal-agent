# Phase 10a: Presenter Layer Stub Verification

## Phase ID

`PLAN-20250125-REFACTOR.P10A`

## Prerequisites

- Required: Phase 10 (Presenter Stub) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P10" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P10.md`
  - All presenter stubs implemented and compiling
  - All 4 presenters structurally defined
  - ViewCommand enum defined
- Preflight verification: Phases 01-10 completed

## Purpose

Verify that all presenter layer stubs are properly structured and ready for TDD phase. This phase:

1. Confirms all presenter stubs exist and compile
2. Verifies ViewCommand enum is complete
3. Validates stub structure for all presenters
4. Confirms code compiles successfully
5. Documents stub state for Phase 11

**Note:** This is a VERIFICATION phase. No code changes expected. Stubs should compile but not function.

## Requirements Verified

### REQ-025.1: Presenter Module Structure
- Verify module exports all presenters
- Verify ViewCommand enum exported
- Verify module compiles

### REQ-025.2: ChatPresenter Structure
- Verify ChatPresenter struct defined
- Verify event_rx field present
- Verify services field present
- Verify view_tx field present

### REQ-025.3: McpPresenter Structure
- Verify McpPresenter struct defined
- Verify event_rx field present
- Verify services field present
- Verify view_tx field present

### REQ-025.4: SettingsPresenter Structure
- Verify SettingsPresenter struct defined
- Verify event_rx field present
- Verify services field present
- Verify view_tx field present

### REQ-025.5: ErrorPresenter Structure
- Verify ErrorPresenter struct defined
- Verify event_rx field present
- Verify view_tx field present

### REQ-025.6: ViewCommand Type
- Verify ViewCommand enum complete
- Verify all chat commands defined
- Verify all MCP commands defined
- Verify all settings commands defined
- Verify all error commands defined

## Verification Tasks

### Structural Verification

```bash
# Verify all presenter files exist
echo "=== Checking presenter file existence ==="
test -f src/presentation/mod.rs && echo "[OK] mod.rs exists" || echo " mod.rs missing"
test -f src/presentation/view_command.rs && echo "[OK] view_command.rs exists" || echo " view_command.rs missing"
test -f src/presentation/chat.rs && echo "[OK] chat.rs exists" || echo " chat.rs missing"
test -f src/presentation/mcp.rs && echo "[OK] mcp.rs exists" || echo " mcp.rs missing"
test -f src/presentation/settings.rs && echo "[OK] settings.rs exists" || echo " settings.rs missing"
test -f src/presentation/error.rs && echo "[OK] error.rs exists" || echo " error.rs missing"
```

### Module Exports Verification

```bash
# Verify module exports
echo ""
echo "=== Checking module exports ==="
grep "pub mod chat" src/presentation/mod.rs && echo "[OK] chat exported" || echo " chat not exported"
grep "pub mod mcp" src/presentation/mod.rs && echo "[OK] mcp exported" || echo " mcp not exported"
grep "pub mod settings" src/presentation/mod.rs && echo "[OK] settings exported" || echo " settings not exported"
grep "pub mod error" src/presentation/mod.rs && echo "[OK] error exported" || echo " error not exported"
grep "pub use.*ViewCommand" src/presentation/mod.rs && echo "[OK] ViewCommand exported" || echo " ViewCommand not exported"

# Verify presentation module exported in lib.rs
grep "pub mod presentation;" src/lib.rs && echo "[OK] presentation module exported" || echo " presentation not exported"
```

### Presenter Structure Verification

```bash
# Verify ChatPresenter structure
echo ""
echo "=== Checking ChatPresenter structure ==="
grep "pub struct ChatPresenter" src/presentation/chat.rs && echo "[OK] ChatPresenter defined" || echo " ChatPresenter missing"
grep "rx:" src/presentation/chat.rs && echo "[OK] event_rx field present" || echo " event_rx missing"
grep "services:" src/presentation/chat.rs && echo "[OK] services field present" || echo " services missing"
grep "view_tx:" src/presentation/chat.rs && echo "[OK] view_tx field present" || echo " view_tx missing"
grep "running:" src/presentation/chat.rs && echo "[OK] running field present" || echo " running missing"

# Verify McpPresenter structure
echo ""
echo "=== Checking McpPresenter structure ==="
grep "pub struct McpPresenter" src/presentation/mcp.rs && echo "[OK] McpPresenter defined" || echo " McpPresenter missing"
grep "rx:" src/presentation/mcp.rs && echo "[OK] event_rx field present" || echo " event_rx missing"
grep "services:" src/presentation/mcp.rs && echo "[OK] services field present" || echo " services missing"
grep "view_tx:" src/presentation/mcp.rs && echo "[OK] view_tx field present" || echo " view_tx missing"

# Verify SettingsPresenter structure
echo ""
echo "=== Checking SettingsPresenter structure ==="
grep "pub struct SettingsPresenter" src/presentation/settings.rs && echo "[OK] SettingsPresenter defined" || echo " SettingsPresenter missing"
grep "rx:" src/presentation/settings.rs && echo "[OK] event_rx field present" || echo " event_rx missing"
grep "services:" src/presentation/settings.rs && echo "[OK] services field present" || echo " services missing"

# Verify ErrorPresenter structure
echo ""
echo "=== Checking ErrorPresenter structure ==="
grep "pub struct ErrorPresenter" src/presentation/error.rs && echo "[OK] ErrorPresenter defined" || echo " ErrorPresenter missing"
grep "rx:" src/presentation/error.rs && echo "[OK] event_rx field present" || echo " event_rx missing"
grep "view_tx:" src/presentation/error.rs && echo "[OK] view_tx field present" || echo " view_tx missing"
```

### ViewCommand Enum Verification

```bash
# Verify ViewCommand enum structure
echo ""
echo "=== Checking ViewCommand enum ==="
grep "pub enum ViewCommand" src/presentation/view_command.rs && echo "[OK] ViewCommand defined" || echo " ViewCommand missing"

# Check for chat command variants
echo ""
echo "Chat command variants:"
grep "ConversationCreated" src/presentation/view_command.rs && echo "[OK] ConversationCreated" || echo " Missing"
grep "MessageAppended" src/presentation/view_command.rs && echo "[OK] MessageAppended" || echo " Missing"
grep "ShowThinking" src/presentation/view_command.rs && echo "[OK] ShowThinking" || echo " Missing"
grep "HideThinking" src/presentation/view_command.rs && echo "[OK] HideThinking" || echo " Missing"
grep "AppendStream" src/presentation/view_command.rs && echo "[OK] AppendStream" || echo " Missing"
grep "FinalizeStream" src/presentation/view_command.rs && echo "[OK] FinalizeStream" || echo " Missing"

# Check for MCP command variants
echo ""
echo "MCP command variants:"
grep "McpServerStarted" src/presentation/view_command.rs && echo "[OK] McpServerStarted" || echo " Missing"
grep "McpToolsUpdated" src/presentation/view_command.rs && echo "[OK] McpToolsUpdated" || echo " Missing"

# Check for settings command variants
echo ""
echo "Settings command variants:"
grep "ShowSettings" src/presentation/view_command.rs && echo "[OK] ShowSettings" || echo " Missing"
grep "ShowNotification" src/presentation/view_command.rs && echo "[OK] ShowNotification" || echo " Missing"

# Check for error command variants
echo ""
echo "Error command variants:"
grep "ShowError" src/presentation/view_command.rs && echo "[OK] ShowError" || echo " Missing"
```

### Stub Method Verification

```bash
# Verify stub methods exist (unimplemented! is EXPECTED)
echo ""
echo "=== Checking stub methods (unimplemented! expected) ==="

# ChatPresenter methods
echo "ChatPresenter methods:"
grep "pub fn new" src/presentation/chat.rs && echo "[OK] new() defined" || echo " new() missing"
grep "pub async fn start" src/presentation/chat.rs && echo "[OK] start() defined" || echo " start() missing"
grep "pub async fn stop" src/presentation/chat.rs && echo "[OK] stop() defined" || echo " stop() missing"
grep "fn handle_event" src/presentation/chat.rs && echo "[OK] handle_event() defined" || echo " handle_event() missing"
grep "fn handle_user_event" src/presentation/chat.rs && echo "[OK] handle_user_event() defined" || echo " handle_user_event() missing"
grep "fn handle_chat_event" src/presentation/chat.rs && echo "[OK] handle_chat_event() defined" || echo " handle_chat_event() missing"

# McpPresenter methods
echo ""
echo "McpPresenter methods:"
grep "pub fn new" src/presentation/mcp.rs && echo "[OK] new() defined" || echo " new() missing"
grep "pub async fn start" src/presentation/mcp.rs && echo "[OK] start() defined" || echo " start() missing"
grep "pub async fn stop" src/presentation/mcp.rs && echo "[OK] stop() defined" || echo " stop() missing"
grep "fn handle_event" src/presentation/mcp.rs && echo "[OK] handle_event() defined" || echo " handle_event() missing"
grep "fn handle_user_event" src/presentation/mcp.rs && echo "[OK] handle_user_event() defined" || echo " handle_user_event() missing"
grep "fn handle_mcp_event" src/presentation/mcp.rs && echo "[OK] handle_mcp_event() defined" || echo " handle_mcp_event() missing"

# SettingsPresenter methods
echo ""
echo "SettingsPresenter methods:"
grep "pub fn new" src/presentation/settings.rs && echo "[OK] new() defined" || echo " new() missing"
grep "pub async fn start" src/presentation/settings.rs && echo "[OK] start() defined" || echo " start() missing"
grep "pub async fn stop" src/presentation/settings.rs && echo "[OK] stop() defined" || echo " stop() missing"

# ErrorPresenter methods
echo ""
echo "ErrorPresenter methods:"
grep "pub fn new" src/presentation/error.rs && echo "[OK] new() defined" || echo " new() missing"
grep "pub async fn start" src/presentation/error.rs && echo "[OK] start() defined" || echo " start() missing"
grep "pub async fn stop" src/presentation/error.rs && echo "[OK] stop() defined" || echo " stop() missing"
```

### Stub Implementation Verification

```bash
# Verify unimplemented! in presenter methods (EXPECTED in this phase)
echo ""
echo "=== Checking stub implementations (unimplemented! expected) ==="
grep -r "unimplemented!" src/presentation/*.rs | grep -v "test" | wc -l
echo "Expected: 25+ unimplemented! calls"

# Verify no real implementation yet (spawn, async tasks)
echo ""
echo "=== Checking no real implementation yet ==="
if grep -r "spawn(" src/presentation/*.rs | grep -v "test"; then
  echo "WARNING: Found spawn() calls (should be stubs)"
else
  echo "PASS: No spawn() calls (stubs only)"
fi

if grep -r "WHILE.*running" src/presentation/*.rs | grep -v "test"; then
  echo "WARNING: Found event loop implementations (should be stubs)"
else
  echo "PASS: No event loop implementations (stubs only)"
fi
```

### Plan Marker Verification

```bash
# Verify plan markers
echo ""
echo "=== Checking plan markers ==="
grep -r "@plan:PLAN-20250125-REFACTOR.P10" src/presentation/ | wc -l
echo "Expected: 30+ occurrences"

# List files with plan markers
for file in src/presentation/*.rs; do
  count=$(grep "@plan:PLAN-20250125-REFACTOR.P10" "$file" | wc -l)
  echo "$(basename $file): $count markers"
done

# Verify requirement markers
echo ""
echo "=== Checking requirement markers ==="
grep -r "@requirement:REQ-025" src/presentation/ | wc -l
echo "Expected: 15+ occurrences"
```

### Compilation Verification

```bash
# Verify code compiles
echo ""
echo "=== Compiling presentation module ==="
cargo build --lib 2>&1 | tee compile_verification.log

# Check for compilation errors
if grep -i "error:" compile_verification.log; then
  echo " Compilation errors found"
  grep "error\[" compile_verification.log
  exit 1
else
  echo "PASS: Code compiles successfully"
fi
```

## Success Criteria

- All 6 presenter files created (mod.rs + 5 files)
- All presenter structs defined
- ViewCommand enum fully defined
- All stub methods present (unimplemented!)
- Plan markers present in all files
- Requirement markers traceable
- Code compiles successfully
- No real implementation yet (stubs only)

## Failure Recovery

If verification fails:

1. If files missing:
   - Create missing presenter files
   - Add missing struct definitions
   - Add missing stub methods

2. If ViewCommand incomplete:
   - Add missing command variants
   - Ensure all categories covered

3. If compilation fails:
   - Fix syntax errors
   - Fix type mismatches
   - Ensure all imports correct

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P10A.md`

Contents:

```markdown
Phase: P10A
Completed: YYYY-MM-DD HH:MM
Verification Results:
  Presenter Files: 6/6 created
  Presenter Structs: 4/4 defined
  ViewCommand Enum: Complete
  Stub Methods: 25+
  Plan Markers: 30+ found
  Requirement Markers: 15+ found
  Compilation: PASS
Structures Verified:
  - ChatPresenter: PASS
  - McpPresenter: PASS
  - SettingsPresenter: PASS
  - ErrorPresenter: PASS
  - ViewCommand: PASS (all variants)
Stubs Verified:
  - unimplemented! calls: 25+
  - spawn() calls: 0 (stubs only)
  - Event loops: 0 (stubs only)
Ready for Phase 11: YES
```

## Next Steps

After successful verification:

1. All presenter stubs are in place
2. ViewCommand enum is complete
3. Code compiles successfully
4. Proceed to Phase 11: Presenter TDD (write tests)
5. Tests will fail against stubs (expected)

## Important Notes

- This is a VERIFICATION phase - no code changes expected
- All methods should be stubs (unimplemented!)
- ViewCommand enum should be fully defined
- Presenters are stateless (except event receivers)
- Next phase will write comprehensive tests
- Phase 12 will implement real functionality
