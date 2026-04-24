//! Sidebar rendering for popout mode.
//!
//! Thin wrapper over the shared `ConversationListView` rendered in
//! `Inline` mode. The chat view owns an embedded entity (see
//! `ChatView::conversation_list`); state is mirrored from `ChatState`
//! via `sync_conversation_list_state` before each render so the shared
//! component sees the current conversations, selection, streaming
//! indicators, search query/results, inline rename buffer, and
//! delete-confirmation guard.
//!
//! All conversation list rendering lives in
//! `crate::ui_gpui::views::conversation_list::render`.
//!
//! @plan PLAN-20260420-ISSUE180.P03
//! @requirement REQ-180-001

use super::ChatView;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px};

/// Fixed popout sidebar width. The shared component does NOT set its own
/// width — width is container-controlled here.
///
/// @plan PLAN-20260420-ISSUE180.P03
const SIDEBAR_WIDTH: f32 = 260.0;

impl ChatView {
    /// Render the popout sidebar (~260px) by wrapping the shared
    /// `ConversationListView` in an Inline-mode container.
    pub(super) fn render_sidebar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        // Mirror current ChatState into the embedded list before rendering so
        // the shared component reflects every recent edit (search typing,
        // inline rename buffer changes, delete-confirm toggles, etc.).
        self.sync_conversation_list_state(cx);

        div()
            .id("sidebar")
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .h_full()
            .bg(Theme::bg_darker())
            .border_r_1()
            .border_color(Theme::border())
            .child(self.conversation_list.clone())
    }
}
