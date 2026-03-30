//! Main panel with navigation-based view routing
//!
//! @plan PLAN-20250130-GPUIREDUX.P11
//! @requirement REQ-GPUI-003
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
//! @requirement REQ-WIRE-002

mod command;
mod render;
mod routing;
mod startup;

pub use routing::{
    route_view_command, CommandTargets, NavigateBack, NavigateToHistory, NavigateToSettings,
    NewConversation,
};
pub use startup::MainPanelAppState;

use crate::presentation::view_command::ViewId;
use crate::ui_gpui::navigation::NavigationState;

use gpui::{prelude::*, Entity, FocusHandle, Subscription, Task};

use crate::ui_gpui::views::api_key_manager_view::ApiKeyManagerView;
use crate::ui_gpui::views::chat_view::{ChatState, ChatView};
use crate::ui_gpui::views::history_view::HistoryView;
use crate::ui_gpui::views::mcp_add_view::McpAddView;
use crate::ui_gpui::views::mcp_configure_view::McpConfigureView;
use crate::ui_gpui::views::model_selector_view::ModelSelectorView;
use crate::ui_gpui::views::profile_editor_view::ProfileEditorView;
use crate::ui_gpui::views::settings_view::SettingsView;

/// Main panel component with navigation-based view routing
/// @plan PLAN-20250130-GPUIREDUX.P11
pub struct MainPanel {
    pub(super) navigation: NavigationState,
    pub focus_handle: FocusHandle,
    pub(super) chat_view: Option<Entity<ChatView>>,
    pub(super) history_view: Option<Entity<HistoryView>>,
    pub(super) settings_view: Option<Entity<SettingsView>>,
    pub(super) model_selector_view: Option<Entity<ModelSelectorView>>,
    pub(super) profile_editor_view: Option<Entity<ProfileEditorView>>,
    pub(super) mcp_add_view: Option<Entity<McpAddView>>,
    pub(super) mcp_configure_view: Option<Entity<McpConfigureView>>,
    pub(super) api_key_manager_view: Option<Entity<ApiKeyManagerView>>,
    pub(super) runtime_started: bool,
    pub store_snapshot_revision: u64,

    pub(super) store_subscription_task: Option<Task<()>>,

    pub(super) bridge_poll_task: Option<Task<()>>,
    pub(super) test_conversation_switch_task: Option<Task<()>>,
    pub(super) child_observations: Vec<Subscription>,
}

impl MainPanel {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            navigation: NavigationState::new(),
            focus_handle: cx.focus_handle(),
            chat_view: None,
            history_view: None,
            settings_view: None,
            model_selector_view: None,
            profile_editor_view: None,
            mcp_add_view: None,
            mcp_configure_view: None,
            api_key_manager_view: None,
            runtime_started: false,
            store_snapshot_revision: 0,
            store_subscription_task: None,

            bridge_poll_task: None,
            test_conversation_switch_task: None,
            child_observations: Vec::new(),
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P06
    /// @requirement REQ-ARCH-002.5
    /// @requirement REQ-ARCH-006.3
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:072-088
    pub fn apply_store_snapshot(
        &mut self,
        snapshot: crate::ui_gpui::app_store::GpuiAppSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        self.store_snapshot_revision = snapshot.revision;

        if let Some(ref chat_view) = self.chat_view {
            let chat_snapshot = snapshot.chat.clone();
            let settings_snapshot = snapshot.settings.clone();
            chat_view.update(cx, |view, cx| {
                view.apply_store_snapshot(chat_snapshot, cx);
                // Apply profile data from store so profiles are available on first render
                if !settings_snapshot.profiles.is_empty() {
                    view.apply_settings_snapshot(settings_snapshot);
                }
            });
        }

        if let Some(ref history_view) = self.history_view {
            let history_snapshot = snapshot.history;
            history_view.update(cx, |view, cx| {
                view.apply_store_snapshot(&history_snapshot, cx);
            });
        }

        cx.notify();
    }

