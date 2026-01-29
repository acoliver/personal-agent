# Phase 15: Deprecation Phase

## Phase ID

`PLAN-20250125-REFACTOR.P15`

## Prerequisites

- Required: Phase 14a (Migration Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P14A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P14A.md`
  - Data migration verified and complete
  - All user data accessible through new architecture
  - Rollback tested and functional
- Preflight verification: Phases 01-14a completed

## Purpose

Remove deprecated code paths and old implementations that have been replaced by the new architecture. This phase:

1. **Removes direct service calls** from UI layer (already replaced by presenters)
2. **Removes old helper functions** that are now handled by services
3. **Removes deprecated storage access patterns** (now through services)
4. **Cleans up unused imports and dependencies**
5. **Simplifies codebase** by removing redundant code

**Note:** This is a CLEANUP phase. Only remove code that is confirmed to be unused after migration.

## Requirements Implemented (Expanded)

### REQ-026.1: Remove Direct Service Access

**Full Text**: UI layer MUST NOT access services directly (only via presenters).

**Behavior**:
- GIVEN: UI code with direct service calls
- WHEN: Deprecation phase runs
- THEN: All direct service calls removed
- AND: All service access via presenters only
- AND: Code compiles without removed code

**Why This Matters**: Enforces architecture boundaries and simplifies maintenance.

### REQ-026.2: Remove Old Helper Functions

**Full Text**: Old UI helper functions MUST be removed if replaced by service methods.

**Behavior**:
- GIVEN: Helper functions in chat_view_helpers/
- WHEN: Functionality moved to services
- THEN: Old helper functions removed
- AND: Import statements updated
- AND: No compilation errors

**Why This Matters**: Reduces code duplication and confusion.

### REQ-026.3: Remove Deprecated Storage Patterns

**Full Text**: Direct storage access patterns MUST be removed (use services instead).

**Behavior**:
- GIVEN: Direct ConversationStorage access in UI code
- WHEN: Deprecation phase runs
- THEN: All storage access via ConversationService
- AND: Direct storage instantiation removed
- AND: Storage pattern centralized in service layer

**Why This Matters**: Centralizes data access logic.

## Files to Modify

### Chat View Cleanup

#### `src/ui/chat_view.rs`

**Code to Remove**:

1. **Direct Service Instantiation** (lines 40-60):
   ```rust
   // REMOVE: Direct LlmService instantiation
   use personal_agent::llm::LlmService;
   
   // REMOVE: Direct ConversationStorage instantiation
   if let Ok(storage) = ConversationStorage::with_default_path() {
       let conversations = storage.load_all()?;
       // ... direct storage access
   }
   ```

2. **Direct Service Calls** (lines 200-300):
   ```rust
   // REMOVE: Direct LlmService calls
   let response = self.llm_service.start_streaming_request(&content).await?;
   
   // REMOVE: Direct McpService calls
   let tools = self.mcp_service.fetch_tools().await?;
   ```

3. **Old Streaming Logic** (lines 600-700):
   ```rust
   // REMOVE: Direct streaming handling
   // Now handled by ChatPresenter via ViewCommands
   ```

**Lines to Delete**:
- Lines 40-60: Direct service imports and instantiation
- Lines 200-300: Direct service method calls
- Lines 600-700: Direct streaming logic
- Any remaining direct storage access patterns

**Expected Reduction**: ~200-300 lines

#### `src/ui/chat_view_helpers/streaming.rs`

**Code to Remove**:

1. **Streaming Request Function** (if entire functionality moved to ChatService):
   ```rust
   // REMOVE: start_streaming_request() if now in ChatService
   pub async fn start_streaming_request(...) -> Result<String> {
       // Moved to ChatService::send_message_stream
   }
   ```

2. **Stream Buffer Management** (if now handled by presenter):
   ```rust
   // REMOVE: Stream buffer functions if handled by ViewCommands
   pub fn reset_streaming_buffers(...) {
       // Now handled via ViewCommand::FinalizeStream
   }
   ```

**Lines to Delete**:
- Functions moved to ChatService (~50-100 lines)
- Functions now handled by ViewCommands (~30-50 lines)

#### `src/ui/chat_view_helpers/helpers.rs`

**Code to Remove**:

1. **Direct Storage Access** (lines 169-198):
   ```rust
   // REMOVE: Direct conversation loading
   if let Ok(storage) = ConversationStorage::with_default_path() {
       let conversations = storage.load_all()?;
       // ... direct access
   }
   ```

2. **Profile Collection** (if now handled by ProfileService):
   ```rust
   // REMOVE: collect_profile() if now in ProfileService
   pub fn collect_profile(...) -> ModelProfile {
       // Moved to ProfileService::get_profile
   }
   ```

**Lines to Delete**:
- Direct storage access (~30-50 lines)
- Functions moved to services (~40-60 lines)

### MCP View Cleanup

#### `src/ui/mcp_configure_view.rs`

**Code to Remove**:

1. **Direct McpService Access**:
   ```rust
   // REMOVE: Direct McpService instantiation
   use personal_agent::mcp::McpService;
   
   // REMOVE: Direct service calls
   mcp_service.start_server(config).await?;
   ```

2. **Old Configuration Management** (if now in McpService):
   ```rust
   // REMOVE: Direct config manipulation
   ```

**Lines to Delete**: ~50-100 lines

#### `src/ui/mcp_add_view.rs`

**Similar cleanup as mcp_configure_view.rs**

**Lines to Delete**: ~40-80 lines

### Settings View Cleanup

#### `src/ui/settings_view.rs`

**Code to Remove**:

1. **Direct ProfileService Access**:
   ```rust
   // REMOVE: Direct ProfileService instantiation
   use personal_agent::models::ModelProfile;
   
   // REMOVE: Direct profile manipulation
   profile.update_model(new_model);
   profile.save()?;
   ```

**Lines to Delete**: ~60-100 lines

### History View Cleanup

#### `src/ui/history_view.rs`

**Code to Remove**:

1. **Direct ConversationStorage Access** (lines 168-219):
   ```rust
   // REMOVE: Direct storage access
   if let Ok(storage) = ConversationStorage::with_default_path() {
       let conversations = storage.load_all()?;
       // ... direct access
   }
   ```

**Lines to Delete**: ~50-80 lines

## Deprecation Strategy

### Safe Removal Process

1. **Identify Unused Code**:
   - Search for direct service calls
   - Search for direct storage access
   - Search for old helper functions

2. **Verify Replacement Exists**:
   - Confirm service method exists
   - Confirm presenter handles event
   - Confirm ViewCommand emitted

3. **Remove Code**:
   - Delete unused functions
   - Remove unused imports
   - Update call sites (if any remain)

4. **Verify Compilation**:
   - Build application
   - Fix any remaining references
   - Run tests

### Dependency Cleanup

**Remove Unused Dependencies** (if any):
```bash
# Check for unused dependencies
cargo machete  # If installed

# Or manually review Cargo.toml
# Remove dependencies that are now unused
```

### Import Cleanup

**Remove Unused Imports**:
```bash
# Auto-fix unused imports
cargo clippy --fix --allow-dirty --allow-staged

# Manually review and remove
# - Direct service imports
# - Old storage imports
# - Unused helper imports
```

## Pseudocode References

### ViewCommand Usage (lines 510-541)
- Replace direct UI updates with ViewCommand emission
- Remove direct service calls

### Presenter Event Handling (lines 80-177)
- Verify all events handled by presenters
- Remove duplicate event handling in views

## Removal Patterns

### Pattern 1: Direct Service Call Removal

**Before**:
```rust
// REMOVE THIS
use personal_agent::llm::LlmService;

let response = self.llm_service.start_streaming_request(&content).await?;
```

**After**:
```rust
// Already replaced by (Phase 13)
let _ = self.event_tx.send(AppEvent::User(UserEvent::SendMessage {
    conversation_id: self.current_conversation_id,
    content,
}));
```

### Pattern 2: Direct Storage Access Removal

**Before**:
```rust
// REMOVE THIS
use personal_agent::storage::ConversationStorage;

if let Ok(storage) = ConversationStorage::with_default_path() {
    let conversations = storage.load_all()?;
    // ... process conversations
}
```

**After**:
```rust
// Already replaced by (Phase 13)
// Access via ConversationService through presenter
// Or via ViewCommands that update UI
```

### Pattern 3: Helper Function Removal

**Before**:
```rust
// REMOVE THIS (if moved to service)
pub fn collect_profile(...) -> ModelProfile {
    // ... implementation
}
```

**After**:
```rust
// Now via ProfileService
let profile = self.services.profiles.get_profile(profile_id)?;
```

## Verification Commands

### Structural Verification

```bash
# Verify no direct service calls remain in UI layer
grep -r "LlmService\|McpService\|ProfileService" src/ui/*.rs | grep -v "/// " | grep -v "// "
# Expected: 0 matches (all removed)

# Verify no direct storage access in UI layer
grep -r "ConversationStorage::with_default_path" src/ui/*.rs | grep -v "/// " | grep -v "// "
# Expected: 0 matches (all removed)

# Verify plan markers for removal
grep -r "@plan:PLAN-20250125-REFACTOR.P15" src/ui/*.rs | wc -l
# Expected: 10+ occurrences (marking removal sites)

# Check for unused imports
cargo clippy -- -W unused_imports 2>&1 | grep "unused_import"
# Expected: 0 unused import warnings (after cleanup)
```

### Semantic Verification

```bash
# Build application
cargo build --release 2>&1 | tee build.log

# Check for errors
grep -E "^error" build.log
# Expected: 0 matches

# Check for warnings
grep -E "^warning" build.log
# Expected: 0 warnings (or only acceptable warnings)

# Run tests
cargo test --lib 2>&1 | tee test_results.log

# Check test results
grep -E "test result:" test_results.log | tail -1
# Expected: test result: ok. X passed in Ys

# Verify no test failures
grep -E "FAILED" test_results.log
# Expected: 0 matches
```

### Functional Verification

```bash
# Manual testing of all views
# Start application
cargo run --release

# Verify each view works:
# - Chat view: Send message, receive response
# - MCP view: Start server, see tools
# - Settings view: Update profile, see changes
# - History view: Load conversations
```

## Success Criteria

- All direct service calls removed from UI layer
- All direct storage access removed from UI layer
- All old helper functions removed (if replaced)
- All unused imports removed
- Application builds successfully
- All tests pass
- All views functional (manual testing)
- Code reduced by ~500-800 lines
- No compilation errors or warnings

## Failure Recovery

If deprecation breaks functionality:

1. **Identify Breakage**:
   ```bash
   cargo build 2>&1 | grep "error:"
   ```

2. **Restore Deleted Code**:
   ```bash
   git checkout HEAD~1 -- src/ui/chat_view.rs
   ```

3. **Incremental Removal**:
   - Remove one function at a time
   - Test after each removal
   - Identify safe removal candidates

4. **Feature Flag** (if needed):
   ```rust
   #[cfg(feature = "legacy_helpers")]
   mod legacy_helpers {
       // Keep old code behind feature flag
   }
   ```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P15.md`

Contents:

```markdown
Phase: P15
Completed: YYYY-MM-DD HH:MM
Files Modified:
  - src/ui/chat_view.rs (removed ~200-300 lines)
  - src/ui/chat_view_helpers/streaming.rs (removed ~50-100 lines)
  - src/ui/chat_view_helpers/helpers.rs (removed ~30-50 lines)
  - src/ui/mcp_configure_view.rs (removed ~50-100 lines)
  - src/ui/mcp_add_view.rs (removed ~40-80 lines)
  - src/ui/settings_view.rs (removed ~60-100 lines)
  - src/ui/history_view.rs (removed ~50-80 lines)

Removal Summary:
  - Direct service calls: REMOVED (0 remaining)
  - Direct storage access: REMOVED (0 remaining)
  - Old helper functions: REMOVED (replaced by services)
  - Unused imports: REMOVED
  - Total lines removed: ~500-800

Verification:
  - cargo build --release: PASS
  - cargo test --lib: PASS
  - Manual testing: PASS
  - All views functional: YES

Code Quality:
  - Compilation errors: 0
  - Warnings: 0
  - Clippy warnings: 0
  - Architecture enforced: YES
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 15a: Deprecation Verification
2. Verify all deprecated code removed
3. Verify no functionality broken
4. Then proceed to Phase 16: End-to-End Testing

## Important Notes

- **CAREFUL REMOVAL**: Only remove code confirmed unused
- **INCREMENTAL**: Can remove one file at a time if needed
- **VERIFICATION REQUIRED**: Test after each removal
- **BACKWARD COMPATIBILITY**: Not needed (migration complete)
- **CLEAN BUILD**: Must build without errors
- **NO FUNCTIONALITY LOSS**: All features must still work
