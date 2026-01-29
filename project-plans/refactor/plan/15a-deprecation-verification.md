# Phase 15a: Deprecation Verification

## Phase ID

`PLAN-20250125-REFACTOR.P15A`

## Prerequisites

- Required: Phase 15 (Deprecation) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P15" src/ui/*.rs`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P15.md`
  - Deprecated code removed
  - Application builds successfully
  - All tests pass
- Preflight verification: Phases 01-15 completed

## Purpose

Verify that the deprecation phase was completed correctly and all old code has been safely removed without breaking functionality. This phase:

1. **Verifies code removal** - All deprecated code successfully removed
2. **Verifies no regressions** - All functionality still works
3. **Verifies architecture compliance** - No direct service access from UI
4. **Verifies code quality** - No warnings, clean build
5. **Documents cleanup completeness**

**Note:** This is a VERIFICATION phase. No code changes expected.

## Verification Tasks

### Structural Verification

#### 1.1: Direct Service Call Removal

**Check**: No direct service calls remain in UI layer

```bash
# Check for direct LlmService calls
grep -r "LlmService" src/ui/*.rs | grep -v "/// " | grep -v "// " | grep -v "use "
# Expected: 0 matches

# Check for direct McpService calls
grep -r "McpService" src/ui/*.rs | grep -v "/// " | grep -v "// " | grep -v "use "
# Expected: 0 matches

# Check for direct ProfileService calls
grep -r "ProfileService" src/ui/*.rs | grep -v "/// " | grep -v "// " | grep -v "use "
# Expected: 0 matches

# Check for direct ConversationStorage instantiation
grep -r "ConversationStorage::with_default_path" src/ui/*.rs | grep -v "/// " | grep -v "// "
# Expected: 0 matches
```

**Expected Results**:
- [OK] 0 direct LlmService calls
- [OK] 0 direct McpService calls
- [OK] 0 direct ProfileService calls
- [OK] 0 direct ConversationStorage instantiation

#### 1.2: Helper Function Removal

**Check**: Old helper functions removed

```bash
# Check for removed helper functions in chat_view_helpers
grep -r "start_streaming_request\|collect_profile\|reset_streaming_buffers" src/ui/chat_view_helpers/*.rs | grep -v "/// " | grep -v "// " | grep "pub fn\|pub async fn"
# Expected: 0 matches (functions removed)

# Check if file is significantly smaller or empty
wc -l src/ui/chat_view_helpers/streaming.rs
# Expected: Significantly reduced line count (or file removed if empty)

# Check for unused helper imports
grep -r "use.*chat_view_helpers" src/ui/*.rs
# Expected: 0 matches (or only necessary imports)
```

**Expected Results**:
- [OK] Old helper functions removed
- [OK] Unused imports removed
- [OK] File sizes reduced

#### 1.3: Plan Markers

**Check**: Plan markers present for removal tracking

```bash
# Check for Phase 15 plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P15" src/ui/*.rs | wc -l
# Expected: 5+ occurrences (marking removal sites)

# Check for requirement markers
grep -r "@requirement:REQ-026" src/ui/*.rs | wc -l
# Expected: 3+ occurrences
```

**Expected Results**:
- [OK] Plan markers present
- [OK] Requirement markers present
- [OK] Removal sites documented

### Semantic Verification

#### 2.1: Architecture Compliance

**Check**: UI layer uses presenters only

```bash
# Check for UserEvent emission (correct pattern)
grep -r "UserEvent::" src/ui/*.rs | grep "event_tx.send" | wc -l
# Expected: 10+ emission points

# Check for ViewCommand reception (correct pattern)
grep -r "ViewCommand::" src/ui/*.rs | grep "handle_view_command\|match.*cmd" | wc -l
# Expected: 20+ handling points

# Check for presenter references (correct pattern)
grep -r "presenter:" src/ui/*.rs | wc -l
# Expected: 4+ presenter references
```