    /// @plan PLAN-20250130-GPUIREDUX.P11
    pub fn init(&mut self, cx: &mut gpui::Context<Self>) {
        // Get the bridge from global state
        let bridge = cx
            .try_global::<MainPanelAppState>()
            .map(|s| s.gpui_bridge.clone());
        tracing::info!("MainPanel::init - bridge is_some: {}", bridge.is_some());

        // Child view input already schedules redraws, so no render self-polling is needed.
        self.ensure_store_subscription(cx);

        // Chat view
        let chat_state = ChatState::default();
        self.chat_view = Some(cx.new(|cx: &mut gpui::Context<ChatView>| {
            tracing::info!(chat_view_entity_id = ?cx.entity_id(), "MainPanel::init created ChatView");
            let mut view = ChatView::new(chat_state, cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));
        // History view
        self.history_view = Some(cx.new(|cx: &mut gpui::Context<HistoryView>| {
            let mut view = HistoryView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // Settings view
        self.settings_view = Some(cx.new(|cx: &mut gpui::Context<SettingsView>| {
            let mut view = SettingsView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // Model Selector view
        self.model_selector_view = Some(cx.new(|cx: &mut gpui::Context<ModelSelectorView>| {
            let mut view = ModelSelectorView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // Profile Editor view
        self.profile_editor_view = Some(cx.new(|cx: &mut gpui::Context<ProfileEditorView>| {
            let mut view = ProfileEditorView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // MCP Add view
        self.mcp_add_view = Some(cx.new(|cx: &mut gpui::Context<McpAddView>| {
            let mut view = McpAddView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // MCP Configure view
        self.mcp_configure_view = Some(cx.new(|cx: &mut gpui::Context<McpConfigureView>| {
            let mut view = McpConfigureView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        // API Key Manager view
        self.api_key_manager_view = Some(cx.new(|cx: &mut gpui::Context<ApiKeyManagerView>| {
            let mut view = ApiKeyManagerView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone());
            }
            view
        }));

        self.apply_startup_state(cx);
    }

    /// Check if all views are initialized
    pub(super) const fn is_initialized(&self) -> bool {
        self.chat_view.is_some()
            && self.history_view.is_some()
            && self.settings_view.is_some()
            && self.model_selector_view.is_some()
            && self.profile_editor_view.is_some()
            && self.mcp_add_view.is_some()
            && self.mcp_configure_view.is_some()
            && self.api_key_manager_view.is_some()
    }

    #[must_use]
    pub const fn is_runtime_started(&self) -> bool {
        self.runtime_started
    }

    pub fn start_runtime(&mut self, cx: &mut gpui::Context<Self>) {
        if self.runtime_started || !self.is_initialized() {
            return;
        }

        let has_popup_window = cx
            .try_global::<MainPanelAppState>()
            .and_then(|app_state| app_state.popup_window)
            .is_some();
        if !has_popup_window {
            tracing::info!(
                "MainPanel: delaying runtime start until popup window handle is available"
            );
            return;
        }

        self.runtime_started = true;
        self.ensure_bridge_polling(cx);
        Self::request_runtime_snapshots(cx);
        self.maybe_start_test_conversation_switch(cx);
        cx.notify();
    }

    /// Get the current view ID
    #[must_use]
    pub fn current_view(&self) -> ViewId {
        self.navigation.current()
    }
}

impl gpui::Focusable for MainPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
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

    fn conversation_summary(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
        ConversationSummary {
            id,
            title: title.to_string(),
            updated_at: Utc::now(),
            message_count,
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
            },
            user_rx,
            first_conversation_id,
            second_conversation_id,
            selected_profile_id,
        )
    }

    #[gpui::test]
    async fn init_and_startup_state_seed_child_views_from_store(cx: &mut TestAppContext) {
        let (
            app_state,
            _user_rx,
            first_conversation_id,
            _second_conversation_id,
            _selected_profile_id,
        ) = build_app_state();
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
    async fn start_runtime_requires_popup_window_before_emitting_refreshes(
        cx: &mut TestAppContext,
    ) {
        let (
            app_state,
            user_rx,
            _first_conversation_id,
            _second_conversation_id,
            _selected_profile_id,
        ) = build_app_state();
        cx.set_global(app_state);

        let panel = cx.new(MainPanel::new);

        panel.update(cx, |panel: &mut MainPanel, cx| {
            panel.init(cx);
            panel.start_runtime(cx);
            assert!(!panel.runtime_started);
            assert!(panel.bridge_poll_task.is_none());
            assert!(panel.test_conversation_switch_task.is_none());
        });

        assert_eq!(
            user_rx.recv().expect("profile editor refresh"),
            UserEvent::RefreshApiKeys
        );
        assert_eq!(
            user_rx.recv().expect("api key manager refresh"),
            UserEvent::RefreshApiKeys
        );
        assert!(
            user_rx.try_recv().is_err(),
            "runtime should not emit snapshot refreshes before popup window exists"
        );
    }

    #[gpui::test]
    async fn ensure_store_subscription_only_subscribes_once_and_applies_published_updates(
        cx: &mut TestAppContext,
    ) {
        let (
            app_state,
            _user_rx,
            first_conversation_id,
            second_conversation_id,
            selected_profile_id,
        ) = build_app_state();
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
                assert_eq!(view.state.current_model, "claude-3-7-sonnet");
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
    #[allow(clippy::too_many_lines)]
    async fn handle_command_forwards_settings_profiles_and_routes_mcp_commands_to_expected_targets(
        cx: &mut TestAppContext,
    ) {
        let (app_state, _user_rx, _first_id, _second_id, selected_profile_id) = build_app_state();
        cx.set_global(app_state);
        let panel = cx.new(MainPanel::new);
        let saved_mcp_id = Uuid::new_v4();

        panel.update(cx, |panel: &mut MainPanel, cx| {
            panel.init(cx);

            let profile_id = selected_profile_id;
            panel.handle_command(
                ViewCommand::ShowSettings {
                    profiles: vec![profile_summary(
                        profile_id,
                        "Workspace Default",
                        "openai",
                        "gpt-4.1",
                        true,
                    )],
                    selected_profile_id: Some(profile_id),
                },
                cx,
            );

            let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
            chat_view.read_with(cx, |view, _| {
                assert_eq!(view.state.profiles.len(), 1);
                assert_eq!(view.state.selected_profile_id, Some(profile_id));
                assert_eq!(view.state.current_model, "gpt-4.1");
            });

            let mut targets = CommandTargets::default();
            route_view_command(
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
                &mut targets,
            );
            assert_eq!(targets.mcp_configure_draft_loaded_count, 1);

            route_view_command(
                ViewCommand::ShowError {
                    title: "MCP auth failed".to_string(),
                    message: "token expired".to_string(),
                    severity: crate::presentation::view_command::ErrorSeverity::Error,
                },
                &mut targets,
            );
            assert_eq!(targets.mcp_error_commands_count, 1);
            assert_eq!(targets.chat_error_commands, 0);

            route_view_command(
                ViewCommand::ShowError {
                    title: "Save Conversation".to_string(),
                    message: "disk unavailable".to_string(),
                    severity: crate::presentation::view_command::ErrorSeverity::Error,
                },
                &mut targets,
            );
            assert_eq!(targets.mcp_error_commands_count, 2);
            assert_eq!(targets.chat_error_commands, 1);

            route_view_command(
                ViewCommand::ShowNotification {
                    message: "connected-user".to_string(),
                },
                &mut targets,
            );
            assert_eq!(targets.settings_notifications_count, 1);
            assert_eq!(targets.chat_notification_commands, 0);

            route_view_command(
                ViewCommand::ShowNotification {
                    message: "Conversation saved as /tmp/chat.md (MD)".to_string(),
                },
                &mut targets,
            );
            assert_eq!(targets.settings_notifications_count, 2);
            assert_eq!(targets.chat_notification_commands, 1);

            route_view_command(
                ViewCommand::ShowConversationExportFormat {
                    format: ConversationExportFormat::Md,
                },
                &mut targets,
            );
            assert_eq!(targets.chat_export_format_commands, 1);

            route_view_command(
                ViewCommand::McpConfigSaved {
                    id: saved_mcp_id,
                    name: Some("Workspace MCP Saved".to_string()),
                },
                &mut targets,
            );
            assert_eq!(targets.mcp_config_saved_count, 1);

            route_view_command(
                ViewCommand::ShowSettingsTheme {
                    options: vec![theme_summary("Midnight Nebula", "default")],
                    selected_slug: "default".to_string(),
                },
                &mut targets,
            );
            assert_eq!(targets.settings_theme_commands, 1);
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
                ViewCommand::ShowNotification {
                    message: "Conversation saved as /tmp/chat.md (MD)".to_string(),
                },
                cx,
            );
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
                    view.state.conversation_export_format,
                    ConversationExportFormat::Json
                );
                assert_eq!(
                    view.state.export_feedback_message.as_deref(),
                    Some("Save Conversation: disk unavailable")
                );
                assert!(view.state.export_feedback_is_error);
            });
        });
    }

    #[gpui::test]
    async fn handle_command_does_not_forward_non_export_feedback_to_chat_view(
        cx: &mut TestAppContext,
    ) {
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
}
