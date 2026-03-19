//! GPUI Wiring - `ViewCommand` Routing Tests
//!
//! Tests that `MainPanel` routes all relevant `ViewCommand` variants to the
//! concrete view handlers that update observable view state.
//!
//! REQ-WIRE-002 requires that the presentation layer provides a
//! `route_view_command` function (or equivalent) that dispatches each
//! `ViewCommand` variant to the appropriate target view state.  This function
//! does not yet exist; the tests below fail to compile pre-implementation,
//! which is the expected TDD outcome for this phase.
//!
//! The test harness uses lightweight plain-Rust state structs that mirror
//! each view's observable state, coupled to a `CommandTarget` enum that
//! `route_view_command` should populate.  Once the implementation exists
//! all tests must pass.
//!
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
//! @requirement REQ-WIRE-002

use std::sync::Arc;

use uuid::Uuid;

use personal_agent::events::types::UserEvent;
use personal_agent::presentation::view_command::{
    McpRegistryResult, McpStatus, MessageRole, ModelInfo,
};
use personal_agent::presentation::ViewCommand;
use personal_agent::ui_gpui::bridge::{GpuiBridge, GpuiNotifier, ViewCommandSink};

// Import the routing function that MUST exist post-implementation.
// This import intentionally fails to compile pre-implementation:
//   personal_agent::ui_gpui::views::main_panel::route_view_command
//
// The function signature that the implementation must provide:
//
//   pub fn route_view_command(
//       cmd: ViewCommand,
//       target: &mut CommandTargets,
//   )
//
// Where CommandTargets is a struct exported from the same module.
use personal_agent::ui_gpui::views::main_panel::{route_view_command, CommandTargets};

// ============================================================
// Mock notifier for bridge/sink setup
// ============================================================

