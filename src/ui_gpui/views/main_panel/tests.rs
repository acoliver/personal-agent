#![allow(clippy::future_not_send)]

use super::*;
use chrono::Utc;
use flume;
use gpui::{AppContext, TestAppContext};
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::models::ConversationExportFormat;
use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ThemeSummary,
    ViewCommand,
};
use crate::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, StartupInputs, StartupMode,
    StartupSelectedConversation, StartupTranscriptResult,
};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::GpuiAppStore;

fn assert_route_count(
    cmd: ViewCommand,
    expected: usize,
    read_count: impl Fn(&CommandTargets) -> usize,
) {
    let mut targets = CommandTargets::default();
    route_view_command(cmd, &mut targets);
    assert_eq!(read_count(&targets), expected);
}

fn assert_mcp_and_chat_error_counts(cmd: ViewCommand, expected_mcp: usize, expected_chat: usize) {
    let mut targets = CommandTargets::default();
    route_view_command(cmd, &mut targets);
    assert_eq!(targets.mcp_error_commands_count, expected_mcp);
    assert_eq!(targets.chat_error_commands, expected_chat);
}

fn assert_settings_and_chat_notification_counts(
    cmd: ViewCommand,
    expected_settings: usize,
    expected_chat: usize,
) {
    let mut targets = CommandTargets::default();
    route_view_command(cmd, &mut targets);
    assert_eq!(targets.settings_notifications_count, expected_settings);
    assert_eq!(targets.chat_notification_commands, expected_chat);
}

fn assert_profile_forwarding_via_store(
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

fn assert_mcp_routing_targets(saved_mcp_id: Uuid) {
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

fn assert_settings_theme_routing_targets() {
    assert_route_count(
        ViewCommand::ShowSettingsTheme {
            options: vec![theme_summary("Midnight Nebula", "default")],
            selected_slug: "default".to_string(),
        },
        1,
        |targets| targets.settings_theme_commands,
    );
}

fn conversation_summary(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
        preview: None,
    }
}
fn profile_summary(
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
fn theme_summary(name: &str, slug: &str) -> ThemeSummary {
    ThemeSummary {
        name: name.to_string(),
        slug: slug.to_string(),
    }
}
fn transcript_message(role: MessageRole, content: &str) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role,
        content: content.to_string(),
        thinking_content: None,
        timestamp: None,
    }
}

fn build_app_state() -> (
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

#[gpui::test]
async fn init_and_startup_state_seed_child_views_from_store(cx: &mut TestAppContext) {
    let (app_state, _user_rx, first_conversation_id, _second_conversation_id, _selected_profile_id) =
        build_app_state();
    cx.set_global(app_state);

    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        assert!(panel.is_initialized());
        assert_eq!(panel.store_snapshot_revision, 1);
        assert!(panel.store_subscription_task.is_some());

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        let history_view = panel
            .history_view
            .as_ref()
            .expect("history view initialized");

        chat_view.read_with(cx, |view, _| {
            assert_eq!(
                view.state.active_conversation_id,
                Some(first_conversation_id)
            );
            assert_eq!(view.state.messages.len(), 2);
            assert_eq!(view.state.conversation_title, "Startup Conversation");
            assert_eq!(view.state.selected_profile_id, None);
            assert!(view.state.profiles.is_empty());
            assert_eq!(view.state.current_model, "No profile selected");
        });

        history_view.read_with(cx, |view, _| {
            assert_eq!(view.conversations().len(), 2);
            assert_eq!(view.conversations()[0].id, first_conversation_id);
            assert_eq!(view.conversations()[0].title, "Startup Conversation");
        });
    });
}

#[gpui::test]
async fn start_runtime_requires_popup_window_before_emitting_refreshes(cx: &mut TestAppContext) {
    let (app_state, user_rx, _first_conversation_id, _second_conversation_id, _selected_profile_id) =
        build_app_state();
    cx.set_global(app_state);

    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);
        panel.start_runtime(cx);
        assert!(!panel.runtime_started);
        assert!(panel.bridge_poll_task.is_none());
        assert!(panel.test_conversation_switch_task.is_none());
    });

    let mut pre_popup_events = Vec::new();
    while let Ok(event) = user_rx.try_recv() {
        pre_popup_events.push(event);
    }
    assert_eq!(
        pre_popup_events,
        vec![UserEvent::RefreshApiKeys, UserEvent::RefreshApiKeys],
        "runtime should only emit RefreshApiKeys events (no snapshot refreshes) before popup window exists"
    );
}

