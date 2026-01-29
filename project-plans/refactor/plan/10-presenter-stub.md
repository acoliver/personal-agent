# Phase 10: Presenter Layer Stub Phase

## Phase ID

`PLAN-20250125-REFACTOR.P10`

## Prerequisites

- Required: Phase 09a (Service Implementation Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P09A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P09A.md`
  - All service implementations complete and verified
  - All service tests passing
- Preflight verification: Phases 01-09a completed

## Purpose

Create minimal stub implementations for the presenter layer. This phase:

1. Creates the presentation module structure
2. Defines all 4 presenter structs with placeholder methods
3. Defines ViewCommand enum for UI communication
4. Sets up module exports
5. Ensures code compiles (with stub implementations)

**Note:** This is a STUB phase. Methods will return `unimplemented!()` or placeholder values. Tests will be written in Phase 11.

## Requirements Implemented (Expanded)

### REQ-025.1: Presenter Module Structure

**Full Text**: The application MUST provide a presentation module with all presenter components.

**Behavior**:
- GIVEN: Application is being organized
- WHEN: src/presentation/mod.rs is created
- THEN: Module exports ChatPresenter, McpPresenter, SettingsPresenter, ErrorPresenter
- AND: Module exports ViewCommand enum

**Why This Matters**: Centralized presentation layer separates UI logic from business logic.

### REQ-025.2: ChatPresenter Structure

**Full Text**: ChatPresenter MUST handle user chat events and service coordination.

**Behavior**:
- GIVEN: Application needs to handle chat UI
- WHEN: ChatPresenter is created
- THEN: Presenter has event_rx for receiving events
- AND: Presenter has services reference for business logic
- AND: Presenter has view_tx for UI commands

**Why This Matters**: ChatPresenter coordinates between UI events and service calls.

### REQ-025.3: McpPresenter Structure

**Full Text**: McpPresenter MUST handle MCP server management UI.

**Behavior**:
- GIVEN: Application needs to manage MCP servers
- WHEN: McpPresenter is created
- THEN: Presenter has event_rx for receiving events
- AND: Presenter has services reference
- AND: Presenter has view_tx for UI commands

**Why This Matters**: McpPresenter enables MCP server management through UI.

### REQ-025.4: SettingsPresenter Structure

**Full Text**: SettingsPresenter MUST handle settings and profile management UI.

**Behavior**:
- GIVEN: Application needs to manage settings
- WHEN: SettingsPresenter is created
- THEN: Presenter has event_rx for receiving events
- AND: Presenter has services reference
- AND: Presenter has view_tx for UI commands

**Why This Matters**: SettingsPresenter enables profile management through UI.

### REQ-025.5: ErrorPresenter Structure

**Full Text**: ErrorPresenter MUST handle error display and logging.

**Behavior**:
- GIVEN: Application needs to display errors
- WHEN: ErrorPresenter is created
- THEN: Presenter has event_rx for receiving events
- AND: Presenter has view_tx for UI commands

**Why This Matters**: Centralized error handling ensures consistent error UX.

### REQ-025.6: ViewCommand Type

**Full Text**: ViewCommand enum MUST define all UI update commands.

**Behavior**:
- GIVEN: Presenter needs to update UI
- WHEN: ViewCommand is emitted
- THEN: UI receives command via broadcast channel
- AND: UI updates accordingly

**Why This Matters**: ViewCommand decouples presenters from UI framework.

## Implementation Tasks

### Files to Create

- `src/presentation/mod.rs`
  - Module declaration file
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P10`
  - Exports: ChatPresenter, McpPresenter, SettingsPresenter, ErrorPresenter, ViewCommand
  - Implements: `@requirement:REQ-025.1`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 1-10

- `src/presentation/view_command.rs`
  - ViewCommand enum definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P10`
  - Implements: `@requirement:REQ-025.6`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 510-541
  - Variants:
    - Chat commands (ConversationCreated, MessageAppended, ShowThinking, etc.)
    - MCP commands (McpServerStarted, McpToolsUpdated)
    - Settings commands (ShowSettings, ShowNotification)
    - Error commands (ShowError with severity)

- `src/presentation/chat.rs`
  - ChatPresenter struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P10`
  - Implements: `@requirement:REQ-025.2`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 20-240
  - Stub methods: new(), start(), stop(), handle_event(), handle_user_event(), handle_chat_event()

- `src/presentation/mcp.rs`
  - McpPresenter struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P10`
  - Implements: `@requirement:REQ-025.3`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 260-371
  - Stub methods: new(), start(), stop(), handle_event(), handle_user_event(), handle_mcp_event()

- `src/presentation/settings.rs`
  - SettingsPresenter struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P10`
  - Implements: `@requirement:REQ-025.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 380-444
  - Stub methods: new(), start(), stop(), handle_event(), handle_user_event()

- `src/presentation/error.rs`
  - ErrorPresenter struct definition
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P10`
  - Implements: `@requirement:REQ-025.5`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 450-505
  - Stub methods: new(), start(), stop(), handle_event()

### Files to Modify

- `src/lib.rs`
  - ADD line: `pub mod presentation;`
  - ADD comment: `@plan:PLAN-20250125-REFACTOR.P10`
  - Makes presentation module available to crate

- `src/main.rs`
  - No changes in this phase (integration happens in later phase)

### Required Code Markers

Every struct/enum/function created in this phase MUST include:

```rust
/// ChatPresenter stub implementation
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.2
/// @pseudocode presenters.md lines 20-25
pub struct ChatPresenter {
    // Stub fields
}
```

### Stub Implementation Guidelines

**In this phase, ALL methods should use stub implementations:**

```rust
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.2
impl ChatPresenter {
    pub fn new(
        services: Arc<ServiceRegistry>,
        view_tx: broadcast::Sender<ViewCommand>
    ) -> Self {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub async fn start(&mut self) -> Result<(), PresenterError> {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        // STUB: Return placeholder
        unimplemented!()
    }

    pub fn is_running(&self) -> bool {
        // STUB: Return placeholder
        unimplemented!()
    }

    async fn handle_event(&self, event: AppEvent) {
        // STUB: Return placeholder
        unimplemented!()
    }

    async fn handle_user_event(&self, event: UserEvent) {
        // STUB: Return placeholder
        unimplemented!()
    }

    async fn handle_chat_event(&self, event: ChatEvent) {
        // STUB: Return placeholder
        unimplemented!()
    }
}
```

## Pseudocode References

### Module Structure (mod.rs)
- Lines 1-10: Module exports and structure

### ViewCommand (view_command.rs)
- Lines 510-541: All ViewCommand variants

### ChatPresenter (chat.rs)
- Lines 20-25: Struct definition with fields
- Lines 30-42: new() constructor
- Lines 50-69: start() method with event loop
- Lines 80-85: handle_event() dispatcher
- Lines 100-107: handle_user_event() dispatcher
- Lines 120-146: on_send_message() handler
- Lines 160-177: handle_chat_event() dispatcher
- Lines 190-241: Individual event handlers

### McpPresenter (mcp.rs)
- Lines 260-265: Struct definition with fields
- Lines 270-282: new() constructor
- Lines 280-289: handle_user_event() dispatcher
- Lines 300-316: on_start_server() handler
- Lines 330-339: handle_mcp_event() dispatcher
- Lines 350-371: Individual event handlers

### SettingsPresenter (settings.rs)
- Lines 380-385: Struct definition with fields
- Lines 400-409: handle_user_event() dispatcher
- Lines 420-444: Individual event handlers

### ErrorPresenter (error.rs)
- Lines 450-453: Struct definition with fields
- Lines 470-479: handle_event() dispatcher
- Lines 490-505: Error handlers

## Verification Commands

### Structural Verification

```bash
# Check module file exists
test -f src/presentation/mod.rs
echo "Expected: File exists"

# Check all presenter files created
test -f src/presentation/view_command.rs
test -f src/presentation/chat.rs
test -f src/presentation/mcp.rs
test -f src/presentation/settings.rs
test -f src/presentation/error.rs
echo "Expected: All 5 presenter files exist"

# Check plan markers in presentation module
grep -r "@plan:PLAN-20250125-REFACTOR.P10" src/presentation/ | wc -l
# Expected: 30+ occurrences (all structs, enums, functions)

# Check requirement markers
grep -r "@requirement:REQ-025" src/presentation/ | wc -l
# Expected: 15+ occurrences (all components tagged)

# Check module is exported in lib.rs
grep "pub mod presentation;" src/lib.rs
# Expected: Line found

# Verify code compiles (STUB compilation allowed)
cargo build --lib 2>&1 | tee build.log
# Expected: Compiles successfully (may have warnings about unused code)
```

### Stub Detection (EXPECTED in this phase)

```bash
# Verify stub methods exist (unimplemented! is OK in this phase)
grep -r "unimplemented!" src/presentation/*.rs | grep -v "test"
# Expected: 25+ matches (all methods are stubs)

# Verify no real implementation exists yet
grep -r "spawn(" src/presentation/*.rs | grep -v "test"
# Expected: 0 matches (real implementation in Phase 12)
```

### Manual Verification Checklist

Read each file and verify:

#### src/presentation/mod.rs
- [ ] Module exports ChatPresenter
- [ ] Module exports McpPresenter
- [ ] Module exports SettingsPresenter
- [ ] Module exports ErrorPresenter
- [ ] Module exports ViewCommand
- [ ] Plan marker present

#### src/presentation/view_command.rs
- [ ] ViewCommand enum defined
- [ ] Chat command variants defined
- [ ] MCP command variants defined
- [ ] Settings command variants defined
- [ ] Error command variants defined
- [ ] Plan marker present
- [ ] Requirement REQ-025.6 marker

#### src/presentation/chat.rs
- [ ] ChatPresenter struct defined
- [ ] new() method defined (stub)
- [ ] start() method defined (stub)
- [ ] stop() method defined (stub)
- [ ] handle_event() method defined (stub)
- [ ] handle_user_event() method defined (stub)
- [ ] handle_chat_event() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-025.2 marker

#### src/presentation/mcp.rs
- [ ] McpPresenter struct defined
- [ ] new() method defined (stub)
- [ ] start() method defined (stub)
- [ ] stop() method defined (stub)
- [ ] handle_event() method defined (stub)
- [ ] handle_user_event() method defined (stub)
- [ ] handle_mcp_event() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-025.3 marker

#### src/presentation/settings.rs
- [ ] SettingsPresenter struct defined
- [ ] new() method defined (stub)
- [ ] start() method defined (stub)
- [ ] stop() method defined (stub)
- [ ] handle_event() method defined (stub)
- [ ] handle_user_event() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-025.4 marker

#### src/presentation/error.rs
- [ ] ErrorPresenter struct defined
- [ ] new() method defined (stub)
- [ ] start() method defined (stub)
- [ ] stop() method defined (stub)
- [ ] handle_event() method defined (stub)
- [ ] All methods use unimplemented!()
- [ ] Plan marker present
- [ ] Requirement REQ-025.5 marker

#### src/lib.rs
- [ ] pub mod presentation; line added
- [ ] Plan marker comment added

## Success Criteria

- All presenter module files created (mod.rs + 5 files)
- Code compiles successfully
- All methods are stubs (unimplemented!())
- All structs and ViewCommand enum defined
- Plan markers present in all files
- Requirement markers traceable
- Module exported in lib.rs

## Failure Recovery

If this phase fails:

1. Rollback commands:
   ```bash
   git checkout -- src/presentation/ src/lib.rs
   rm -rf src/presentation/
   ```

2. Files to revert:
   - src/presentation/mod.rs
   - src/presentation/view_command.rs
   - src/presentation/chat.rs
   - src/presentation/mcp.rs
   - src/presentation/settings.rs
   - src/presentation/error.rs
   - src/lib.rs

3. Cannot proceed to Phase 10a until stub structure compiles

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P10.md`

Contents:

```markdown
Phase: P10
Completed: YYYY-MM-DD HH:MM
Files Created:
  - src/presentation/mod.rs (N lines)
  - src/presentation/view_command.rs (N lines)
  - src/presentation/chat.rs (N lines)
  - src/presentation/mcp.rs (N lines)
  - src/presentation/settings.rs (N lines)
  - src/presentation/error.rs (N lines)
Files Modified:
  - src/lib.rs (+1 line)
Tests Added: 0 (stub phase)
Verification:
  - cargo build --lib: PASS
  - Plan markers: 30+ found
  - Requirement markers: 15+ found
  - All methods: Stub (unimplemented!)
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 10a: Presenter Stub Verification
2. Verify stub structure compiles and all types are defined
3. Then proceed to Phase 11: Presenter TDD (write tests)

## Important Notes

- This is a STUB phase - no real implementation
- All methods should use `unimplemented!()`
- Compilation is required, but functionality is not
- ViewCommand enum should be fully defined (it's just data)
- Presenters are event-driven (subscribe to EventBus)
- Presenters emit ViewCommands (not UI framework specific)
- Next phase will write tests that fail against these stubs
- Phase 12 will implement real functionality to make tests pass