#[derive(Clone)]
struct MockNotifier {
    count: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockNotifier {
    fn new() -> Self {
        Self {
            count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

impl GpuiNotifier for MockNotifier {
    fn notify(&self) {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

// ============================================================
// Helper: create bridge + sink pair
// ============================================================

fn make_bridge_and_sink() -> (GpuiBridge, ViewCommandSink<MockNotifier>) {
    let (user_tx, _user_rx) = flume::bounded::<UserEvent>(16);
    let (view_tx, view_rx) = flume::bounded::<ViewCommand>(64);
    let notifier = MockNotifier::new();
    let bridge = GpuiBridge::new(user_tx, view_rx);
    let sink = ViewCommandSink::new(view_tx, notifier);
    (bridge, sink)
}

// ============================================================
// REQ-WIRE-002: Chat view routing
// ============================================================

/// REQ-WIRE-002: `MessageAppended` reaches `ChatView` observable state
///
/// GIVEN: a `ViewCommand` bridge + sink
/// WHEN:  `MessageAppended` is sent and drained
/// THEN:  `route_view_command` places it in CommandTargets.chat.messages
///
/// Fails pre-implementation because `route_view_command` does not exist.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_chat_view_message_appended_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let conv_id = Uuid::new_v4();

    sink.send(ViewCommand::MessageAppended {
        conversation_id: conv_id,
        role: MessageRole::Assistant,
        content: "Hello from AI".to_string(),
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.chat_messages_received, 1,
        "route_view_command must forward MessageAppended to ChatView; \
         currently MainPanel drops it in the _ => {{}} catch-all"
    );
}

/// REQ-WIRE-002: `AppendStream` and `FinalizeStream` reach `ChatView`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_chat_view_stream_commands_are_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let conv_id = Uuid::new_v4();

    sink.send(ViewCommand::AppendStream {
        conversation_id: conv_id,
        chunk: "Hello".to_string(),
    });
    sink.send(ViewCommand::AppendStream {
        conversation_id: conv_id,
        chunk: " world".to_string(),
    });
    sink.send(ViewCommand::FinalizeStream {
        conversation_id: conv_id,
        tokens: 10,
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.chat_stream_chunks_received, 2,
        "route_view_command must forward AppendStream chunks to ChatView"
    );
    assert!(
        targets.chat_stream_finalized,
        "route_view_command must forward FinalizeStream to ChatView"
    );
}

// ============================================================
// REQ-WIRE-002: History view routing
// ============================================================

/// REQ-WIRE-002: `ConversationListRefreshed` reaches `HistoryView`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_history_view_conversation_list_refreshed_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let conv_id = Uuid::new_v4();

    sink.send(ViewCommand::ConversationListRefreshed {
        conversations: vec![
            personal_agent::presentation::view_command::ConversationSummary {
                id: conv_id,
                title: "Conversation A".to_string(),
                updated_at: chrono::Utc::now(),
                message_count: 2,
            },
        ],
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.history_conversations_received, 1,
        "route_view_command must forward ConversationListRefreshed to HistoryView; \
         currently MainPanel drops it"
    );
}

/// REQ-WIRE-002: `ConversationActivated` reaches `HistoryView`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_history_view_conversation_activated_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let conv_id = Uuid::new_v4();

    sink.send(ViewCommand::ConversationActivated {
        id: conv_id,
        selection_generation: 7,
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.history_activated_id,
        Some(conv_id),
        "route_view_command must forward ConversationActivated to HistoryView; \
         currently MainPanel drops it"
    );
}

// ============================================================
// REQ-WIRE-002: Settings view routing
// ============================================================

/// REQ-WIRE-002: `ProfileCreated` reaches `SettingsView`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_settings_view_profile_created_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let profile_id = Uuid::new_v4();

    sink.send(ViewCommand::ProfileCreated {
        id: profile_id,
        name: "My Profile".to_string(),
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.settings_profile_commands, 1,
        "route_view_command must forward ProfileCreated to SettingsView; \
         currently MainPanel drops it"
    );
}

/// REQ-WIRE-002: `McpStatusChanged` reaches `SettingsView`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_settings_view_mcp_status_changed_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let mcp_id = Uuid::new_v4();

    sink.send(ViewCommand::McpStatusChanged {
        id: mcp_id,
        status: McpStatus::Running,
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.settings_mcp_status_updates, 1,
        "route_view_command must forward McpStatusChanged to SettingsView; \
         currently MainPanel drops it"
    );
}

/// REQ-WIRE-002: `McpServerStarted` and `McpServerFailed` reach `SettingsView`
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_settings_view_mcp_server_lifecycle_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let mcp_id = Uuid::new_v4();

    sink.send(ViewCommand::McpServerStarted {
        id: mcp_id,
        tool_count: 3,
    });
    sink.send(ViewCommand::McpServerFailed {
        id: mcp_id,
        error: "Timeout".to_string(),
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.settings_mcp_status_updates, 2,
        "route_view_command must forward McpServerStarted and McpServerFailed; \
         currently MainPanel drops both"
    );
}

/// REQ-WIRE-002: `McpConfigSaved` and `McpDeleted` reach settings/configure routing targets
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_mcp_config_saved_and_deleted_are_routed() {
    let (bridge, sink) = make_bridge_and_sink();
    let mcp_id = Uuid::new_v4();

    sink.send(ViewCommand::McpConfigSaved {
        id: mcp_id,
        name: Some("Fetch".to_string()),
    });
    sink.send(ViewCommand::McpDeleted { id: mcp_id });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.mcp_config_saved_count, 1,
        "route_view_command must route McpConfigSaved for MCP configure/settings state"
    );
    assert_eq!(
        targets.mcp_deleted_count, 1,
        "route_view_command must route McpDeleted for settings state"
    );
}

/// REQ-WIRE-002: `ShowNotification` reaches settings routing target
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_show_notification_is_routed_to_settings_target() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::ShowNotification {
        message: "Registry loaded".to_string(),
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.settings_notifications_count, 1,
        "route_view_command must route ShowNotification for settings UI feedback"
    );
}

/// REQ-WIRE-002: `McpRegistrySearchResults` reaches MCP Add routing target
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_mcp_registry_search_results_are_routed() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::McpRegistrySearchResults {
        results: vec![
            McpRegistryResult {
                id: "fetch".to_string(),
                name: "Fetch".to_string(),
                description: "HTTP fetch server".to_string(),
                source: "official".to_string(),
                command: "npx -y @modelcontextprotocol/server-fetch".to_string(),
                args: vec![],
                env: None,
                url: None,
            },
            McpRegistryResult {
                id: "filesystem".to_string(),
                name: "Filesystem".to_string(),
                description: "Filesystem access".to_string(),
                source: "official".to_string(),
                command: "npx -y @modelcontextprotocol/server-filesystem".to_string(),
                args: vec![],
                env: None,
                url: None,
            },
        ],
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.mcp_registry_results_count, 2,
        "route_view_command must route McpRegistrySearchResults to MCP add state"
    );
}

/// REQ-WIRE-002: `McpConfigureDraftLoaded` reaches MCP configure routing target
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_mcp_configure_draft_loaded_is_routed() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::McpConfigureDraftLoaded {
        id: "fetch".to_string(),
        name: "Fetch".to_string(),
        package: "fetch".to_string(),
        env_var_name: "FETCH_API_KEY".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-fetch".to_string(),
        ],
        env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
        url: None,
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.mcp_configure_draft_loaded_count, 1,
        "route_view_command must route McpConfigureDraftLoaded to MCP configure flow"
    );
}

/// REQ-WIRE-002: `ShowError` reaches MCP error routing target
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_show_error_is_routed_to_mcp_error_target() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::ShowError {
        title: "Selection Failed".to_string(),
        message: "MCP missing-server not found".to_string(),
        severity: personal_agent::presentation::view_command::ErrorSeverity::Warning,
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.mcp_error_commands_count, 1,
        "route_view_command must route ShowError commands for MCP views"
    );
}