#[gpui::test]
async fn ensure_store_subscription_only_subscribes_once_and_applies_published_updates(
    cx: &mut TestAppContext,
) {
    let (app_state, _user_rx, first_conversation_id, second_conversation_id, selected_profile_id) =
        build_app_state();
    let store = Arc::clone(&app_state.app_store);
    cx.set_global(app_state);

    let panel = cx.new(MainPanel::new);
    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);
        assert_eq!(store.subscriber_count(), 1);

        panel.ensure_store_subscription(cx);
        assert_eq!(
            store.subscriber_count(),
            1,
            "re-subscribing should be a no-op once task exists"
        );
    });

    let updated_messages = vec![
        transcript_message(MessageRole::User, "follow-up user"),
        transcript_message(MessageRole::Assistant, "follow-up assistant"),
        transcript_message(MessageRole::Assistant, "final assistant"),
    ];
    let generation = match store.begin_selection(
        second_conversation_id,
        BeginSelectionMode::PublishImmediately,
    ) {
        BeginSelectionResult::NoOpSameSelection => 1,
        BeginSelectionResult::BeganSelection { generation } => generation,
    };
    let changed = store.reduce_batch(vec![
        ViewCommand::ConversationListRefreshed {
            conversations: vec![
                conversation_summary(first_conversation_id, "Startup Conversation", 2),
                conversation_summary(
                    second_conversation_id,
                    "Updated Conversation",
                    updated_messages.len(),
                ),
            ],
        },
        ViewCommand::ConversationMessagesLoaded {
            conversation_id: second_conversation_id,
            selection_generation: generation,
            messages: updated_messages,
        },
        ViewCommand::ChatProfilesUpdated {
            profiles: vec![profile_summary(
                selected_profile_id,
                "Updated Default",
                "anthropic",
                "claude-3-7-sonnet",
                true,
            )],
            selected_profile_id: Some(selected_profile_id),
        },
    ]);
    assert!(
        changed,
        "the store publication should report a changed snapshot"
    );

    cx.run_until_parked();

    panel.read_with(cx, |panel, cx| {
        assert_eq!(panel.store_snapshot_revision, 3);

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert_eq!(
                view.state.active_conversation_id,
                Some(second_conversation_id)
            );
            assert_eq!(view.state.conversation_title, "Later Conversation");
            assert_eq!(view.state.messages.len(), 3);
            assert_eq!(view.state.selected_profile_id, Some(selected_profile_id));
            assert_eq!(view.state.current_model, "Updated Default");
            assert_eq!(view.state.profiles.len(), 1);
        });

        let history_view = panel
            .history_view
            .as_ref()
            .expect("history view initialized");
        history_view.read_with(cx, |view, _| {
            assert_eq!(view.conversations().len(), 2);
            assert_eq!(view.conversations()[1].id, second_conversation_id);
            assert_eq!(view.conversations()[1].title, "Updated Conversation");
            assert!(view.conversations()[1].is_selected);
        });
    });
}

fn remote_model(
    provider_id: &str,
    model_id: &str,
    context_length: Option<u32>,
) -> crate::presentation::view_command::ModelInfo {
    crate::presentation::view_command::ModelInfo {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        name: model_id.to_string(),
        context_length,
    }
}

#[allow(clippy::too_many_arguments)]
fn registry_result(
    id: &str,
    name: &str,
    description: &str,
    source: &str,
    command: &str,
    args: Vec<&str>,
    env: Option<Vec<(&str, &str)>>,
    package_type: Option<crate::mcp::McpPackageType>,
    runtime_hint: Option<&str>,
    url: Option<&str>,
) -> crate::presentation::view_command::McpRegistryResult {
    crate::presentation::view_command::McpRegistryResult {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        source: source.to_string(),
        command: command.to_string(),
        args: args.into_iter().map(str::to_string).collect(),
        env: env.map(|pairs| {
            pairs
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        }),
        package_type,
        runtime_hint: runtime_hint.map(str::to_string),
        url: url.map(str::to_string),
    }
}

