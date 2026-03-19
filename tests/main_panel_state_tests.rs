use chrono::Utc;
use personal_agent::presentation::view_command::{
    ApiKeyInfo, ConversationMessagePayload, ConversationSummary, ErrorSeverity, McpRegistryResult,
    McpStatus, MessageRole, ModelInfo, ProfileSummary, ViewCommand, ViewId,
};
use personal_agent::ui_gpui::navigation::NavigationState;
use personal_agent::ui_gpui::views::main_panel::route_view_command;
use uuid::Uuid;

fn profile_summary(id: Uuid, name: &str) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: "openai".to_string(),
        model_id: "gpt-4o".to_string(),
        is_default: false,
    }
}

#[test]
fn command_targets_default_starts_empty() {
    let targets = personal_agent::ui_gpui::views::main_panel::CommandTargets::default();

    assert_eq!(targets.chat_messages_received, 0);
    assert_eq!(targets.chat_stream_chunks_received, 0);
    assert!(!targets.chat_stream_finalized);
    assert_eq!(targets.history_conversations_received, 0);
    assert_eq!(targets.history_activated_id, None);
    assert_eq!(targets.settings_profile_commands, 0);
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

#[test]
#[allow(clippy::too_many_lines)]
fn route_view_command_counts_chat_history_settings_and_model_variants() {
    let conversation_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let mcp_id = Uuid::new_v4();
    let mut targets = personal_agent::ui_gpui::views::main_panel::CommandTargets::default();

    let commands = vec![
        ViewCommand::ConversationMessagesLoaded {
            conversation_id,
            selection_generation: 3,
            messages: vec![
                ConversationMessagePayload {
                    role: MessageRole::User,
                    content: "hello".to_string(),
                    thinking_content: None,
                    timestamp: Some(1),
                },
                ConversationMessagePayload {
                    role: MessageRole::Assistant,
                    content: "world".to_string(),
                    thinking_content: Some("thought".to_string()),
                    timestamp: Some(2),
                },
            ],
        },
        ViewCommand::MessageAppended {
            conversation_id,
            role: MessageRole::Assistant,
            content: "another".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id,
            chunk: "stream-1".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id,
            chunk: "stream-2".to_string(),
        },
        ViewCommand::FinalizeStream {
            conversation_id,
            tokens: 42,
        },
        ViewCommand::ConversationListRefreshed {
            conversations: vec![
                ConversationSummary {
                    id: Uuid::new_v4(),
                    title: "One".to_string(),
                    updated_at: Utc::now(),
                    message_count: 1,
                },
                ConversationSummary {
                    id: Uuid::new_v4(),
                    title: "Two".to_string(),
                    updated_at: Utc::now(),
                    message_count: 2,
                },
            ],
        },
        ViewCommand::ConversationActivated {
            id: conversation_id,
            selection_generation: 9,
        },
        ViewCommand::ShowSettings {
            profiles: vec![profile_summary(profile_id, "Primary")],
            selected_profile_id: Some(profile_id),
        },
        ViewCommand::ProfileCreated {
            id: profile_id,
            name: "Primary".to_string(),
        },
        ViewCommand::ProfileUpdated {
            id: profile_id,
            name: "Primary v2".to_string(),
        },
        ViewCommand::ProfileDeleted { id: profile_id },
        ViewCommand::DefaultProfileChanged {
            profile_id: Some(profile_id),
        },
        ViewCommand::ChatProfilesUpdated {
            profiles: vec![profile_summary(profile_id, "Primary")],
            selected_profile_id: Some(profile_id),
        },
        ViewCommand::McpStatusChanged {
            id: mcp_id,
            status: McpStatus::Running,
        },
        ViewCommand::McpServerStarted {
            id: mcp_id,
            tool_count: 2,
        },
        ViewCommand::McpServerFailed {
            id: mcp_id,
            error: "boom".to_string(),
        },
        ViewCommand::McpConfigSaved {
            id: mcp_id,
            name: Some("Filesystem".to_string()),
        },
        ViewCommand::McpDeleted { id: mcp_id },
        ViewCommand::ShowNotification {
            message: "saved".to_string(),
        },
        ViewCommand::ShowError {
            title: "Warning".to_string(),
            message: "be careful".to_string(),
            severity: ErrorSeverity::Warning,
        },
        ViewCommand::McpRegistrySearchResults {
            results: vec![
                McpRegistryResult {
                    id: "filesystem".to_string(),
                    name: "Filesystem".to_string(),
                    description: "Browse files".to_string(),
                    source: "official".to_string(),
                    command: "npx".to_string(),
                    args: vec![],
                    env: None,
                    url: None,
                },
                McpRegistryResult {
                    id: "fetch".to_string(),
                    name: "Fetch".to_string(),
                    description: "HTTP".to_string(),
                    source: "official".to_string(),
                    command: "npx".to_string(),
                    args: vec![],
                    env: Some(vec![("API_KEY".to_string(), "value".to_string())]),
                    url: None,
                },
            ],
        },
        ViewCommand::McpConfigureDraftLoaded {
            id: "filesystem".to_string(),
            name: "Filesystem".to_string(),
            package: "filesystem".to_string(),
            env_var_name: "API_KEY".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string()],
            env: Some(vec![("API_KEY".to_string(), "value".to_string())]),
            url: None,
        },
        ViewCommand::ModelSearchResults {
            models: vec![
                ModelInfo {
                    provider_id: "anthropic".to_string(),
                    model_id: "claude-3-7-sonnet".to_string(),
                    name: "Claude 3.7 Sonnet".to_string(),
                    context_length: Some(200_000),
                },
                ModelInfo {
                    provider_id: "openai".to_string(),
                    model_id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_length: Some(128_000),
                },
            ],
        },
        ViewCommand::ModelSelected {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-7-sonnet".to_string(),
            provider_api_url: Some("https://api.anthropic.com/v1".to_string()),
            context_length: Some(200_000),
        },
        ViewCommand::ProfileEditorLoad {
            id: profile_id,
            name: "Primary".to_string(),
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-7-sonnet".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            api_key_label: "anthropic-key".to_string(),
            temperature: 0.2,
            max_tokens: 4096,
            context_limit: Some(200_000),
            show_thinking: true,
            enable_thinking: true,
            thinking_budget: Some(512),
            system_prompt: "Be helpful".to_string(),
        },
    ];

    for command in commands {
        route_view_command(command, &mut targets);
    }

    assert_eq!(targets.chat_messages_received, 3);
    assert_eq!(targets.chat_stream_chunks_received, 2);
    assert!(targets.chat_stream_finalized);
    assert_eq!(targets.history_conversations_received, 2);
    assert_eq!(targets.history_activated_id, Some(conversation_id));
    assert_eq!(targets.settings_profile_commands, 6);
    assert_eq!(targets.settings_mcp_status_updates, 3);
    assert_eq!(targets.mcp_config_saved_count, 1);
    assert_eq!(targets.mcp_deleted_count, 1);
    assert_eq!(targets.settings_notifications_count, 1);
    assert_eq!(targets.mcp_error_commands_count, 1);
    assert_eq!(targets.mcp_registry_results_count, 2);
    assert_eq!(targets.mcp_configure_draft_loaded_count, 1);
    assert_eq!(targets.model_selector_results_count, 2);
    assert_eq!(targets.profile_prefill_selected_count, 2);
}

