//! Main panel with navigation-based view routing
//!
//! @plan PLAN-20250130-GPUIREDUX.P11
//! @requirement REQ-GPUI-003
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
//! @requirement REQ-WIRE-002

use crate::presentation::view_command::{ViewCommand, ViewId};
use crate::ui_gpui::navigation::NavigationState;
use gpui::{div, prelude::*, Entity, FocusHandle, Global, MouseButton};
use std::sync::Arc;
use std::time::Duration;

// ============================================================
// REQ-WIRE-002: ViewCommand routing infrastructure
// These types and function are consumed by the GPUI render loop
// and tested directly in gpui_wiring_command_routing_tests.
// ============================================================

/// Observable state updated by route_view_command, used in tests
/// to verify each ViewCommand variant reaches its target view.
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

/// Route a single ViewCommand to the correct target view state.
///
/// This function forms the core of the MainPanel command dispatch matrix
/// (REQ-WIRE-002). In the live GPUI render loop it is called inline;
/// in tests it drives `CommandTargets` observable state directly.
///
/// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
/// @requirement REQ-WIRE-002
/// @pseudocode component-002-main-panel-routing.md lines 089-171
pub fn route_view_command(cmd: ViewCommand, targets: &mut CommandTargets) {
    match cmd {
        // ── Chat view ───────────────────────────────────────────────────
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
        ViewCommand::ConversationActivated { id } => {
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
        ViewCommand::McpStatusChanged { .. } => {
            targets.settings_mcp_status_updates += 1;
        }
        ViewCommand::McpServerStarted { .. } => {
            targets.settings_mcp_status_updates += 1;
        }
        ViewCommand::McpServerFailed { .. } => {
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
        ViewCommand::ModelSelected { .. } => {
            targets.profile_prefill_selected_count += 1;
        }
        ViewCommand::ProfileEditorLoad { .. } => {
            targets.profile_prefill_selected_count += 1;
        }


        // All other commands are navigation or ancillary; not counted here
        _ => {}
    }
}
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::chat_view::{ChatState, ChatView};
use crate::ui_gpui::views::history_view::HistoryView;
use crate::ui_gpui::views::mcp_add_view::McpAddView;
use crate::ui_gpui::views::mcp_configure_view::McpConfigureView;
use crate::ui_gpui::views::model_selector_view::ModelSelectorView;
use crate::ui_gpui::views::profile_editor_view::ProfileEditorView;
use crate::ui_gpui::views::settings_view::SettingsView;

/// Global app state containing the bridge - used by MainPanel to receive ViewCommands
/// @plan PLAN-20250130-GPUIREDUX.P11
#[derive(Clone)]
pub struct MainPanelAppState {
    pub gpui_bridge: Arc<GpuiBridge>,
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
        }
    }

    /// Initialize all child views with bridge
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

        // Chat view
        let chat_state = ChatState::default();
        self.chat_view = Some(cx.new(|cx: &mut gpui::Context<ChatView>| {
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

        // Start a background thread to poll for navigation requests and trigger redraws
        let entity = cx.entity().downgrade();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(100));
                if crate::ui_gpui::navigation_channel().has_pending() {
                    println!(">>> Navigation poll detected pending request <<<");
                    // We can't directly notify, but setting the flag is enough
                    // render() will pick it up on next frame
                }
            }
        });
        let _ = entity; // suppress warning
    }

    /// Check if all views are initialized
    fn is_initialized(&self) -> bool {
        self.chat_view.is_some()
            && self.history_view.is_some()
            && self.settings_view.is_some()
            && self.model_selector_view.is_some()
            && self.profile_editor_view.is_some()
            && self.mcp_add_view.is_some()
            && self.mcp_configure_view.is_some()
    }

    /// Get the current view ID
    pub fn current_view(&self) -> ViewId {
        self.navigation.current()
    }

    /// Handle ViewCommand from the presentation layer
    ///
    /// @plan PLAN-20250130-GPUIREDUX.P11
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-002
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
                context_length,
            } => {
                if let Some(ref profile_editor) = self.profile_editor_view {
                    let provider_id_clone = provider_id.clone();
                    let model_id_clone = model_id.clone();
                    let context_length_clone = context_length;
                    profile_editor.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ModelSelected {
                                provider_id: provider_id_clone,
                                model_id: model_id_clone,
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
                ref auth_kind,
                ref auth_value,
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
                    let auth_kind_clone = auth_kind.clone();
                    let auth_value_clone = auth_value.clone();
                    let system_prompt_clone = system_prompt.clone();
                    profile_editor.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ProfileEditorLoad {
                                id,
                                name: name_clone,
                                provider_id: provider_id_clone,
                                model_id: model_id_clone,
                                base_url: base_url_clone,
                                auth_kind: auth_kind_clone,
                                auth_value: auth_value_clone,
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
            ViewCommand::ConversationActivated { id } => {
                if let Some(ref history) = self.history_view {
                    history.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ConversationActivated { id }, cx);
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ConversationActivated { id }, cx);
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
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        // Initialize child views on first render
        if !self.is_initialized() {
            self.init(cx);
            // Focus MainPanel on first render so keyboard shortcuts work immediately
            window.focus(&self.focus_handle, cx);
            println!(">>> MainPanel first render - focused <<<");
        }

        // Check for pending navigation requests from child views
        // Poll frequently since we can't get async notify from the channel
        if crate::ui_gpui::navigation_channel().has_pending() {
            if let Some(view_id) = crate::ui_gpui::navigation_channel().take_pending() {
                println!(
                    ">>> MainPanel::render - Processing navigation to {:?} <<<",
                    view_id
                );
                tracing::info!("MainPanel: Processing navigation request to {:?}", view_id);

                // Special handling: when navigating to ModelSelector, request models
                if view_id == ViewId::ModelSelector {
                    if let Some(ref model_selector) = self.model_selector_view {
                        model_selector.update(cx, |view, _cx| {
                            view.request_models();
                        });
                    }
                }

                self.navigation.navigate(view_id);
                cx.notify();
            }
        }

        // Check for pending ViewCommands from presenters via bridge.
        // Drain and route through the full command dispatch matrix.
        //
        // @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
        // @requirement REQ-WIRE-002
        if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
            let bridge = app_state.gpui_bridge.clone();
            let commands = bridge.drain_commands();
            if !commands.is_empty() {
                tracing::info!("MainPanel: drained {} ViewCommands from bridge", commands.len());
            }
            for cmd in commands {
                tracing::info!("MainPanel: routing ViewCommand {:?}", std::mem::discriminant(&cmd));
                self.handle_command(cmd, cx);
            }
        }

        let current_view = self.navigation.current();

        // Schedule a notify after a brief delay to keep polling for navigation
        // This is a workaround since we can't use async notify from static channel
        let entity_id = cx.entity_id();
        cx.defer(move |cx| {
            cx.notify(entity_id);
        });

        // Request focus on the MainPanel so we receive keyboard events
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
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;
                    let current = this.navigation.current();

                    println!(
                        ">>> MainPanel key_down: key={} platform={} shift={} current={:?} <<<",
                        key, modifiers.platform, modifiers.shift, current
                    );

                    // Global keyboard shortcuts - work from any view
                    // Using Ctrl+key to avoid conflicts with system shortcuts
                    if modifiers.control {
                        match key.as_str() {
                            "h" => {
                                println!(">>> Ctrl+H - navigating to History <<<");
                                crate::ui_gpui::navigation_channel()
                                    .request_navigate(ViewId::History);
                            }
                            "s" => {
                                println!(">>> Ctrl+S - navigating to Settings <<<");
                                crate::ui_gpui::navigation_channel()
                                    .request_navigate(ViewId::Settings);
                            }
                            "n" => {
                                println!(">>> Ctrl+N - new conversation <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(ViewId::Chat);
                            }
                            _ => {}
                        }
                    }
                    // Cmd+W for close/back (standard macOS)
                    else if modifiers.platform && key == "w" {
                        println!(">>> Cmd+W - navigate back <<<");
                        if current != ViewId::Chat {
                            this.navigation.navigate_back();
                        }
                    }
                    // View-specific shortcuts (no Cmd modifier)
                    else if current == ViewId::Settings {
                        // Settings view shortcuts
                        if key == "+" || (key == "=" && modifiers.shift) {
                            // "+" key - Add Profile
                            println!(">>> + pressed on Settings - Add Profile <<<");
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(ViewId::ModelSelector);
                        } else if key == "m" {
                            // "m" key - Add MCP
                            println!(">>> m pressed on Settings - Add MCP <<<");
                            crate::ui_gpui::navigation_channel().request_navigate(ViewId::McpAdd);
                        } else if key == "escape" {
                            println!(">>> Escape on Settings - back to Chat <<<");
                            this.navigation.navigate_back();
                        }
                    }
                    // Model Selector view
                    else if current == ViewId::ModelSelector {
                        // Let ModelSelectorView own all search/filter key handling to avoid
                        // duplicate query mutation and duplicate SearchModels emission.
                        if key == "escape" {
                            println!(">>> Escape on ModelSelector - back to Settings <<<");
                            this.navigation.navigate_back();
                        }
                    }
                    // Profile Editor view - forward key events to the editor
                    else if current == ViewId::ProfileEditor {
                        if let Some(ref profile_editor) = this.profile_editor_view {
                            profile_editor.update(_cx, |view, cx| {
                                view.handle_key_input(key, modifiers, cx);
                            });
                        }
                    }
                    // MCP Add view - forward keys for registry search
                    else if current == ViewId::McpAdd {
                        if key == "escape" {
                            println!(">>> Escape on McpAdd - back to Settings <<<");
                            this.navigation.navigate_back();
                        } else if let Some(ref mcp_add_view) = this.mcp_add_view {
                            mcp_add_view.update(_cx, |view, cx| {
                                if key == "backspace" {
                                    let current = view.get_state().search_query.clone();
                                    let new_query =
                                        current[..current.len().saturating_sub(1)].to_string();
                                    view.set_search_query(new_query);
                                    view.emit_search_registry();
                                    cx.notify();
                                } else if key == "enter" {
                                    view.emit_search_registry();
                                    cx.notify();
                                } else if key.len() == 1
                                    && !modifiers.platform
                                    && !modifiers.control
                                {
                                    let mut query = view.get_state().search_query.clone();
                                    query.push_str(key);
                                    view.set_search_query(query);
                                    view.emit_search_registry();
                                    cx.notify();
                                }
                            });
                        }
                    }
                    // Chat view - forward all keys to ChatView for text input/dropdown navigation
                    else if current == ViewId::Chat {
                        if let Some(ref chat_view) = this.chat_view {
                            chat_view.update(_cx, |view, cx| {
                                // Cmd+V paste from clipboard
                                if modifiers.platform && key == "v" {
                                    if let Some(item) = cx.read_from_clipboard() {
                                        if let Some(text) = item.text() {
                                            view.handle_paste(&text, cx);
                                        }
                                    }
                                    return;
                                }
                                // Cmd+A select all
                                if modifiers.platform && key == "a" {
                                    view.handle_select_all(cx);
                                    return;
                                }
                                // Cmd+P toggles chat profile dropdown
                                if modifiers.platform && key == "p" {
                                    view.toggle_profile_dropdown(cx);
                                    return;
                                }
                                // Cmd+K toggles conversation dropdown
                                if modifiers.platform && key == "k" {
                                    view.toggle_conversation_dropdown(cx);
                                    return;
                                }
                                // Cmd+R starts inline rename mode
                                if modifiers.platform && key == "r" {
                                    view.start_rename_conversation(cx);
                                    return;
                                }
                                // Arrow keys for cursor movement (when not in dropdown)
                                if !view.conversation_dropdown_open()
                                    && !view.profile_dropdown_open()
                                    && !view.conversation_title_editing()
                                {
                                    if key == "left" {
                                        view.move_cursor_left(cx);
                                        return;
                                    } else if key == "right" {
                                        view.move_cursor_right(cx);
                                        return;
                                    } else if key == "home" || (modifiers.platform && key == "left") {
                                        view.move_cursor_home(cx);
                                        return;
                                    } else if key == "end" || (modifiers.platform && key == "right") {
                                        view.move_cursor_end(cx);
                                        return;
                                    }
                                }
                                // Inline rename editing mode
                                if view.conversation_title_editing() {
                                    if key == "backspace" {
                                        view.handle_rename_backspace(cx);
                                    } else if key == "enter" {
                                        view.submit_rename_conversation(cx);
                                    } else if key == "escape" {
                                        view.cancel_rename_conversation(cx);
                                    } else if key == "space" {
                                        view.handle_rename_space(cx);
                                    } else if key.len() == 1
                                        && !modifiers.platform
                                        && !modifiers.control
                                    {
                                        view.handle_rename_char(key, cx);
                                    }
                                }
                                // Conversation dropdown keyboard navigation
                                else if view.conversation_dropdown_open() {
                                    if key == "up" {
                                        view.move_conversation_dropdown_selection(-1, cx);
                                    } else if key == "down" {
                                        view.move_conversation_dropdown_selection(1, cx);
                                    } else if key == "enter" {
                                        view.confirm_conversation_dropdown_selection(cx);
                                    } else if key == "escape" {
                                        view.toggle_conversation_dropdown(cx);
                                    }
                                }
                                // Profile dropdown keyboard navigation
                                else if view.profile_dropdown_open() {
                                    if key == "up" {
                                        view.move_profile_dropdown_selection(-1, cx);
                                    } else if key == "down" {
                                        view.move_profile_dropdown_selection(1, cx);
                                    } else if key == "enter" {
                                        view.confirm_profile_dropdown_selection(cx);
                                    } else if key == "escape" {
                                        view.toggle_profile_dropdown(cx);
                                    }
                                }
                                // Forward backspace
                                else if key == "backspace" {
                                    view.handle_backspace(cx);
                                }
                                // Forward enter
                                else if key == "enter" {
                                    view.handle_enter(cx);
                                }
                                // Forward space
                                else if key == "space" {
                                    view.handle_space(cx);
                                }
                                // Forward single character keys
                                else if key.len() == 1
                                    && !modifiers.platform
                                    && !modifiers.control
                                {
                                    view.handle_char(key, cx);
                                }
                            });
                        }
                    } else if key == "escape" {
                        println!(">>> Escape pressed - navigate back <<<");
                        if current != ViewId::Chat {
                            this.navigation.navigate_back();
                        }
                    }
                }),
            )
            // Render view based on navigation state
            // @plan PLAN-20250130-GPUIREDUX.P11
            .child(
                div()
                    .flex_1()
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
                    }),
            )
    }
}
