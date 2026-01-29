# Phase 14a: Migration Verification

## Phase ID

`PLAN-20250125-REFACTOR.P14A`

## Prerequisites

- Required: Phase 14 (Data Migration) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P14" src/migration/*.rs`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P14.md`
  - Migration module implemented
  - Migration executed successfully
  - Backup created
- Preflight verification: Phases 01-14 completed

## Purpose

Verify that the data migration phase was completed correctly and all existing data has been preserved and migrated. This phase:

1. **Verifies data integrity** - All data migrated without loss or corruption
2. **Verifies backup creation** - Backups created and accessible
3. **Verifies rollback functionality** - Can revert to original state
4. **Verifies application functionality** - App works with migrated data
5. **Documents migration completeness**

**Note:** This is a VERIFICATION phase. No code changes expected.

## Verification Tasks

### Structural Verification

#### 1.1: Migration Module Implementation

**Check**: Migration module exists and is properly structured

```bash
# Verify migration module exists
ls -la src/migration/mod.rs
# Expected: File exists

# Verify module exported in lib.rs
grep -E "pub mod migration" src/lib.rs
# Expected: Module exported

# Check plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P14" src/migration/*.rs | wc -l
# Expected: 5+ occurrences

# Verify requirement markers
grep -r "@requirement:REQ-028" src/migration/*.rs | wc -l
# Expected: 3+ occurrences
```

**Expected Results**:
- [OK] Migration module exists
- [OK] Module exported in lib.rs
- [OK] Plan markers present
- [OK] Requirement markers present

#### 1.2: Backup Implementation

**Check**: Backup logic implemented

```bash
# Verify backup directory creation
grep -r "backup_before_migration" src/migration/*.rs
# Expected: Backup directory creation logic

# Verify file backup logic
grep -r "fs::copy" src/migration/*.rs | grep -E "conversations|models|mcp"
# Expected: 3+ file backup operations

# Verify backup verification
grep -r "verify.*backup\|backup.*verify" src/migration/*.rs -i
# Expected: Backup verification logic
```

**Expected Results**:
- [OK] Backup directory created
- [OK] All data files backed up
- [OK] Backup integrity verification

#### 1.3: Migration Logic Implementation

**Check**: All data types have migration logic

```bash
# Verify conversation migration
grep -r "migrate_conversations" src/migration/*.rs
# Expected: Conversation migration function

# Verify profile migration
grep -r "migrate_profiles" src/migration/*.rs
# Expected: Profile migration function

# Verify MCP config migration
grep -r "migrate_mcp" src/migration/*.rs
# Expected: MCP config migration function

# Verify migration report structure
grep -r "MigrationReport" src/migration/*.rs
# Expected: Report structure with counts
```

**Expected Results**:
- [OK] Conversation migration implemented
- [OK] Profile migration implemented
- [OK] MCP config migration implemented
- [OK] Migration report structure

#### 1.4: Rollback Implementation

**Check**: Rollback functionality implemented

```bash
# Verify rollback function
grep -r "fn rollback\|async fn rollback" src/migration/*.rs
# Expected: Rollback function

# Verify restore logic
grep -r "restore_backup" src/migration/*.rs
# Expected: Backup restore logic

# Verify rollback clears new data
grep -r "clear\|delete.*cache\|reset" src/migration/*.rs | grep -i rollback
# Expected: Cache clearing or data removal on rollback
```

**Expected Results**:
- [OK] Rollback function implemented
- [OK] Backup restore logic
- [OK] New data cleared on rollback

### Semantic Verification

#### 2.1: Backup Creation Test

**Test**: Verify backup is created when migration runs

```bash
# Run migration with backup
cargo run --release -- --migrate 2>&1 | tee migration.log

# Check backup creation log
grep -E "Backup created|backup_before_migration" migration.log
# Expected: Backup directory path

# Verify backup files exist
ls -la ~/.personal-agent/backup_before_migration/
# Expected: conversations.json.bak, models.json.bak, mcp_config.json.bak

# Verify backup file integrity
# (Compare file sizes or checksums)
```

**Expected Results**:
- [OK] Backup directory created
- [OK] All data files backed up
- [OK] Backup files readable
- [OK] Backup sizes reasonable (>0 bytes)

#### 2.2: Data Integrity Test

**Test**: Verify no data loss during migration

**Conversations**:
```bash
# Count conversations before migration
# (If old system still accessible)
OLD_COUNT=$(jq '.conversations | length' ~/.personal-agent/conversations.json)

# Count conversations after migration
# (Via app API or new storage)
NEW_COUNT=$(cargo run -- --count-conversations 2>&1 | grep -oE '[0-9]+')

# Verify counts match
if [ "$OLD_COUNT" -eq "$NEW_COUNT" ]; then
    echo "[OK] Conversation counts match: $OLD_COUNT"
else
    echo "[ERROR] Conversation count mismatch: $OLD_COUNT vs $NEW_COUNT"
fi
```

