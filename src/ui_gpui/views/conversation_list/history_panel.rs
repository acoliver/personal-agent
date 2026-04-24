//! Popin History panel container.
//!
//! Wraps a `ConversationListView` in `FullPanel` mode and renders a Back
//! button bar at the bottom that returns to the Chat view.
//!
//! @plan PLAN-20260420-ISSUE180.P03
//! @requirement REQ-180-001

use std::sync::Arc;

use gpui::{div, prelude::*, px, Entity, FontWeight, MouseButton};

use super::{ConversationListMode, ConversationListView};
use crate::presentation::view_command::ViewId;
use crate::ui_gpui::app_store::HistoryStoreSnapshot;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::main_panel::MainPanelAppState;

/// Container view used when History is opened as a popin (full panel).
pub struct HistoryPanelView {
    list: Entity<ConversationListView>,
}

impl HistoryPanelView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        let list =
            cx.new(|child_cx| ConversationListView::new(ConversationListMode::FullPanel, child_cx));
        Self { list }
    }

    /// Inject the bridge into the embedded list view.
    pub fn set_bridge(&self, bridge: Arc<GpuiBridge>, cx: &mut gpui::Context<Self>) {
        self.list.update(cx, |list, _cx| {
            list.set_bridge(bridge);
        });
    }

    /// Forward a history snapshot to the embedded list view.
    pub fn apply_store_snapshot(
        &self,
        snapshot: &HistoryStoreSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        self.list.update(cx, |list, list_cx| {
            list.apply_store_snapshot(snapshot, list_cx);
        });
    }

    /// Expose the embedded list entity (used for tests and command routing).
    #[must_use]
    pub const fn list_entity(&self) -> &Entity<ConversationListView> {
        &self.list
    }

    /// Apply backend-supplied search results to the embedded list.
    pub fn apply_search_results(
        &self,
        results: Vec<crate::presentation::view_command::ConversationSearchResult>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.list.update(cx, |list, list_cx| {
            list.apply_search_results(results, list_cx);
        });
    }

    /// Read the current list of conversations from the embedded list view.
    /// Used by debug/test helpers that need to inspect the conversation list.
    pub fn conversation_summaries(
        &self,
        cx: &gpui::App,
    ) -> Vec<crate::presentation::view_command::ConversationSummary> {
        self.list.read(cx).state.conversations.clone()
    }

    fn render_top_bar(cx: &gpui::Context<Self>) -> impl IntoElement {
        let is_popout = cx
            .try_global::<MainPanelAppState>()
            .is_some_and(|s| s.app_mode == crate::presentation::view_command::AppMode::Popout);

        div()
            .id("history-panel-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .pr(px(12.0))
            .pl(px(if is_popout { 72.0 } else { 12.0 }))
            .flex()
            .items_center()
            .child(
                div()
                    .text_size(px(Theme::font_size_body()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("History"),
            )
    }

    fn render_bottom_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("history-panel-bottom-bar")
            .h(px(36.0))
            .w_full()
            .flex_shrink_0()
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .child(
                div()
                    .id("btn-back")
                    .h(px(28.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_body()))
                    .text_color(Theme::text_secondary())
                    .child("\u{2039} Back")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("History panel Back clicked - navigating to Chat");
                            crate::ui_gpui::navigation_channel().request_navigate(ViewId::Chat);
                        }),
                    ),
            )
    }
}

impl gpui::Focusable for HistoryPanelView {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.list.read(cx).focus_handle(cx)
    }
}

impl gpui::Render for HistoryPanelView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("history-panel")
            .size_full()
            .bg(Theme::bg_dark())
            .flex()
            .flex_col()
            .child(Self::render_top_bar(cx))
            .child(
                div()
                    .id("history-panel-list-container")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(self.list.clone()),
            )
            .child(Self::render_bottom_bar(cx))
    }
}
