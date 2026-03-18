//! Main panel with navigation-based view routing
//!
//! @plan PLAN-20250130-GPUIREDUX.P11
//! @requirement REQ-GPUI-003
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
//! @requirement REQ-WIRE-002

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ViewCommand, ViewId};
use crate::ui_gpui::navigation::NavigationState;
use crate::ui_gpui::GpuiAppStore;

use gpui::{
    div, prelude::*, Entity, FocusHandle, Focusable, Global, MouseButton, Subscription, Task,
};
use std::sync::Arc;

// Global navigation actions bound to keyboard shortcuts (registered in main_gpui.rs)
#[allow(clippy::derive_partial_eq_without_eq)]
mod _actions {
    gpui::actions!(
        main_panel,
        [
            NavigateToHistory,
            NavigateToSettings,
            NewConversation,
            NavigateBack
        ]
    );
}
pub use _actions::*;

// ============================================================
// REQ-WIRE-002: ViewCommand routing infrastructure
// These types and function are consumed by the GPUI render loop
// and tested directly in gpui_wiring_command_routing_tests.
// ============================================================

/// Observable state updated by `route_view_command`, used in tests
/// to verify each `ViewCommand` variant reaches its target view.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
/// @requirement REQ-WIRE-002
/// @pseudocode component-002-main-panel-routing.md lines 089-171
#[derive(Debug, Default)]
pub struct CommandTargets {
    // Chat view counters
    pub chat_messages_received: usize,
    pub chat_stream_chunks_received: usize,
    pub chat_stream_finalized: bool,

    // History view state
    pub history_conversations_received: usize,
    pub history_activated_id: Option<uuid::Uuid>,

    // Settings view counters
    pub settings_profile_commands: usize,
    pub settings_mcp_status_updates: usize,

    // Model selector state
    pub model_selector_results_count: usize,

    // MCP add/configure and settings reaction counters
    pub mcp_config_saved_count: usize,
    pub mcp_deleted_count: usize,
    pub settings_notifications_count: usize,
    pub mcp_error_commands_count: usize,
    pub mcp_registry_results_count: usize,
    pub mcp_configure_draft_loaded_count: usize,

    // Profile prefill state from model selector
    pub profile_prefill_selected_count: usize,
}

/// Route a single `ViewCommand` to the correct target view state.
///
/// This function forms the core of the `MainPanel` command dispatch matrix
/// (REQ-WIRE-002). In the live GPUI render loop it is called inline;
/// in tests it drives `CommandTargets` observable state directly.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
/// @requirement REQ-WIRE-002
/// @pseudocode component-002-main-panel-routing.md lines 089-171
pub fn route_view_command(cmd: ViewCommand, targets: &mut CommandTargets) {
    match cmd {
        // ── Chat view ───────────────────────────────────────────────────
        ViewCommand::ConversationMessagesLoaded { messages, .. } => {
            targets.chat_messages_received += messages.len();
        }
        ViewCommand::MessageAppended { .. } => {
            targets.chat_messages_received += 1;
        }
        ViewCommand::AppendStream { .. } => {
            targets.chat_stream_chunks_received += 1;
        }
        ViewCommand::FinalizeStream { .. } => {
            targets.chat_stream_finalized = true;
        }

        // ── History view ────────────────────────────────────────────────
        ViewCommand::ConversationListRefreshed { conversations } => {
            targets.history_conversations_received += conversations.len();
        }
        ViewCommand::ConversationActivated {
            id,
            selection_generation: _,
        } => {
            targets.history_activated_id = Some(id);
        }

        // ── Settings / Profile view ─────────────────────────────────────
        ViewCommand::ShowSettings { .. }
        | ViewCommand::ChatProfilesUpdated { .. }
        | ViewCommand::ProfileCreated { .. }
        | ViewCommand::ProfileUpdated { .. }
        | ViewCommand::ProfileDeleted { .. }
        | ViewCommand::DefaultProfileChanged { .. } => {
            targets.settings_profile_commands += 1;
        }
        ViewCommand::McpStatusChanged { .. }
        | ViewCommand::McpServerStarted { .. }
        | ViewCommand::McpServerFailed { .. } => {
            targets.settings_mcp_status_updates += 1;
        }
        ViewCommand::McpConfigSaved { .. } => {
            targets.mcp_config_saved_count += 1;
        }
        ViewCommand::McpDeleted { .. } => {
            targets.mcp_deleted_count += 1;
        }
        ViewCommand::ShowNotification { .. } => {
            targets.settings_notifications_count += 1;
        }
        ViewCommand::ShowError { .. } => {
            targets.mcp_error_commands_count += 1;
        }
        ViewCommand::McpRegistrySearchResults { results } => {
            targets.mcp_registry_results_count += results.len();
        }
        ViewCommand::McpConfigureDraftLoaded { .. } => {
            targets.mcp_configure_draft_loaded_count += 1;
        }

        // ── Model selector ──────────────────────────────────────────────
        ViewCommand::ModelSearchResults { models } => {
            targets.model_selector_results_count += models.len();
        }

        // ── Profile prefill ────────────────────────────────────────────────
        ViewCommand::ModelSelected { .. } | ViewCommand::ProfileEditorLoad { .. } => {
            targets.profile_prefill_selected_count += 1;
        }

        // All other commands are navigation or ancillary; not counted here
        _ => {}
    }
}
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::api_key_manager_view::ApiKeyManagerView;
use crate::ui_gpui::views::chat_view::{ChatState, ChatView};
use crate::ui_gpui::views::history_view::HistoryView;
use crate::ui_gpui::views::mcp_add_view::McpAddView;
use crate::ui_gpui::views::mcp_configure_view::McpConfigureView;
use crate::ui_gpui::views::model_selector_view::ModelSelectorView;
use crate::ui_gpui::views::profile_editor_view::ProfileEditorView;
use crate::ui_gpui::views::settings_view::SettingsView;

