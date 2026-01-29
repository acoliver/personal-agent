# Architect Specification: Remediate Refactor - Integration-First

**Plan ID:** PLAN-20250127-REMEDIATE
**Author:** Software Architect
**Date:** 2025-01-27
**Status:** Draft

## 1. Overview

This specification defines a focused remediation of the PersonalAgent refactor to deliver a working end-to-end chat flow. The previous refactor created extensive architecture (EventBus, Services, Presenters) but left critical services hollow with placeholder implementations.

**Strategy: Integration-First**

Instead of filling in 47+ stubs across 6+ services, we work backwards from "user sends message, gets response":

1. Make ChatService actually call SerdesAI Agent
2. Wire MCP tools into ChatService 
3. Verify the event flow works end-to-end
4. Wire UI to use the new architecture

## 2. Current State Analysis

### 2.1 What Works (Keep)

| Component | Location | Status |
|-----------|----------|--------|
| EventBus | `src/events/` | [OK] Working, 14 tests pass |
| ConversationService | `src/services/conversation.rs` | [OK] File-based CRUD works |
| AppSettingsService | `src/services/app_settings.rs` | [OK] JSON settings work |
| Event Types | `src/events/types.rs` | [OK] Defined and used |

### 2.2 What's Hollow (Fix)

| Component | Location | Problem |
|-----------|----------|---------|
| ChatServiceImpl | `src/services/chat_impl.rs` | Returns placeholder string, no LLM call |
| McpServiceImpl | `src/services/mcp_impl.rs` | Config CRUD works, lifecycle doesn't |
| ProfileService | `src/services/profile.rs` | 8 `unimplemented!()` stubs |
| SecretsService | `src/services/secrets.rs` | 6 `unimplemented!()` stubs |
| ModelsRegistryService | `src/services/models_registry.rs` | 8 `unimplemented!()` stubs |
| McpRegistryService | `src/services/mcp_registry.rs` | 8 `unimplemented!()` stubs |

### 2.3 What's Not Wired (Connect Later)

| Component | Status |
|-----------|--------|
| ChatPresenter | Subscribes to events, but ChatService is hollow |
| UI Views | Still use old code paths, don't emit UserEvents |

## 3. Target State

After this remediation:

```
User types message in UI
  → UI emits UserEvent::SendMessage
  → ChatPresenter handles event
  → ChatService.send_message() is called
  → ChatService builds SerdesAI Agent with:
      - Profile from ProfileService
      - API key resolved
      - MCP tools from McpService.get_toolsets()
  → Agent streams response
  → ChatService emits ChatEvent::TextDelta, etc.
  → ChatPresenter updates UI
  → User sees real LLM response with tool use
```

## 4. Scope Decisions

### 4.1 In Scope

1. **ChatService calling SerdesAI Agent** - The ONE thing that matters
2. **ProfileService basics** - Get default profile, resolve API key
3. **McpService basics** - Start MCPs, provide toolsets
4. **End-to-end verification** - Prove the flow works

### 4.2 Out of Scope (Future Work)

- Full ProfileService CRUD (use existing `config/settings.rs` for now)
- Full SecretsService (use existing `mcp/secrets.rs` for now)
- ModelsRegistryService (use existing `registry/` module for now)
- McpRegistryService (use existing registry code for now)
- UI modifications (keep using existing chat_view.rs)
- Full Presenter implementation (verify event flow only)

### 4.3 Rationale

The existing modules (`config/settings.rs`, `mcp/secrets.rs`, `registry/`, `mcp/service.rs`) already work. The new service layer was supposed to wrap them but ended up with hollow stubs. Instead of implementing 47 stubs, we:

1. Make ChatService work by using existing working code
2. Prove the architecture works end-to-end
3. Incrementally migrate other services later

## 5. Technical Approach

### 5.1 ChatService Implementation

Replace placeholder with real SerdesAI Agent integration:

```rust
// src/services/chat_impl.rs

async fn send_message(&self, conversation_id: Uuid, content: String) -> ServiceResult<...> {
    // 1. Get conversation history
    let conversation = self.conversation_service.load(conversation_id).await?;
    
    // 2. Get profile and resolve API key
    let profile = self.profile_service.get_default().await?
        .ok_or(ServiceError::NoDefaultProfile)?;
    let api_key = self.resolve_api_key(&profile)?;
    
    // 3. Build SerdesAI Agent
    let model_config = ModelConfig::new(&format!("{}:{}", profile.provider_id, profile.model_id))
        .with_api_key(&api_key)
        .with_base_url(&profile.base_url);
    
    let mut builder = AgentBuilder::from_config(model_config)?
        .system_prompt(&profile.system_prompt);
    
    // 4. Add MCP toolsets
    for toolset in self.mcp_service.get_toolsets() {
        builder = builder.toolset(toolset);
    }
    
    let agent = builder.build();
    
    // 5. Stream response
    let history = self.convert_messages(&conversation.messages);
    let stream = AgentStream::new(&agent, UserContent::text(&content), (), options).await?;
    
    // 6. Map agent events to ChatStreamEvent
    // ... emit ChatEvent via EventBus ...
}
```

### 5.2 McpService Implementation

Wire to existing MCP runtime:

```rust
// src/services/mcp_impl.rs

fn get_toolsets(&self) -> Vec<Arc<dyn AbstractToolset>> {
    // Use existing MCP_SERVICE singleton to get running MCPs
    let configs = self.list_enabled().unwrap_or_default();
    let mut toolsets = Vec::new();
    
    for config in configs {
        if let Some(client) = MCP_SERVICE.get_client(&config.id) {
            toolsets.push(Arc::new(McpToolset::new(client)) as Arc<dyn AbstractToolset>);
        }
    }
    
    toolsets
}
```