#[test]
#[allow(clippy::too_many_lines)]
fn route_view_command_ignores_non_routed_variants() {
    let conversation_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let mut targets = personal_agent::ui_gpui::views::main_panel::CommandTargets::default();

    let commands = vec![
        ViewCommand::ConversationCreated {
            id: conversation_id,
            profile_id,
        },
        ViewCommand::ConversationLoadFailed {
            conversation_id,
            selection_generation: 1,
            message: "failed".to_string(),
        },
        ViewCommand::ShowThinking { conversation_id },
        ViewCommand::HideThinking { conversation_id },
        ViewCommand::StreamCancelled {
            conversation_id,
            partial_content: "partial".to_string(),
        },
        ViewCommand::StreamError {
            conversation_id,
            error: "boom".to_string(),
            recoverable: true,
        },
        ViewCommand::AppendThinking {
            conversation_id,
            content: "thought".to_string(),
        },
        ViewCommand::ShowToolCall {
            conversation_id,
            tool_name: "fetch".to_string(),
            status: "running".to_string(),
        },
        ViewCommand::UpdateToolCall {
            conversation_id,
            tool_name: "fetch".to_string(),
            status: "done".to_string(),
            result: Some("ok".to_string()),
            duration: Some(99),
        },
        ViewCommand::MessageSaved { conversation_id },
        ViewCommand::ToggleThinkingVisibility,
        ViewCommand::ConversationRenamed {
            id: conversation_id,
            new_title: "Renamed".to_string(),
        },
        ViewCommand::ConversationCleared,
        ViewCommand::HistoryUpdated { count: Some(3) },
        ViewCommand::ConversationDeleted {
            id: conversation_id,
        },
        ViewCommand::ConversationTitleUpdated {
            id: conversation_id,
            title: "Updated".to_string(),
        },
        ViewCommand::ApiKeysListed {
            keys: vec![ApiKeyInfo {
                label: "anthropic".to_string(),
                masked_value: "sk-a••••".to_string(),
                used_by: vec!["Primary".to_string()],
            }],
        },
        ViewCommand::ApiKeyStored {
            label: "anthropic".to_string(),
        },
        ViewCommand::ApiKeyDeleted {
            label: "anthropic".to_string(),
        },
        ViewCommand::ProfileTestStarted { id: profile_id },
        ViewCommand::ProfileTestCompleted {
            id: profile_id,
            success: true,
            response_time_ms: Some(12),
            error: None,
        },
        ViewCommand::McpToolsUpdated { tools: vec![] },
        ViewCommand::ClearError,
        ViewCommand::NavigateTo {
            view: ViewId::Settings,
        },
        ViewCommand::NavigateBack,
        ViewCommand::ShowModal {
            modal: personal_agent::presentation::view_command::ModalId::ConfirmDeleteConversation,
        },
        ViewCommand::DismissModal,
    ];

    for command in commands {
        route_view_command(command, &mut targets);
    }

    assert_eq!(targets.chat_messages_received, 0);
    assert_eq!(targets.chat_stream_chunks_received, 0);
    assert!(!targets.chat_stream_finalized);
    assert_eq!(targets.history_conversations_received, 0);
    assert_eq!(targets.history_activated_id, None);
    assert_eq!(targets.settings_profile_commands, 0);
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

#[test]
fn navigation_state_transitions_match_main_panel_navigation_expectations() {
    let mut navigation = NavigationState::new();

    assert_eq!(navigation.current(), ViewId::Chat);
    assert_eq!(navigation.stack_depth(), 1);
    assert!(!navigation.can_go_back());

    navigation.navigate(ViewId::Settings);
    assert_eq!(navigation.current(), ViewId::Settings);
    assert_eq!(navigation.stack_depth(), 2);
    assert!(navigation.can_go_back());

    navigation.navigate(ViewId::ProfileEditor);
    navigation.navigate(ViewId::ModelSelector);
    assert_eq!(navigation.current(), ViewId::ModelSelector);
    assert_eq!(navigation.stack_depth(), 4);

    navigation.navigate(ViewId::ProfileEditor);
    assert_eq!(navigation.current(), ViewId::ProfileEditor);
    assert_eq!(navigation.stack_depth(), 3);

    navigation.navigate(ViewId::ProfileEditor);
    assert_eq!(navigation.stack_depth(), 3);

    assert!(navigation.navigate_back());
    assert_eq!(navigation.current(), ViewId::Settings);
    assert_eq!(navigation.stack_depth(), 2);

    assert!(navigation.navigate_back());
    assert_eq!(navigation.current(), ViewId::Chat);
    assert_eq!(navigation.stack_depth(), 1);
    assert!(!navigation.navigate_back());
    assert_eq!(navigation.current(), ViewId::Chat);
}