**Expected Results**:
- [OK] All UI → Service communication via UserEvents
- [OK] All Service → UI communication via ViewCommands
- [OK] All presenters properly referenced

#### 2.2: Build Verification

**Check**: Application builds without errors

```bash
# Clean build
cargo clean

# Build release
cargo build --release 2>&1 | tee build.log

# Check for errors
grep -E "^error" build.log
# Expected: 0 matches

# Check for warnings
grep -E "^warning" build.log
# Expected: 0 warnings (or only acceptable documented warnings)

# Check for unused imports (should be 0 after cleanup)
grep -E "unused_imports" build.log
# Expected: 0 matches
```

**Expected Results**:
- [OK] Application builds successfully
- [OK] 0 compilation errors
- [OK] 0 warnings (or only acceptable)
- [OK] 0 unused import warnings

#### 2.3: Test Suite Verification

**Check**: All tests still pass after code removal

```bash
# Run all tests
cargo test --lib 2>&1 | tee test_results.log

# Check test summary
grep -E "test result:" test_results.log | tail -1
# Expected: test result: ok. X passed in Ys

# Verify no failures
grep -E "FAILED" test_results.log
# Expected: 0 matches

# Verify no panics
grep -E "panicked" test_results.log
# Expected: 0 matches

# Check for removed tests (if old tests removed)
grep -E "test.*removed\|test.*deprecated" test_results.log
# Expected: 0 matches (or documented)
```

**Expected Results**:
- [OK] All tests pass
- [OK] 0 test failures
- [OK] 0 test panics
- [OK] Test count stable (or documented reductions)

### Functional Verification

#### 3.1: Chat View Functionality

**Test**: Chat view works without old code

**Manual Testing**:
1. **Send Message**:
   - [ ] Open chat view
   - [ ] Type message
   - [ ] Send message
   - [ ] Verify response received
   - [ ] Verify message appears in UI
   - [ ] Verify no errors in logs

2. **Stream Response**:
   - [ ] Send message that generates long response
   - [ ] Verify streaming works
   - [ ] Verify chunks appear progressively
   - [ ] Verify thinking indicator shows/hides
   - [ ] Verify final message complete

3. **Conversation History**:
   - [ ] Load existing conversation
   - [ ] Verify all messages present
   - [ ] Verify scrolling works
   - [ ] Verify message order correct

**Expected Results**:
- [OK] Can send messages
- [OK] Streaming works
- [OK] History accessible
- [OK] No errors in logs

#### 3.2: MCP View Functionality

**Test**: MCP views work without old code

**Manual Testing**:
1. **Start MCP Server**:
   - [ ] Open MCP configuration
   - [ ] Add MCP server (e.g., filesystem)
   - [ ] Start server
   - [ ] Verify server status updates
   - [ ] Verify tools listed
   - [ ] Verify no errors

2. **Stop MCP Server**:
   - [ ] Stop running server
   - [ ] Verify status updates
   - [ ] Verify tools removed
   - [ ] Verify no errors

**Expected Results**:
- [OK] Can start MCP servers
- [OK] Can stop MCP servers
- [OK] Server status updates correctly
- [OK] Tools display correctly

#### 3.3: Settings View Functionality

**Test**: Settings view works without old code

**Manual Testing**:
1. **View Profiles**:
   - [ ] Open settings
   - [ ] Verify all profiles listed
   - [ ] Verify profile details correct
   - [ ] Verify default profile marked

2. **Update Profile**:
   - [ ] Edit existing profile
   - [ ] Change model name
   - [ ] Save changes
   - [ ] Verify notification shown
   - [ ] Verify changes persist

3. **Add Profile**:
   - [ ] Add new profile
   - [ ] Enter details
   - [ ] Save profile
   - [ ] Verify profile appears in list

**Expected Results**:
- [OK] Can view profiles
- [OK] Can update profiles
- [OK] Can add profiles
- [OK] Changes persist