### 5.3 ProfileService Integration

Bridge to existing config:

```rust
// src/services/profile.rs

async fn get_default(&self) -> ServiceResult<Option<ModelProfile>> {
    // Read from existing Settings
    let settings = load_settings()?;
    settings.default_profile.map(|id| {
        settings.profiles.iter().find(|p| p.id == id).cloned()
    })
}
```

## 6. Integration Points

### 6.1 Existing Code That Will Be Used

| Module | Purpose | How Used |
|--------|---------|----------|
| `src/llm/client.rs` | SerdesAI reference | Pattern for agent building |
| `src/mcp/service.rs` | MCP singleton | Get running MCP clients |
| `src/mcp/secrets.rs` | Secret resolution | Resolve API keys |
| `src/config/settings.rs` | Config loading | Get profiles |
| `src/agent/runtime.rs` | Tokio runtime | Spawn async tasks |

### 6.2 Existing Code That Will Be Replaced

None in this phase. We're wiring to existing code, not replacing it.

### 6.3 User Access Points

- Existing ChatView sends messages (no changes to UI)
- Response flows through new ChatService → EventBus → Presenter

## 7. Success Criteria

### 7.1 Functional Requirements

| ID | Requirement | Verification |
|----|-------------|--------------|
| REM-001 | ChatService.send_message calls SerdesAI Agent | Integration test |
| REM-002 | ChatService uses profile from ProfileService | Unit test |
| REM-003 | ChatService resolves API key correctly | Unit test |
| REM-004 | ChatService attaches MCP tools from McpService | Integration test |
| REM-005 | ChatService emits ChatEvent::TextDelta | Event test |
| REM-006 | ChatService emits ChatEvent::StreamCompleted | Event test |
| REM-007 | Tool calls work during streaming | Integration test |

### 7.2 Non-Functional Requirements

| ID | Requirement | Verification |
|----|-------------|--------------|
| REM-NF1 | No `unimplemented!()` in ChatServiceImpl | `grep` check |
| REM-NF2 | No placeholder strings in ChatServiceImpl | `grep` check |
| REM-NF3 | cargo build passes | Build check |
| REM-NF4 | cargo test passes | Test check |
| REM-NF5 | cargo clippy passes | Lint check |

### 7.3 Verification Evidence

Each phase must produce evidence file with:
- Exact grep output showing no placeholders
- Test output showing tests pass
- Manual test showing message → response flow

## 8. Phase Structure

This plan uses 4 focused phases instead of 16 generic phases:

| Phase | Name | Goal |
|-------|------|------|
| 01 | Preflight | Verify SerdesAI API, existing code works |
| 02 | ChatService Implementation | Make ChatService call SerdesAI Agent |
| 03 | MCP Integration | Wire MCP tools into ChatService |
| 04 | End-to-End Verification | Prove full flow works |

Each phase has a verification phase (01a, 02a, 03a, 04a) per PLAN-TEMPLATE.md.

## 9. Risk Assessment

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| SerdesAI API changes | Medium | Low | Check version, use existing working code as reference |
| Existing MCP service incompatible | Medium | Medium | Can fall back to direct MCP calls |
| Profile/settings format mismatch | Low | Medium | Use existing Settings struct directly |
| Event flow breaks | Medium | Low | EventBus is tested and working |

## 10. Anti-Fakery Rules

**CRITICAL: This plan has ZERO TOLERANCE for placeholders in implementation phases.**

### 10.1 Forbidden Patterns

```rust
// FORBIDDEN - will fail verification
fn send_message(&self, ...) -> ... {
    "placeholder response".to_string()  // NO
}

fn get_toolsets(&self) -> Vec<...> {
    Vec::new()  // NO if tools should exist
}

async fn call_llm(&self, ...) -> ... {
    unimplemented!()  // NO
    todo!()           // NO
}
```

### 10.2 Required Patterns

```rust
// REQUIRED - actual implementation
fn send_message(&self, ...) -> ... {
    let agent = self.build_agent(&profile)?;  // Actually builds agent
    let stream = agent.run(&content).await?;   // Actually calls LLM
    // ... actually processes stream ...
}

fn get_toolsets(&self) -> Vec<...> {
    self.running_mcps.iter()
        .filter(|m| m.enabled)
        .map(|m| Arc::new(McpToolset::new(m.client.clone())))
        .collect()  // Actually returns real toolsets
}
```

### 10.3 Verification Commands

Every verification phase MUST run these and show empty output:

```bash
grep -rn "unimplemented!" src/services/chat_impl.rs
grep -rn "todo!" src/services/chat_impl.rs
grep -rn "placeholder" src/services/chat_impl.rs
grep -rn "not yet implemented" src/services/chat_impl.rs
```

## 11. References

- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Target architecture
- `dev-docs/architecture/chat-flow.md` - Chat flow design
- `dev-docs/requirements/services/chat.md` - Chat service requirements
- `dev-docs/requirements/services/mcp.md` - MCP service requirements
- `dev-docs/requirements/events.md` - Event system requirements
- `src/llm/client.rs` - Working SerdesAI integration reference
- `src/mcp/service.rs` - Working MCP integration reference

---

**Next Steps:** Create phase files in `plan/` directory following PLAN-TEMPLATE.md structure.
