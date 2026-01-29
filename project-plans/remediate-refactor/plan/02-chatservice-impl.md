# Phase 02: ChatService Implementation

## Phase ID

`PLAN-20250127-REMEDIATE.P02`

## Prerequisites

- Required: Phase 01a completed with PASS verdict
- Verification: `grep "^## Verdict: PASS" project-plans/remediate-refactor/plan/.completed/P01A.md`
- Evidence file exists: `project-plans/remediate-refactor/plan/.completed/P01A.md`

## Requirements Implemented (Expanded)

### REM-001: ChatService.send_message calls SerdesAI Agent

**Full Text**: ChatService.send_message() must create a SerdesAI Agent and stream the response, not return a placeholder string.

**Behavior**:
- GIVEN: A conversation ID and message content
- WHEN: send_message() is called
- THEN: A SerdesAI Agent is built and executed, streaming real LLM response

**Why This Matters**: This is THE core functionality. Without it, the app doesn't work.

### REM-002: ChatService uses profile from ProfileService

**Full Text**: ChatService must get the active model profile from ProfileService, not hardcode values.

**Behavior**:
- GIVEN: A default profile is configured
- WHEN: send_message() is called
- THEN: Profile is retrieved and used to configure the Agent

**Why This Matters**: Users configure their API keys and models via profiles.

### REM-003: ChatService resolves API key correctly

**Full Text**: API key must be resolved from profile.auth configuration.

**Behavior**:
- GIVEN: Profile has AuthConfig::Key or AuthConfig::Keyfile
- WHEN: Building the Agent
- THEN: API key is resolved and passed to ModelConfig

**Why This Matters**: Can't call LLM without valid API key.

### REM-005: ChatService emits ChatEvent::TextDelta

**Full Text**: As streaming progresses, ChatService must emit ChatEvent::TextDelta via EventBus.

**Behavior**:
- GIVEN: Agent is streaming a response
- WHEN: Text content is received
- THEN: ChatEvent::TextDelta is emitted with the text

**Why This Matters**: UI needs events to update in real-time.

### REM-006: ChatService emits ChatEvent::StreamCompleted

**Full Text**: When streaming finishes, ChatService must emit ChatEvent::StreamCompleted.

**Behavior**:
- GIVEN: Agent has finished streaming
- WHEN: Stream ends
- THEN: ChatEvent::StreamCompleted is emitted

**Why This Matters**: UI needs to know when to finalize the message.

## Implementation Tasks

### Files to Modify

#### 1. `src/services/chat_impl.rs` - MAIN IMPLEMENTATION

Replace the placeholder implementation with real SerdesAI Agent integration.

**Current Code to Replace** (lines ~98-106):

```rust
// TODO: Actually call LLM and stream response
// For now, create a simple stream that emits a placeholder response
let placeholder_response =
    "This is a placeholder response. LLM integration will be implemented in Phase 09 stretch goal.".to_string();
```

**New Implementation Must**:

1. Get conversation history from ConversationService
2. Get profile from ProfileService.get_default()
3. Resolve API key from profile.auth
4. Build SerdesAI Agent with:
   - ModelConfig (provider:model_id format)
   - API key
   - Base URL from profile
   - System prompt from profile
   - Temperature, max_tokens from profile.parameters
5. Create AgentStream with message history
6. Map AgentStreamEvent to ChatStreamEvent
7. Emit ChatEvent variants via EventBus
8. Save assistant message on completion

**Reference Implementation**:

See `src/llm/stream.rs` for the working pattern. Key imports:

```rust
use serdes_ai::agent::{AgentBuilder, AgentStreamEvent, ModelConfig, RunOptions};
use serdes_ai::core::messages::{ModelRequest, ModelRequestPart, ModelResponse, UserPromptPart};
```

The existing `send_message_stream()` function in `src/llm/stream.rs` shows:
- How to resolve API keys from `AuthConfig`
- How to build `ModelConfig` with `provider:model` format
- How to use `AgentBuilder::from_config()`
- How to convert conversation history to `ModelRequest` format
- How to stream with `run_stream_with_options()`

