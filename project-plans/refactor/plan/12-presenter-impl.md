# Phase 12: Presenter Layer Implementation

## Phase ID

`PLAN-20250125-REFACTOR.P12`

## Prerequisites

- Required: Phase 11a (Presenter TDD Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P11A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P11A.md`
  - All presenter tests written and verified
  - Test failures documented (expected against stubs)
- Preflight verification: Phases 01-11a completed

## Purpose

Implement real functionality for all presenter layer components to make tests pass. This phase:

1. Replaces stub implementations with real implementations
2. Implements all presenter event loops
3. Implements event routing and handling
4. Ensures all tests pass (TDD green state)
5. Verifies ViewCommand emission works correctly

**Note:** This is an IMPLEMENTATION phase. All tests from Phase 11 should PASS after this phase.

## Requirements Implemented (Expanded)

### REQ-027.1: ChatPresenter Implementation

**Full Text**: ChatPresenter MUST handle user chat events and service coordination.

**Behavior**:
- GIVEN: ChatPresenter instance with dependencies
- WHEN: start() called
- THEN: Event loop spawned in background
- AND: Subscribes to AppEvent stream
- WHEN: UserEvent::SendMessage received
- THEN: ChatService.send_message_stream() called
- AND: Thinking events monitored
- AND: ViewCommands emitted to UI

**Why This Matters**: ChatPresenter is primary UI-event coordinator.

### REQ-027.2: McpPresenter Implementation

**Full Text**: McpPresenter MUST handle MCP server management UI.

**Behavior**:
- GIVEN: McpPresenter instance
- WHEN: UserEvent::StartMcpServer received
- THEN: McpService.start_server() called
- AND: McpEvents monitored
- AND: ViewCommands emitted for server status

**Why This Matters**: McpPresenter enables tool management through UI.

### REQ-027.3: SettingsPresenter Implementation

**Full Text**: SettingsPresenter MUST handle settings and profile management UI.

**Behavior**:
- GIVEN: SettingsPresenter instance
- WHEN: UserEvent::OpenSettings received
- THEN: ProfileService.list_profiles() called
- AND: ViewCommand::ShowSettings emitted
- WHEN: UserEvent::UpdateProfile received
- THEN: ProfileService.update_profile() called
- AND: Notification emitted

**Why This Matters**: SettingsPresenter enables configuration management.

### REQ-027.4: ErrorPresenter Implementation

**Full Text**: ErrorPresenter MUST handle error display and logging.

**Behavior**:
- GIVEN: ErrorPresenter instance
- WHEN: Error event received
- THEN: ViewCommand::ShowError emitted
- AND: Error severity set correctly
- AND: Error logged appropriately

**Why This Matters**: Centralized error handling ensures consistent error UX.

## Implementation Tasks

### Files to Modify

- `src/presentation/chat.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P12`
  - Implements: `@requirement:REQ-027.1`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 20-240
  - Implement:
    - new(): Initialize event_rx, services, view_tx, running flag
    - start(): Spawn event loop task, set running flag
    - stop(): Set running flag, wait for task completion
    - is_running(): Return running flag state
    - handle_event(): Route to appropriate handler
    - handle_user_event(): Route user events
    - handle_chat_event(): Route chat events, emit ViewCommands
    - Event loop: Receive events, dispatch to handlers

- `src/presentation/mcp.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P12`
  - Implements: `@requirement:REQ-027.2`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 260-371
  - Implement:
    - new(): Initialize event_rx, services, view_tx, running flag
    - start(): Spawn event loop task
    - stop(): Set running flag
    - handle_event(): Route to appropriate handler
    - handle_user_event(): Route MCP user events
    - handle_mcp_event(): Route MCP events, emit ViewCommands

- `src/presentation/settings.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P12`
  - Implements: `@requirement:REQ-027.3`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 380-444
  - Implement:
    - new(): Initialize event_rx, services, view_tx, running flag
    - start(): Spawn event loop task
    - stop(): Set running flag
    - handle_event(): Route to appropriate handler
    - handle_user_event(): Route settings events, emit ViewCommands

- `src/presentation/error.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P12`
  - Implements: `@requirement:REQ-027.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/presenters.md` lines 450-505
  - Implement:
    - new(): Initialize event_rx, view_tx, running flag
    - start(): Spawn event loop task
    - stop(): Set running flag
    - handle_event(): Route error events, emit ViewCommands

## Implementation Patterns

### Event Loop Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
/// @pseudocode presenters.md lines 50-69
impl ChatPresenter {
    pub fn new(
        services: Arc<ServiceRegistry>,
        view_tx: broadcast::Sender<ViewCommand>
    ) -> Self {
        let rx = get_event_bus().subscribe();
        ChatPresenter {
            rx,
            services,
            view_tx,
            running: Arc::new(AtomicBool::new(false)),
            runtime: services.runtime.clone(),
        }
    }

    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, Ordering::Relaxed);

        let rx = self.rx.resubscribe();
        let running = self.running.clone();
        let services = self.services.clone();
        let view_tx = self.view_tx.clone();

        self.runtime.spawn(async move {
            while running.load(Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event_internal(&services, &view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ChatPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ChatPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ChatPresenter event loop ended");
        });

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    async fn handle_event_internal(
        services: &ServiceRegistry,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event_internal(services, view_tx, user_evt).await;
            }
            AppEvent::Chat(chat_evt) => {
                Self::handle_chat_event_internal(view_tx, chat_evt).await;
            }
            _ => {} // Ignore other events
        }
    }
}
```

### Event Handler Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
/// @pseudocode presenters.md lines 160-240
impl ChatPresenter {
    async fn handle_chat_event_internal(
        view_tx: &broadcast::Sender<ViewCommand>,
        event: ChatEvent
    ) {
        match event {
            ChatEvent::ConversationStarted { id, profile_id } => {
                let _ = view_tx.send(ViewCommand::ConversationCreated { id, profile_id });
            }

            ChatEvent::MessageReceived { conversation_id, message } => {
                let _ = view_tx.send(ViewCommand::MessageAppended {
                    conversation_id,
                    role: message.role,
                    content: message.content,
                });
            }

            ChatEvent::ThinkingStarted { conversation_id } => {
                let _ = view_tx.send(ViewCommand::ShowThinking { conversation_id });
            }

            ChatEvent::ThinkingEnded { conversation_id } => {
                let _ = view_tx.send(ViewCommand::HideThinking { conversation_id });
            }

            ChatEvent::StreamChunk { conversation_id, chunk } => {
                let _ = view_tx.send(ViewCommand::AppendStream { conversation_id, chunk });
            }

            ChatEvent::ResponseGenerated { conversation_id, tokens } => {
                let _ = view_tx.send(ViewCommand::FinalizeStream { conversation_id, tokens });
            }

            ChatEvent::Error { conversation_id, error } => {
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Chat Error".to_string(),
                    message: error,
                    severity: ErrorSeverity::Warning,
                });
            }

            _ => {} // Ignore other chat events
        }
    }
}
```

### User Event Handler Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
/// @pseudocode presenters.md lines 120-146
impl ChatPresenter {
    async fn handle_user_event_internal(
        services: &ServiceRegistry,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent
    ) {
        match event {
            UserEvent::SendMessage { conversation_id, content } => {
                // Validate
                if content.trim().is_empty() {
                    return;
                }

                // Create conversation if needed
                let target_id = if conversation_id.is_nil() {
                    match services.profiles.get_default() {
                        Some(profile) => {
                            match services.conversations.create(profile.id).await {
                                Ok(new_id) => {
                                    let _ = services.conversations.set_active(new_id).await;
                                    new_id
                                }
                                Err(e) => {
                                    let _ = view_tx.send(ViewCommand::ShowError {
                                        title: "Error".to_string(),
                                        message: format!("Failed to create conversation: {}", e),
                                        severity: ErrorSeverity::Error,
                                    });
                                    return;
                                }
                            }
                        }
                        None => {
                            let _ = view_tx.send(ViewCommand::ShowError {
                                title: "Error".to_string(),
                                message: "No default profile configured".to_string(),
                                severity: ErrorSeverity::Error,
                            });
                            return;
                        }
                    }
                } else {
                    conversation_id
                };

                // Send message via service
                match services.chat.send_message_stream(target_id, content).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Failed to send message: {}", e);
                        let _ = view_tx.send(ViewCommand::ShowError {
                            title: "Error".to_string(),
                            message: format!("Failed to send message: {}", e),
                            severity: ErrorSeverity::Error,
                        });
                    }
                }
            }

            UserEvent::CancelRequest { conversation_id } => {
                // Handle cancellation
                tracing::info!("Cancel requested for conversation: {}", conversation_id);
            }

            _ => {} // Ignore other user events
        }
    }
}
```

