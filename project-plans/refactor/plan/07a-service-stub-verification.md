# Phase 07a: Service Stub Verification

## Phase ID

`PLAN-20250125-REFACTOR.P07A`

## Prerequisites

- Required: Phase 07 (Service Stub) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P07" src/services/ | wc -l`
- Expected from previous phase:
  - `src/services/mod.rs` - Module exports
  - `src/services/conversation.rs` - ConversationService stub
  - `src/services/chat.rs` - ChatService stub
  - `src/services/mcp.rs` - McpService stub
  - `src/services/profile.rs` - ProfileService stub
  - `src/services/secrets.rs` - SecretsService stub
  - `src/lib.rs` - Module declaration

## Purpose

Verify that the service layer stub phase created all necessary files and code structure. This phase:

1. Verifies all service files were created
2. Verifies code compiles (with stubs)
3. Verifies plan markers are present
4. Verifies requirement markers are present
5. Verifies stub implementations are present (not real implementation)
6. Creates verification report

**Note:** This is a VERIFICATION phase. No code is written. Only verification commands are run.

## Requirements Verified

This phase verifies the stub implementations from Phase 07:

- **REQ-022.1**: ConversationService structure defined (stub)
- **REQ-022.2**: ChatService structure defined (stub)
- **REQ-022.3**: McpService structure defined (stub)
- **REQ-022.4**: ProfileService structure defined (stub)
- **REQ-022.5**: SecretsService structure defined (stub)
- **REQ-022.6**: All services have Arc<Mutex<T>> state (placeholder types)
- **REQ-022.7**: All services accept event_tx parameter (placeholder)

## Verification Tasks

### Structural Verification

#### 1. File Existence Checks

```bash
# Verify all files created
echo "=== File Existence Checks ==="

test -f src/services/mod.rs && echo "[OK] mod.rs exists" || echo " mod.rs missing"
test -f src/services/conversation.rs && echo "[OK] conversation.rs exists" || echo " conversation.rs missing"
test -f src/services/chat.rs && echo "[OK] chat.rs exists" || echo " chat.rs missing"
test -f src/services/mcp.rs && echo "[OK] mcp.rs exists" || echo " mcp.rs missing"
test -f src/services/profile.rs && echo "[OK] profile.rs exists" || echo " profile.rs missing"
test -f src/services/secrets.rs && echo "[OK] secrets.rs exists" || echo " secrets.rs missing"
test -f src/lib.rs && echo "[OK] lib.rs exists" || echo " lib.rs missing"

echo "Expected: All files exist"
```

#### 2. Module Declaration Verification

```bash
# Verify module is declared in lib.rs
echo "=== Module Declaration Check ==="

grep -n "pub mod services;" src/lib.rs
echo "Expected: Line with 'pub mod services;' found"

# Verify mod.rs exports required items
echo ""
echo "=== Module Exports Check ==="

grep -E "pub use (ConversationService|ChatService|McpService|ProfileService|SecretsService);" src/services/mod.rs
echo "Expected: 5 service exports found"
```

#### 3. Compilation Verification

```bash
# Verify code compiles (stubs allowed)
echo "=== Compilation Check ==="

cargo build --lib 2>&1 | tee build.log

if [ $? -eq 0 ]; then
    echo "[OK] Build successful"
else
    echo " Build failed"
    exit 1
fi

echo "Expected: Build succeeds (with warnings about unused code OK)"
```

### Marker Verification

#### 4. Plan Marker Verification

```bash
# Verify plan markers present
echo "=== Plan Marker Check ==="

grep -r "@plan:PLAN-20250125-REFACTOR.P07" src/services/ | wc -l
echo "Expected: 30+ occurrences (all services have multiple markers)"

# List files with plan markers
echo ""
echo "Files with plan markers:"
grep -l "@plan:PLAN-20250125-REFACTOR.P07" src/services/*.rs
echo "Expected: All 6 files listed"
```

#### 5. Requirement Marker Verification