**ChatServiceImpl should reuse this existing functionality rather than reimplementing it.**

**Code Markers Required**:

```rust
/// @plan PLAN-20250127-REMEDIATE.P02
/// @requirement REM-001, REM-002, REM-003, REM-005, REM-006
```

### Implementation Approach

**Option A: Reuse LlmClient (Recommended)**

The existing `src/llm/client.rs` and `src/llm/stream.rs` already have working SerdesAI integration. ChatServiceImpl can:

1. Create an `LlmClient` from the profile
2. Call `send_message_stream()` which already does Agent building
3. Map the resulting stream to emit ChatEvents

This avoids duplicating the Agent building logic.

**Option B: Inline Implementation**

If we need more control (e.g., for MCP toolsets), we can inline the logic from `stream.rs`:

```rust
/// @plan PLAN-20250127-REMEDIATE.P02
/// @requirement REM-003
fn resolve_api_key(&self, profile: &ModelProfile) -> ServiceResult<String> {
    // Same pattern as in stream.rs:65-74
    match &profile.auth {
        AuthConfig::Key { value } => Ok(value.clone()),
        AuthConfig::Keyfile { path } => {
            std::fs::read_to_string(path)
                .map(|s| s.trim().to_string())
                .map_err(|e| ServiceError::Internal(format!("Failed to read keyfile: {}", e)))
        }
        _ => Err(ServiceError::Internal("Unsupported auth method".to_string())),
    }
}
```

**Decision**: Start with Option A (reuse LlmClient). Only switch to Option B if MCP toolset integration requires it.

## CRITICAL: Anti-Placeholder Rules

**THIS IS AN IMPLEMENTATION PHASE. The following are COMPLETE FAILURE:**

- `unimplemented!()` anywhere = FAIL
- `todo!()` anywhere = FAIL
- `// TODO:` comments = FAIL
- Placeholder strings like "placeholder response" = FAIL
- Empty function bodies = FAIL
- Returning defaults without doing real work = FAIL

**The code MUST actually call SerdesAI and stream a response.**

## Verification Commands (Run Before Claiming Done)

```bash
# 1. Placeholder detection (MUST ALL RETURN EMPTY)
grep -rn "unimplemented!" src/services/chat_impl.rs
grep -rn "todo!" src/services/chat_impl.rs
grep -rn "placeholder" src/services/chat_impl.rs
grep -rn "not yet implemented" src/services/chat_impl.rs

# 2. Build check
cargo build --all-targets

# 3. Test check
cargo test services::chat

# 4. Verify markers exist
grep -c "@plan PLAN-20250127-REMEDIATE.P02" src/services/chat_impl.rs
grep -c "@requirement REM-00" src/services/chat_impl.rs
```

## Success Criteria

- [ ] All placeholder detection commands return EMPTY
- [ ] `cargo build --all-targets` passes
- [ ] `cargo test services::chat` passes
- [ ] Code markers present in implementation
- [ ] ChatService.send_message() actually calls SerdesAI Agent
- [ ] API key resolution works for Key and Keyfile auth
- [ ] Events are emitted via EventBus

## Deliverables

1. Modified `src/services/chat_impl.rs` with real implementation
2. Evidence file at `project-plans/remediate-refactor/plan/.completed/P02.md`

## Phase Completion Marker

Create: `project-plans/remediate-refactor/plan/.completed/P02.md`

Contents MUST include:
- All grep outputs (must be empty)
- Build output (must pass)
- Test output (must pass)
- Brief description of what was implemented
- Verdict: PASS or FAIL (no conditional)

## Failure Recovery

If this phase fails:
1. `git checkout -- src/services/chat_impl.rs`
2. Review error messages
3. Fix issues and re-run verification
4. Do NOT proceed to Phase 03 until this passes
