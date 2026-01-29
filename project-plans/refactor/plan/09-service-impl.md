# Phase 09: Service Layer Implementation

## Phase ID

`PLAN-20250125-REFACTOR.P09`

## Prerequisites

- Required: Phase 08a (Service TDD Verification) completed
- Verification: `grep -r "@plan:PLAN-20250125-REFACTOR.P08A" project-plans/`
- Expected files from previous phase:
  - `project-plans/refactor/plan/.completed/P08A.md`
  - All service tests written and verified
  - Test failures documented (expected against stubs)
- Preflight verification: Phases 01-08a completed

## Purpose

Implement real functionality for all service layer components to make tests pass. This phase:

1. Replaces stub implementations with real implementations
2. Implements all service methods with proper business logic
3. Ensures all tests pass (TDD green state)
4. Verifies event emission patterns work correctly
5. Implements proper error handling and concurrent access

**Note:** This is an IMPLEMENTATION phase. All tests from Phase 08 should PASS after this phase.

## Requirements Implemented (Expanded)

### REQ-024.1: ConversationService Implementation

**Full Text**: ConversationService MUST manage conversation state and message history.

**Behavior**:
- GIVEN: ConversationService instance with storage
- WHEN: create_conversation() called
- THEN: Conversation created with unique ID
- AND: Conversation stored in HashMap
- AND: ConversationStarted event emitted
- WHEN: send_message() called
- THEN: Message appended to conversation
- AND: MessageReceived event emitted

**Why This Matters**: Conversation management is core to chat functionality.

### REQ-024.2: ChatService Implementation

**Full Text**: ChatService MUST handle LLM integration and response generation.

**Behavior**:
- GIVEN: ChatService instance with dependencies
- WHEN: send_message_stream() called
- THEN: Background task spawned
- AND: ThinkingStarted event emitted
- AND: Message history built
- AND: LLM service called
- AND: StreamChunk events emitted
- AND: ResponseGenerated event emitted
- AND: ThinkingEnded event emitted

**Why This Matters**: Chat flow coordination requires proper async handling.

### REQ-024.3: McpService Implementation

**Full Text**: McpService MUST manage MCP server lifecycle and tool execution.

**Behavior**:
- GIVEN: McpService instance
- WHEN: start_server() called
- THEN: MCP server spawned as background task
- AND: Tools registered in tool registry
- AND: ServerStarted event emitted
- WHEN: call_tool() called
- THEN: Tool routed to correct MCP server
- AND: ToolCalled event emitted
- AND: ToolResult event emitted

**Why This Matters**: MCP integration enables tool extensibility.

### REQ-024.4: ProfileService Implementation

**Full Text**: ProfileService MUST manage user profile and preferences.

**Behavior**:
- GIVEN: ProfileService instance with storage
- WHEN: add_profile() called
- THEN: Profile validated
- AND: Profile stored in HashMap
- AND: Profile persisted to storage
- AND: ProfileAdded event emitted

**Why This Matters**: Profile management requires validation and persistence.

### REQ-024.5: SecretsService Implementation

**Full Text**: SecretsService MUST securely store and retrieve API keys.

**Behavior**:
- GIVEN: SecretsService instance with keyring
- WHEN: set_api_key() called
- THEN: Key encrypted and stored
- AND: No plaintext in memory
- WHEN: get_api_key() called
- THEN: Key retrieved and decrypted
- AND: Returned to caller

**Why This Matters**: Secure credential storage is critical for security.

## Implementation Tasks

### Files to Modify

- `src/services/conversation.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P09`
  - Implements: `@requirement:REQ-024.1`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 30-164
  - Implement:
    - new(): Initialize HashMap, storage, event_tx
    - create_conversation(): Create and store conversation, emit event
    - send_message(): Append message, emit event
    - get_conversation(): Retrieve from HashMap
    - set_active(): Update active_id, emit event
    - get_active(): Return active_id

- `src/services/chat.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P09`
  - Implements: `@requirement:REQ-024.2`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 170-251
  - Implement:
    - new(): Initialize dependencies
    - send_message_stream(): Spawn task, emit thinking events
    - build_message_history(): Build message list
    - Stream callback to emit StreamChunk events

- `src/services/mcp.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P09`
  - Implements: `@requirement:REQ-024.3`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 260-360
  - Implement:
    - new(): Initialize HashMaps, secrets, runtime
    - start_server(): Spawn MCP task, register tools, emit events
    - call_tool(): Route to MCP, emit events
    - list_tools(): Return all tools from all connections