/// Global app state containing the bridge.
///
/// Used by `MainPanel` to receive `ViewCommands`.
/// @plan PLAN-20250130-GPUIREDUX.P11
/// @plan PLAN-20260304-GPUIREMEDIATE.P04
/// @requirement REQ-ARCH-001.1
/// @requirement REQ-ARCH-001.3
/// @requirement REQ-ARCH-004.1
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:001-035

#[derive(Clone)]
pub struct MainPanelAppState {
    pub gpui_bridge: Arc<GpuiBridge>,
    pub popup_window: Option<gpui::WindowHandle<MainPanel>>,
    pub app_store: Arc<GpuiAppStore>,
}

impl Global for MainPanelAppState {}

/// Main panel component with navigation-based view routing
/// @plan PLAN-20250130-GPUIREDUX.P11
pub struct MainPanel {
    navigation: NavigationState,
    pub focus_handle: FocusHandle,
    chat_view: Option<Entity<ChatView>>,
    history_view: Option<Entity<HistoryView>>,
    settings_view: Option<Entity<SettingsView>>,
    model_selector_view: Option<Entity<ModelSelectorView>>,
    profile_editor_view: Option<Entity<ProfileEditorView>>,
    mcp_add_view: Option<Entity<McpAddView>>,
    mcp_configure_view: Option<Entity<McpConfigureView>>,
    api_key_manager_view: Option<Entity<ApiKeyManagerView>>,
    runtime_started: bool,
    store_snapshot_revision: u64,

    store_subscription_task: Option<Task<()>>,

    bridge_poll_task: Option<Task<()>>,
    test_conversation_switch_task: Option<Task<()>>,
    child_observations: Vec<Subscription>,
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

    /// @plan PLAN-20260304-GPUIREMEDIATE.P04
    /// @requirement REQ-ARCH-001.3
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:022-035
    fn ensure_store_subscription(&mut self, cx: &mut gpui::Context<Self>) {
        if self.store_subscription_task.is_some() {
            return;
        }

        let Some(app_state) = cx.try_global::<MainPanelAppState>() else {
            tracing::warn!("MainPanel: no app state available for store subscription");
            return;
        };

        let store_rx = app_state.app_store.subscribe();
        let entity = cx.entity();
        self.store_subscription_task = Some(cx.spawn(async move |_, cx| {
            while let Ok(snapshot) = store_rx.recv_async().await {
                let () = entity.update(cx, |this, cx| {
                    this.apply_store_snapshot(snapshot, cx);
                });
            }
        }));
    }

