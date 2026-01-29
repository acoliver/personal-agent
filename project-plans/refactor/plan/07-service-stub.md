# Phase 07: Service Layer Stub Phase

## Phase ID

`PLAN-20250125-REFACTOR.P07`

## Prerequisites

- Required: Phase 06a (Event Implementation Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P06A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P06A.md`
  - All EventBus functionality implemented and verified
- Preflight verification: Phases 01-06a completed

## Purpose

Create minimal stub implementations for the service layer. This phase:

1. Creates the services module structure
2. Defines all 5 service structs with placeholder methods
3. Sets up module exports
4. Ensures code compiles (with stub implementations)

**Note:** This is a STUB phase. Methods will return `unimplemented!()` or placeholder values. Tests will be written in Phase 08.

## Requirements Implemented (Expanded)

### REQ-022.1: Service Module Structure

**Full Text**: The application MUST provide a services module with all business logic services.

**Behavior**:
- GIVEN: Application is being organized
- WHEN: src/services/mod.rs is created
- THEN: Module exports ConversationService, ChatService, McpService, ProfileService, SecretsService

**Why This Matters**: Centralized business logic layer separates concerns from UI and event handling.

### REQ-022.2: ConversationService Structure

**Full Text**: ConversationService MUST manage conversation state and message history.

**Behavior**:
- GIVEN: Application needs to manage conversations
- WHEN: ConversationService is created
- THEN: Service has conversation_id and messages field
- AND: Service has event_tx for emitting events

**Why This Matters**: Centralized conversation management prevents state scattered across UI components.

### REQ-022.3: ChatService Structure

**Full Text**: ChatService MUST handle LLM integration and response generation.

**Behavior**:
- GIVEN: Application needs to generate responses
- WHEN: ChatService is created
- THEN: Service has model_client field
- AND: Service has event_tx for emitting events

**Why This Matters**: Abstracting LLM interaction enables easy model switching.

### REQ-022.4: McpService Structure

**Full Text**: McpService MUST manage MCP server lifecycle and tool execution.

**Behavior**:
- GIVEN: Application needs to use MCP tools
- WHEN: McpService is created
- THEN: Service has servers field
- AND: Service has event_tx for emitting events

**Why This Matters**: MCP protocol abstraction enables tool extensibility.

### REQ-022.5: ProfileService Structure

**Full Text**: ProfileService MUST manage user profile and preferences.

**Behavior**:
- GIVEN: Application needs to store user preferences
- WHEN: ProfileService is created
- THEN: Service has profile field
- AND: Service has event_tx for emitting events

**Why This Matters**: Persistent preferences enable personalized experience.

### REQ-022.6: SecretsService Structure

**Full Text**: SecretsService MUST securely store and retrieve sensitive data (API keys, tokens).

**Behavior**:
- GIVEN: Application needs to store API keys
- WHEN: SecretsService is created
- THEN: Service has keyring field
- AND: Service has event_tx for emitting events

**Why This Matters**: Secure credential storage prevents hardcoding sensitive data.

### REQ-022.7: Service Event Emission

**Full Text**: All services MUST emit events via event_tx broadcast channel.

**Behavior**:
- GIVEN: Service performs significant action
- WHEN: Action completes
- THEN: Service emits appropriate event via event_tx.send()
- AND: Event is broadcast to all subscribers

**Why This Matters**: Event-driven architecture enables loose coupling between services and UI.

## Implementation Tasks

### Files to Create

- `src/services/mod.rs`
  - Module declaration file
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P07`
  - Exports: ConversationService, ChatService, McpService, ProfileService, SecretsService
  - Implements: `@requirement:REQ-022.1`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 1-10

- `src/services/conversation.rs`
  - ConversationService struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P07`
  - Implements: `@requirement:REQ-022.2`, `@requirement:REQ-022.7`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 15-50
  - Stub methods: new(), send_message(), get_messages(), get_conversation_id()

- `src/services/chat.rs`
  - ChatService struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P07`
  - Implements: `@requirement:REQ-022.3`, `@requirement:REQ-022.7`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 55-90
  - Stub methods: new(), generate_response(), stream_response()

- `src/services/mcp.rs`
  - McpService struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P07`
  - Implements: `@requirement:REQ-022.4`, `@requirement:REQ-022.7`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 95-140
  - Stub methods: new(), start_server(), stop_server(), call_tool()

- `src/services/profile.rs`
  - ProfileService struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P07`
  - Implements: `@requirement:REQ-022.5`, `@requirement:REQ-022.7`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 145-180
  - Stub methods: new(), get_profile(), update_profile(), get_preferences()

- `src/services/secrets.rs`
  - SecretsService struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P07`
  - Implements: `@requirement:REQ-022.6`, `@requirement:REQ-022.7`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 185-220
  - Stub methods: new(), store_secret(), get_secret(), delete_secret()

### Files to Modify

- `src/lib.rs`
  - ADD line: `pub mod services;`
  - ADD comment: `@plan:PLAN-20250125-REFACTOR.P07`
  - Makes services module available to crate

- `src/main.rs`
  - No changes in this phase (integration happens in later phase)
  - Note: Module must compile but not yet integrated

### Required Code Markers

Every struct/enum/function created in this phase MUST include:

```rust
/// ConversationService stub implementation
///
/// @plan PLAN-20250125-REFACTOR.P07
/// @requirement REQ-022.2
/// @pseudocode services.md lines 15-20
pub struct ConversationService {
    // Stub fields
}
```

### Stub Implementation Guidelines

**In this phase, ALL methods should use stub implementations:**

```rust
/// @plan PLAN-20250125-REFACTOR.P07
/// @requirement REQ-022.2
impl ConversationService {
    pub fn new(event_tx: broadcast::Sender<AppEvent>) -> Self {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn send_message(&self, content: String) -> Result<(), ServiceError> {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn get_messages(&self) -> Vec<Message> {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn get_conversation_id(&self) -> Uuid {
        // STUB: Return placeholder
        unimplemented!()
    }
}
```

## Pseudocode References

### Module Structure (mod.rs)
- Lines 1-10: Module exports and structure

### ConversationService (conversation.rs)
- Lines 15-20: Struct definition with fields
- Lines 25-30: new() constructor
- Lines 35-40: send_message() method
- Lines 45-48: get_messages() method
- Lines 50-52: get_conversation_id() method

### ChatService (chat.rs)
- Lines 55-60: Struct definition with fields
- Lines 65-70: new() constructor
- Lines 75-85: generate_response() method
- Lines 90-100: stream_response() method

### McpService (mcp.rs)
- Lines 95-100: Struct definition with fields
- Lines 105-110: new() constructor
- Lines 115-125: start_server() method
- Lines 130-135: stop_server() method
- Lines 140-150: call_tool() method

### ProfileService (profile.rs)
- Lines 145-150: Struct definition with fields
- Lines 155-160: new() constructor
- Lines 165-170: get_profile() method
- Lines 175-180: update_profile() method
- Lines 185-190: get_preferences() method

### SecretsService (secrets.rs)
- Lines 185-190: Struct definition with fields
- Lines 195-200: new() constructor
- Lines 205-210: store_secret() method
- Lines 215-220: get_secret() method
- Lines 225-230: delete_secret() method

## Verification Commands

### Structural Verification

```bash
# Check module file exists
test -f src/services/mod.rs
echo "Expected: File exists"

# Check all service files created
test -f src/services/conversation.rs
test -f src/services/chat.rs
test -f src/services/mcp.rs
test -f src/services/profile.rs
test -f src/services/secrets.rs
echo "Expected: All 5 service files exist"

# Check plan markers in services module
grep -r "@plan:PLAN-20250125-REFACTOR.P07" src/services/ | wc -l
# Expected: 30+ occurrences (all structs, enums, functions)

# Check requirement markers
grep -r "@requirement:REQ-022" src/services/ | wc -l
# Expected: 20+ occurrences (all components tagged)

# Check module is exported in lib.rs
grep "pub mod services;" src/lib.rs
# Expected: Line found

# Verify code compiles (STUB compilation allowed)
cargo build --lib 2>&1 | tee build.log
# Expected: Compiles successfully (may have warnings about unused code)
```

### Stub Detection (EXPECTED in this phase)

```bash
# Verify stub methods exist (unimplemented! is OK in this phase)
grep -r "unimplemented!" src/services/*.rs | grep -v "tests"
# Expected: 25+ matches (all methods are stubs)

# Verify no real implementation exists yet
grep -r "Arc::new\|Mutex::new" src/services/*.rs | grep -v "tests"
# Expected: 0 matches (real implementation in Phase 09)
```

### Manual Verification Checklist

Read each file and verify:

#### src/services/mod.rs
- [ ] Module exports ConversationService
- [ ] Module exports ChatService
- [ ] Module exports McpService
- [ ] Module exports ProfileService
- [ ] Module exports SecretsService
- [ ] Plan marker present

#### src/services/conversation.rs
- [ ] ConversationService struct defined
- [ ] new() method defined (stub)
- [ ] send_message() method defined (stub)
- [ ] get_messages() method defined (stub)
- [ ] get_conversation_id() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.2 marker
- [ ] Requirement REQ-022.7 marker

#### src/services/chat.rs
- [ ] ChatService struct defined
- [ ] new() method defined (stub)
- [ ] generate_response() method defined (stub)
- [ ] stream_response() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.3 marker
- [ ] Requirement REQ-022.7 marker

#### src/services/mcp.rs
- [ ] McpService struct defined
- [ ] new() method defined (stub)
- [ ] start_server() method defined (stub)
- [ ] stop_server() method defined (stub)
- [ ] call_tool() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.4 marker
- [ ] Requirement REQ-022.7 marker

#### src/services/profile.rs
- [ ] ProfileService struct defined
- [ ] new() method defined (stub)
- [ ] get_profile() method defined (stub)
- [ ] update_profile() method defined (stub)
- [ ] get_preferences() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.5 marker
- [ ] Requirement REQ-022.7 marker

#### src/services/secrets.rs
- [ ] SecretsService struct defined
- [ ] new() method defined (stub)
- [ ] store_secret() method defined (stub)
- [ ] get_secret() method defined (stub)
- [ ] delete_secret() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-022.6 marker
- [ ] Requirement REQ-022.7 marker

#### src/lib.rs
- [ ] pub mod services; line added
- [ ] Plan marker comment added

## Success Criteria

- All service module files created (mod.rs + 5 service files)
- Code compiles successfully
- All methods are stubs (unimplemented!())
- All structs defined
- Plan markers present in all files
- Requirement markers traceable
- Module exported in lib.rs

## Failure Recovery

If this phase fails:

1. Rollback commands:
   ```bash
   git checkout -- src/services/ src/lib.rs
   rm -rf src/services/
   ```

2. Files to revert:
   - src/services/mod.rs
   - src/services/conversation.rs
   - src/services/chat.rs
   - src/services/mcp.rs
   - src/services/profile.rs
   - src/services/secrets.rs
   - src/lib.rs

3. Cannot proceed to Phase 07a until stub structure compiles

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P07.md`

Contents:

```markdown
Phase: P07
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/services/mod.rs (N lines)
  - src/services/conversation.rs (N lines)
  - src/services/chat.rs (N lines)
  - src/services/mcp.rs (N lines)
  - src/services/profile.rs (N lines)
  - src/services/secrets.rs (N lines)
Files Modified:
  - src/lib.rs (+1 line)
Tests Added: 0 (stub phase)
Verification:
  - cargo build --lib: PASS
  - Plan markers: 30+ found
  - Requirement markers: 20+ found
  - All methods: Stub (unimplemented!)
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 07a: Service Stub Verification
2. Verify stub structure compiles and all types are defined
3. Then proceed to Phase 08: Service TDD (write tests)

## Important Notes

- This is a STUB phase - no real implementation
- All methods should use `unimplemented!()`
- Compilation is required, but functionality is not
- Next phase will write tests that fail against these stubs
- Phase 09 will implement real functionality to make tests pass