#### 3.4: History View Functionality

**Test**: History view works without old code

**Manual Testing**:
1. **View Conversations**:
   - [ ] Open history view
   - [ ] Verify all conversations listed
   - [ ] Verify conversation titles correct
   - [ ] Verify timestamps correct

2. **Load Conversation**:
   - [ ] Select conversation
   - [ ] Load conversation
   - [ ] Verify all messages present
   - [ ] Verify message content correct

**Expected Results**:
- [OK] Can view conversation list
- [OK] Can load conversations
- [OK] All data accessible

### Code Quality Verification

#### 4.1: Line Count Reduction

**Check**: Code reduced as expected

```bash
# Count lines in modified files
wc -l src/ui/chat_view.rs
# Expected: Reduced by ~200-300 lines from original

wc -l src/ui/chat_view_helpers/streaming.rs
# Expected: Reduced by ~50-100 lines

wc -l src/ui/chat_view_helpers/helpers.rs
# Expected: Reduced by ~30-50 lines

# Total UI lines
find src/ui -name "*.rs" -exec wc -l {} + | tail -1
# Expected: Total reduced by ~500-800 lines
```

**Expected Results**:
- [OK] chat_view.rs reduced
- [OK] streaming.rs reduced
- [OK] helpers.rs reduced
- [OK] Total reduction ~500-800 lines

#### 4.2: Import Cleanup

**Check**: Unused imports removed

```bash
# Run clippy to check for unused imports
cargo clippy -- -W unused_imports 2>&1 | grep "unused_import"
# Expected: 0 matches

# Check for obviously unused imports
grep -r "^use " src/ui/*.rs | while read line; do
    import=$(echo "$line" | sed 's/use //;s/;.*//');
    if ! grep -q "$(echo $import | sed 's/.*:://')" src/ui/*.rs; then
        echo "Possibly unused: $line";
    fi;
done
# Expected: 0 unused imports
```

**Expected Results**:
- [OK] 0 unused import warnings
- [OK] All imports used
- [OK] No dead imports

#### 4.3: Code Complexity

**Check**: Code complexity reduced

```bash
# Check for long functions (should be fewer)
grep -r "^pub fn\|^pub async fn" src/ui/chat_view.rs | awk '{print $0}' | while read line; do
    # Count lines in function (manual check needed)
done
# Expected: Fewer long functions (logic moved to presenters)

# Check for nested match/if statements (should be simpler)
grep -r "^    [[:space:]]*if\|^    [[:space:]]*match" src/ui/chat_view.rs | wc -l
# Expected: Reduced complexity (logic in presenters)
```

**Expected Results**:
- [OK] Fewer long functions
- [OK] Reduced nesting
- [OK] Simpler control flow

### Integration Verification

#### 5.1: Presenter Integration

**Check**: Presenters handle all business logic

```bash
# Verify presenters are instantiated in views
grep -r "Presenter::new" src/ui/*.rs
# Expected: 4+ instantiations (ChatPresenter, McpPresenter, etc.)

# Verify presenters started
grep -r "\.start()" src/ui/*.rs | grep -i presenter
# Expected: 4+ start calls

# Verify event loop running
grep -r "spawn.*presenter\|presenter.*spawn" src/ui/*.rs
# Expected: Background tasks for presenters
```

**Expected Results**:
- [OK] All presenters instantiated
- [OK] All presenters started
- [OK] Event loops running

#### 5.2: Service Layer Integration

**Check**: Services handle all data operations

```bash
# Verify service registry used
grep -r "ServiceRegistry\|get_service_registry" src/ui/*.rs
# Expected: References to service registry

# Verify no direct service instantiation (already checked)
# All service access via presenters
```

**Expected Results**:
- [OK] Service registry used
- [OK] No direct instantiation
- [OK] All access via presenters

## Verification Checklist