#[gpui::test]
async fn handle_command_navigates_and_forwards_model_results_to_real_selector(
    cx: &mut TestAppContext,
) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);
        assert_eq!(
            panel.current_view(),
            crate::presentation::view_command::ViewId::Chat
        );

        panel.handle_command(
            ViewCommand::NavigateTo {
                view: crate::presentation::view_command::ViewId::Settings,
            },
            cx,
        );
        assert_eq!(
            panel.current_view(),
            crate::presentation::view_command::ViewId::Settings
        );

        panel.handle_command(ViewCommand::NavigateBack, cx);
        assert_eq!(
            panel.current_view(),
            crate::presentation::view_command::ViewId::Chat
        );

        panel.handle_command(
            ViewCommand::ModelSearchResults {
                models: vec![
                    remote_model("anthropic", "claude-3-7-sonnet", Some(200_000)),
                    remote_model("openai", "gpt-4.1", Some(128_000)),
                ],
            },
            cx,
        );

        let model_selector = panel
            .model_selector_view
            .as_ref()
            .expect("model selector initialized");
        model_selector.read_with(cx, |view, _| {
            let state = view.get_state();
            assert_eq!(state.models.len(), 2);
            assert_eq!(state.providers.len(), 2);
            assert_eq!(state.models[0].id, "claude-3-7-sonnet");
            assert_eq!(state.models[1].provider_id, "openai");
        });

        panel.handle_command(
            ViewCommand::ModelSelected {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-7-sonnet".to_string(),
                provider_api_url: Some("https://api.anthropic.com/v1".to_string()),
                context_length: Some(200_000),
            },
            cx,
        );
        assert_eq!(
            panel.current_view(),
            crate::presentation::view_command::ViewId::ProfileEditor
        );
    });
}

#[gpui::test]
async fn handle_command_forwards_registry_results_and_errors_to_real_mcp_add_view(
    cx: &mut TestAppContext,
) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);
        panel.handle_command(
            ViewCommand::McpRegistrySearchResults {
                results: vec![
                    registry_result(
                        "exa",
                        "Exa Remote",
                        "Remote MCP",
                        "official",
                        "",
                        vec![],
                        None,
                        Some(crate::mcp::McpPackageType::Http),
                        None,
                        Some("https://exa.example/mcp"),
                    ),
                    registry_result(
                        "fetch",
                        "Fetch",
                        "HTTP fetch server",
                        "smithery",
                        "npx",
                        vec!["-y", "@modelcontextprotocol/server-fetch"],
                        Some(vec![("FETCH_API_KEY", "")]),
                        Some(crate::mcp::McpPackageType::Npm),
                        Some("npx"),
                        None,
                    ),
                ],
            },
            cx,
        );

        let mcp_add = panel.mcp_add_view.as_ref().expect("mcp add initialized");
        mcp_add.read_with(cx, |view, _| {
            let state = view.get_state();
            assert_eq!(state.results.len(), 2);
            assert_eq!(
                state.search_state,
                crate::ui_gpui::views::mcp_add_view::SearchState::Results
            );
            assert_eq!(
                state.results[0].url.as_deref(),
                Some("https://exa.example/mcp")
            );
            assert_eq!(
                state.results[0].registry,
                crate::ui_gpui::views::mcp_add_view::McpRegistry::Official
            );
            assert_eq!(
                state.results[1].registry,
                crate::ui_gpui::views::mcp_add_view::McpRegistry::Smithery
            );
            assert_eq!(state.results[1].source, "smithery");
            assert_eq!(
                state.results[1].env,
                Some(vec![("FETCH_API_KEY".to_string(), String::new())])
            );
        });

        panel.handle_command(
            ViewCommand::ShowError {
                title: "Registry Failed".to_string(),
                message: "registry unavailable".to_string(),
                severity: crate::presentation::view_command::ErrorSeverity::Warning,
            },
            cx,
        );

        let mcp_add = panel.mcp_add_view.as_ref().expect("mcp add initialized");
        mcp_add.read_with(cx, |view, _| {
            assert_eq!(
                view.get_state().search_state,
                crate::ui_gpui::views::mcp_add_view::SearchState::Error(
                    "registry unavailable".to_string()
                )
            );
        });
    });
}

