# Phase 14: Data Migration Phase

## Phase ID

`PLAN-20250125-REFACTOR.P14`

## Prerequisites

- Required: Phase 13a (UI Integration Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P13A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P13A.md`
  - UI integration verified and working
  - All user workflows functional with new architecture
- Preflight verification: Phases 01-13a completed

## Purpose

Migrate existing data to work with the new architecture. This phase:

1. **Migrates conversation data** from old storage format to new service-based format
2. **Migrates profile configurations** to new ProfileService format
3. **Migrates MCP server configurations** to new McpService format
4. **Preserves user data** - No data loss during migration
5. **Provides rollback capability** - Can revert to old format if needed

**Note:** This is a DATA MIGRATION phase. All existing user data must be preserved.

## Requirements Implemented (Expanded)

### REQ-028.1: Conversation Data Migration

**Full Text**: Existing conversations MUST be migrated to new ConversationService format.

**Behavior**:
- GIVEN: Existing conversations in JSON format (storage/conversations.json)
- WHEN: Migration runs
- THEN: All conversations loaded and converted to new format
- AND: ConversationService in-memory cache populated
- AND: Original JSON file preserved (backup created)
- AND: Migration status logged

**Why This Matters**: Users must not lose existing chat history.

### REQ-028.2: Profile Configuration Migration

**Full Text**: Existing model profiles MUST be migrated to new ProfileService format.

**Behavior**:
- GIVEN: Existing profiles in configuration (models.json or similar)
- WHEN: Migration runs
- THEN: All profiles loaded and converted to ModelProfile format
- AND: ProfileService in-memory cache populated
- AND: Original configuration backed up
- AND: Default profile preserved

**Why This Matters**: Users must not lose model configurations.

### REQ-028.3: MCP Server Configuration Migration

**Full Text**: Existing MCP server configurations MUST be migrated to new McpService format.

**Behavior**:
- GIVEN: Existing MCP configurations in storage
- WHEN: Migration runs
- THEN: All configurations loaded and converted to McpConfig format
- AND: McpService registry populated
- AND: Original configurations backed up

**Why This Matters**: Users must not lose MCP server setups.

### REQ-028.4: Migration Rollback

**Full Text**: Migration MUST be reversible with no data loss.

**Behavior**:
- GIVEN: Migration completed
- WHEN: Rollback triggered
- THEN: New data removed
- AND: Original data restored from backup
- AND: System state reverted to pre-migration

**Why This Matters**: Safety net if migration causes issues.

## Files to Modify

### Migration Script

#### `src/migration/mod.rs` (NEW FILE)

**Purpose**: Central migration coordinator

**Implementation**:

```rust
/// @plan PLAN-20250125-REFACTOR.P14
/// @requirement REQ-028.1, REQ-028.2, REQ-028.3
use std::path::PathBuf;
use anyhow::{Result, Context};
use tracing::{info, warn, error};

pub struct MigrationRunner {
    data_dir: PathBuf,
    backup_dir: PathBuf,
}

impl MigrationRunner {
    pub fn new(data_dir: PathBuf) -> Self {
        let backup_dir = data_dir.join("backup_before_migration");
        MigrationRunner {
            data_dir,
            backup_dir,
        }
    }

    /// Run all migrations in sequence
    pub async fn run_migrations(&self) -> Result<MigrationReport> {
        info!("Starting data migration...");

        // Create backup directory
        tokio::fs::create_dir_all(&self.backup_dir).await
            .context("Failed to create backup directory")?;

        // Run migrations in sequence
        let mut report = MigrationReport::default();

        // 1. Backup existing data
        self.backup_existing_data().await?;
        info!("Backup created at: {:?}", self.backup_dir);

        // 2. Migrate conversations
        match self.migrate_conversations().await {
            Ok(stats) => {
                report.conversations_migrated = stats.count;
                info!("Migrated {} conversations", stats.count);
            }
            Err(e) => {
                error!("Failed to migrate conversations: {}", e);
                return Err(e);
            }
        }

        // 3. Migrate profiles
        match self.migrate_profiles().await {
            Ok(stats) => {
                report.profiles_migrated = stats.count;
                info!("Migrated {} profiles", stats.count);
            }
            Err(e) => {
                error!("Failed to migrate profiles: {}", e);
                return Err(e);
            }
        }

        // 4. Migrate MCP configurations
        match self.migrate_mcp_configs().await {
            Ok(stats) => {
                report.mcp_configs_migrated = stats.count;
                info!("Migrated {} MCP configurations", stats.count);
            }
            Err(e) => {
                warn!("Failed to migrate MCP configs: {}", e);
                // Non-fatal: continue with other migrations
            }
        }

        info!("Migration completed successfully: {:?}", report);
        Ok(report)
    }

    /// Rollback all migrations
    pub async fn rollback(&self) -> Result<()> {
        info!("Rolling back migration...");

        // Restore from backup
        self.restore_backup().await?;

        info!("Rollback completed");
        Ok(())
    }

    async fn backup_existing_data(&self) -> Result<()> {
        use tokio::fs;

        // Backup conversations
        let conversations_path = self.data_dir.join("conversations.json");
        if conversations_path.exists() {
            let backup_path = self.backup_dir.join("conversations.json.bak");
            fs::copy(&conversations_path, &backup_path).await
                .context("Failed to backup conversations")?;
        }

        // Backup profiles
        let profiles_path = self.data_dir.join("models.json");
        if profiles_path.exists() {
            let backup_path = self.backup_dir.join("models.json.bak");
            fs::copy(&profiles_path, &backup_path).await
                .context("Failed to backup profiles")?;
        }

        // Backup MCP configs
        let mcp_path = self.data_dir.join("mcp_config.json");
        if mcp_path.exists() {
            let backup_path = self.backup_dir.join("mcp_config.json.bak");
            fs::copy(&mcp_path, &backup_path).await
                .context("Failed to backup MCP configs")?;
        }

        Ok(())
    }

    async fn migrate_conversations(&self) -> MigrationStats {
        use crate::storage::ConversationStorage;
        use crate::services::ConversationService;

        // Load existing conversations
        let storage = ConversationStorage::with_default_path()?;
        let old_conversations = storage.load_all()?;

        let count = old_conversations.len();

        // Migrate to new service (in-memory)
        // Note: New service will persist in same format on save
        for (id, old_conv) in old_conversations {
            // Convert to new format if needed
            // (Likely compatible, just load into service)
        }

        Ok(MigrationStats { count: count as u32 })
    }

    async fn migrate_profiles(&self) -> MigrationStats {
        use crate::models::ModelProfile;
        use crate::services::ProfileService;

        // Load existing profiles from config
        // (Implementation depends on current storage format)

        let count = 0; // Placeholder

        Ok(MigrationStats { count })
    }

    async fn migrate_mcp_configs(&self) -> MigrationStats {
        use crate::mcp::McpConfig;
        use crate::services::McpService;

        // Load existing MCP configs
        // (Implementation depends on current storage format)

        let count = 0; // Placeholder

        Ok(MigrationStats { count })
    }

    async fn restore_backup(&self) -> Result<()> {
        use tokio::fs;

        // Restore conversations
        let conversations_backup = self.backup_dir.join("conversations.json.bak");
        if conversations_backup.exists() {
            let target = self.data_dir.join("conversations.json");
            fs::copy(&conversations_backup, &target).await
                .context("Failed to restore conversations")?;
        }

        // Restore profiles
        let profiles_backup = self.backup_dir.join("models.json.bak");
        if profiles_backup.exists() {
            let target = self.data_dir.join("models.json");
            fs::copy(&profiles_backup, &target).await
                .context("Failed to restore profiles")?;
        }

        // Restore MCP configs
        let mcp_backup = self.backup_dir.join("mcp_config.json.bak");
        if mcp_backup.exists() {
            let target = self.data_dir.join("mcp_config.json");
            fs::copy(&mcp_backup, &target).await
                .context("Failed to restore MCP configs")?;
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MigrationReport {
    pub conversations_migrated: u32,
    pub profiles_migrated: u32,
    pub mcp_configs_migrated: u32,
}

#[derive(Debug)]
pub struct MigrationStats {
    pub count: u32,
}
```

### Application Integration

#### `src/main.rs` (MODIFY)

**Add Migration Call on Startup**:

```rust
/// @plan PLAN-20250125-REFACTOR.P14
/// @requirement REQ-028.1
fn main() {
    // ... existing setup ...

    // Run migration on first launch of new architecture
    let data_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".personal-agent");

    let migration_runner = migration::MigrationRunner::new(data_dir);

    // Run migration (on startup)
    if let Err(e) = migration_runner.run_migrations() {
        eprintln!("Migration failed: {}", e);
        // Optionally rollback or exit
    }

    // ... rest of application ...
}
```

## Migration Strategy

### Backup Strategy

1. **Pre-Migration Backup**:
   - Copy all data files to `backup_before_migration/` directory
   - Include: conversations.json, models.json, mcp_config.json
   - Timestamp backup directory for traceability

2. **Backup Verification**:
   - Verify backup files exist and are readable
   - Calculate checksums to ensure integrity
   - Log backup status

### Migration Steps

1. **Conversation Migration**:
   - Load all conversations from existing JSON
   - Validate structure
   - Load into ConversationService in-memory cache
   - Verify all conversations loaded
   - Log migration count

2. **Profile Migration**:
   - Load all profiles from existing config
   - Validate structure
   - Load into ProfileService in-memory cache
   - Preserve default profile setting
   - Log migration count

3. **MCP Configuration Migration**:
   - Load all MCP configs from existing storage
   - Validate structure
   - Load into McpService registry
   - Verify all configs loaded
   - Log migration count

### Rollback Strategy

1. **Trigger Rollback**:
   - Command-line flag: `--rollback-migration`
   - Environment variable: `PERSONAL_AGENT_ROLLBACK=true`
   - Manual API call

2. **Rollback Steps**:
   - Stop all services
   - Clear in-memory caches
   - Restore files from backup
   - Restart services
   - Verify system state

## Data Format Compatibility

### Conversation Format

**Old Format** (storage/conversations.json):
```json
{
  "conversations": [
    {
      "id": "uuid",
      "title": "Conversation Title",
      "profile_id": "uuid",
      "messages": [
        {
          "id": "uuid",
          "role": "user|assistant",
          "content": "Message text",
          "created_at": "2025-01-25T12:00:00Z"
        }
      ],
      "created_at": "2025-01-25T12:00:00Z",
      "updated_at": "2025-01-25T12:00:00Z"
    }
  ]
}
```

**New Format** (Compatible):
```rust
// Same format, loaded via ConversationStorage
// No conversion needed (backward compatible)
// Just loaded into ConversationService cache
```

### Profile Format

**Old Format** (models.json):
```json
{
  "profiles": [
    {
      "id": "uuid",
      "name": "GPT-4",
      "provider": "openai",
      "model": "gpt-4",
      "api_key": "...",
      "is_default": true
    }
  ]
}
```

**New Format** (ModelProfile struct):
```rust
// Same fields, loaded into ProfileService
// API keys moved to SecretsManager
```

## Verification Commands

### Structural Verification

```bash
# Verify migration module exists
ls -la src/migration/mod.rs
# Expected: File exists

# Verify plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P14" src/migration/*.rs
# Expected: Multiple occurrences

# Verify backup directory creation
grep -r "backup_before_migration" src/migration/*.rs
# Expected: Backup creation logic

# Verify rollback implementation
grep -r "rollback\|restore_backup" src/migration/*.rs
# Expected: Rollback logic implemented
```

### Semantic Verification

```bash
# Run migration in test mode
cargo run -- --migrate-dry-run 2>&1 | tee migration_test.log

# Check migration plan
grep -E "Migrating|Backing up" migration_test.log
# Expected: Migration steps listed

# Verify backup creation
ls -la ~/.personal-agent/backup_before_migration/
# Expected: Backup files present

# Run actual migration
cargo run --release 2>&1 | tee migration_run.log

# Check migration results
grep -E "Migration completed|Migrated.*conversations|Migrated.*profiles" migration_run.log
# Expected: Success message with counts

# Verify data integrity
cargo run -- --verify-migration 2>&1 | tee verify_migration.log
# Expected: All data verified
```

### Data Integrity Verification

```bash
# Verify conversations loaded
# (Manual: Check app shows existing conversations)

# Verify profiles loaded
# (Manual: Check settings show existing profiles)

# Verify MCP configs loaded
# (Manual: Check MCP configuration shows existing servers)

# Verify no data loss
# Count conversations before/after migration
```

## Success Criteria

- Migration module implemented
- Backup created before migration
- All conversations migrated (0 data loss)
- All profiles migrated (0 data loss)
- All MCP configs migrated (0 data loss)
- Migration log complete
- Rollback functional
- Application starts successfully after migration
- User can access all existing data

## Failure Recovery

If migration fails:

1. **Identify Failure Point**:
   ```bash
   grep -E "Failed to migrate|Migration error" migration_run.log
   ```

2. **Run Rollback**:
   ```bash
   cargo run -- --rollback-migration
   ```

3. **Verify Rollback**:
   ```bash
   ls -la ~/.personal-agent/backup_before_migration/
   # Verify original files restored
   ```

4. **Fix Migration Logic**:
   - Update migration code
   - Test with sample data
   - Re-run migration

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P14.md`

Contents:

```markdown
Phase: P14
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/migration/mod.rs (N lines, migration logic)
Files Modified:
  - src/main.rs (added migration call)
Migration Results:
  - Conversations migrated: N
  - Profiles migrated: N
  - MCP configs migrated: N
  - Data loss: 0
Backup:
  - Location: ~/.personal-agent/backup_before_migration/
  - Files backed up: N
  - Integrity verified: YES
Rollback:
  - Tested: YES
  - Functional: YES
Verification:
  - Application starts: PASS
  - Data accessible: PASS
  - No corruption: PASS
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 14a: Migration Verification
2. Verify all data migrated correctly
3. Verify rollback works
4. Then proceed to Phase 15: Deprecation

## Important Notes

- **DATA LOSS PREVENTION**: Always create backup before migration
- **ROLLBACK REQUIRED**: Must be able to revert if issues arise
- **TEST ON SAMPLE DATA**: Test migration on copy of real data first
- **LOG EVERYTHING**: Detailed logging for debugging
- **USER DATA**: All user data must be preserved
