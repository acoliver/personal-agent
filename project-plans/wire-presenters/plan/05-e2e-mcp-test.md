# Phase 05: E2E Test - MCP Lifecycle Events

**Phase ID**: P05
**Type**: Implementation
**Status**: Pending
**Prerequisites**: P04a completion marker exists with PASS verdict

## Objective

Create end-to-end integration tests that verify SettingsPresenter correctly responds to MCP lifecycle events from the event bus. Per dev-docs/COORDINATING.md, this is an IMPLEMENTATION phase - test code must be complete, not stubbed.

## Test Scenarios

### Test 1: MCP Server Start Success

**Events to emit:**
1. `AppEvent::User(UserEvent::ToggleMcp { id: mcp_id, enabled: true })`
2. `AppEvent::Mcp(McpEvent::Starting { id: mcp_id, name: "exa".to_string() })`
3. `AppEvent::Mcp(McpEvent::Started { id: mcp_id, name: "exa".to_string(), tools: vec!["search".to_string(), "crawl".to_string()], tool_count: 2 })`

**Expected ViewCommands:**
- `ViewCommand::UpdateMcpStatus { id: mcp_id, status: McpStatus::Starting }`
- `ViewCommand::UpdateMcpStatus { id: mcp_id, status: McpStatus::Running, tool_count: 2 }`
- `ViewCommand::ShowMcpTools { id: mcp_id, tools: vec!["search", "crawl"] }`

### Test 2: MCP Server Start Failure

**Events to emit:**
1. `AppEvent::User(UserEvent::ToggleMcp { id: mcp_id, enabled: true })`
2. `AppEvent::Mcp(McpEvent::Starting { id: mcp_id, name: "broken-mcp".to_string() })`
3. `AppEvent::Mcp(McpEvent::StartFailed { id: mcp_id, name: "broken-mcp".to_string(), error: "Connection refused".to_string() })`

**Expected ViewCommands:**
- `ViewCommand::UpdateMcpStatus { id: mcp_id, status: McpStatus::Starting }`
- `ViewCommand::ShowMcpError { id: mcp_id, error: "Connection refused" }`
- `ViewCommand::UpdateMcpStatus { id: mcp_id, status: McpStatus::Failed }`

### Test 3: MCP Becomes Unhealthy Then Recovers

**Events to emit:**
1. `AppEvent::Mcp(McpEvent::Started { id: mcp_id, name: "exa".to_string(), tools: vec![], tool_count: 0 })`
2. `AppEvent::Mcp(McpEvent::Unhealthy { id: mcp_id, name: "exa".to_string(), error: "Health check timeout".to_string() })`
3. `AppEvent::Mcp(McpEvent::Recovered { id: mcp_id, name: "exa".to_string() })`

**Expected ViewCommands:**
- `ViewCommand::UpdateMcpStatus { id: mcp_id, status: McpStatus::Running, tool_count: 0 }`
- `ViewCommand::ShowMcpWarning { id: mcp_id, warning: "Unhealthy: Health check timeout" }`
- `ViewCommand::UpdateMcpStatus { id: mcp_id, status: McpStatus::Running }`

### Test 4: MCP Configuration Update

**Events to emit:**
1. `AppEvent::User(UserEvent::ConfigureMcp { id: mcp_id })`
2. `AppEvent::User(UserEvent::SaveMcpConfig { id: mcp_id, config: McpConfig { id: mcp_id, name: "updated-exa".to_string() } })`
3. `AppEvent::Mcp(McpEvent::ConfigSaved { id: mcp_id })`

**Expected ViewCommands:**
- `ViewCommand::ShowMcpConfigEditor { id: mcp_id }`
- `ViewCommand::UpdateMcpConfig { id: mcp_id, config: ... }`
- `ViewCommand::ShowConfigSavedConfirmation { message: "MCP configuration saved" }`

### Test 5: MCP Deletion Flow

**Events to emit:**
1. `AppEvent::User(UserEvent::DeleteMcp { id: mcp_id })`
2. `AppEvent::User(UserEvent::ConfirmDeleteMcp { id: mcp_id })`
3. `AppEvent::Mcp(McpEvent::Deleted { id: mcp_id, name: "exa".to_string() })`

**Expected ViewCommands:**
- `ViewCommand::ShowDeleteConfirmation { id: mcp_id, item_type: "MCP Server" }`
- `ViewCommand::RemoveMcpItem { id: mcp_id }`
- `ViewCommand::ShowDeletionSuccess { message: "exa has been removed" }`

### Test 6: MCP Tool Call Events

**Events to emit:**
1. `AppEvent::Mcp(McpEvent::ToolCalled { mcp_id: mcp_id, tool_name: "exa.search".to_string(), tool_call_id: "tc1".to_string() })`
2. `AppEvent::Mcp(McpEvent::ToolCompleted { mcp_id: mcp_id, tool_name: "exa.search".to_string(), tool_call_id: "tc1".to_string(), success: true, duration_ms: 150 })`