- `src/services/profile.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P09`
  - Implements: `@requirement:REQ-024.4`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 370-426
  - Implement:
    - new(): Initialize HashMap, storage, event_tx
    - add_profile(): Validate, store, persist, emit event
    - get_profile(): Retrieve from HashMap
    - update_profile(): Update in HashMap, persist, emit event
    - list_profiles(): Return all profiles

- `src/services/secrets.rs`
  - Replace stub implementations with real implementations
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P09`
  - Implements: `@requirement:REQ-024.5`
  - Reference: `project-plans/refactor/analysis/pseudocode/services.md` lines 430-452
  - Implement:
    - new(): Initialize SecretsManager
    - get_api_key(): Retrieve and decrypt key
    - set_api_key(): Encrypt and store key
    - delete_api_key(): Remove key from storage

## Implementation Patterns

### Arc<Mutex<T>> Pattern for Shared State

```rust
/// @plan PLAN-20250125-REFACTOR.P09
/// @requirement REQ-024.1
/// @pseudocode services.md lines 30-51
impl ConversationService {
    pub fn new(
        storage: Arc<ConversationRepository>,
        event_tx: broadcast::Sender<AppEvent>
    ) -> Self {
        ConversationService {
            conversations: Arc::new(Mutex::new(HashMap::new())),
            storage,
            active_id: Arc::new(Mutex::new(None)),
            event_tx,
            metrics: Arc::new(Mutex::new(ServiceMetrics::default())),
        }
    }

    pub async fn create_conversation(
        &self,
        profile_id: Uuid
    ) -> Result<Uuid, ServiceError> {
        // Validate
        if !self.validate_profile_id(profile_id).await {
            return Err(ServiceError::InvalidInput("Invalid profile_id".to_string()));
        }

        // Create conversation
        let id = Uuid::new_v4();
        let conversation = Conversation {
            id,
            profile_id,
            messages: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Store in HashMap (minimal lock scope)
        {
            let mut convs = self.conversations.lock().unwrap();
            convs.insert(id, conversation.clone());
        }  // Lock released here

        // Emit event
        let _ = self.event_tx.send(AppEvent::Chat(
            ChatEvent::ConversationStarted { id, profile_id }
        ));

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.creation_count += 1;
        }

        Ok(id)
    }
}
```

### Event Emission Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P09
/// @requirement REQ-024.2
impl ChatService {
    pub async fn send_message_stream(
        &self,
        conversation_id: Uuid,
        message: String
    ) -> Result<(), ServiceError> {
        // Validate
        let conversation = self.conversations.get_conversation(conversation_id)
            .ok_or(ServiceError::NotFound("Conversation not found".to_string()))?;

        let profile = self.profiles.get_profile(conversation.profile_id)
            .ok_or(ServiceError::NotFound("Profile not found".to_string()))?;

        // Emit thinking started
        let _ = self.event_tx.send(AppEvent::Chat(
            ChatEvent::ThinkingStarted { conversation_id }
        ));

        // Spawn background task for streaming
        let event_tx = self.event_tx.clone();
        let conv_service = self.conversations.clone();
        let llm = self.llm.clone();

        self.runtime.spawn(async move {
            // Build message history
            let history = conversation.messages.clone();

            // Stream callback
            let callback = |chunk: String| {
                let _ = event_tx.send(AppEvent::Chat(
                    ChatEvent::StreamChunk { conversation_id, chunk }
                ));
            };

            // Call LLM
            match llm.request_stream(&profile, history, callback).await {
                Ok(response) => {
                    let _ = event_tx.send(AppEvent::Chat(
                        ChatEvent::ResponseGenerated {
                            conversation_id,
                            tokens: response.tokens
                        }
                    ));

                    // Store assistant message
                    let _ = conv_service.send_message(conversation_id, response.content).await;
                }
                Err(e) => {
                    let _ = event_tx.send(AppEvent::Chat(
                        ChatEvent::Error {
                            conversation_id,
                            error: e.to_string()
                        }
                    ));
                }
            }

            // Emit thinking ended
            let _ = event_tx.send(AppEvent::Chat(
                ChatEvent::ThinkingEnded { conversation_id }
            ));
        });

        Ok(())
    }
}
```

### Error Handling Pattern