## Pseudocode References

### ChatPresenter (lines 20-240)
- Lines 30-42: new() constructor
- Lines 50-69: start() with event loop
- Lines 80-85: handle_event() dispatcher
- Lines 100-107: handle_user_event() dispatcher
- Lines 120-146: on_send_message() handler
- Lines 160-177: handle_chat_event() dispatcher
- Lines 190-241: Individual event handlers

### McpPresenter (lines 260-371)
- Lines 270-282: new() constructor
- Lines 280-289: handle_user_event() dispatcher
- Lines 300-316: on_start_server() handler
- Lines 330-339: handle_mcp_event() dispatcher
- Lines 350-371: Individual event handlers

### SettingsPresenter (lines 380-444)
- Lines 380-385: new() constructor
- Lines 400-409: handle_user_event() dispatcher
- Lines 420-444: Individual event handlers

### ErrorPresenter (lines 450-505)
- Lines 450-453: new() constructor
- Lines 470-479: handle_event() dispatcher
- Lines 490-505: Error handlers

## Verification Commands

### Implementation Verification

```bash
# Verify unimplemented! removed from implementations
grep -r "unimplemented!" src/presentation/*.rs | grep -v "test" | grep -v "// STUB"
# Expected: 0 matches (all stubs replaced)

# Verify event loop patterns
grep -r "WHILE.*running" src/presentation/*.rs | grep -v "test"
# Expected: 4+ event loops (one per presenter)

# Verify spawn() patterns
grep -r "spawn(" src/presentation/*.rs | grep -v "test"
# Expected: 4+ spawn calls (event loops)

# Verify ViewCommand emission
grep -r "view_tx.send" src/presentation/*.rs | grep -v "test"
# Expected: 20+ ViewCommand emissions

# Check plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P12" src/presentation/*.rs | wc -l
# Expected: 30+ occurrences
```