```bash
# Verify requirement markers present
echo "=== Requirement Marker Check ==="

grep -r "@requirement:REQ-022" src/services/ | wc -l
echo "Expected: 20+ occurrences"

# Breakdown by requirement
echo ""
echo "Requirement breakdown:"
echo "REQ-022.1:" $(grep -r "@requirement:REQ-022.1" src/services/ | wc -l)
echo "REQ-022.2:" $(grep -r "@requirement:REQ-022.2" src/services/ | wc -l)
echo "REQ-022.3:" $(grep -r "@requirement:REQ-022.3" src/services/ | wc -l)
echo "REQ-022.4:" $(grep -r "@requirement:REQ-022.4" src/services/ | wc -l)
echo "REQ-022.5:" $(grep -r "@requirement:REQ-022.5" src/services/ | wc -l)
echo "REQ-022.6:" $(grep -r "@requirement:REQ-022.6" src/services/ | wc -l)
echo "REQ-022.7:" $(grep -r "@requirement:REQ-022.7" src/services/ | wc -l)
echo "Expected: Each requirement has 2+ markers"
```

#### 6. Pseudocode Reference Verification

```bash
# Verify pseudocode references
echo "=== Pseudocode Reference Check ==="

grep -r "@pseudocode services.md" src/services/ | wc -l
echo "Expected: 20+ occurrences"

# List pseudocode line references
echo ""
echo "Pseudocode line references:"
grep -r "@pseudocode" src/services/ | grep "services.md" | head -20
echo "Expected: Line numbers like 'lines 10-25', 'lines 35-50', etc."
```

### Stub Implementation Verification

#### 7. Stub Method Verification

```bash
# Verify all methods are stubs
echo "=== Stub Method Check ==="

grep -r "unimplemented!" src/services/*.rs | grep -v "tests" | wc -l
echo "Expected: 25+ unimplemented!() calls (all service methods are stubs)"

# List stub methods by service
echo ""
echo "Stub methods by service:"
echo "ConversationService:"
grep -n "unimplemented!" src/services/conversation.rs | wc -l
echo "ChatService:"
grep -n "unimplemented!" src/services/chat.rs | wc -l
echo "McpService:"
grep -n "unimplemented!" src/services/mcp.rs | wc -l
echo "ProfileService:"
grep -n "unimplemented!" src/services/profile.rs | wc -l
echo "SecretsService:"
grep -n "unimplemented!" src/services/secrets.rs | wc -l
```

#### 8. No Real Implementation Verification

```bash
# Verify NO real implementation yet
echo "=== No Real Implementation Check ==="

# Check for Arc<Mutex<T>> usage (should be minimal/placeholder in stubs)
if grep -q "Arc::new(Mutex::new" src/services/*.rs; then
    echo " Real implementation found (Arc::new(Mutex::new))"
    grep -n "Arc::new(Mutex::new" src/services/*.rs
else
    echo "[OK] No Arc::new(Mutex::new) (correct for stub phase)"
fi

# Check for real event emission (should NOT exist yet)
if grep -q "event_tx\.send" src/services/*.rs; then
    echo " Real implementation found (event_tx.send)"
    grep -n "event_tx\.send" src/services/*.rs
else
    echo "[OK] No event_tx.send (correct for stub phase)"
fi

echo "Expected: No real implementation, only stubs"
```

### Type Definition Verification

#### 9. Service Struct Verification

```bash
# Verify all service structs exist
echo "=== Service Struct Check ==="

echo "ConversationService:"
grep -A 10 "pub struct ConversationService" src/services/conversation.rs
echo "Expected: Struct definition found"

echo ""
echo "ChatService:"
grep -A 10 "pub struct ChatService" src/services/chat.rs
echo "Expected: Struct definition found"

echo ""
echo "McpService:"
grep -A 10 "pub struct McpService" src/services/mcp.rs
echo "Expected: Struct definition found"

echo ""
echo "ProfileService:"
grep -A 10 "pub struct ProfileService" src/services/profile.rs
echo "Expected: Struct definition found"

echo ""
echo "SecretsService:"
grep -A 10 "pub struct SecretsService" src/services/secrets.rs
echo "Expected: Struct definition found"
```

