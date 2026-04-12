//! History view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P05
//! @requirement REQ-UI-HS

use gpui::{
    div, prelude::*, px, FocusHandle, FontWeight, IntoElement, MouseButton, ParentElement,
    ScrollHandle, Styled,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::app_store::HistoryStoreSnapshot;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::selection_intent_channel;
use crate::ui_gpui::theme::Theme;

/// Represents a conversation in the history list
/// @plan PLAN-20250130-GPUIREDUX.P05
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationItem {
    pub id: Uuid,
    pub title: String,
    pub is_selected: bool,
    pub date_display: String,
    pub message_count: usize,
}

impl ConversationItem {
    pub fn new(id: Uuid, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            is_selected: false,
            date_display: "Just now".to_string(),
            message_count: 0,
        }
    }

    #[must_use]
    pub fn with_date(mut self, date: impl Into<String>) -> Self {
        self.date_display = date.into();
        self
    }

    #[must_use]
    pub const fn with_message_count(mut self, count: usize) -> Self {
        self.message_count = count;
        self
    }

    #[must_use]
    pub const fn with_selected(mut self, is_selected: bool) -> Self {
        self.is_selected = is_selected;
        self
    }
}

/// History view state
/// @plan PLAN-20250130-GPUIREDUX.P05
#[derive(Clone, Default)]
pub struct HistoryState {
    pub conversations: Vec<ConversationItem>,
    pub selected_conversation_id: Option<Uuid>,
}

impl HistoryState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_conversations(mut self, conversations: Vec<ConversationItem>) -> Self {
        self.conversations = conversations;
        self
    }

    #[must_use]
    pub const fn with_selected_conversation_id(
        mut self,
        selected_conversation_id: Option<Uuid>,
    ) -> Self {
        self.selected_conversation_id = selected_conversation_id;
        self
    }
}

/// History view component
/// @plan PLAN-20250130-GPUIREDUX.P05
pub struct HistoryView {
    state: HistoryState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
}

impl HistoryView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: HistoryState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// Set the event bridge for cross-runtime communication
    /// @plan PLAN-20250130-GPUIREDUX.P05
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P04
    /// @requirement REQ-ARCH-001.1
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:022-035
    pub fn apply_store_snapshot(
        &mut self,
        snapshot: &HistoryStoreSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state = HistoryState::new()
            .with_selected_conversation_id(snapshot.selected_conversation_id)
            .with_conversations(Self::items_from_snapshot(snapshot));
        cx.notify();
    }

    /// Set conversations from presenter
    pub fn set_conversations(&mut self, conversations: Vec<ConversationItem>) {
        self.state.conversations = conversations;
    }

    #[must_use]
    pub fn conversations(&self) -> &[ConversationItem] {
        &self.state.conversations
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    fn format_date(dt: chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(dt);

        if diff.num_minutes() < 1 {
            "Just now".to_string()
        } else if diff.num_hours() < 1 {
            format!("{}m ago", diff.num_minutes())
        } else if diff.num_days() < 1 {
            format!("{}h ago", diff.num_hours())
        } else {
            format!("{}d ago", diff.num_days())
        }
    }

    fn items_from_snapshot(snapshot: &HistoryStoreSnapshot) -> Vec<ConversationItem> {
        snapshot
            .conversations
            .iter()
            .map(|conversation| {
                let title = if conversation.title.trim().is_empty() {
                    "Untitled Conversation".to_string()
                } else {
                    conversation.title.clone()
                };

                ConversationItem::new(conversation.id, title)
                    .with_date(Self::format_date(conversation.updated_at))
                    .with_message_count(conversation.message_count)
                    .with_selected(Some(conversation.id) == snapshot.selected_conversation_id)
            })
            .collect()
    }

    /// Handle `ViewCommand` from presenter.
    ///
    /// Store-managed commands ([`ConversationListRefreshed`], [`ConversationActivated`],
    /// [`ConversationCreated`], [`ConversationDeleted`], [`ConversationRenamed`]) are handled
    /// exclusively via [`apply_store_snapshot`](Self::apply_store_snapshot).
    /// This dispatch is reserved for commands the store does NOT own:
    ///
    /// - [`ConversationCleared`] — emits [`RefreshHistory`](UserEvent::RefreshHistory) side-effect.
    pub fn handle_command(&mut self, command: &ViewCommand, cx: &mut gpui::Context<Self>) {
        if command == &ViewCommand::ConversationCleared {
            self.emit(&UserEvent::RefreshHistory);
            cx.notify();
        }
        // All other commands are store-managed and arrive via apply_store_snapshot
    }

    /// Render the top bar with back button and title
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_popout = cx
            .try_global::<crate::ui_gpui::views::main_panel::MainPanelAppState>()
            .is_some_and(|s| s.app_mode == crate::presentation::view_command::AppMode::Popout);

        div()
            .id("history-top-bar")
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
            .id("history-bottom-bar")
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
                            tracing::info!("Back clicked - navigating to Chat");
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(crate::presentation::view_command::ViewId::Chat);
                        }),
                    ),
            )
    }

    /// Render the action buttons bar for a conversation card
    fn render_card_actions(conv_id: uuid::Uuid, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_end()
            .gap(px(8.0))
            .pt(px(4.0))
            .child(
                div()
                    .id(gpui::SharedString::from(format!("load-{conv_id}")))
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .bg(Theme::bg_dark())
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::accent()).text_color(Theme::accent_fg()))
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child("Load")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_this, _, _window, _cx| {
                            tracing::info!("Load clicked for conversation: {}", conv_id);
                            selection_intent_channel().request_select(conv_id);
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(crate::presentation::view_command::ViewId::Chat);
                        }),
                    ),
            )
            .child(
                div()
                    .id(gpui::SharedString::from(format!("delete-{conv_id}")))
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .bg(Theme::bg_dark())
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::danger()))
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child("Delete")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, _cx| {
                            tracing::info!("Delete clicked for conversation: {}", conv_id);
                            this.emit(&UserEvent::DeleteConversation { id: conv_id });
                        }),
                    ),
            )
    }

    /// Render a single conversation card
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn render_conversation_card(
        conv: &ConversationItem,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let conv_id = conv.id;
        let title = if conv.title.is_empty() {
            "Untitled Conversation".to_string()
        } else {
            conv.title.clone()
        };
        let date = conv.date_display.clone();
        let msg_count = conv.message_count;
        let msg_text = if msg_count == 1 {
            "1 message".to_string()
        } else {
            format!("{msg_count} messages")
        };
        let is_selected = conv.is_selected;

        let mut card = div()
            .id(gpui::SharedString::from(format!("conv-{conv_id}")))
            .w_full()
            .p(px(12.0))
            .rounded(px(8.0))
            .flex()
            .flex_col()
            .gap(px(4.0));

        if is_selected {
            card = card
                .bg(Theme::accent())
                .border_1()
                .border_color(Theme::border());
        } else {
            card = card.bg(Theme::bg_darker());
        }

        let title_color = if is_selected {
            Theme::bg_dark()
        } else {
            Theme::text_primary()
        };
        let subtitle_color = if is_selected {
            Theme::bg_darker()
        } else {
            Theme::text_secondary()
        };

        card.child(
            div()
                .text_size(px(Theme::font_size_mono()))
                .font_weight(FontWeight::BOLD)
                .text_color(title_color)
                .overflow_hidden()
                .text_ellipsis()
                .child(title),
        )
        .child(
            div()
                .text_size(px(Theme::font_size_ui()))
                .text_color(subtitle_color)
                .child(format!("{date} • {msg_text}")),
        )
        .child(Self::render_card_actions(conv_id, cx))
        .into_any_element()
    }
}