### Test Execution Verification

```bash
# Run tests (EXPECTED: All tests PASS)
cargo test --lib -- --test-threads=1 2>&1 | tee test_results.log

# Check test summary
grep -E "test result:" test_results.log | tail -1
# Expected: test result: ok. X passed in Ys

# Verify no test failures
grep -E "FAILED" test_results.log
# Expected: 0 matches

# Verify no panics
grep -E "panicked" test_results.log
# Expected: 0 matches
```

## Success Criteria

- All stub implementations replaced with real implementations
- All tests pass (0 failures)
- All unimplemented!() calls removed
- Event loops implemented for all presenters
- ViewCommand emission verified in all presenters
- Event routing works correctly
- Error handling implemented for all methods
- Plan markers present in all implementations

## Failure Recovery

If implementation fails:

1. Identify failing tests:
   ```bash
   cargo test --lib 2>&1 | grep FAILED
   ```

2. Debug specific test:
   ```bash
   cargo test --lib test_name -- --nocapture
   ```

3. Fix implementation and re-test

4. If unable to fix, can rollback to stubs:
   ```bash
   git checkout -- src/presentation/
   ```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P12.md`

Contents:

```markdown
Phase: P12
Completed: YYYY-MM-DD HH:MM
Files Modified:
  - src/presentation/chat.rs (N lines, implemented)
  - src/presentation/mcp.rs (N lines, implemented)
  - src/presentation/settings.rs (N lines, implemented)
  - src/presentation/error.rs (N lines, implemented)
Tests Status:
  - Total tests: 40+
  - Passed: 40+
  - Failed: 0
Implementation:
  - Stub methods replaced: 25+
  - Event loops: 4
  - ViewCommand emissions: 20+
  - Error handling: Complete
Verification:
  - cargo test --lib: PASS
  - Plan markers: 30+ found
  - Unimplemented removed: YES
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 12a: Presenter Implementation Verification
2. Verify all tests pass
3. Verify event loops work correctly
4. Then proceed to Phase 13: Integration Testing

## Important Notes

- This is an IMPLEMENTATION phase - make tests pass
- Event-driven architecture (subscribe to EventBus)
- Presenters are stateless (except event receivers)
- Always emit ViewCommands (never direct UI manipulation)
- Handle lag in event loops (RecvError::Lagged)
- Graceful shutdown (AtomicBool running flag)
- Next phase will verify implementation