**Expected ViewCommands:**
- `ViewCommand::ShowMcpToolCallStarted { mcp_id: mcp_id, tool_name: "exa.search", tool_call_id: "tc1" }`
- `ViewCommand::ShowMcpToolCallCompleted { tool_call_id: "tc1", success: true, duration_ms: 150 }`

## Implementation Requirements

### Test File Structure

Add to existing `tests/e2e_presenter_tests.rs`:

```rust
#[cfg(test)]
mod e2e_mcp_tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_server_start_success() {
        // Setup
        let event_bus = Arc::new(EventBus::new());
        let mut presenter = SettingsPresenter::new(event_bus.clone());
        let mut receiver = presenter.subscribe_view_commands();

        let mcp_id = Uuid::new_v4();

        // Emit user toggle event
        event_bus.emit(AppEvent::User(UserEvent::ToggleMcp {
            id: mcp_id,
            enabled: true,
        }));

        // Emit MCP lifecycle events
        event_bus.emit(AppEvent::Mcp(McpEvent::Starting {
            id: mcp_id,
            name: "exa".to_string(),
        }));

        event_bus.emit(AppEvent::Mcp(McpEvent::Started {
            id: mcp_id,
            name: "exa".to_string(),
            tools: vec!["search".to_string(), "crawl".to_string()],
            tool_count: 2,
        }));

        // Verify ViewCommands
        let commands = receiver.collect_view_commands(100).await;

        assert_eq!(commands[0], ViewCommand::UpdateMcpStatus {
            id: mcp_id,
            status: McpStatus::Starting,
        });

        assert_eq!(commands[1], ViewCommand::UpdateMcpStatus {
            id: mcp_id,
            status: McpStatus::Running,
            tool_count: Some(2),
        });
    }

    #[tokio::test]
    async fn test_mcp_server_start_failure() {
        // Similar structure for start failure
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_mcp_unhealthy_then_recovers() {
        // Similar structure for unhealthy/recover
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_mcp_configuration_update() {
        // Similar structure for config update
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_mcp_deletion_flow() {
        // Similar structure for deletion
        // NO unimplemented!() placeholders
    }

    #[tokio::test]
    async fn test_mcp_tool_call_events() {
        // Similar structure for tool calls
        // NO unimplemented!() placeholders
    }
}
```

### MCP Status Enum

Tests should reference proper MCP status values:

```rust
#[derive(Debug, Clone, PartialEq)]
enum McpStatus {
    Stopped,
    Starting,
    Running,
    Failed,
    Unhealthy,
}
```

## Inputs

### Files to Read
- `src/presentation/settings_presenter.rs` - SettingsPresenter implementation (must be complete from P03)
- `src/events/types.rs` - Event enum definitions (McpEvent variants)
- `src/events/bus.rs` - EventBus API
- `src/presentation/view_command.rs` - ViewCommand enum definitions
- `tests/e2e_presenter_tests.rs` - Existing test structure from P04
- `dev-docs/COORDINATING.md` - Test requirements (no placeholders)

### State Required
- SettingsPresenter is fully wired to EventBus (P03a passed)
- ViewCommand enum has MCP-related variants
- EventBus supports test mode

## Outputs

### Files to Modify
- `tests/e2e_presenter_tests.rs` - Add 6 MCP test functions to existing file

### Evidence Files
- `project-plans/wire-presenters/plan/.completed/P05.md` - Phase completion evidence
- `evidence/PLAN-20250128-PRESENTERS/phase-05/test-output.txt` - Test run output (created in P05a)

## Verification Commands

```bash
# Build check
cargo build --all-targets

# Placeholder detection
grep -rn "unimplemented!\|todo!" tests/e2e_presenter_tests.rs
grep -rn "placeholder\|not yet implemented" tests/e2e_presenter_tests.rs

# Test compilation (don't run yet - that's P05a)
cargo test --test e2e_presenter_tests --no-run

# Count MCP tests
grep -c "test_mcp_" tests/e2e_presenter_tests.rs
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- Exit code 0 from `cargo test --test e2e_presenter_tests --no-run`
- `grep -rn "unimplemented!" tests/e2e_presenter_tests.rs` returns no matches
- `grep -rn "todo!" tests/e2e_presenter_tests.rs` returns no matches
- At least 6 MCP test functions exist
- Each test has real assertions
- Total test count (chat + MCP) >= 11

### FAIL Conditions
- Build fails with compilation errors
- Any `unimplemented!()`, `todo!()`, or placeholder strings found
- Missing MCP test scenarios
- Tests have placeholder assertions

## Related Requirements

- REQ-019.2: Event-driven architecture must be testable
- dev-docs/COORDINATING.md: ZERO TOLERANCE for placeholders in implementation phases
- dev-docs/goodtests.md: Tests must verify real behavior, not just compile
- ARCHITECTURE_IMPROVEMENTS.md: MCP presenters must be testable via EventBus