impl gpui::Focusable for HistoryView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for HistoryView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("history-view")
            .size_full()
            .bg(Theme::bg_dark())
            .flex()
            .flex_col()
            .child(Self::render_top_bar(cx))
            .child(
                div()
                    .id("history-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        div().p(px(12.0)).flex().flex_col().gap(px(8.0)).children(
                            self.state
                                .conversations
                                .iter()
                                .map(|conv| Self::render_conversation_card(conv, cx)),
                        ),
                    ),
            )
            .child(Self::render_bottom_bar(cx))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use chrono::{Duration, Utc};
    use flume;
    use gpui::{AppContext, TestAppContext};

    use crate::events::types::UserEvent;
    use crate::presentation::view_command::{ConversationSummary, ViewCommand};
    use crate::ui_gpui::app_store::HistoryStoreSnapshot;

    fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
    }

    fn conversation_summary(
        id: Uuid,
        title: &str,
        updated_at: chrono::DateTime<Utc>,
        message_count: usize,
    ) -> ConversationSummary {
        ConversationSummary {
            id,
            title: title.to_string(),
            updated_at,
            message_count,
            preview: None,
        }
    }

    #[gpui::test]
    async fn apply_store_snapshot_projects_titles_dates_counts_and_selection(
        cx: &mut TestAppContext,
    ) {
        let selected_id = Uuid::new_v4();
        let older_id = Uuid::new_v4();
        let snapshot = HistoryStoreSnapshot {
            conversations: vec![
                conversation_summary(older_id, "", Utc::now() - Duration::days(2), 9),
                conversation_summary(
                    selected_id,
                    "Selected conversation",
                    Utc::now() - Duration::minutes(5),
                    1,
                ),
            ],
            selected_conversation_id: Some(selected_id),
        };
        let view = cx.new(HistoryView::new);

        view.update(cx, |view: &mut HistoryView, cx| {
            view.apply_store_snapshot(&snapshot, cx);

            let conversations = view.conversations();
            assert_eq!(conversations.len(), 2);
            assert_eq!(conversations[0].id, older_id);
            assert_eq!(conversations[0].title, "Untitled Conversation");
            assert_eq!(conversations[0].date_display, "2d ago");
            assert_eq!(conversations[0].message_count, 9);
            assert!(!conversations[0].is_selected);

            assert_eq!(conversations[1].id, selected_id);
            assert_eq!(conversations[1].title, "Selected conversation");
            assert_eq!(conversations[1].date_display, "5m ago");
            assert_eq!(conversations[1].message_count, 1);
            assert!(conversations[1].is_selected);
        });
    }

    #[gpui::test]
    async fn snapshot_delivery_covers_list_refresh_and_activation(cx: &mut TestAppContext) {
        let older_id = Uuid::new_v4();
        let selected_id = Uuid::new_v4();
        let view = cx.new(HistoryView::new);

        view.update(cx, |view: &mut HistoryView, cx| {
            let snapshot = HistoryStoreSnapshot {
                conversations: vec![
                    conversation_summary(older_id, "", Utc::now() - Duration::hours(3), 4),
                    conversation_summary(
                        selected_id,
                        "Selected",
                        Utc::now() - Duration::minutes(2),
                        2,
                    ),
                ],
                selected_conversation_id: None,
            };
            view.apply_store_snapshot(&snapshot, cx);

            let conversations = view.conversations();
            assert_eq!(conversations.len(), 2);
            assert_eq!(conversations[0].title, "Untitled Conversation");
            assert_eq!(conversations[0].date_display, "3h ago");
            assert_eq!(conversations[1].title, "Selected");
            assert_eq!(conversations[1].date_display, "2m ago");
            assert!(conversations.iter().all(|c| !c.is_selected));

            // Simulate activation via updated snapshot with selection
            let snapshot_with_selection = HistoryStoreSnapshot {
                conversations: snapshot.conversations,
                selected_conversation_id: Some(selected_id),
            };
            view.apply_store_snapshot(&snapshot_with_selection, cx);
            assert_eq!(view.state.selected_conversation_id, Some(selected_id));
            assert!(view.conversations()[1].is_selected);
            assert!(!view.conversations()[0].is_selected);
        });
    }

    #[gpui::test]
    async fn snapshot_delivery_covers_create_rename_delete_and_cleared_emits_refresh(
        cx: &mut TestAppContext,
    ) {
        let selected_id = Uuid::new_v4();
        let created_id = Uuid::new_v4();
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(HistoryView::new);

        view.update(cx, |view: &mut HistoryView, cx| {
            view.set_bridge(Arc::clone(&bridge));

            // Initial state with one conversation
            let initial_snapshot = HistoryStoreSnapshot {
                conversations: vec![conversation_summary(selected_id, "Selected", Utc::now(), 0)],
                selected_conversation_id: Some(selected_id),
            };
            view.apply_store_snapshot(&initial_snapshot, cx);
            assert_eq!(view.conversations().len(), 1);
            assert_eq!(view.conversations()[0].title, "Selected");

            // New conversation created: store snapshot now includes it
            let created_snapshot = HistoryStoreSnapshot {
                conversations: vec![
                    conversation_summary(created_id, "New Conversation", Utc::now(), 0),
                    conversation_summary(selected_id, "Selected", Utc::now(), 0),
                ],
                selected_conversation_id: Some(created_id),
            };
            view.apply_store_snapshot(&created_snapshot, cx);
            assert_eq!(view.conversations()[0].id, created_id);
            assert_eq!(view.conversations()[0].title, "New Conversation");
            assert_eq!(view.conversations()[0].message_count, 0);

            // Renamed conversation: store snapshot reflects the new title
            let renamed_snapshot = HistoryStoreSnapshot {
                conversations: vec![
                    conversation_summary(created_id, "Renamed conversation", Utc::now(), 0),
                    conversation_summary(selected_id, "Selected", Utc::now(), 0),
                ],
                selected_conversation_id: Some(created_id),
            };
            view.apply_store_snapshot(&renamed_snapshot, cx);
            assert_eq!(view.conversations()[0].title, "Renamed conversation");

            // Deleted conversation: store snapshot removes it
            let deleted_snapshot = HistoryStoreSnapshot {
                conversations: vec![conversation_summary(
                    created_id,
                    "Renamed conversation",
                    Utc::now(),
                    0,
                )],
                selected_conversation_id: Some(created_id),
            };
            view.apply_store_snapshot(&deleted_snapshot, cx);
            assert_eq!(view.state.selected_conversation_id, Some(created_id));
            assert!(view.conversations()[0].is_selected);
            assert!(view.conversations().iter().all(|c| c.id != selected_id));

            // ConversationCleared is the one command still handled directly
            view.handle_command(&ViewCommand::ConversationCleared, cx);
        });

        assert_eq!(
            user_rx.recv().expect("refresh history event"),
            UserEvent::RefreshHistory
        );
        assert!(
            user_rx.try_recv().is_err(),
            "history view should emit only the explicit refresh request in this scenario"
        );
    }
}