```rust
/// @plan PLAN-20250125-REFACTOR.P09
/// @requirement REQ-024.3
impl McpService {
    pub async fn call_tool(
        &self,
        tool_name: &str,
        args: Value
    ) -> Result<Value, ServiceError> {
        // Lookup tool in registry (minimal lock scope)
        let mcp_id = {
            let tools = self.tools.lock().unwrap();
            tools.get(tool_name).copied()
        };

        let mcp_id = mcp_id.ok_or_else(|| {
            ServiceError::NotFound(format!("Tool not found: {}", tool_name))
        })?;

        // Get connection (minimal lock scope)
        let connection = {
            let conns = self.connections.lock().unwrap();
            conns.get(&mcp_id).cloned()
        };

        let mut connection = connection.ok_or_else(|| {
            ServiceError::NotFound(format!("MCP connection not found: {}", mcp_id))
        })?;

        // Emit tool called event
        let _ = self.event_tx.send(AppEvent::Mcp(
            McpEvent::ToolCalled {
                tool_name: tool_name.to_string(),
                args: args.clone()
            }
        ));

        // Call tool
        let result = connection.call_tool(tool_name, args).await
            .map_err(|e| ServiceError::Request(e.to_string()))?;

        // Emit tool result event
        let _ = self.event_tx.send(AppEvent::Mcp(
            McpEvent::ToolResult {
                tool_name: tool_name.to_string(),
                result: result.clone()
            }
        ));

        Ok(result)
    }
}
```

## Pseudocode References

### ConversationService (lines 30-164)
- Lines 40-52: new() constructor
- Lines 60-79: create_conversation() method
- Lines 90-117: send_message() method
- Lines 130-134: get_conversation() method
- Lines 140-144: list_conversations() method
- Lines 150-158: set_active() method
- Lines 160-164: get_active() method

### ChatService (lines 170-251)
- Lines 180-189: new() constructor
- Lines 200-234: send_message_stream() method
- Lines 250-251: build_message_history() method

### McpService (lines 260-360)
- Lines 270-282: new() constructor
- Lines 290-319: start_server() method
- Lines 340-360: call_tool() method

### ProfileService (lines 370-426)
- Lines 380-389: new() constructor
- Lines 390-400: add_profile() method
- Lines 410-414: get_profile() method
- Lines 420-426: list_profiles() method

### SecretsService (lines 430-452)
- Lines 440-442: new() constructor
- Lines 445-448: get_api_key() method
- Lines 450-452: set_api_key() method

## Verification Commands

### Implementation Verification

```bash
# Verify unimplemented! removed from implementations
grep -r "unimplemented!" src/services/*.rs | grep -v "test" | grep -v "// STUB"
# Expected: 0 matches (all stubs replaced)

# Verify Arc<Mutex<T>> patterns
grep -r "Arc::new(Mutex::new" src/services/*.rs | grep -v "test"
# Expected: 10+ matches (shared state initialized)

# Verify event emission patterns
grep -r "event_tx.send" src/services/*.rs | grep -v "test"
# Expected: 20+ matches (events emitted)

# Check plan markers
grep -r "@plan:PLAN-20250125-REFACTOR.P09" src/services/*.rs | wc -l
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

### Coverage Verification

```bash
# Generate coverage report
cargo tarpaulin --lib --output-dir coverage --output Html

# Check coverage improved from Phase 08
# Expected: Coverage >= 80% (up from X% in stub phase)
```

## Success Criteria

- All stub implementations replaced with real implementations
- All tests pass (0 failures)
- All unimplemented!() calls removed
- Event emission verified in all services
- Concurrent access patterns correct (minimal lock scope)
- Error handling implemented for all methods
- Plan markers present in all implementations
- Test coverage >= 80%

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
   git checkout -- src/services/
   ```

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P09.md`

Contents:

```markdown
Phase: P09
Completed: YYYY-MM-DD HH:MM
Files Modified:
  - src/services/conversation.rs (N lines, implemented)
  - src/services/chat.rs (N lines, implemented)
  - src/services/mcp.rs (N lines, implemented)
  - src/services/profile.rs (N lines, implemented)
  - src/services/secrets.rs (N lines, implemented)
Tests Status:
  - Total tests: 50+
  - Passed: 50+
  - Failed: 0
  - Coverage: >= 80%
Implementation:
  - Stub methods replaced: 25+
  - Event emissions: 20+
  - Arc<Mutex<T>> patterns: 10+
  - Error handling: Complete
Verification:
  - cargo test --lib: PASS
  - Plan markers: 30+ found
  - Unimplemented removed: YES
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 09a: Service Implementation Verification
2. Verify all tests pass
3. Verify coverage meets requirements
4. Then proceed to Phase 10: Presenter Layer Stub Phase

## Important Notes

- This is an IMPLEMENTATION phase - make tests pass
- Focus on correctness, not optimization
- Follow Arc<Mutex<T>> patterns for thread safety
- Always emit domain events
- Keep lock scopes minimal
- Handle all error cases
- Next phase will start Presenter layer