/// REQ-WIRE-002: `McpConfigureDraftLoaded` does not increment `McpConfigSaved` counter
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_mcp_configure_draft_loaded_does_not_count_as_mcp_config_saved() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::McpConfigureDraftLoaded {
        id: "filesystem".to_string(),
        name: "Filesystem".to_string(),
        package: "filesystem".to_string(),
        env_var_name: String::new(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
        ],
        env: None,
        url: None,
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.mcp_config_saved_count, 0,
        "McpConfigureDraftLoaded should not be counted as an MCP config save"
    );
    assert_eq!(
        targets.mcp_configure_draft_loaded_count, 1,
        "McpConfigureDraftLoaded should only increment draft-loaded counter"
    );
}

// ============================================================
// REQ-WIRE-002: Model selector routing
// ============================================================

/// REQ-WIRE-002: `ModelSearchResults` reaches `ModelSelectorView`
///
/// NOTE: `ModelSearchResults` IS currently partially routed by `MainPanel`.
/// This test asserts the routing is maintained through `route_view_command`.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_model_selector_search_results_are_routed() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::ModelSearchResults {
        models: vec![
            ModelInfo {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-sonnet".to_string(),
                name: "Claude 3.5 Sonnet".to_string(),
                context_length: Some(200_000),
            },
            ModelInfo {
                provider_id: "openai".to_string(),
                model_id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                context_length: Some(128_000),
            },
        ],
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.model_selector_results_count, 2,
        "route_view_command must forward ModelSearchResults to ModelSelectorView"
    );
}

