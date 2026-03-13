//! History view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P05
//! @requirement REQ-UI-HS

use gpui::{
    div, prelude::*, px, FocusHandle, FontWeight, IntoElement, MouseButton, ParentElement, Styled,
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
#[derive(Clone, Debug, PartialEq)]
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

    pub fn with_date(mut self, date: impl Into<String>) -> Self {
        self.date_display = date.into();
        self
    }

    pub fn with_message_count(mut self, count: usize) -> Self {
        self.message_count = count;
        self
    }

    pub fn with_selected(mut self, is_selected: bool) -> Self {
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_conversations(mut self, conversations: Vec<ConversationItem>) -> Self {
        self.conversations = conversations;
        self
    }

    pub fn with_selected_conversation_id(mut self, selected_conversation_id: Option<Uuid>) -> Self {
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
}

impl HistoryView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: HistoryState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
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
        snapshot: HistoryStoreSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state = HistoryState::new()
            .with_selected_conversation_id(snapshot.selected_conversation_id)
            .with_conversations(Self::items_from_snapshot(&snapshot));
        cx.notify();
    }

    /// Set conversations from presenter
    pub fn set_conversations(&mut self, conversations: Vec<ConversationItem>) {
        self.state.conversations = conversations;
    }

    pub fn conversations(&self) -> &[ConversationItem] {
        &self.state.conversations
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn emit(&self, event: UserEvent) {
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

    fn refresh_selection_flags(&mut self) {
        let selected_conversation_id = self.state.selected_conversation_id;
        for conversation in &mut self.state.conversations {
            conversation.is_selected = Some(conversation.id) == selected_conversation_id;
        }
    }

    /// Handle ViewCommand from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P05
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::ConversationListRefreshed { conversations } => {
                let selected_conversation_id = self.state.selected_conversation_id;
                self.state.conversations = conversations
                    .into_iter()
                    .map(|conversation| {
                        let title = if conversation.title.trim().is_empty() {
                            "Untitled Conversation".to_string()
                        } else {
                            conversation.title
                        };

                        ConversationItem::new(conversation.id, title)
                            .with_date(Self::format_date(conversation.updated_at))
                            .with_message_count(conversation.message_count)
                            .with_selected(Some(conversation.id) == selected_conversation_id)
                    })
                    .collect();
                cx.notify();
            }
            ViewCommand::ConversationActivated {
                id,
                selection_generation: _,
            } => {
                self.state.selected_conversation_id = Some(id);
                self.refresh_selection_flags();
                cx.notify();
            }
            ViewCommand::ConversationCreated { id, .. } => {
                if !self
                    .state
                    .conversations
                    .iter()
                    .any(|conversation| conversation.id == id)
                {
                    self.state.conversations.insert(
                        0,
                        ConversationItem::new(id, "New Conversation")
                            .with_date("Just now")
                            .with_message_count(0)
                            .with_selected(Some(id) == self.state.selected_conversation_id),
                    );
                }
                cx.notify();
            }
            ViewCommand::ConversationCleared => {
                // A conversation was deleted, refresh needed
                self.emit(UserEvent::RefreshHistory);
                cx.notify();
            }
            ViewCommand::ConversationRenamed { id, new_title } => {
                if let Some(conv) = self.state.conversations.iter_mut().find(|c| c.id == id) {
                    conv.title = new_title;
                }
                cx.notify();
            }
            _ => {
                // Other commands not relevant to history view
            }
        }
    }

    /// Render the top bar with back button and title
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("history-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("btn-back")
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(14.0))
                    .text_color(Theme::text_secondary())
                    .child("<")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("Back clicked - navigating to Chat");
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(crate::presentation::view_command::ViewId::Chat);
                        }),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("History"),
            )
    }

    /// Render a single conversation card
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn render_conversation_card(
        &self,
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
            format!("{} messages", msg_count)
        };

        let mut card = div()
            .id(gpui::SharedString::from(format!("conv-{}", conv_id)))
            .w_full()
            .p(px(12.0))
            .rounded(px(8.0))
            .flex()
            .flex_col()
            .gap(px(4.0));

        if conv.is_selected {
            card = card
                .bg(Theme::accent())
                .border_1()
                .border_color(Theme::border());
        } else {
            card = card.bg(Theme::bg_darker());
        }

        card.child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_primary())
                .overflow_hidden()
                .text_ellipsis()
                .child(title),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(Theme::text_secondary())
                .child(format!("{} • {}", date, msg_text)),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.0))
                .pt(px(4.0))
                .child(
                    div()
                        .id(gpui::SharedString::from(format!("load-{}", conv_id)))
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded(px(6.0))
                        .bg(Theme::bg_dark())
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::accent()))
                        .text_size(px(12.0))
                        .text_color(Theme::text_primary())
                        .child("Load")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_this, _, _window, _cx| {
                                tracing::info!("Load clicked for conversation: {}", conv_id);
                                selection_intent_channel().request_select(conv_id);
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::Chat,
                                );
                            }),
                        ),
                )
                .child(
                    div()
                        .id(gpui::SharedString::from(format!("delete-{}", conv_id)))
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded(px(6.0))
                        .bg(Theme::bg_dark())
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::danger()))
                        .text_size(px(12.0))
                        .text_color(Theme::text_primary())
                        .child("Delete")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _window, _cx| {
                                tracing::info!("Delete clicked for conversation: {}", conv_id);
                                this.emit(UserEvent::DeleteConversation { id: conv_id });
                            }),
                        ),
                ),
        )
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
            .child(self.render_top_bar(cx))
            .child(
                div().flex_1().p(px(12.0)).children(
                    self.state
                        .conversations
                        .iter()
                        .map(|conv| self.render_conversation_card(conv, cx)),
                ),
            )
    }
}
