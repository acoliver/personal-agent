# Phase 13: UI Integration Phase

## Phase ID

`PLAN-20250125-REFACTOR.P13`

## Prerequisites

- Required: Phase 12a (Presenter Implementation Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P12A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P12A.md`
  - All presenter implementations complete and tested
  - Event bus operational
  - Service layer fully functional
- Preflight verification: Phases 01-12a completed

## Purpose

Integrate the new presenter-based architecture with the existing UIKit-based UI layer. This phase:

1. **Modifies existing view controllers** to use presenters instead of direct service calls
2. **Replaces direct UI manipulation** with ViewCommand emission and handling
3. **Implements event subscriptions** in view controllers to receive ViewCommands
4. **Maintains backwards compatibility** during transition
5. **Ensures user access** to all existing features through new architecture

**Note:** This is a CRITICAL integration phase. It connects the new architecture to the existing UI.

## Requirements Implemented (Expanded)

### REQ-025.1: Chat View Integration

**Full Text**: ChatView MUST use ChatPresenter for all chat operations.

**Behavior**:
- GIVEN: ChatViewController with existing UIKit implementation
- WHEN: User sends message
- THEN: UserEvent::SendMessage emitted to EventBus
- AND: ChatPresenter handles event (not ChatView directly)
- AND: ViewCommands received and applied to UI
- AND: No direct service calls from ChatView

**Why This Matters**: Decouples UI from business logic, enables testing.

### REQ-025.2: MCP View Integration

**Full Text**: McpConfigureView and McpAddView MUST use McpPresenter for MCP operations.

**Behavior**:
- GIVEN: MCP view controllers with existing implementation
- WHEN: User starts MCP server
- THEN: UserEvent::StartMcpServer emitted to EventBus
- AND: McpPresenter handles event
- AND: ViewCommands update UI state

**Why This Matters**: Centralizes MCP management logic in presenter layer.

### REQ-025.3: Settings View Integration

**Full Text**: SettingsView MUST use SettingsPresenter for settings operations.

**Behavior**:
- GIVEN: SettingsViewController with existing implementation
- WHEN: User updates profile
- THEN: UserEvent::UpdateProfile emitted to EventBus
- AND: SettingsPresenter handles event
- AND: ViewCommands update UI

**Why This Matters**: Separates configuration management from UI concerns.

### REQ-025.4: Error Display Integration

**Full Text**: All views MUST display errors via ErrorPresenter ViewCommands.

**Behavior**:
- GIVEN: Any view controller
- WHEN: Error occurs
- THEN: ErrorPresenter emits ViewCommand::ShowError
- AND: UI displays error dialog via ViewCommand handler

**Why This Matters**: Consistent error UX across application.

## Files to Modify

### Chat View Integration

#### `src/ui/chat_view.rs`

**Current Implementation** (approx. 981 lines):
- Lines 107-146: ChatViewController struct with ivars
- Lines 200-300: Direct service calls (McpService, LlmService)
- Lines 400-500: Message handling and UI updates
- Lines 600-700: Streaming response handling

**Modifications Required**:

1. **Add Presenter Dependencies** (after line 146):
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.1
   struct ChatViewController {
       // EXISTING: ivars, message_store, etc.
       
       // NEW: Presenter-based architecture
       event_rx: broadcast::Receiver<ViewCommand>,
       event_tx: broadcast::Sender<AppEvent>,
       chat_presenter: Arc<ChatPresenter>,
   }
   ```

2. **Replace Direct Service Calls** (lines 200-300):
   - **OLD**: Direct `llm_service.start_streaming_request()` calls
   - **NEW**: Emit `UserEvent::SendMessage { conversation_id, content }`
   - **Reference**: `pseudocode/presenters.md` lines 120-146

3. **Implement ViewCommand Handler** (new method):
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.1
   impl ChatViewController {
       fn handle_view_command(&self, cmd: ViewCommand) {
           match cmd {
               ViewCommand::ConversationCreated { id, profile_id } => {
                   self.on_conversation_created(id, profile_id);
               }
               ViewCommand::MessageAppended { conversation_id, role, content } => {
                   self.on_message_appended(conversation_id, role, content);
               }
               ViewCommand::ShowThinking { conversation_id } => {
                   self.on_show_thinking(conversation_id);
               }
               ViewCommand::HideThinking { conversation_id } => {
                   self.on_hide_thinking(conversation_id);
               }
               ViewCommand::AppendStream { conversation_id, chunk } => {
                   self.on_append_stream(conversation_id, chunk);
               }
               ViewCommand::FinalizeStream { conversation_id, tokens } => {
                   self.on_finalize_stream(conversation_id, tokens);
               }
               ViewCommand::ShowError { title, message, severity } => {
                   self.on_show_error(title, message, severity);
               }
               _ => {}
           }
       }
   }
   ```

4. **Start Presenter Event Loop** (in `viewDidLoad` or similar):
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.1
   fn setup_presenter(&mut self) {
       // Get event bus
       let event_bus = get_event_bus();
       
       // Subscribe to ViewCommands
       let (view_tx, view_rx) = broadcast::channel(100);
       self.event_rx = view_rx;
       self.event_tx = event_bus.subscribe(); // For emitting UserEvents
       
       // Create and start presenter
       self.chat_presenter = Arc::new(ChatPresenter::new(
           get_service_registry(),
           view_tx,
       ));
       
       // Spawn background task to receive ViewCommands
       let rx = self.event_rx.resubscribe();
       let weak_self = // Create weak reference to self
       tokio::spawn(async move {
           while let Ok(cmd) = rx.recv().await {
               if let Some strong) = weak_self.upgrade() {
                   strong.handle_view_command(cmd);
               }
           }
       });
   }
   ```

**Lines to Change**:
- Lines 107-146: Add presenter fields to struct
- Lines 200-300: Replace direct service calls with UserEvent emission
- Lines 400-500: Replace direct UI updates with ViewCommand emission
- Lines 600-700: Replace streaming logic with ViewCommand handling
- Add `handle_view_command()` method
- Add `setup_presenter()` method

### MCP View Integration

#### `src/ui/mcp_configure_view.rs`

**Current Implementation** (approx. 250+ lines):
- Direct MCP service calls for server management
- UI updates inline with business logic

**Modifications Required**:

1. **Add Presenter Dependencies**:
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.2
   pub struct McpConfigureViewController {
       // EXISTING: ivars
       
       // NEW: Presenter-based architecture
       event_rx: broadcast::Receiver<ViewCommand>,
       event_tx: broadcast::Sender<AppEvent>,
       mcp_presenter: Arc<McpPresenter>,
   }
   ```

2. **Replace Direct MCP Calls**:
   - **OLD**: `mcp_service.start_server(config)` direct calls
   - **NEW**: Emit `UserEvent::StartMcpServer { config }`
   - **Reference**: `pseudocode/presenters.md` lines 300-316

3. **Implement ViewCommand Handler**:
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.2
   fn handle_view_command(&self, cmd: ViewCommand) {
       match cmd {
               self.on_server_started(id, tool_count);
           }
           ViewCommand::ShowError { title, message, severity } => {
               self.on_show_error(title, message, severity);
           }
           _ => {}
       }
   }
   ```

#### `src/ui/mcp_add_view.rs`

**Similar modifications as mcp_configure_view.rs**:
- Add presenter dependencies
- Replace direct service calls with UserEvent emission
- Implement ViewCommand handler

### Settings View Integration

#### `src/ui/settings_view.rs`

**Current Implementation** (approx. 400+ lines):
- Direct profile service calls
- Inline settings management

**Modifications Required**:

1. **Add Presenter Dependencies**:
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.3
   pub struct SettingsViewController {
       // EXISTING: ivars
       
       // NEW: Presenter-based architecture
       event_rx: broadcast::Receiver<ViewCommand>,
       event_tx: broadcast::Sender<AppEvent>,
       settings_presenter: Arc<SettingsPresenter>,
   }
   ```

2. **Replace Direct Profile Calls**:
   - **OLD**: `profile_service.update_profile(profile)` direct calls
   - **NEW**: Emit `UserEvent::UpdateProfile { profile }`
   - **Reference**: `pseudocode/presenters.md` lines 430-444

3. **Implement ViewCommand Handler**:
   ```rust
   /// @plan PLAN-20250125-REFACTOR.P13
   /// @requirement REQ-025.3
   fn handle_view_command(&self, cmd: ViewCommand) {
       match cmd {
           ViewCommand::ShowSettings { profiles } => {
               self.on_show_settings(profiles);
           }
           ViewCommand::ShowNotification { message } => {
               self.on_show_notification(message);
           }
           ViewCommand::ShowError { title, message, severity } => {
               self.on_show_error(title, message, severity);
           }
           _ => {}
       }
   }
   ```

### Error Handling Integration

#### All View Controllers

**Modifications Required**:

1. **Subscribe to Error ViewCommands**:
   - All views must handle `ViewCommand::ShowError`
   - Display error alerts consistently

2. **Remove Inline Error Handling**:
   - Replace `NSAlert` inline displays with ViewCommand emission
   - Centralize error display logic

## Implementation Patterns

### Event Emission Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P13
/// @requirement REQ-025.1
impl ChatViewController {
    fn send_message(&self, content: String) {
        // OLD: Direct service call
        // let response = self.llm_service.start_streaming_request(&content).await;
        
        // NEW: Emit event to presenter
        let _ = self.event_tx.send(AppEvent::User(UserEvent::SendMessage {
            conversation_id: self.current_conversation_id,
            content,
        }));
    }
}
```

### ViewCommand Handling Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P13
/// @requirement REQ-025.1
impl ChatViewController {
    fn on_append_stream(&self, conversation_id: Uuid, chunk: String) {
        // Verify conversation matches active
        if conversation_id != self.current_conversation_id {
            return;
        }
        
        // Update UI (existing UIKit code)
        self.append_message_to_ui(&chunk);
        
        // Trigger layout update
        self.update_layout();
    }
}
```

### Backwards Compatibility Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P13
/// Transitional wrapper for backwards compatibility
fn legacy_send_message(&self, content: String) {
    // During transition, support both old and new paths
    if cfg!(feature = "use_presenters") {
        self.send_message(content); // New event-based path
    } else {
        self.legacy_send_message_direct(content); // Old direct path
    }
}
```

## Pseudocode References

### ChatPresenter Integration (lines 120-146)
- Line 120-146: Message send flow with UserEvent emission
- Line 190-241: Chat event handlers that emit ViewCommands

### McpPresenter Integration (lines 300-316)
- Line 300-316: MCP server start flow with UserEvent emission
- Line 350-371: MCP event handlers that emit ViewCommands

### SettingsPresenter Integration (lines 430-444)
- Line 430-444: Profile update flow with UserEvent emission
- Line 420-444: Settings event handlers

## User Access Verification

### Feature Access Matrix

| Feature | Old Path | New Path | Verification |
|---------|----------|----------|--------------|
| Send message | ChatView → LlmService | ChatView → UserEvent → ChatPresenter → ChatService | Manual test: Send message, verify response |
| Start MCP server | McpView → McpService | McpView → UserEvent → McpPresenter → McpService | Manual test: Start server, verify running |
| Update profile | SettingsView → ProfileService | SettingsView → UserEvent → SettingsPresenter → ProfileService | Manual test: Update profile, verify saved |
| View history | HistoryView → ConversationStorage | HistoryView → ViewCommands (via presenter) | Manual test: Load conversation, verify displayed |

### Manual Test Scenarios

1. **Chat Flow**:
   - User opens app → ChatViewController loads
   - User types message → UserEvent::SendMessage emitted
   - ChatPresenter receives event → ChatService.send_message_stream called
   - ViewCommands received → UI updates progressively
   - **Verify**: Message appears in chat, streaming works

2. **MCP Server Flow**:
   - User opens MCP configuration → McpConfigureViewController loads
   - User configures server → UserEvent::StartMcpServer emitted
   - McpPresenter receives event → McpService.start_server called
   - ViewCommand::McpServerStarted received → UI updates
   - **Verify**: Server status shows as running, tools listed

3. **Settings Flow**:
   - User opens settings → SettingsViewController loads
   - User updates profile → UserEvent::UpdateProfile emitted
   - SettingsPresenter receives event → ProfileService.update_profile called
   - ViewCommand::ShowNotification received → UI confirms
   - **Verify**: Profile changes persist, notification shown

## Verification Commands

### Structural Verification

```bash
# Verify ViewCommand receivers in view controllers
grep -r "event_rx: broadcast::Receiver<ViewCommand>" src/ui/*.rs
# Expected: Found in chat_view.rs, mcp_configure_view.rs, mcp_add_view.rs, settings_view.rs

# Verify UserEvent emission
grep -r "UserEvent::" src/ui/*.rs | grep "event_tx.send"
# Expected: 10+ emission points

# Verify ViewCommand handlers
grep -r "handle_view_command" src/ui/*.rs
# Expected: Found in all modified view controllers

# Verify plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P13" src/ui/*.rs | wc -l
# Expected: 20+ occurrences

# Check no direct service calls (should be removed)
grep -r "llm_service\." src/ui/chat_view.rs | grep -v "/// "
# Expected: 0 matches (all direct calls replaced)
```

### Semantic Verification

```bash
# Build the application
cargo build --release 2>&1 | tee build.log
# Expected: Builds successfully, no errors

# Check for warnings
grep -E "warning:" build.log
# Expected: 0 warnings (or only acceptable warnings)

# Run unit tests
cargo test --lib 2>&1 | tee test_results.log
# Expected: All tests pass

# Verify no regressions
grep -E "FAILED" test_results.log
# Expected: 0 matches
```

## Success Criteria

- All view controllers modified to use presenters
- UserEvent emission replaces direct service calls
- ViewCommand handlers implemented for all views
- Event bus integration complete
- No direct service calls from UI layer
- Backwards compatibility maintained (if needed)
- All existing features accessible through new architecture
- Manual testing confirms user workflows work

## Failure Recovery

If integration fails:

1. **Identify Failing Component**:
   ```bash
   cargo build 2>&1 | grep "error:"
   ```

2. **Revert Specific File**:
   ```bash
   git checkout -- src/ui/chat_view.rs
   ```

3. **Continue with Other Views**:
   - Integration can proceed incrementally
   - Not all views need to convert simultaneously

4. **Feature Flag for Rollback**:
   ```rust
   if cfg!(feature = "use_presenters") {
       // New path
   } else {
       // Old path (working)
   }
   ```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P13.md`

Contents:

```markdown
Phase: P13
Completed: YYYY-MM-DD HH:MM
Files Modified:
  - src/ui/chat_view.rs (N lines changed, presenter integrated)
  - src/ui/mcp_configure_view.rs (N lines changed, presenter integrated)
  - src/ui/mcp_add_view.rs (N lines changed, presenter integrated)
  - src/ui/settings_view.rs (N lines changed, presenter integrated)
Integration:
  - UserEvent emission points: 10+
  - ViewCommand handlers: 4
  - Direct service calls removed: 15+
  - Event bus subscribers: 4
User Access:
  - Chat functionality: WORKING
  - MCP management: WORKING
  - Settings management: WORKING
  - Error display: WORKING
Verification:
  - cargo build --release: PASS
  - cargo test --lib: PASS
  - Manual testing: PASS
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 13a: UI Integration Verification
2. Verify all view controllers work with presenters
3. Verify user workflows are intact
4. Then proceed to Phase 14: Data Migration

## Important Notes

- **CRITICAL**: This phase connects new architecture to existing UI
- **BACKWARDS COMPATIBILITY**: Maintain during transition
- **INCREMENTAL**: Can convert one view at a time if needed
- **MANUAL TESTING**: Required for each view controller
- **NO LOGIC IN VIEW**: All business logic in presenters
- **EVENT-DRIVEN**: All communication via EventBus
