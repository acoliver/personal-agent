#![allow(clippy::future_not_send)]

use super::*;
use chrono::Utc;
use flume;
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::models::ConversationExportFormat;
use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ThemeSummary,
    ViewCommand,
};
use crate::ui_gpui::app_store::{
    StartupInputs, StartupMode, StartupSelectedConversation, StartupTranscriptResult,
};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::GpuiAppStore;

pub(super) fn assert_route_count(
    cmd: ViewCommand,
    expected: usize,
    read_count: impl Fn(&CommandTargets) -> usize,
) {
    let mut targets = CommandTargets::default();
    route_view_command(cmd, &mut targets);
    assert_eq!(read_count(&targets), expected);
}

pub(super) fn assert_mcp_and_chat_error_counts(
    cmd: ViewCommand,
    expected_mcp: usize,
    expected_chat: usize,
) {
    let mut targets = CommandTargets::default();
    route_view_command(cmd, &mut targets);
    assert_eq!(targets.mcp_error_commands_count, expected_mcp);
    assert_eq!(targets.chat_error_commands, expected_chat);
}

pub(super) fn assert_settings_and_chat_notification_counts(
    cmd: ViewCommand,
    expected_settings: usize,
    expected_chat: usize,
) {
    let mut targets = CommandTargets::default();
    route_view_command(cmd, &mut targets);
    assert_eq!(targets.settings_notifications_count, expected_settings);
    assert_eq!(targets.chat_notification_commands, expected_chat);
}

pub(super) fn assert_profile_forwarding_via_store(
    store: &Arc<GpuiAppStore>,
    panel: &mut MainPanel,
    profile_id: Uuid,
    cx: &mut gpui::Context<MainPanel>,
) {
    store.reduce_batch(vec![ViewCommand::ChatProfilesUpdated {
        profiles: vec![profile_summary(
            profile_id,
            "Workspace Default",
            "openai",
            "gpt-4.1",
            true,
        )],
        selected_profile_id: Some(profile_id),
    }]);
    let snapshot = store.current_snapshot();
    panel.apply_store_snapshot(snapshot, cx);

    let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
    chat_view.read_with(cx, |view, _| {
        assert_eq!(view.state.profiles.len(), 1);
        assert_eq!(view.state.selected_profile_id, Some(profile_id));
        assert_eq!(view.state.current_model, "Workspace Default");
    });
}

pub(super) fn assert_mcp_routing_targets(saved_mcp_id: Uuid) {
    assert_route_count(
        ViewCommand::McpConfigureDraftLoaded {
            id: saved_mcp_id.to_string(),
            name: "Workspace MCP".to_string(),
            package: "@example/workspace-mcp".to_string(),
            package_type: crate::mcp::McpPackageType::Npm,
            runtime_hint: Some("npx".to_string()),
            env_var_name: "WORKSPACE_TOKEN".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@example/workspace-mcp".to_string()],
            env: Some(vec![("WORKSPACE_TOKEN".to_string(), String::new())]),
            url: None,
        },
        1,
        |targets| targets.mcp_configure_draft_loaded_count,
    );

    assert_mcp_and_chat_error_counts(
        ViewCommand::ShowError {
            title: "MCP auth failed".to_string(),
            message: "token expired".to_string(),
            severity: crate::presentation::view_command::ErrorSeverity::Error,
        },
        1,
        0,
    );

    assert_mcp_and_chat_error_counts(
        ViewCommand::ShowError {
            title: "Save Conversation".to_string(),
            message: "disk unavailable".to_string(),
            severity: crate::presentation::view_command::ErrorSeverity::Error,
        },
        1,
        1,
    );

    assert_settings_and_chat_notification_counts(
        ViewCommand::ShowNotification {
            message: "connected-user".to_string(),
        },
        1,
        0,
    );

    assert_settings_and_chat_notification_counts(
        ViewCommand::ShowNotification {
            message: "Conversation saved as /tmp/chat.md (MD)".to_string(),
        },
        1,
        1,
    );

    assert_route_count(
        ViewCommand::ShowConversationExportFormat {
            format: ConversationExportFormat::Md,
        },
        1,
        |targets| targets.chat_export_format_commands,
    );

    assert_route_count(
        ViewCommand::ExportCompleted {
            path: "/tmp/chat.md".to_string(),
            format_label: "Markdown".to_string(),
        },
        1,
        |targets| targets.chat_export_completed_commands,
    );

    assert_route_count(
        ViewCommand::McpConfigSaved {
            id: saved_mcp_id,
            name: Some("Workspace MCP Saved".to_string()),
        },
        1,
        |targets| targets.mcp_config_saved_count,
    );

    assert_route_count(
        ViewCommand::ErrorLogExportCompleted {
            path: "/tmp/error-log.txt".to_string(),
        },
        1,
        |targets| targets.error_log_export_completed_commands,
    );
}

pub(super) fn assert_settings_theme_routing_targets() {
    assert_route_count(
        ViewCommand::ShowSettingsTheme {
            options: vec![theme_summary("Midnight Nebula", "default")],
            selected_slug: "default".to_string(),
        },
        1,
        |targets| targets.settings_theme_commands,
    );
}

pub(super) fn conversation_summary(
    id: Uuid,
    title: &str,
    message_count: usize,
) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
        preview: None,
    }
}

pub(super) fn profile_summary(
    id: Uuid,
    name: &str,
    provider: &str,
    model: &str,
    is_default: bool,
) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: provider.to_string(),
        model_id: model.to_string(),
        is_default,
    }
}

pub(super) fn theme_summary(name: &str, slug: &str) -> ThemeSummary {
    ThemeSummary {
        name: name.to_string(),
        slug: slug.to_string(),
    }
}

pub(super) fn transcript_message(role: MessageRole, content: &str) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role,
        content: content.to_string(),
        thinking_content: None,
        timestamp: None,
    }
}

pub(super) fn build_app_state() -> (
    MainPanelAppState,
    flume::Receiver<UserEvent>,
    Uuid,
    Uuid,
    Uuid,
) {
    let first_conversation_id = Uuid::new_v4();
    let second_conversation_id = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();
    let startup_transcript = vec![
        transcript_message(MessageRole::User, "startup user"),
        transcript_message(MessageRole::Assistant, "startup assistant"),
    ];

    let store = Arc::new(GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: vec![
            profile_summary(selected_profile_id, "Default", "openai", "gpt-4o", true),
            profile_summary(Uuid::new_v4(), "Secondary", "anthropic", "claude", false),
        ],
        selected_profile_id: Some(selected_profile_id),
        conversations: vec![
            conversation_summary(
                first_conversation_id,
                "Startup Conversation",
                startup_transcript.len(),
            ),
            conversation_summary(second_conversation_id, "Later Conversation", 0),
        ],
        selected_conversation: Some(StartupSelectedConversation {
            conversation_id: first_conversation_id,
            mode: StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Success(startup_transcript),
            },
        }),
    }));

    let (user_tx, user_rx) = flume::bounded(32);
    let (_view_tx, view_rx) = flume::bounded(32);
    let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));

    (
        MainPanelAppState {
            gpui_bridge: bridge,
            popup_window: None,
            app_store: store,
            app_mode: crate::presentation::view_command::AppMode::Popup,
        },
        user_rx,
        first_conversation_id,
        second_conversation_id,
        selected_profile_id,
    )
}