#### 10. Service Method Verification

```bash
# Verify service methods exist (as stubs)
echo "=== Service Method Check ==="

echo "ConversationService methods:"
grep -n "pub fn" src/services/conversation.rs
echo "Expected: new(), start_conversation(), send_message(), cancel_request(), get_conversation()"

echo ""
echo "ChatService methods:"
grep -n "pub fn" src/services/chat.rs
echo "Expected: new(), set_model(), set_temperature(), set_max_tokens()"

echo ""
echo "McpService methods:"
grep -n "pub fn" src/services/mcp.rs
echo "Expected: new(), start_server(), stop_server(), list_tools(), call_tool()"

echo ""
echo "ProfileService methods:"
grep -n "pub fn" src/services/profile.rs
echo "Expected: new(), load_profile(), save_profile(), get_default_profile()"

echo ""
echo "SecretsService methods:"
grep -n "pub fn" src/services/secrets.rs
echo "Expected: new(), get_secret(), set_secret(), delete_secret()"
```

### Manual Verification Checklist

#### Code Review Checks

**Review each file and answer:**

##### src/services/mod.rs
- [ ] File exists
- [ ] Exports ConversationService
- [ ] Exports ChatService
- [ ] Exports McpService
- [ ] Exports ProfileService
- [ ] Exports SecretsService
- [ ] Plan marker present
- [ ] No implementation code (only exports)

##### src/services/conversation.rs
- [ ] File exists
- [ ] ConversationService struct defined
- [ ] new() method signature correct (takes event_tx)
- [ ] All other methods defined as stubs
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.1 marker
- [ ] Pseudocode reference present
- [ ] No real implementation

##### src/services/chat.rs
- [ ] File exists
- [ ] ChatService struct defined
- [ ] new() method signature correct (takes event_tx)
- [ ] All configuration methods defined as stubs
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.2 marker
- [ ] Pseudocode reference present
- [ ] No real implementation

##### src/services/mcp.rs
- [ ] File exists
- [ ] McpService struct defined
- [ ] new() method signature correct (takes event_tx)
- [ ] All server/tool methods defined as stubs
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.3 marker
- [ ] Pseudocode reference present
- [ ] No real implementation

##### src/services/profile.rs
- [ ] File exists
- [ ] ProfileService struct defined
- [ ] new() method signature correct (takes event_tx)
- [ ] All profile methods defined as stubs
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.4 marker
- [ ] Pseudocode reference present
- [ ] No real implementation

##### src/services/secrets.rs
- [ ] File exists
- [ ] SecretsService struct defined
- [ ] new() method signature correct (takes event_tx)
- [ ] All secret methods defined as stubs
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.5 marker
- [ ] Pseudocode reference present
- [ ] No real implementation

##### src/lib.rs
- [ ] pub mod services; line added
- [ ] Plan marker comment added
- [ ] No other changes (minimal modification)

## Success Criteria

### Automated Checks

- All 6 files created (mod.rs + 5 service files)
- lib.rs modified to declare services module
- Code compiles successfully
- 30+ plan markers found
- 20+ requirement markers found
- 25+ stub methods (unimplemented!())
- No real implementation found

### Manual Verification

- All service structs defined with correct fields
- All service methods defined as stubs
- All services accept event_tx parameter
- All stubs use unimplemented!()
- Plan markers present in all files
- Pseudocode references present
- Module exports correct services

## Failure Recovery

If verification fails:

### If files are missing