#[gpui::test]
async fn handle_command_routes_mcp_commands_to_expected_targets(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, selected_profile_id) = build_app_state();
    let store = Arc::clone(&app_state.app_store);
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);
    let saved_mcp_id = Uuid::new_v4();

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);
        assert_profile_forwarding_via_store(&store, panel, selected_profile_id, cx);
        assert_mcp_routing_targets(saved_mcp_id);
        assert_settings_theme_routing_targets();
    });
}

#[gpui::test]
async fn handle_command_forwards_settings_theme_to_settings_view(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ShowSettingsTheme {
                options: vec![
                    theme_summary("Midnight Nebula", "default"),
                    theme_summary("Green Screen", "green-screen"),
                ],
                selected_slug: "green-screen".to_string(),
            },
            cx,
        );

        let settings_view = panel
            .settings_view
            .as_ref()
            .expect("settings view initialized");
        settings_view.read_with(cx, |view, _| {
            let state = view.get_state();
            assert_eq!(state.available_themes.len(), 2);
            assert_eq!(state.available_themes[0].slug, "default");
            assert_eq!(state.available_themes[1].slug, "green-screen");
            assert_eq!(state.selected_theme_slug, "green-screen");
        });
    });
}

#[gpui::test]
async fn handle_command_forwards_export_format_and_export_feedback_to_chat_view(
    cx: &mut TestAppContext,
) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ShowConversationExportFormat {
                format: ConversationExportFormat::Json,
            },
            cx,
        );
        panel.handle_command(
            ViewCommand::ExportCompleted {
                path: "/tmp/chat.md".to_string(),
                format_label: "Markdown".to_string(),
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert_eq!(
                view.state.conversation_export_format,
                ConversationExportFormat::Json
            );
            assert_eq!(
                view.state.export_feedback_message.as_deref(),
                Some("Conversation saved as /tmp/chat.md (Markdown)")
            );
            assert!(!view.state.export_feedback_is_error);
            assert_eq!(
                view.state.export_feedback_path.as_deref(),
                Some("/tmp/chat.md")
            );
        });

        // Error feedback clears the path
        panel.handle_command(
            ViewCommand::ShowError {
                title: "Save Conversation".to_string(),
                message: "disk unavailable".to_string(),
                severity: crate::presentation::view_command::ErrorSeverity::Error,
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert_eq!(
                view.state.export_feedback_message.as_deref(),
                Some("Save Conversation: disk unavailable")
            );
            assert!(view.state.export_feedback_is_error);
            assert!(view.state.export_feedback_path.is_none());
        });
    });
}

#[gpui::test]
async fn handle_command_forwards_error_log_export_feedback_to_error_log_view(
    cx: &mut TestAppContext,
) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ErrorLogExportCompleted {
                path: "/tmp/error-log.txt".to_string(),
            },
            cx,
        );

        let error_log_view = panel
            .error_log_view
            .as_ref()
            .expect("error log view initialized");
        error_log_view.read_with(cx, |view, _| {
            let (message, is_error, path) = view.export_feedback_state();
            assert_eq!(
                message.as_deref(),
                Some("Error log saved as /tmp/error-log.txt (TXT)")
            );
            assert!(!is_error);
            assert_eq!(path.as_deref(), Some("/tmp/error-log.txt"));
        });

        panel.handle_command(
            ViewCommand::ShowNotification {
                message: "No errors recorded".to_string(),
            },
            cx,
        );

        let error_log_view = panel
            .error_log_view
            .as_ref()
            .expect("error log view initialized");
        error_log_view.read_with(cx, |view, _| {
            let (message, is_error, path) = view.export_feedback_state();
            assert_eq!(message.as_deref(), Some("No errors recorded"));
            assert!(!is_error);
            assert!(path.is_none());
        });

        panel.handle_command(
            ViewCommand::ShowError {
                title: "Save Error Log".to_string(),
                message: "disk unavailable".to_string(),
                severity: crate::presentation::view_command::ErrorSeverity::Error,
            },
            cx,
        );

        let error_log_view = panel
            .error_log_view
            .as_ref()
            .expect("error log view initialized");
        error_log_view.read_with(cx, |view, _| {
            let (message, is_error, path) = view.export_feedback_state();
            assert_eq!(message.as_deref(), Some("Save Error Log: disk unavailable"));
            assert!(is_error);
            assert!(path.is_none());
        });
    });
}