    /// Initialize all child views with bridge
    fn request_runtime_snapshots(cx: &mut gpui::Context<Self>) {
        if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
            let bridge = app_state.gpui_bridge.clone();
            let _ = bridge.emit(UserEvent::RefreshProfiles);
            let _ = bridge.emit(UserEvent::RefreshHistory);
            let _ = bridge.emit(UserEvent::RefreshApiKeys);
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P08
    /// @requirement REQ-ARCH-005.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-127
    fn apply_startup_state(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
            self.apply_store_snapshot(app_state.app_store.current_snapshot(), cx);
        }
    }

    /// @plan PLAN-20250130-GPUIREDUX.P11
    pub fn init(&mut self, cx: &mut gpui::Context<Self>) {
        // Get the bridge from global state
        let bridge = cx
            .try_global::<MainPanelAppState>()
            .map(|s| s.gpui_bridge.clone());
        tracing::info!("MainPanel::init - bridge is_some: {}", bridge.is_some());

        // Set up navigation channel notify callback to trigger MainPanel redraw
        let _entity_id = cx.entity_id();
        // We can't directly call cx.notify() from outside, so we use a shared flag
        // that render() will check
        println!(">>> MainPanel::init - setting up navigation notify callback <<<");
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

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.4
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:079-088
    fn ensure_bridge_polling(&mut self, _cx: &mut gpui::Context<Self>) {
        if self.bridge_poll_task.is_none() {
            tracing::debug!(
                "MainPanel: bridge polling retained as no-op; app-root pump owns bridge draining"
            );
        }
    }

    fn maybe_start_test_conversation_switch(&mut self, cx: &mut gpui::Context<Self>) {
        if self.test_conversation_switch_task.is_some() {
            return;
        }

        let enabled = std::env::var("PA_TEST_CONVERSATION_SWITCH").ok().as_deref() == Some("1");
        if !enabled {
            return;
        }

        self.test_conversation_switch_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1200))
                .await;

            let first_target = this
                .read_with(cx, |this, cx| {
                    let chat_view = this.chat_view.as_ref()?;
                    let history_view = this.history_view.as_ref()?;
                    let current_conversation_id = chat_view.read(cx).state.active_conversation_id;
                    history_view
                        .read(cx)
                        .conversations()
                        .iter()
                        .find(|conversation| {
                            Some(conversation.id) != current_conversation_id
                                && conversation.message_count > 0
                        })
                        .map(|conversation| conversation.id)
                })
                .ok()
                .flatten();

            let Some(first_target) = first_target else {
                tracing::warn!(
                    "MainPanel: test conversation switch mode could not find a switch target"
                );
                return;
            };

            tracing::info!(
                conversation_id = %first_target,
                "MainPanel: test conversation switch selecting alternate conversation"
            );
            let _ = this.update(cx, |this, cx| {
                if let Some(chat_view) = this.chat_view.as_ref() {
                    chat_view.update(cx, |view, cx| {
                        view.select_conversation_by_id(first_target, cx);
                    });
                }
            });

            cx.background_executor()
                .timer(std::time::Duration::from_millis(1200))
                .await;

            let second_target = this
                .read_with(cx, |this, cx| {
                    let chat_view = this.chat_view.as_ref()?;
                    let history_view = this.history_view.as_ref()?;
                    let current_conversation_id = chat_view.read(cx).state.active_conversation_id;
                    history_view
                        .read(cx)
                        .conversations()
                        .iter()
                        .find(|conversation| Some(conversation.id) != current_conversation_id)
                        .map(|conversation| conversation.id)
                })
                .ok()
                .flatten();

            let Some(second_target) = second_target else {
                tracing::warn!(
                    "MainPanel: test conversation switch mode could not find a return target"
                );
                return;
            };