```bash
# Identify missing files
test -f src/services/mod.rs || echo "mod.rs missing"
test -f src/services/conversation.rs || echo "conversation.rs missing"
test -f src/services/chat.rs || echo "chat.rs missing"
test -f src/services/mcp.rs || echo "mcp.rs missing"
test -f src/services/profile.rs || echo "profile.rs missing"
test -f src/services/secrets.rs || echo "secrets.rs missing"

# Re-run Phase 07 to create missing files
```

### If compilation fails

```bash
# Check build errors
cat build.log

# Fix compilation errors:
# 1. Missing imports
# 2. Syntax errors
# 3. Type mismatches
# 4. Missing derives

# DO NOT add real implementation - fix only compilation issues
```

### If markers are missing

```bash
# Find files without plan markers
for file in src/services/*.rs; do
    if ! grep -q "@plan:PLAN-20250125-REFACTOR.P07" "$file"; then
        echo "Missing plan marker: $file"
    fi
done

# Add missing markers manually
```

### If real implementation found

```bash
# This is verification phase - should NOT have implementation
# If found, revert to stub:

# Check what implementation exists
grep -rn "Arc::new(Mutex::new\|event_tx\.send" src/services/

# Revert to Phase 07 stub approach
```

## Verification Report Template

After running all verification commands, create a report:

```markdown
# Service Stub Verification Report

**Date**: YYYY-MM-DD HH:MM
**Phase**: P07A
**Previous Phase**: P07 (Service Stub)

## File Creation Status

- [ ] src/services/mod.rs - Created
- [ ] src/services/conversation.rs - Created
- [ ] src/services/chat.rs - Created
- [ ] src/services/mcp.rs - Created
- [ ] src/services/profile.rs - Created
- [ ] src/services/secrets.rs - Created
- [ ] src/lib.rs - Modified

## Compilation Status

- [ ] cargo build --lib - PASS/FAIL
- Build time: X.XXs
- Warnings: N
- Errors: N

## Marker Status

- Plan markers: N (expected 30+)
- Requirement markers: N (expected 20+)
- Pseudocode references: N (expected 20+)

## Stub Implementation Status

- Stub methods: N (expected 25+)
- Real implementation found: YES/NO

## Service Struct Status

- [ ] ConversationService defined
- [ ] ChatService defined
- [ ] McpService defined
- [ ] ProfileService defined
- [ ] SecretsService defined

## Manual Verification Status

- [ ] All files reviewed
- [ ] All signatures correct
- [ ] All stubs confirmed
- [ ] No real implementation found

## Issues Found

[List any issues discovered during verification]

## Resolution

[Any issues resolved? If yes, how?]

## Recommendation

- [ ] Proceed to Phase 08 (Service TDD)
- [ ] Re-run Phase 07 (fix issues first)
- [ ] Blocked - requires [what?]
```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P07A.md`

Contents:

```markdown
Phase: P07A
Completed: YYYY-MM-DD HH:MM
Files Verified:
  - src/services/mod.rs
  - src/services/conversation.rs
  - src/services/chat.rs
  - src/services/mcp.rs
  - src/services/profile.rs
  - src/services/secrets.rs
  - src/lib.rs
Files Created: 0 (verification phase)
Files Modified: 0 (verification phase)
Tests Added: 0 (verification phase)
Verification Results:
  - File existence: PASS (7/7 files)
  - Compilation: PASS
  - Plan markers: PASS (N markers found)
  - Requirement markers: PASS (N markers found)
  - Stub methods: PASS (N stubs found)
  - No real implementation: PASS
Report: service-stub-verification-report.md
```

## Next Steps

After successful verification:

1. Verification report confirms all stubs are in place
2. Proceed to Phase 08: Service TDD (write tests)
3. Tests will fail against stubs (expected)
4. Phase 09 will implement to make tests pass

## Important Notes

- This is a VERIFICATION ONLY phase
- DO NOT write any code
- DO NOT fix any issues found (document them in report)
- IF issues found, decide: proceed with issues or revert to P07
- Stub methods with `unimplemented!()` are EXPECTED and CORRECT
- Real implementation should NOT be present
- All 5 services must be verified before proceeding