/// REQ-WIRE-002: `ModelSelected` reaches `ProfileEditor` routing target
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_model_selected_is_routed_to_profile_prefill_target() {
    let (bridge, sink) = make_bridge_and_sink();

    sink.send(ViewCommand::ModelSelected {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-5-sonnet".to_string(),
        provider_api_url: Some("https://api.anthropic.com/v1".to_string()),
        context_length: Some(200_000),
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.model_selector_results_count, 0,
        "ModelSelected should not be counted as search results"
    );
    assert_eq!(
        targets.profile_prefill_selected_count, 1,
        "ModelSelected must be routed as a profile prefill signal"
    );
}

/// REQ-WIRE-002: `ProfileEditorLoad` reaches `ProfileEditor` routing target
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_profile_editor_load_is_routed_to_profile_prefill_target() {
    let (bridge, sink) = make_bridge_and_sink();
    let profile_id = Uuid::new_v4();

    sink.send(ViewCommand::ProfileEditorLoad {
        id: profile_id,
        name: "Existing Profile".to_string(),
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        api_key_label: "test-key".to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        context_limit: Some(200_000),
        show_thinking: false,
        enable_thinking: false,
        thinking_budget: None,
        system_prompt: "You are helpful".to_string(),
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.model_selector_results_count, 0,
        "ProfileEditorLoad should not be counted as model search results"
    );
    assert_eq!(
        targets.profile_prefill_selected_count, 1,
        "ProfileEditorLoad must be routed as a profile prefill signal"
    );
}

// ============================================================
// REQ-WIRE-002: Mixed-command drain cycle
// ============================================================

/// REQ-WIRE-002: Mixed drain cycle routes each command to its target view
///
/// Sends commands for chat, history, settings and model-selector in a single
/// drain batch and asserts every target view receives its updates.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_mixed_drain_routes_to_all_target_views() {
    let (bridge, sink) = make_bridge_and_sink();
    let conv_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let mcp_id = Uuid::new_v4();

    sink.send(ViewCommand::MessageAppended {
        conversation_id: conv_id,
        role: MessageRole::User,
        content: "hello".to_string(),
    });
    sink.send(ViewCommand::ConversationActivated {
        id: conv_id,
        selection_generation: 11,
    });
    sink.send(ViewCommand::ProfileCreated {
        id: profile_id,
        name: "Profile A".to_string(),
    });
    sink.send(ViewCommand::McpStatusChanged {
        id: mcp_id,
        status: McpStatus::Starting,
    });
    sink.send(ViewCommand::ModelSearchResults {
        models: vec![ModelInfo {
            provider_id: "anthropic".to_string(),
            model_id: "claude-opus-4".to_string(),
            name: "Claude Opus 4".to_string(),
            context_length: None,
        }],
    });

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(
        targets.chat_messages_received, 1,
        "ChatView must receive MessageAppended in mixed drain; currently dropped"
    );
    assert_eq!(
        targets.history_activated_id,
        Some(conv_id),
        "HistoryView must receive ConversationActivated in mixed drain; currently dropped"
    );
    assert_eq!(
        targets.settings_profile_commands, 1,
        "SettingsView must receive ProfileCreated in mixed drain; currently dropped"
    );
    assert_eq!(
        targets.settings_mcp_status_updates, 1,
        "SettingsView must receive McpStatusChanged in mixed drain; currently dropped"
    );
    assert_eq!(
        targets.model_selector_results_count, 1,
        "ModelSelectorView must receive ModelSearchResults in mixed drain"
    );
}

/// REQ-WIRE-002: Ancillary `ViewCommand` variants do not mutate routing counters
///
/// Covers command variants that are currently handled directly by child views or
/// navigation/modal layers and therefore should not increment `route_view_command`
/// observable counters.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P04
/// @requirement REQ-WIRE-002
#[test]
fn test_route_view_command_ignores_ancillary_variants_without_side_effects() {
    let (bridge, sink) = make_bridge_and_sink();
    let conv_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    sink.send(ViewCommand::ConversationCreated {
        id: conv_id,
        profile_id,
    });
    sink.send(ViewCommand::HistoryUpdated { count: Some(3) });
    sink.send(ViewCommand::ProfileTestStarted { id: profile_id });
    sink.send(ViewCommand::ProfileTestCompleted {
        id: profile_id,
        success: true,
        response_time_ms: Some(42),
        error: None,
    });
    sink.send(ViewCommand::ShowThinking {
        conversation_id: conv_id,
    });
    sink.send(ViewCommand::HideThinking {
        conversation_id: conv_id,
    });
    sink.send(ViewCommand::ShowToolCall {
        conversation_id: conv_id,
        tool_name: "fetch".to_string(),
        status: "running".to_string(),
    });
    sink.send(ViewCommand::UpdateToolCall {
        conversation_id: conv_id,
        tool_name: "fetch".to_string(),
        status: "done".to_string(),
        result: Some("ok".to_string()),
        duration: Some(12),
    });
    sink.send(ViewCommand::MessageSaved {
        conversation_id: conv_id,
    });
    sink.send(ViewCommand::StreamCancelled {
        conversation_id: conv_id,
        partial_content: "partial".to_string(),
    });
    sink.send(ViewCommand::StreamError {
        conversation_id: conv_id,
        error: "boom".to_string(),
        recoverable: false,
    });
    sink.send(ViewCommand::AppendThinking {
        conversation_id: conv_id,
        content: "thought".to_string(),
    });
    sink.send(ViewCommand::ConversationRenamed {
        id: conv_id,
        new_title: "Renamed".to_string(),
    });
    sink.send(ViewCommand::ConversationCleared);
    sink.send(ViewCommand::McpToolsUpdated { tools: vec![] });
    sink.send(ViewCommand::ClearError);
    sink.send(ViewCommand::ShowModal {
        modal: personal_agent::presentation::view_command::ModalId::ConfirmDeleteConversation,
    });
    sink.send(ViewCommand::DismissModal);

    let mut targets = CommandTargets::default();
    for cmd in bridge.drain_commands() {
        route_view_command(cmd, &mut targets);
    }

    assert_eq!(targets.chat_messages_received, 0);
    assert_eq!(targets.chat_stream_chunks_received, 0);
    assert!(!targets.chat_stream_finalized);
    assert_eq!(targets.history_conversations_received, 0);
    assert_eq!(targets.history_activated_id, None);
    assert_eq!(
        targets.settings_profile_commands, 0,
        "No profile routing commands were sent in this ancillary command set"
    );
    assert_eq!(targets.settings_mcp_status_updates, 0);
    assert_eq!(targets.model_selector_results_count, 0);
    assert_eq!(targets.mcp_config_saved_count, 0);
    assert_eq!(targets.mcp_deleted_count, 0);
    assert_eq!(targets.settings_notifications_count, 0);
    assert_eq!(targets.mcp_error_commands_count, 0);
    assert_eq!(targets.mcp_registry_results_count, 0);
    assert_eq!(targets.mcp_configure_draft_loaded_count, 0);
    assert_eq!(targets.profile_prefill_selected_count, 0);
}