            tracing::info!(
                conversation_id = %second_target,
                "MainPanel: test conversation switch returning to original conversation"
            );
            let _ = this.update(cx, |this, cx| {
                if let Some(chat_view) = this.chat_view.as_ref() {
                    chat_view.update(cx, |view, cx| {
                        view.select_conversation_by_id(second_target, cx);
                    });
                }
            });
        }));
    }

    /// Check if all views are initialized
    const fn is_initialized(&self) -> bool {
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

    /// Handle `ViewCommand` from the presentation layer
    ///
    /// @plan PLAN-20250130-GPUIREDUX.P11
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-002
    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::NavigateTo { view } => {
                tracing::info!("MainPanel: NavigateTo {:?}", view);
                self.navigation.navigate(view);
                cx.notify();
            }
            ViewCommand::NavigateBack => {
                tracing::info!("MainPanel: NavigateBack");
                self.navigation.navigate_back();
                cx.notify();
            }
            ViewCommand::ModelSearchResults { ref models } => {
                if let Some(ref model_selector) = self.model_selector_view {
                    let models_clone = models.clone();
                    model_selector.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ModelSearchResults {
                                models: models_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ModelSelected {
                ref provider_id,
                ref model_id,
                ref provider_api_url,
                context_length,
            } => {
                if let Some(ref profile_editor) = self.profile_editor_view {
                    let provider_id_clone = provider_id.clone();
                    let model_id_clone = model_id.clone();
                    let provider_api_url_clone = provider_api_url.clone();
                    let context_length_clone = context_length;
                    profile_editor.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ModelSelected {
                                provider_id: provider_id_clone,
                                model_id: model_id_clone,
                                provider_api_url: provider_api_url_clone,
                                context_length: context_length_clone,
                            },
                            cx,
                        );
                    });
                }

                // Ensure new profile editor opens from model selector consistently,
                // even if navigation stack currently contains stale intermediate views.
                self.navigation.navigate(ViewId::ProfileEditor);
                cx.notify();
            }
            ViewCommand::ProfileEditorLoad {
                id,
                ref name,
                ref provider_id,
                ref model_id,
                ref base_url,
                ref api_key_label,
                temperature,
                max_tokens,
                context_limit,
                show_thinking,
                enable_thinking,
                thinking_budget,
                ref system_prompt,
            } => {
                if let Some(ref profile_editor) = self.profile_editor_view {
                    let name_clone = name.clone();
                    let provider_id_clone = provider_id.clone();
                    let model_id_clone = model_id.clone();
                    let base_url_clone = base_url.clone();
                    let api_key_label_clone = api_key_label.clone();
                    let system_prompt_clone = system_prompt.clone();
                    profile_editor.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ProfileEditorLoad {
                                id,
                                name: name_clone,
                                provider_id: provider_id_clone,
                                model_id: model_id_clone,
                                base_url: base_url_clone,
                                api_key_label: api_key_label_clone,
                                temperature,
                                max_tokens,
                                context_limit,
                                show_thinking,
                                enable_thinking,
                                thinking_budget,
                                system_prompt: system_prompt_clone,
                            },
                            cx,
                        );
                    });
                }
                self.navigation.navigate(ViewId::ProfileEditor);
                cx.notify();
            }

            ViewCommand::ConversationMessagesLoaded {
                ref conversation_id,
                selection_generation,
                ref messages,
            } => {
                if let Some(ref chat) = self.chat_view {
                    tracing::info!(chat_view_entity_id = ?chat.entity_id(), conversation_id = %conversation_id, message_count = messages.len(), "MainPanel forwarding ConversationMessagesLoaded to ChatView");
                    let cmd_clone = ViewCommand::ConversationMessagesLoaded {
                        conversation_id: *conversation_id,
                        selection_generation,
                        messages: messages.clone(),
                    };
                    chat.update(cx, |view, cx| {
                        view.handle_command(cmd_clone, cx);
                    });
                }
            }

            ViewCommand::MessageAppended {
                ref conversation_id,
                ref role,
                ref content,
            } => {
                if let Some(ref chat) = self.chat_view {
                    let cmd_clone = ViewCommand::MessageAppended {
                        conversation_id: *conversation_id,
                        role: *role,
                        content: content.clone(),
                    };
                    chat.update(cx, |view, cx| {
                        view.handle_command(cmd_clone, cx);
                    });
                }
            }
            ViewCommand::ShowThinking { conversation_id } => {
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ShowThinking { conversation_id }, cx);
                    });
                }
            }
            ViewCommand::HideThinking { conversation_id } => {
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::HideThinking { conversation_id }, cx);
                    });
                }
            }
            ViewCommand::AppendStream {
                ref conversation_id,
                ref chunk,
            } => {
                if let Some(ref chat) = self.chat_view {
                    let cmd_clone = ViewCommand::AppendStream {
                        conversation_id: *conversation_id,
                        chunk: chunk.clone(),
                    };
                    chat.update(cx, |view, cx| {
                        view.handle_command(cmd_clone, cx);
                    });
                }
            }
            ViewCommand::FinalizeStream {
                conversation_id,
                tokens,
            } => {
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::FinalizeStream {
                                conversation_id,
                                tokens,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::StreamCancelled {
                ref conversation_id,
                ref partial_content,
            } => {
                if let Some(ref chat) = self.chat_view {
                    let cmd_clone = ViewCommand::StreamCancelled {
                        conversation_id: *conversation_id,
                        partial_content: partial_content.clone(),
                    };
                    chat.update(cx, |view, cx| {
                        view.handle_command(cmd_clone, cx);
                    });
                }
            }
            ViewCommand::StreamError {
                ref conversation_id,
                ref error,
                recoverable,
            } => {
                if let Some(ref chat) = self.chat_view {
                    let cmd_clone = ViewCommand::StreamError {
                        conversation_id: *conversation_id,
                        error: error.clone(),
                        recoverable,
                    };
                    chat.update(cx, |view, cx| {
                        view.handle_command(cmd_clone, cx);
                    });
                }
            }
            ViewCommand::AppendThinking {
                ref conversation_id,
                ref content,
            } => {
                if let Some(ref chat) = self.chat_view {
                    let cmd_clone = ViewCommand::AppendThinking {
                        conversation_id: *conversation_id,
                        content: content.clone(),
                    };
                    chat.update(cx, |view, cx| {
                        view.handle_command(cmd_clone, cx);
                    });
                }
            }
            ViewCommand::ToggleThinkingVisibility => {
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ToggleThinkingVisibility, cx);
                    });
                }
            }
            ViewCommand::ConversationListRefreshed { ref conversations } => {
                tracing::info!("MainPanel: ConversationListRefreshed with {} conversations, chat_view={}, history_view={}", conversations.len(), self.chat_view.is_some(), self.history_view.is_some());
                if let Some(ref history) = self.history_view {
                    let convs = conversations.clone();
                    history.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationListRefreshed {
                                conversations: convs,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    tracing::info!(chat_view_entity_id = ?chat.entity_id(), "MainPanel forwarding ConversationListRefreshed to ChatView");
                    let convs = conversations.clone();
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationListRefreshed {
                                conversations: convs,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ConversationActivated {
                id,
                selection_generation,
            } => {
                if let Some(ref history) = self.history_view {
                    history.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationActivated {
                                id,
                                selection_generation,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    tracing::info!(chat_view_entity_id = ?chat.entity_id(), conversation_id = %id, "MainPanel forwarding ConversationActivated to ChatView");
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationActivated {
                                id,
                                selection_generation,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ConversationCreated { id, profile_id } => {
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationCreated { id, profile_id },
                            cx,
                        );
                    });
                }
                self.navigation.navigate(ViewId::Chat);
                cx.notify();
            }
            ViewCommand::ConversationRenamed { id, ref new_title } => {
                if let Some(ref chat) = self.chat_view {
                    let title = new_title.clone();
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationRenamed {
                                id,
                                new_title: title,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ConversationTitleUpdated { id, ref title } => {
                if let Some(ref history) = self.history_view {
                    let next_title = title.clone();
                    history.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationTitleUpdated {
                                id,
                                title: next_title,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    let next_title = title.clone();
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ConversationTitleUpdated {
                                id,
                                title: next_title,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ConversationDeleted { id } => {
                if let Some(ref history) = self.history_view {
                    history.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ConversationDeleted { id }, cx);
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ConversationDeleted { id }, cx);
                    });
                }
            }
            ViewCommand::ConversationCleared => {
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ConversationCleared, cx);
                    });
                }
            }
            ViewCommand::ShowSettings {
                ref profiles,
                selected_profile_id,
            }
            | ViewCommand::ChatProfilesUpdated {
                ref profiles,
                selected_profile_id,
            } => {
                tracing::info!("MainPanel: ShowSettings/ChatProfilesUpdated with {} profiles, default={:?}, settings_view={}, chat_view={}", profiles.len(), selected_profile_id, self.settings_view.is_some(), self.chat_view.is_some());
                if let Some(ref settings) = self.settings_view {
                    let profiles_clone = profiles.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ShowSettings {
                                profiles: profiles_clone,
                                selected_profile_id,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    let profiles_clone = profiles.clone();
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ChatProfilesUpdated {
                                profiles: profiles_clone,
                                selected_profile_id,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ProfileCreated { id, ref name } => {
                if let Some(ref settings) = self.settings_view {
                    let name_clone = name.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ProfileCreated {
                                id,
                                name: name_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ProfileUpdated { id, ref name } => {
                if let Some(ref settings) = self.settings_view {
                    let name_clone = name.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ProfileUpdated {
                                id,
                                name: name_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ProfileDeleted { id } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ProfileDeleted { id }, cx);
                    });
                }
            }
            ViewCommand::DefaultProfileChanged { profile_id } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::DefaultProfileChanged { profile_id }, cx);
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::DefaultProfileChanged { profile_id }, cx);
                    });
                }
            }
            ViewCommand::McpStatusChanged { id, status } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpStatusChanged { id, status }, cx);
                    });
                }
            }
            ViewCommand::McpServerStarted { id, tool_count } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpServerStarted { id, tool_count }, cx);
                    });
                }
            }
            ViewCommand::McpServerFailed { id, ref error } => {
                if let Some(ref settings) = self.settings_view {
                    let err_clone = error.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpServerFailed {
                                id,
                                error: err_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::McpConfigSaved { id, ref name } => {
                if let Some(ref settings) = self.settings_view {
                    let name_clone = name.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpConfigSaved {
                                id,
                                name: name_clone,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref mcp_configure) = self.mcp_configure_view {
                    let name_clone = name.clone();
                    mcp_configure.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpConfigSaved {
                                id,
                                name: name_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::McpDeleted { id } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpDeleted { id }, cx);
                    });
                }
            }
            ViewCommand::ShowNotification { ref message } => {
                if let Some(ref settings) = self.settings_view {
                    let msg = message.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ShowNotification { message: msg }, cx);
                    });
                }
            }
            ViewCommand::ShowError {
                ref title,
                ref message,
                severity,
            } => {
                if let Some(ref mcp_add) = self.mcp_add_view {
                    let t = title.clone();
                    let m = message.clone();
                    mcp_add.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ShowError {
                                title: t,
                                message: m,
                                severity,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref mcp_configure) = self.mcp_configure_view {
                    let t = title.clone();
                    let m = message.clone();
                    mcp_configure.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ShowError {
                                title: t,
                                message: m,
                                severity,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::McpRegistrySearchResults { ref results } => {
                if let Some(ref mcp_add) = self.mcp_add_view {
                    let results_clone = results.clone();
                    mcp_add.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpRegistrySearchResults {
                                results: results_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::McpConfigureDraftLoaded {
                ref id,
                ref name,
                ref package,
                ref env_var_name,
                ref command,
                ref args,
                ref env,
            } => {
                if let Some(ref mcp_configure) = self.mcp_configure_view {
                    let id_clone = id.clone();
                    let name_clone = name.clone();
                    let package_clone = package.clone();
                    let env_var_name_clone = env_var_name.clone();
                    let command_clone = command.clone();
                    let args_clone = args.clone();
                    let env_clone = env.clone();
                    mcp_configure.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpConfigureDraftLoaded {
                                id: id_clone,
                                name: name_clone,
                                package: package_clone,
                                env_var_name: env_var_name_clone,
                                command: command_clone,
                                args: args_clone,
                                env: env_clone,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ApiKeysListed { ref keys } => {
                tracing::info!(
                    key_count = keys.len(),
                    "MainPanel: forwarding ApiKeysListed to GPUI views"
                );
                if let Some(ref akm) = self.api_key_manager_view {
                    let keys_clone = keys.clone();
                    akm.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeysListed { keys: keys_clone }, cx);
                    });
                }
                // Also forward to profile editor so its key dropdown refreshes
                if let Some(ref pe) = self.profile_editor_view {
                    let keys_clone = keys.clone();
                    pe.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeysListed { keys: keys_clone }, cx);
                    });
                }
            }
            ViewCommand::ApiKeyStored { ref label } => {
                if let Some(ref akm) = self.api_key_manager_view {
                    let l = label.clone();
                    akm.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeyStored { label: l }, cx);
                    });
                }
            }
            ViewCommand::ApiKeyDeleted { ref label } => {
                if let Some(ref akm) = self.api_key_manager_view {
                    let l = label.clone();
                    akm.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeyDeleted { label: l }, cx);
                    });
                }
            }
            other => {
                tracing::debug!("MainPanel: command {:?} not forwarded to child view", other);
            }
        }
    }
}

