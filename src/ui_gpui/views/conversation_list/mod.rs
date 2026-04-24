//! Shared `ConversationListView` used by the popout sidebar (Inline mode)
//! and the popin History panel (`FullPanel` mode).
//!
//! - `Inline` mode: row click selects the conversation only.
//! - `FullPanel` mode: row click selects AND requests a navigation back to
//!   the Chat view.
//!
//! Width and outer layout are container-controlled. The shared component
//! never sets its own width.
//!
//! @plan PLAN-20260420-ISSUE180
//! @requirement REQ-180-001

mod history_panel;
mod render;
pub mod state;

use std::sync::Arc;

use gpui::{FocusHandle, ScrollHandle};
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewId;
use crate::ui_gpui::app_store::HistoryStoreSnapshot;
use crate::ui_gpui::bridge::GpuiBridge;

pub use history_panel::HistoryPanelView;
pub use state::ConversationListState;

/// Container mode for the shared list. Controls click semantics only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationListMode {
    /// Embedded in the chat sidebar — click selects only.
    Inline,
    /// Embedded in the popin History panel — click selects + navigates to Chat.
    FullPanel,
}

/// Shared conversation list view.
pub struct ConversationListView {
    pub(super) mode: ConversationListMode,
    pub state: ConversationListState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) scroll_handle: ScrollHandle,
}

impl ConversationListView {
    pub fn new(mode: ConversationListMode, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            mode,
            state: ConversationListState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// Set the event bridge for cross-runtime communication.
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    #[must_use]
    pub const fn mode(&self) -> ConversationListMode {
        self.mode
    }

    /// Apply a history store snapshot to the embedded state.
    pub fn apply_store_snapshot(
        &mut self,
        snapshot: &HistoryStoreSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state.apply_history_snapshot(snapshot);
        cx.notify();
    }

    /// Currently selected conversation id, if any.
    #[must_use]
    pub const fn active_conversation_id(&self) -> Option<Uuid> {
        self.state.active_conversation_id
    }

    /// Set the selected conversation id (used when `ChatView` needs to mirror
    /// state ahead of a snapshot, e.g. during selection intent).
    pub fn set_active_conversation_id(&mut self, id: Option<Uuid>, cx: &mut gpui::Context<Self>) {
        self.state.active_conversation_id = id;
        cx.notify();
    }

    /// Emit a `UserEvent` through the bridge.
    pub(super) fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Apply selection-after-click semantics common to both modes:
    /// always request selection, additionally navigate to Chat in `FullPanel`.
    pub(super) fn handle_row_click(&self, conversation_id: Uuid) {
        crate::ui_gpui::selection_intent_channel().request_select(conversation_id);
        if matches!(self.mode, ConversationListMode::FullPanel) {
            crate::ui_gpui::navigation_channel().request_navigate(ViewId::Chat);
        }
    }

    // ── Inline rename flow ───────────────────────────────────────────

    pub fn start_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(id) = self.state.active_conversation_id else {
            return;
        };
        let title = self
            .state
            .conversations
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.title.clone())
            .unwrap_or_default();
        self.state.conversation_title_editing = true;
        self.state.conversation_title_input = title;
        self.state.rename_replace_on_next_char = true;
        cx.notify();
    }

    pub fn submit_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        let Some(id) = self.state.active_conversation_id else {
            self.cancel_rename_conversation(cx);
            return;
        };
        let title = self.state.conversation_title_input.trim().to_string();
        if title.is_empty() {
            self.cancel_rename_conversation(cx);
            return;
        }
        if let Some(conversation) = self.state.conversations.iter_mut().find(|c| c.id == id) {
            conversation.title.clone_from(&title);
        }
        self.state.conversation_title_editing = false;
        self.state.conversation_title_input.clear();
        self.state.rename_replace_on_next_char = false;
        self.emit(&UserEvent::ConfirmRenameConversation { id, title });
        cx.notify();
    }

    pub fn cancel_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        self.state.conversation_title_editing = false;
        self.state.conversation_title_input.clear();
        self.state.rename_replace_on_next_char = false;
        self.emit(&UserEvent::CancelRenameConversation);
        cx.notify();
    }

    pub fn handle_rename_backspace(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        if self.state.rename_replace_on_next_char {
            self.state.conversation_title_input.clear();
            self.state.rename_replace_on_next_char = false;
        } else {
            self.state.conversation_title_input.pop();
        }
        cx.notify();
    }

    // ── Search ───────────────────────────────────────────────────────

    /// Emit a search event for the current sidebar search query.
    pub fn trigger_sidebar_search(&mut self, cx: &mut gpui::Context<Self>) {
        let query = self.state.sidebar_search_query.clone();
        if query.trim().is_empty() {
            self.state.sidebar_search_results = None;
        } else {
            self.emit(&UserEvent::SearchConversations { query });
        }
        cx.notify();
    }

    /// Reset all sidebar search state (clears focus, query, results).
    pub fn clear_search(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.clear_search();
        cx.notify();
    }

    /// Apply backend-supplied search results to the state.
    pub fn apply_search_results(
        &mut self,
        results: Vec<crate::presentation::view_command::ConversationSearchResult>,
        cx: &mut gpui::Context<Self>,
    ) {
        if results.is_empty() && self.state.sidebar_search_query.is_empty() {
            self.state.sidebar_search_results = None;
        } else {
            self.state.sidebar_search_results = Some(results);
        }
        cx.notify();
    }
}

impl gpui::Focusable for ConversationListView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