**Profiles**:
```bash
# Count profiles before migration
OLD_PROFILES=$(jq '.profiles | length' ~/.personal-agent/models.json)

# Count profiles after migration
NEW_PROFILES=$(cargo run -- --list-profiles 2>&1 | grep -c "Profile:")

# Verify counts match
if [ "$OLD_PROFILES" -eq "$NEW_PROFILES" ]; then
    echo "[OK] Profile counts match: $OLD_PROFILES"
else
    echo "[ERROR] Profile count mismatch: $OLD_PROFILES vs $NEW_PROFILES"
fi
```

**Expected Results**:
- [OK] Conversation count matches (0 loss)
- [OK] Profile count matches (0 loss)
- [OK] MCP config count matches (0 loss)
- [OK] Data content intact (manual verification)

#### 2.3: Data Accessibility Test

**Test**: Verify migrated data is accessible through application

**Manual Verification**:

1. **Conversations Accessible**:
   - [ ] Open application
   - [ ] Navigate to chat view
   - [ ] Verify all conversations appear in history
   - [ ] Open existing conversation
   - [ ] Verify all messages present
   - [ ] Verify message content correct

2. **Profiles Accessible**:
   - [ ] Open settings
   - [ ] Verify all profiles listed
   - [ ] Verify profile names correct
   - [ ] Verify default profile preserved
   - [ ] Select different profile
   - [ ] Verify profile switches

3. **MCP Configs Accessible**:
   - [ ] Open MCP configuration
   - [ ] Verify all MCP servers listed
   - [ ] Verify server names correct
   - [ ] Verify tool counts match
   - [ ] Start MCP server
   - [ ] Verify server starts correctly

**Expected Results**:
- [OK] All conversations accessible
- [OK] All profiles accessible
- [OK] All MCP configs accessible
- [OK] No data corruption visible

#### 2.4: Application Functionality Test

**Test**: Verify application works correctly with migrated data

**Functional Tests**:

1. **Send Message**:
   - Open existing conversation
   - Send new message
   - Verify response received
   - Verify new message appended to history
   - Verify conversation saved

2. **Create New Conversation**:
   - Create new conversation
   - Send message
   - Verify conversation appears in list
   - Verify conversation persisted

3. **Update Profile**:
   - Edit existing profile
   - Update model name
   - Save changes
   - Verify profile updated
   - Verify changes persist

4. **Add MCP Server**:
   - Add new MCP server
   - Start server
   - Verify server runs
   - Verify configuration saved

**Expected Results**:
- [OK] Can send messages in migrated conversations
- [OK] Can create new conversations
- [OK] Can update migrated profiles
- [OK] Can add new MCP servers
- [OK] All changes persist

#### 2.5: Rollback Test

**Test**: Verify rollback functionality works correctly

```bash
# Run rollback
cargo run --release -- --rollback-migration 2>&1 | tee rollback.log

# Check rollback log
grep -E "Rollback completed|Restored" rollback.log
# Expected: Rollback success message

# Verify original files restored
ls -la ~/.personal-agent/conversations.json
# Expected: Original file restored (check timestamp)

# Verify backup still exists
ls -la ~/.personal-agent/backup_before_migration/
# Expected: Backup files still present

# Re-run migration after rollback
cargo run --release -- --migrate 2>&1 | tee re_migration.log

# Verify migration succeeds again
grep -E "Migration completed" re_migration.log
# Expected: Migration success
```

**Expected Results**:
- [OK] Rollback executes successfully
- [OK] Original files restored
- [OK] Backup preserved
- [OK] Re-migration works

### Build Verification

#### 3.1: Compilation

```bash
# Clean build
cargo clean

# Build with migration
cargo build --release 2>&1 | tee build.log

# Check for errors
grep -E "^error" build.log
# Expected: 0 matches

# Check for warnings
grep -E "^warning" build.log
# Expected: 0 warnings (or only acceptable warnings)
```

#### 3.2: Unit Tests

```bash
# Run migration tests (if implemented)
cargo test --lib migration 2>&1 | tee migration_tests.log

# Check test results
grep -E "test result:" migration_tests.log | tail -1
# Expected: test result: ok. X passed in Ys

# Verify no failures
grep -E "FAILED" migration_tests.log
# Expected: 0 matches
```

### Integration Verification

#### 4.1: Service Integration

**Check**: Migrated data integrates with new services

```bash
# Verify ConversationService has data
# (Via app logs or debug API)
cargo run --release 2>&1 | grep -E "ConversationService.*loaded|conversations loaded"

# Verify ProfileService has data
cargo run --release 2>&1 | grep -E "ProfileService.*loaded|profiles loaded"

# Verify McpService has data
cargo run --release 2>&1 | grep -E "McpService.*loaded|MCP configs loaded"
```

**Expected Results**:
- [OK] ConversationService cache populated
- [OK] ProfileService cache populated
- [OK] McpService registry populated

#### 4.2: UI Integration

**Check**: UI displays migrated data correctly

```bash
# Start application
cargo run --release &

# Wait for app to load
sleep 5

# Check logs for data loading
grep -E "Loaded.*conversations|Loaded.*profiles" ~/.personal-agent/debug.log
# Expected: Data loading confirmations
```

**Manual Verification**:
- [ ] UI shows all conversations
- [ ] UI shows all profiles
- [ ] UI shows all MCP configs
- [ ] No missing data visible