impl gpui::Focusable for MainPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for MainPanel {
    #[allow(clippy::too_many_lines, clippy::derive_partial_eq_without_eq)]
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        tracing::info!(
            main_panel_entity_id = ?cx.entity_id(),
            current_view = ?self.navigation.current(),
            runtime_started = self.runtime_started,
            chat_view_entity_id = ?self.chat_view.as_ref().map(gpui::Entity::entity_id),
            child_observation_count = self.child_observations.len(),
            "MainPanel::render state snapshot"
        );

        // Initialize child views on first render
        if !self.is_initialized() {
            self.init(cx);
            // Focus MainPanel on first render so keyboard shortcuts work immediately
            window.focus(&self.focus_handle, cx);
            println!(">>> MainPanel first render - focused <<<");
        }

        if !self.runtime_started && self.is_initialized() {
            self.start_runtime(cx);
        }

        if self.child_observations.is_empty() {
            if let Some(ref chat_view) = self.chat_view {
                self.child_observations.push(cx.observe_in(
                    chat_view,
                    window,
                    |_, _, _window, cx| cx.notify(),
                ));
            }
        }

        // Check for pending navigation requests from child views
        // Poll frequently since we can't get async notify from the channel
        if crate::ui_gpui::navigation_channel().has_pending() {
            if let Some(view_id) = crate::ui_gpui::navigation_channel().take_pending() {
                println!(">>> MainPanel::render - Processing navigation to {view_id:?} <<<");
                tracing::info!("MainPanel: Processing navigation request to {:?}", view_id);

                // Special handling: when navigating to ModelSelector, request models
                if view_id == ViewId::ModelSelector {
                    if let Some(ref model_selector) = self.model_selector_view {
                        model_selector.update(cx, |view, _cx| {
                            view.request_models();
                        });
                    }
                }

                if view_id == ViewId::ProfileEditor {
                    if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
                        let _ = app_state.gpui_bridge.emit(UserEvent::RefreshApiKeys);
                    }
                }

                self.navigation.navigate(view_id);
                cx.notify();
            }
        }

        let current_view = self.navigation.current();

        match current_view {
            ViewId::Chat => {
                if let Some(chat_view) = &self.chat_view {
                    let focus = chat_view.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
            ViewId::ProfileEditor => {
                if let Some(pe_view) = &self.profile_editor_view {
                    let focus = pe_view.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
            ViewId::McpAdd => {
                if let Some(mcp_view) = &self.mcp_add_view {
                    let focus = mcp_view.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
            ViewId::ModelSelector => {
                if let Some(ms_view) = &self.model_selector_view {
                    let focus = ms_view.read(cx).focus_handle(cx);
                    window.focus(&focus, cx);
                }
            }
            ViewId::ApiKeyManager => {
                if let Some(akm_view) = &self.api_key_manager_view {
                    let focus = akm_view.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                }
            }
            _ => {
                window.focus(&self.focus_handle, cx);
            }
        }

        // Schedule a notify after a brief delay to keep polling for navigation
        // This is a workaround since we can't use async notify from static channel
        let entity_id = cx.entity_id();
        cx.defer(move |cx| {
            cx.notify(entity_id);
        });

        let focus_handle = self.focus_handle.clone();

        div()
            .id("main-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            // Click to get focus
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_this, _, window, cx| {
                    println!(">>> MainPanel clicked - requesting focus <<<");
                    window.focus(&focus_handle, cx);
                    cx.notify();
                }),
            )
            // Global navigation actions (keybindings registered in main_gpui.rs)
            .on_action(cx.listener(|_this, _: &NavigateToHistory, _window, _cx| {
                crate::ui_gpui::navigation_channel().request_navigate(ViewId::History);
            }))
            .on_action(cx.listener(|_this, _: &NavigateToSettings, _window, _cx| {
                crate::ui_gpui::navigation_channel().request_navigate(ViewId::Settings);
            }))
            .on_action(cx.listener(|_this, _: &NewConversation, _window, _cx| {
                crate::ui_gpui::navigation_channel().request_navigate(ViewId::Chat);
            }))
            .on_action(cx.listener(|this, _: &NavigateBack, _window, _cx| {
                if this.navigation.current() != ViewId::Chat {
                    this.navigation.navigate_back();
                }
            }))
            // View-specific bare-key shortcuts (escape, +, m) that can't be global
            // bindings because they'd conflict with text input in other views
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;
                    if modifiers.platform || modifiers.control {
                        return; // handled by GPUI action bindings
                    }
                    let current = this.navigation.current();
                    match current {
                        ViewId::Settings => match key.as_str() {
                            "escape" => {
                                this.navigation.navigate_back();
                            }
                            "+" => crate::ui_gpui::navigation_channel()
                                .request_navigate(ViewId::ProfileEditor),
                            "=" if modifiers.shift => crate::ui_gpui::navigation_channel()
                                .request_navigate(ViewId::ProfileEditor),
                            "m" => crate::ui_gpui::navigation_channel()
                                .request_navigate(ViewId::McpAdd),
                            _ => {}
                        },
                        ViewId::ModelSelector => {
                            if key == "escape" {
                                this.navigation.navigate_back();
                            }
                        }
                        _ => {}
                    }
                }),
            )
            // Render view based on navigation state
            // @plan PLAN-20250130-GPUIREDUX.P11
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    // Chat view
                    .when(current_view == ViewId::Chat, |d| {
                        if let Some(view) = &self.chat_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading chat..."))
                        }
                    })
                    // History view
                    .when(current_view == ViewId::History, |d| {
                        if let Some(view) = &self.history_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading history..."))
                        }
                    })
                    // Settings view
                    .when(current_view == ViewId::Settings, |d| {
                        if let Some(view) = &self.settings_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading settings..."))
                        }
                    })
                    // Model Selector view
                    .when(current_view == ViewId::ModelSelector, |d| {
                        if let Some(view) = &self.model_selector_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading model selector..."))
                        }
                    })
                    // Profile Editor view
                    .when(current_view == ViewId::ProfileEditor, |d| {
                        if let Some(view) = &self.profile_editor_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading profile editor..."))
                        }
                    })
                    // MCP Add view
                    .when(current_view == ViewId::McpAdd, |d| {
                        if let Some(view) = &self.mcp_add_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading MCP add..."))
                        }
                    })
                    // MCP Configure view
                    .when(current_view == ViewId::McpConfigure, |d| {
                        if let Some(view) = &self.mcp_configure_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading MCP configure..."))
                        }
                    })
                    // API Key Manager view
                    .when(current_view == ViewId::ApiKeyManager, |d| {
                        if let Some(view) = &self.api_key_manager_view {
                            d.child(view.clone())
                        } else {
                            d.child(div().child("Loading API key manager..."))
                        }
                    }),
            )
    }
}
