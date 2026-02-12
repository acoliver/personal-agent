//! History view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P05
//! @requirement REQ-UI-HS

use gpui::{div, px, prelude::*, IntoElement, MouseButton, ParentElement, Styled, FocusHandle, FontWeight};
use std::sync::Arc;
use uuid::Uuid;

use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;

/// Represents a conversation in the history list
/// @plan PLAN-20250130-GPUIREDUX.P05
#[derive(Clone, Debug, PartialEq)]
pub struct ConversationItem {
    pub id: Uuid,
    pub title: String,
    pub date_display: String,
    pub message_count: usize,
}

impl ConversationItem {
    pub fn new(id: Uuid, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
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
}

/// History view state
/// @plan PLAN-20250130-GPUIREDUX.P05
#[derive(Clone, Default)]
pub struct HistoryState {
    pub conversations: Vec<ConversationItem>,
}

impl HistoryState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_conversations(mut self, conversations: Vec<ConversationItem>) -> Self {
        self.conversations = conversations;
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

    /// Set conversations from presenter
    pub fn set_conversations(&mut self, conversations: Vec<ConversationItem>) {
        self.state.conversations = conversations;
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

    /// Handle ViewCommand from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P05
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::ConversationCleared => {
                // A conversation was deleted, refresh needed
                self.emit(UserEvent::RefreshHistory);
                cx.notify();
            }
            ViewCommand::ConversationRenamed { id, new_title } => {
                // Update title in local state
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
            // Back button - uses navigation_channel for direct navigation
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
                    .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                        tracing::info!("Back clicked - navigating to Chat");
                        crate::ui_gpui::navigation_channel().request_navigate(
                            crate::presentation::view_command::ViewId::Chat
                        );
                    }))
            )
            // Title
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("History")
            )
    }

    /// Render a single conversation card
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn render_conversation_card(&self, conv: &ConversationItem, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
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

        div()
            .id(gpui::SharedString::from(format!("conv-{}", conv_id)))
            .w_full()
            .p(px(12.0))
            .rounded(px(8.0))
            .bg(Theme::bg_darker())
            .flex()
            .flex_col()
            .gap(px(4.0))
            // Title row
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(title)
            )
            // Date and message count row
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_secondary())
                    .child(format!("{} • {}", date, msg_text))
            )
            // Button row
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .pt(px(4.0))
                    // Load button - select conversation and navigate to chat
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
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, _cx| {
                                tracing::info!("Load clicked for conversation: {}", conv_id);
                                this.emit(UserEvent::SelectConversation { id: conv_id });
                                // Navigate to chat view after selecting
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::Chat
                                );
                            }))
                    )
                    // Delete button
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
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, _cx| {
                                tracing::info!("Delete clicked for conversation: {}", conv_id);
                                this.emit(UserEvent::DeleteConversation { id: conv_id });
                            }))
                    )
            )
            .into_any_element()
    }

    /// Render the conversation list or empty state
    /// @plan PLAN-20250130-GPUIREDUX.P05
    fn render_conversation_list(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let conversations = &self.state.conversations;

        div()
            .id("history-list")
            .flex_1()
            .w_full()
            .bg(Theme::bg_darkest())
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .overflow_y_scroll()
            .when(conversations.is_empty(), |d| {
                d.items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(Theme::text_secondary())
                                    .child("No saved conversations")
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::text_muted())
                                    .child("Start chatting to create history")
                            )
                    )
            })
            .when(!conversations.is_empty(), |d| {
                d.children(
                    conversations.iter().map(|conv| {
                        self.render_conversation_card(conv, cx)
                    })
                )
            })
    }
}

impl gpui::Focusable for HistoryView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for HistoryView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("history-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|_this, event: &gpui::KeyDownEvent, _window, _cx| {
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;
                
                // Escape or Cmd+W: Go back to Chat
                if key == "escape" || (modifiers.platform && key == "w") {
                    println!(">>> Escape/Cmd+W pressed - navigating to Chat <<<");
                    crate::ui_gpui::navigation_channel().request_navigate(
                        crate::presentation::view_command::ViewId::Chat
                    );
                }
            }))
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Conversation list (flex)
            .child(self.render_conversation_list(cx))
    }
}
