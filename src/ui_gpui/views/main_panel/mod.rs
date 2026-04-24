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
    NewConversation, ToggleSidebar, ToggleWindowMode, ZoomIn, ZoomOut, ZoomReset,
};
pub use startup::MainPanelAppState;

use crate::presentation::view_command::ViewId;
use crate::ui_gpui::navigation::NavigationState;

use gpui::{prelude::*, Entity, FocusHandle, Subscription, Task};

use crate::ui_gpui::views::api_key_manager_view::ApiKeyManagerView;
use crate::ui_gpui::views::chat_view::{ChatState, ChatView};
use crate::ui_gpui::views::conversation_list::HistoryPanelView;
use crate::ui_gpui::views::error_log_view::ErrorLogView;
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
    pub(super) history_panel: Option<Entity<HistoryPanelView>>,
    pub(super) settings_view: Option<Entity<SettingsView>>,
    pub(super) model_selector_view: Option<Entity<ModelSelectorView>>,
    pub(super) profile_editor_view: Option<Entity<ProfileEditorView>>,
    pub(super) mcp_add_view: Option<Entity<McpAddView>>,
    pub(super) mcp_configure_view: Option<Entity<McpConfigureView>>,
    pub(super) api_key_manager_view: Option<Entity<ApiKeyManagerView>>,
    pub(super) error_log_view: Option<Entity<ErrorLogView>>,
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
            history_panel: None,
            settings_view: None,
            model_selector_view: None,
            profile_editor_view: None,
            mcp_add_view: None,
            mcp_configure_view: None,
            api_key_manager_view: None,
            error_log_view: None,
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
            // @plan PLAN-20260416-ISSUE173.P11
            // @requirement REQ-173-004.3
            let streaming_ids = snapshot.history.streaming_conversation_ids.clone();
            chat_view.update(cx, |view, cx| {
                view.apply_store_snapshot(chat_snapshot, cx);
                view.apply_settings_snapshot(settings_snapshot);
                // @plan PLAN-20260416-ISSUE173.P11
                // @requirement REQ-173-004.3
                view.state.streaming_conversation_ids = streaming_ids;
            });
        }

        if let Some(ref history_panel) = self.history_panel {
            let history_snapshot = snapshot.history;
            history_panel.update(cx, |view, cx| {
                view.apply_store_snapshot(&history_snapshot, cx);
            });
        }

        if let Some(ref settings_view) = self.settings_view {
            let settings = snapshot.settings;
            settings_view.update(cx, |view, _cx| {
                view.apply_profile_summaries(settings.profiles, settings.selected_profile_id);
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
                view.set_bridge_with_cx(b.clone(), cx);
            }
            view
        }));
        // History panel (popin) wraps the shared ConversationListView in FullPanel mode.
        // @plan PLAN-20260420-ISSUE180.P03
        // @requirement REQ-180-001
        self.history_panel = Some(cx.new(|cx: &mut gpui::Context<HistoryPanelView>| {
            let view = HistoryPanelView::new(cx);
            if let Some(ref b) = bridge {
                view.set_bridge(b.clone(), cx);
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

        // Error Log view
        self.error_log_view = Some(cx.new(|cx: &mut gpui::Context<ErrorLogView>| {
            let mut view = ErrorLogView::new(cx);
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
            && self.history_panel.is_some()
            && self.settings_view.is_some()
            && self.model_selector_view.is_some()
            && self.profile_editor_view.is_some()
            && self.mcp_add_view.is_some()
            && self.mcp_configure_view.is_some()
            && self.api_key_manager_view.is_some()
            && self.error_log_view.is_some()
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
mod tests;