### Performance Verification

#### 5.1: Migration Performance

**Test**: Verify migration completes in acceptable time

```bash
# Time migration execution
time cargo run --release -- --migrate 2>&1 | tee migration_time.log

# Check migration time
# Expected: <5 seconds for typical data (<100 conversations)
```

**Acceptable Performance**:
- 0-50 conversations: <2 seconds
- 50-200 conversations: <5 seconds
- 200+ conversations: <10 seconds

#### 5.2: Application Startup Performance

**Test**: Verify application starts quickly with migrated data

```bash
# Time application startup
time cargo run --release 2>&1 | tee startup_time.log

# Check startup time
# Expected: <3 seconds
```

**Acceptable Performance**:
- Cold start: <3 seconds
- Warm start: <1 second

## Verification Checklist

### Structural Checks

- [ ] Migration module exists
- [ ] Module exported in lib.rs
- [ ] Plan markers present
- [ ] Requirement markers present
- [ ] Backup logic implemented
- [ ] Migration logic implemented (all data types)
- [ ] Rollback logic implemented
- [ ] Migration report structure defined

### Data Integrity Checks

- [ ] Backup created successfully
- [ ] All files backed up
- [ ] Backup files readable
- [ ] Conversation count matches (0 loss)
- [ ] Profile count matches (0 loss)
- [ ] MCP config count matches (0 loss)
- [ ] Data content intact
- [ ] No corruption detected

### Accessibility Checks

- [ ] Conversations accessible in UI
- [ ] Messages display correctly
- [ ] Profiles accessible in settings
- [ ] Profile details correct
- [ ] MCP configs accessible
- [ ] MCP server names correct
- [ ] Tool counts match

### Functionality Checks

- [ ] Can send messages
- [ ] Can create conversations
- [ ] Can update profiles
- [ ] Can add MCP servers
- [ ] Changes persist correctly

### Rollback Checks

- [ ] Rollback executes
- [ ] Original files restored
- [ ] Backup preserved
- [ ] Re-migration works
- [ ] No data corruption after rollback

### Build Checks

- [ ] Application builds
- [ ] No compilation errors
- [ ] No new warnings
- [ ] Unit tests pass
- [ ] Integration tests pass

### Performance Checks

- [ ] Migration time acceptable
- [ ] Application startup acceptable
- [ ] No performance regression

## Success Criteria

- ALL structural checks pass (15/15)
- ALL data integrity checks pass (8/8)
- ALL accessibility checks pass (7/7)
- ALL functionality checks pass (5/5)
- ALL rollback checks pass (5/5)
- ALL build checks pass (5/5)
- ALL performance checks pass (2/2)
- **Overall**: 47/47 checks pass

## Failure Recovery

If verification fails:

1. **Structural Failures**:
   - Add missing migration functions
   - Implement backup logic
   - Add rollback mechanism

2. **Data Integrity Failures**:
   - **CRITICAL**: Rollback immediately
   - Investigate data loss cause
   - Fix migration logic
   - Test on sample data
   - Re-migrate with fixed logic

3. **Accessibility Failures**:
   - Verify service cache population
   - Check UI integration
   - Verify data format compatibility

4. **Functionality Failures**:
   - Debug application logs
   - Verify service initialization
   - Check event flow

5. **Performance Failures**:
   - Profile migration code
   - Optimize data loading
   - Add progress indicators

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P14A.md`

Contents:

```markdown
Phase: P14A
Completed: YYYY-MM-DD HH:MM
Verification Status:
  Structural: 15/15 PASSED
  Data Integrity: 8/8 PASSED
  Accessibility: 7/7 PASSED
  Functionality: 5/5 PASSED
  Rollback: 5/5 PASSED
  Build: 5/5 PASSED
  Performance: 2/2 PASSED
  Total: 47/47 PASSED

Migration Details:
  Conversations migrated: N (verified: N)
  Profiles migrated: N (verified: N)
  MCP configs migrated: N (verified: N)
  Data loss: 0
  Corruption: 0

Backup Status:
  Location: ~/.personal-agent/backup_before_migration/
  Files: N
  Integrity: VERIFIED

Rollback Status:
  Tested: YES
  Functional: YES
  Re-migration: VERIFIED

Application Status:
  Starts successfully: YES
  Data accessible: YES
  Functionality: VERIFIED
  Performance: ACCEPTABLE

Notes:
  - All data migrated successfully
  - Zero data loss confirmed
  - Rollback functional
  - Application works with migrated data
  - Ready for Phase 15
```

## Next Steps

After successful completion of this phase:

1. Migration verified complete
2. All data integrity confirmed
3. Rollback tested and functional
4. Proceed to Phase 15: Deprecation
5. Remove old code paths

## Important Notes

- **DATA INTEGRITY CRITICAL**: Zero tolerance for data loss
- **ROLLBACK SAFETY**: Must be functional before proceeding
- **MANUAL VERIFICATION**: Cannot fully automate data verification
- **USER IMPACT**: Migration must be transparent to users
- **BACKUP PRESERVATION**: Keep backups until after deprecation phase
