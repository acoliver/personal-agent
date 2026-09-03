//! The composer row: the text field and the controls beside it.
//!
//! While a turn runs the row carries two controls, not one. Stop ends the
//! turn; Send joins it. They are separate elements so neither can hide the
//! other, and the whole row lives here rather than in `render.rs` because
//! that file is at the length CI rejects.
//!
//! @plan PLAN-20260903-ISSUE222.P04
//! @requirement REQ-222-001

use super::state::StreamingState;
use super::ChatView;
use crate::events::types::UserEvent;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton};

impl ChatView {
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_input_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_streaming = matches!(self.state.streaming, StreamingState::Streaming { .. });
        let input_text = self.state.input_text.clone();
        let has_text = !input_text.trim().is_empty();
        let focus_handle = self.focus_handle.clone();

        let wrapped_line_count = if input_text.is_empty() {
            1
        } else {
            input_text
                .split('\n')
                .map(|line| {
                    let len = line.chars().count();
                    if len == 0 {
                        1
                    } else {
                        let approx_chars_per_line = 65usize;
                        len.div_ceil(approx_chars_per_line)
                    }
                })
                .sum::<usize>()
                .max(1)
        };

        let max_composer_height = 150.0;
        let min_composer_height = 44.0;
        let line_height = Theme::font_size_mono().mul_add(0.4, Theme::font_size_mono());
        #[allow(clippy::cast_precision_loss)]
        let computed_height = (wrapped_line_count as f32).mul_add(line_height, 14.0);
        let input_box_height = computed_height.clamp(min_composer_height, max_composer_height);
        let focused = self.composer_has_focus(cx);
        let text_content = self.composer_display_text(cx);

        div()
            .id("input-bar-container")
            .w_full()
            .flex()
            .debug_selector(|| "chat-input-bar".to_string())
            .items_end()
            .justify_between()
            .min_h(px(56.0))
            .gap(px(Theme::SPACING_SM))
            .p(px(Theme::SPACING_MD))
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::bg_dark())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    if this.sidebar_search_focused(cx) {
                        this.set_sidebar_search_focused(false, cx);
                        cx.notify();
                    }
                }),
            )
            .child(Self::render_composer_field(
                focus_handle,
                input_box_height,
                max_composer_height,
                line_height,
                &input_text,
                text_content,
                focused,
                cx,
            ))
            // Stop only exists while there is a turn to stop; Send is always
            // there, so a running turn shows both.
            // @requirement REQ-222-001
            .when(is_streaming, |d| d.child(self.render_stop_button(cx)))
            .child(self.render_send_button(has_text, cx))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_composer_field(
        focus_handle: gpui::FocusHandle,
        input_box_height: f32,
        max_composer_height: f32,
        line_height: f32,
        input_text: &str,
        text_content: String,
        focused: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("input-field")
            .debug_selector(|| "chat-input-field".to_string())
            .flex_1()
            .min_w(px(0.0))
            .h(px(input_box_height))
            .max_h(px(max_composer_height))
            .px(px(Theme::SPACING_SM))
            .py(px(7.0))
            .bg(Theme::bg_darkest())
            .border_1()
            .border_color(if focused {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(Theme::RADIUS_MD))
            .overflow_x_hidden()
            .overflow_y_scroll()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    window.activate_window();
                    window.focus(&focus_handle, cx);
                    this.focus_composer_dismissing_overlays(cx);
                }),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(Theme::font_size_mono()))
                    .line_height(px(line_height))
                    .text_color(if input_text.is_empty() && !focused {
                        Theme::text_secondary()
                    } else {
                        Theme::text_primary()
                    })
                    .whitespace_normal()
                    .child(text_content),
            )
    }

    /// Ends the running turn.
    ///
    /// Rendered only while a turn runs, and unchanged in what it does: emit
    /// `StopStreaming`, drop the view out of streaming, and put the keyboard
    /// back in the composer.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-001
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    fn render_stop_button(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("stop-btn")
            .debug_selector(|| "chat-stop-button".to_string())
            .flex_shrink_0()
            .min_h(px(36.0))
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_MD))
            .cursor_pointer()
            .bg(Theme::error())
            .text_color(Theme::selection_fg())
            .child("Stop")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if let Some(conversation_id) = this.state.active_conversation_id {
                        tracing::info!("Stop button clicked - emitting StopStreaming");
                        this.emit(UserEvent::StopStreaming { conversation_id });
                    }
                    this.state.streaming = StreamingState::Idle;
                    this.refresh_transcript_selection_revisions();
                    this.maybe_scroll_chat_to_bottom();
                    // Refocus the composer so keyboard input works
                    // immediately after stopping — without this,
                    // GPUI leaves focus on the now-vanished Stop
                    // button div, making all text inputs unresponsive
                    // until the popup is toggled.
                    this.focus_composer(cx);
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                }),
            )
    }

    /// Submits the composer.
    ///
    /// Rendered in every state, because what a submit means is decided by
    /// `submit_composer`, not by which button is on screen: idle it starts a
    /// turn, mid-turn it steers the one already running.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-001
    /// @requirement REQ-222-002
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    fn render_send_button(&self, has_text: bool, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("send-btn")
            .debug_selector(|| "chat-send-button".to_string())
            .flex_shrink_0()
            .min_h(px(36.0))
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_MD))
            .cursor_pointer()
            .bg(Theme::bg_dark())
            .child("Send")
            .when(!has_text, |d| d.text_color(Theme::text_secondary()))
            .when(has_text, |d| {
                d.text_color(Theme::text_primary())
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.submit_composer(cx);
                        }),
                    )
            })
    }
}