#[gpui::test]
async fn handle_command_does_not_forward_non_export_feedback_to_chat_view(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ShowNotification {
                message: "settings updated".to_string(),
            },
            cx,
        );
        panel.handle_command(
            ViewCommand::ShowError {
                title: "MCP auth failed".to_string(),
                message: "token expired".to_string(),
                severity: crate::presentation::view_command::ErrorSeverity::Error,
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert_eq!(view.state.export_feedback_message, None);
            assert!(!view.state.export_feedback_is_error);
        });
    });
}

#[gpui::test]
async fn route_tool_approval_policy_updated_increments_counter(cx: &mut TestAppContext) {
    let _ = cx;
    assert_route_count(
        ViewCommand::ToolApprovalPolicyUpdated {
            yolo_mode: true,
            auto_approve_reads: false,
            mcp_approval_mode: crate::agent::McpApprovalMode::PerTool,
            persistent_allowlist: vec!["git".to_string()],
            persistent_denylist: vec!["rm".to_string()],
        },
        1,
        |targets| targets.tool_approval_policy_count,
    );
}

#[gpui::test]
async fn route_yolo_mode_changed_increments_counter(cx: &mut TestAppContext) {
    let _ = cx;
    assert_route_count(
        ViewCommand::YoloModeChanged { active: true },
        1,
        |targets| targets.yolo_mode_changed_count,
    );
}

#[gpui::test]
async fn handle_command_forwards_tool_approval_policy_to_settings_view(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ToolApprovalPolicyUpdated {
                yolo_mode: true,
                auto_approve_reads: true,
                mcp_approval_mode: crate::agent::McpApprovalMode::PerServer,
                persistent_allowlist: vec!["git".to_string(), "ls".to_string()],
                persistent_denylist: vec!["rm".to_string()],
            },
            cx,
        );

        let settings_view = panel
            .settings_view
            .as_ref()
            .expect("settings view initialized");
        settings_view.read_with(cx, |view, _| {
            let state = view.get_state();
            assert!(state.yolo_mode);
            assert!(state.auto_approve_reads);
            assert_eq!(
                state.mcp_approval_mode,
                crate::agent::McpApprovalMode::PerServer
            );
            assert_eq!(state.persistent_allowlist, vec!["git", "ls"]);
            assert_eq!(state.persistent_denylist, vec!["rm"]);
        });
    });
}

#[gpui::test]
async fn handle_command_forwards_yolo_mode_changed_to_settings_and_chat(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(ViewCommand::YoloModeChanged { active: true }, cx);

        let settings_view = panel
            .settings_view
            .as_ref()
            .expect("settings view initialized");
        settings_view.read_with(cx, |view, _| {
            assert!(view.get_state().yolo_mode);
        });

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert!(view.state.yolo_mode);
        });
    });
}

#[gpui::test]
async fn handle_command_forwards_tool_approval_commands_to_chat(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ToolApprovalRequest {
                request_id: "req-1".to_string(),
                tool_name: "WriteFile".to_string(),
                tool_argument: "/tmp/example.txt".to_string(),
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert_eq!(view.state.approval_bubbles.len(), 1);
            assert_eq!(view.state.approval_bubbles[0].request_id, "req-1");
            assert_eq!(view.state.approval_bubbles[0].tool_name, "WriteFile");
            assert_eq!(
                view.state.approval_bubbles[0].tool_argument,
                "/tmp/example.txt"
            );
            assert_eq!(
                view.state.approval_bubbles[0].state,
                crate::ui_gpui::views::chat_view::ApprovalBubbleState::Pending
            );
        });

        panel.handle_command(
            ViewCommand::ToolApprovalResolved {
                request_id: "req-1".to_string(),
                approved: false,
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert!(
                view.state.approval_bubbles.is_empty(),
                "resolved approval bubble should be removed"
            );
        });
    });
}
