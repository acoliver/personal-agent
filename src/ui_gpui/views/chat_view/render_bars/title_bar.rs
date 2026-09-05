//! Title-bar internals of the chat view: the conversation and profile
//! selectors, the new-conversation button, and the bug-report button.

use super::super::state::StreamingState;
use super::ChatView;
use crate::events::types::UserEvent;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, FontWeight, MouseButton};

impl ChatView {
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    pub(super) fn render_bug_icon_btn(
        &self,
        unviewed: usize,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let count_label = if unviewed > 99 {
            "99+".to_string()
        } else {
            unviewed.to_string()
        };

        div()
            .id("btn-error-log")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .relative()
            // Preserve layout when no errors (like CSS visibility:hidden)
            .opacity(if unviewed == 0 { 0.0 } else { 1.0 })
            .when(unviewed > 0, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            crate::ui_gpui::navigation_channel().request_navigate(
                                crate::presentation::view_command::ViewId::ErrorLog,
                            );
                        }),
                    )
            })
            .child(crate::ui_gpui::components::bug_icon::bug_icon(14.0).text_color(Theme::error()))
            // Count badge — top-right corner, styled like the YOLO badge
            .when(unviewed > 0, |d| {
                d.child(
                    div()
                        .id("error-log-badge")
                        .absolute()
                        .top(px(1.0))
                        .right(px(1.0))
                        .min_w(px(13.0))
                        .h(px(13.0))
                        .rounded(px(7.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .px(px(2.0))
                        .bg(Theme::error())
                        .text_size(px(Theme::font_size_small()))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::selection_fg())
                        .child(count_label),
                )
            })
    }

    /// Conversation title / rename field.
    pub(super) fn render_conversation_selector(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        if self.state.conversation_title_editing {
            let input = self.state.conversation_title_input.clone();
            div()
                .id("conversation-title-input")
                .min_w(px(220.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(Theme::bg_dark())
                .border_1()
                .border_color(Theme::accent())
                .child(
                    div()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_primary())
                        .child(if input.is_empty() {
                            "Enter conversation name".to_string()
                        } else {
                            input
                        }),
                )
        } else {
            let title = self.state.conversation_title.clone();
            let open = self.state.conversation_dropdown_open;
            div()
                .id("conversation-dropdown")
                .min_w(px(220.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(Theme::bg_dark())
                .border_1()
                .border_color(if open {
                    Theme::accent()
                } else {
                    Theme::border()
                })
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::text_primary())
                        .child(if open { "\u{25B2}" } else { "\u{25BC}" }),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.toggle_conversation_dropdown(cx);
                        this.blur_composer();
                    }),
                )
        }
    }

    /// "+" new conversation button.
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    pub(super) fn render_new_conversation_btn(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("btn-new")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_primary())
            .child("+")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    tracing::info!("New conversation clicked - emitting UserEvent");
                    this.emit(UserEvent::NewConversation);
                    this.state.messages.clear();
                    this.state.input_text.clear();
                    this.state.cursor_position = 0;
                    this.state.streaming = StreamingState::Idle;
                    this.state.thinking_content = None;
                    this.state.active_conversation_id = None;
                    this.conversation_id = None;
                    this.state.conversation_title = "New Conversation".to_string();
                    this.state.conversation_dropdown_open = false;
                    this.state.conversation_title_editing = false;
                    this.state.conversation_title_input.clear();
                    this.state.profile_dropdown_open = false;
                    this.state.chat_autoscroll_enabled = true;
                    this.scroll_transcript_to_bottom();
                    this.refresh_transcript_selection_revisions();
                    cx.notify();
                }),
            )
    }

    /// Profile selector pill in the title bar.
    pub(super) fn render_profile_selector(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let selected_profile = self.state.selected_profile().map_or_else(
            || "Select profile".to_string(),
            |profile| profile.name.clone(),
        );
        let open = self.state.profile_dropdown_open;

        div()
            .id("chat-profile-dropdown")
            .max_w(px(225.0 * Theme::ui_scale()))
            .min_w(px(100.0))
            .px(px(Theme::spacing_sm_scaled()))
            .py(px(Theme::spacing_xs_scaled()))
            .rounded(px(4.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(if open {
                Theme::accent()
            } else {
                Theme::border()
            })
            .cursor_pointer()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis_start()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child(selected_profile),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_secondary())
                            .child(if open { "\u{25B2}" } else { "\u{25BC}" }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.blur_composer();

                    this.toggle_profile_dropdown(cx);
                }),
            )
    }
}
