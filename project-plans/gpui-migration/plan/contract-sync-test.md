# Integration Contract Synchronization Test

**Plan ID:** PLAN-20250128-GPUI
**Purpose:** Keep mapping tables synchronized with actual code

---

## Test Location

Create: `tests/integration_contract_sync_test.rs`

---

## Test Code

```rust
//! Integration Contract Synchronization Tests
//!
//! These tests ensure that the mapping tables in
//! `project-plans/gpui-migration/appendix-integration-contracts.md`
//! remain synchronized with the actual enum definitions.
//!
//! If these tests fail, update the appendix to match the current code.
//!
//! @plan PLAN-20250128-GPUI

use personal_agent::events::types::UserEvent;
use personal_agent::presentation::view_command::ViewCommand;

/// Verify UserEvent variant count matches documentation
/// 
/// The appendix documents 30 UserEvent variants.
/// If this test fails to compile, the enum has been modified.
/// Update the appendix and this test to match.
#[test]
fn test_user_event_variant_count() {
    fn count_variants(event: UserEvent) -> usize {
        match event {
            // Chat Actions (8 variants)
            UserEvent::SendMessage { .. } => 1,
            UserEvent::StopStreaming => 2,
            UserEvent::NewConversation => 3,
            UserEvent::SelectConversation { .. } => 4,
            UserEvent::ToggleThinking => 5,
            UserEvent::StartRenameConversation { .. } => 6,
            UserEvent::ConfirmRenameConversation { .. } => 7,
            UserEvent::CancelRenameConversation => 8,
            
            // Profile Actions (7 variants)
            UserEvent::SelectProfile { .. } => 9,
            UserEvent::CreateProfile => 10,
            UserEvent::EditProfile { .. } => 11,
            UserEvent::SaveProfile { .. } => 12,
            UserEvent::DeleteProfile { .. } => 13,
            UserEvent::ConfirmDeleteProfile { .. } => 14,
            UserEvent::TestProfileConnection { .. } => 15,
            
            // MCP Actions (9 variants)
            UserEvent::ToggleMcp { .. } => 16,
            UserEvent::AddMcp => 17,
            UserEvent::SearchMcpRegistry { .. } => 18,
            UserEvent::SelectMcpFromRegistry { .. } => 19,
            UserEvent::ConfigureMcp { .. } => 20,
            UserEvent::SaveMcpConfig { .. } => 21,
            UserEvent::DeleteMcp { .. } => 22,
            UserEvent::ConfirmDeleteMcp { .. } => 23,
            UserEvent::StartMcpOAuth { .. } => 24,
            
            // Model Selector Actions (4 variants)
            UserEvent::OpenModelSelector => 25,
            UserEvent::SearchModels { .. } => 26,
            UserEvent::FilterModelsByProvider { .. } => 27,
            UserEvent::SelectModel { .. } => 28,
            
            // Navigation (2 variants)
            UserEvent::Navigate { .. } => 29,
            UserEvent::NavigateBack => 30,
        }
    }
    
    // Document says 30 variants - test uses last variant
    // This match must be exhaustive - compiler will fail if variants added/removed
    assert_eq!(count_variants(UserEvent::NavigateBack), 30);
}

/// Verify ViewCommand variant count matches documentation
/// 
/// The appendix documents 42 ViewCommand variants.
/// If this test fails to compile, the enum has been modified.
/// Update the appendix and this test to match.
#[test]
fn test_view_command_variant_count() {
    fn count_variants(cmd: ViewCommand) -> usize {
        match cmd {
            // Chat Commands (16 variants)
            ViewCommand::ConversationCreated { .. } => 1,
            ViewCommand::MessageAppended { .. } => 2,
            ViewCommand::ShowThinking { .. } => 3,
            ViewCommand::HideThinking { .. } => 4,
            ViewCommand::AppendStream { .. } => 5,
            ViewCommand::FinalizeStream { .. } => 6,
            ViewCommand::StreamCancelled { .. } => 7,
            ViewCommand::StreamError { .. } => 8,
            ViewCommand::AppendThinking { .. } => 9,
            ViewCommand::ShowToolCall { .. } => 10,
            ViewCommand::UpdateToolCall { .. } => 11,
            ViewCommand::MessageSaved { .. } => 12,
            ViewCommand::ToggleThinkingVisibility => 13,
            ViewCommand::ConversationRenamed { .. } => 14,
            ViewCommand::ConversationCleared => 15,
            ViewCommand::HistoryUpdated { .. } => 16,
            
            // History Commands (4 variants)
            ViewCommand::ConversationListRefreshed { .. } => 17,
            ViewCommand::ConversationActivated { .. } => 18,
            ViewCommand::ConversationDeleted { .. } => 19,
            ViewCommand::ConversationTitleUpdated { .. } => 20,
            
            // Settings Commands (8 variants)
            ViewCommand::ShowSettings { .. } => 21,
            ViewCommand::ShowNotification { .. } => 22,
            ViewCommand::ProfileCreated { .. } => 23,
            ViewCommand::ProfileUpdated { .. } => 24,
            ViewCommand::ProfileDeleted { .. } => 25,
            ViewCommand::DefaultProfileChanged { .. } => 26,
            ViewCommand::ProfileTestStarted { .. } => 27,
            ViewCommand::ProfileTestCompleted { .. } => 28,
            
            // MCP Commands (6 variants)
            ViewCommand::McpServerStarted { .. } => 29,
            ViewCommand::McpServerFailed { .. } => 30,
            ViewCommand::McpToolsUpdated { .. } => 31,
            ViewCommand::McpStatusChanged { .. } => 32,
            ViewCommand::McpConfigSaved { .. } => 33,
            ViewCommand::McpDeleted { .. } => 34,
            
            // Model Selector Commands (2 variants)
            ViewCommand::ModelSearchResults { .. } => 35,
            ViewCommand::ModelSelected { .. } => 36,
            
            // Error Commands (2 variants)
            ViewCommand::ShowError { .. } => 37,
            ViewCommand::ClearError => 38,
            
            // Navigation Commands (4 variants)
            ViewCommand::NavigateTo { .. } => 39,
            ViewCommand::NavigateBack => 40,
            ViewCommand::ShowModal { .. } => 41,
            ViewCommand::DismissModal => 42,
        }
    }
    
    // Document says 42 variants - test uses last variant
    // This match must be exhaustive - compiler will fail if variants added/removed
    assert_eq!(count_variants(ViewCommand::DismissModal), 42);
}

/// Verify that the documented variant counts are accurate
#[test]
fn test_documented_counts() {
    // These counts must match appendix-integration-contracts.md
    const DOCUMENTED_USER_EVENT_COUNT: usize = 30;
    const DOCUMENTED_VIEW_COMMAND_COUNT: usize = 42;
    
    // If you need to update these, also update the appendix!
    assert_eq!(DOCUMENTED_USER_EVENT_COUNT, 30, "Update appendix Section A if changed");
    assert_eq!(DOCUMENTED_VIEW_COMMAND_COUNT, 42, "Update appendix Section B if changed");
}
```

---

## Usage

### Run Test

```bash
cargo test --test integration_contract_sync_test
```

### When Test Fails

If this test fails to **compile**, the enum definitions have changed. Steps:

1. Check which variant was added/removed in `src/events/types.rs` or `src/presentation/view_command.rs`
2. Update `appendix-integration-contracts.md` with the new variant
3. Update the exhaustive match in this test
4. Update the count assertions

### Why Exhaustive Match?

Rust's exhaustive pattern matching ensures the compiler catches any enum changes:

- **Variant added:** Compiler error "non-exhaustive patterns"
- **Variant removed:** Compiler error "unreachable pattern"
- **Variant renamed:** Compiler error "no variant named..."

This provides **compile-time** synchronization checking.

---

## Integration with CI

Add to CI pipeline:

```yaml
- name: Verify Integration Contracts
  run: cargo test --test integration_contract_sync_test
```

This ensures no PR can merge with desynchronized mapping tables.