### Structural Checks

- [ ] No direct LlmService calls in UI
- [ ] No direct McpService calls in UI
- [ ] No direct ProfileService calls in UI
- [ ] No direct ConversationStorage instantiation
- [ ] Old helper functions removed
- [ ] Unused imports removed
- [ ] Plan markers present
- [ ] Requirement markers present

### Semantic Checks

- [ ] UserEvent emission correct
- [ ] ViewCommand handling correct
- [ ] Presenters referenced
- [ ] Application builds
- [ ] No compilation errors
- [ ] No warnings
- [ ] All tests pass
- [ ] No test failures

### Functional Checks

- [ ] Chat view functional
- [ ] Streaming works
- [ ] History accessible
- [ ] MCP views functional
- [ ] Server management works
- [ ] Settings view functional
- [ ] Profile management works
- [ ] History view functional
- [ ] Conversations load

### Code Quality Checks

- [ ] Code reduced (~500-800 lines)
- [ ] Unused imports removed
- [ ] Functions simpler
- [ ] Complexity reduced
- [ ] No dead code

### Integration Checks

- [ ] Presenters instantiated
- [ ] Presenters started
- [ ] Event loops running
- [ ] Service registry used
- [ ] No direct instantiation

## Success Criteria

- ALL structural checks pass (8/8)
- ALL semantic checks pass (8/8)
- ALL functional checks pass (9/9)
- ALL code quality checks pass (5/5)
- ALL integration checks pass (5/5)
- **Overall**: 35/35 checks pass

## Failure Recovery

If verification fails:

1. **Structural Failures**:
   - Remove remaining direct service calls
   - Remove remaining direct storage access
   - Remove remaining old helpers

2. **Semantic Failures**:
   - Fix compilation errors
   - Address warnings
   - Fix failing tests

3. **Functional Failures**:
   - **CRITICAL**: Restore accidentally removed code
   - Verify functionality wasn't tied to removed code
   - Re-implement if needed (in presenter layer)

4. **Code Quality Failures**:
   - Remove remaining unused imports
   - Simplify overly complex functions
   - Further reduction if possible

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P15A.md`

Contents:

```markdown
Phase: P15A
Completed: YYYY-MM-DD HH:MM
Verification Status:
  Structural: 8/8 PASSED
  Semantic: 8/8 PASSED
  Functional: 9/9 PASSED
  Code Quality: 5/5 PASSED
  Integration: 5/5 PASSED
  Total: 35/35 PASSED

Code Removal Summary:
  - Direct service calls: REMOVED (0 remaining)
  - Direct storage access: REMOVED (0 remaining)
  - Old helper functions: REMOVED
  - Unused imports: REMOVED
  - Total lines removed: ~500-800

Architecture Compliance:
  - UI → Presenter → Service: ENFORCED
  - Event-driven: VERIFIED
  - No direct access: VERIFIED

Build Verification:
  - Compilation: PASS
  - Warnings: 0
  - Tests: PASS

Functional Verification:
  - Chat view: WORKING
  - MCP views: WORKING
  - Settings view: WORKING
  - History view: WORKING

Code Quality:
  - Complexity: REDUCED
  - Maintainability: IMPROVED
  - Architecture: CLEAN

Notes:
  - All deprecated code successfully removed
  - No functionality lost
  - Architecture boundaries enforced
  - Ready for Phase 16
```

## Next Steps

After successful completion of this phase:

1. Deprecation verified complete
2. All old code safely removed
3. Architecture clean and compliant
4. Proceed to Phase 16: End-to-End Testing
5. Perform full system testing

## Important Notes

- **CLEAN ARCHITECTURE**: UI layer now completely decoupled from services
- **EVENT-DRIVEN**: All communication via events
- **NO REGRESSIONS**: All functionality verified
- **READY FOR E2E**: System clean for comprehensive testing
- **MAINTAINABILITY**: Significantly improved
