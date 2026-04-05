//! Chat view content render subtrees.
//!
//! Contains `render_chat_area`, message rendering helpers, `render_input_bar`,
//! and the root `impl Render for ChatView`. These are the content-area methods
//! below the navigation bars.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use super::state::{ApprovalBubbleState, ChatMessage, MessageRole, StreamingState};
use super::ChatView;
use crate::events::types::{ToolApprovalResponseAction, UserEvent};
use crate::presentation::view_command::AppMode;
use crate::ui_gpui::components::{ApprovalBubble, AssistantBubble};
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::main_panel::MainPanelAppState;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, MouseButton, Pixels,
    ScrollWheelEvent, SharedString,
};

impl ChatView {
    /// Dispatch a `KeyDownEvent` from the root render node.
    ///
    /// Extracted from `render()` to keep the root Render impl under the
    /// lizard -L 100 length threshold.
    pub(super) fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut gpui::Context<Self>,
    ) {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if modifiers.platform {
            self.handle_platform_key(key, cx);
            return;
        }

        if self.state.conversation_title_editing {
            match key.as_str() {
                "escape" => self.cancel_rename_conversation(cx),
                "backspace" => self.handle_rename_backspace(cx),
                "enter" => self.submit_rename_conversation(cx),
                _ => {}
            }
            return;
        }

        if self.state.conversation_dropdown_open {
            match key.as_str() {
                "escape" => {
                    self.state.conversation_dropdown_open = false;
                    cx.notify();
                }
                "up" => self.move_conversation_dropdown_selection(-1, cx),
                "down" => self.move_conversation_dropdown_selection(1, cx),
                "enter" => self.confirm_conversation_dropdown_selection(cx),
                _ => {}
            }
            return;
        }

        if self.state.profile_dropdown_open {
            match key.as_str() {
                "escape" => {
                    self.state.profile_dropdown_open = false;
                    cx.notify();
                }
                "up" => self.move_profile_dropdown_selection(-1, cx),
                "down" => self.move_profile_dropdown_selection(1, cx),
                "enter" => self.confirm_profile_dropdown_selection(cx),
                _ => {}
            }
            return;
        }

        match key.as_str() {
            "left" => self.move_cursor_left(cx),
            "right" => self.move_cursor_right(cx),
            "home" => self.scroll_chat_to_top(cx),
            "end" => self.scroll_chat_to_end(cx),
            "pageup" => self.scroll_chat_page_up(cx),
            "pagedown" => self.scroll_chat_page_down(cx),
            "backspace" => self.handle_backspace(cx),
            "enter" => self.handle_enter(cx),
            "escape" => {
                if matches!(self.state.streaming, StreamingState::Streaming { .. }) {
                    println!(">>> Escape pressed - stopping stream <<<");
                    self.emit(UserEvent::StopStreaming);
                    self.state.streaming = StreamingState::Idle;
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    /// Handle Cmd+key shortcuts.
    fn handle_platform_key(&mut self, key: &str, cx: &mut gpui::Context<Self>) {
        match key {
            "h" => {
                println!(">>> Cmd+H pressed - navigating to History <<<");
                crate::ui_gpui::navigation_channel()
                    .request_navigate(crate::presentation::view_command::ViewId::History);
            }
            "," => {
                println!(">>> Cmd+, pressed - navigating to Settings <<<");
                crate::ui_gpui::navigation_channel()
                    .request_navigate(crate::presentation::view_command::ViewId::Settings);
            }
            "n" => {
                println!(">>> Cmd+N pressed - new conversation <<<");
                self.emit(UserEvent::NewConversation);
                self.state.messages.clear();
                self.state.input_text.clear();
                self.state.cursor_position = 0;
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.active_conversation_id = None;
                self.conversation_id = None;
                self.state.conversation_title = "New Conversation".to_string();
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.profile_dropdown_open = false;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                cx.notify();
            }
            "t" => {
                println!(">>> Cmd+T pressed - toggle thinking <<<");
                self.emit(UserEvent::ToggleThinking);
            }
            "p" => self.toggle_profile_dropdown(cx),
            "k" => self.toggle_conversation_dropdown(cx),
            "r" => self.start_rename_conversation(cx),
            "v" => {
                if let Some(item) = cx.read_from_clipboard() {
                    if let Some(text) = item.text() {
                        self.handle_paste(&text, cx);
                    }
                }
            }
            "a" => self.handle_select_all(cx),
            "c" => {
                let text = if self.state.conversation_title_editing {
                    self.state.conversation_title_input.clone()
                } else {
                    self.state.input_text.clone()
                };
                if !text.is_empty() {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                }
            }
            "x" => {
                self.handle_select_all(cx);
                let text = if self.state.conversation_title_editing {
                    self.state.conversation_title_input.clone()
                } else {
                    self.state.input_text.clone()
                };
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                if self.state.conversation_title_editing {
                    self.state.conversation_title_input.clear();
                    self.state.rename_replace_on_next_char = false;
                } else if !self.state.conversation_dropdown_open
                    && !self.state.profile_dropdown_open
                {
                    self.state.input_text.clear();
                    self.state.cursor_position = 0;
                    self.state.marked_range = None;
                }
                cx.notify();
            }
            "left" => self.move_cursor_home(cx),
            "right" => self.scroll_chat_to_end(cx),
            _ => {}
        }
    }

    /// Render the chat area with messages
    /// @plan PLAN-20250130-GPUIREDUX.P03
    pub(super) fn render_chat_area(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let messages = self.state.messages.clone();
        let streaming = self.state.streaming.clone();
        let show_thinking = self.state.show_thinking;
        div()
            .id("chat-area")
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_y_scroll()
            .track_scroll(&self.chat_scroll_handle)
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                this.refresh_autoscroll_state_after_wheel(event);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.refresh_autoscroll_state_from_handle();
                    cx.notify();
                }),
            )
            .p(px(12.0))
            .flex()
            .flex_col()
            .items_stretch()
            .justify_start()
            .gap(px(8.0))
            // Empty state
            .when(
                messages.is_empty() && !matches!(streaming, StreamingState::Streaming { .. }),
                |d| {
                    d.items_center().justify_center().child(
                        div()
                            .text_size(px(Theme::font_size_body()))
                            .text_color(Theme::text_secondary())
                            .child("No messages yet"),
                    )
                },
            )
            // Messages
            .when(!messages.is_empty(), |d| {
                d.children(messages.into_iter().enumerate().map(|(i, msg)| {
                    let id = SharedString::from(format!("msg-{i}"));
                    div()
                        .id(id)
                        .w_full()
                        .flex()
                        .justify_start()
                        .child(Self::render_message(&msg, show_thinking))
                }))
            })
            // Approval bubbles (inline in message stream)
            .children(
                self.state
                    .approval_bubbles
                    .iter()
                    .enumerate()
                    .map(|(i, bubble)| {
                        let id = SharedString::from(format!("approval-{i}"));
                        div()
                            .id(id)
                            .w_full()
                            .flex()
                            .justify_start()
                            .child(self.render_approval_bubble(bubble, cx))
                    }),
            )
            // Streaming message
            .when(matches!(streaming, StreamingState::Streaming { .. }), |d| {
                let (content, _done) = match &streaming {
                    StreamingState::Streaming { content, done } => {
                        tracing::debug!(
                            stream_buffer_len = content.len(),
                            "rendering streaming assistant bubble"
                        );
                        (content.clone(), *done)
                    }
                    _ => (String::new(), false),
                };
                let mut bubble = AssistantBubble::new(content)
                    .model_id("streaming")
                    .show_thinking(show_thinking)
                    .streaming(true);
                if let Some(ref thinking) = self.state.thinking_content {
                    if !thinking.is_empty() {
                        bubble = bubble.thinking(thinking.clone());
                    }
                }
                d.child(div().id("streaming-msg").child(bubble))
            })
    }

    /// Render a single message
    /// @plan PLAN-20250130-GPUIREDUX.P03
    pub(super) fn render_message(msg: &ChatMessage, show_thinking: bool) -> impl IntoElement {
        match msg.role {
            MessageRole::User => Self::render_user_message(&msg.content),
            MessageRole::Assistant => Self::render_assistant_message(msg, show_thinking),
        }
    }

    /// Render user message - right aligned, green bubble
    pub(super) fn render_user_message(content: &str) -> gpui::AnyElement {
        let content_owned = content.to_string();
        div()
            .w_full()
            .flex()
            .justify_end()
            .child({
                let text = content_owned.clone();
                div()
                    .id(SharedString::from(format!("ubbl-{}", content.len())))
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::user_bubble())
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::user_bubble_text())
                    .cursor_pointer()
                    .on_click(move |_event, _window, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                    })
                    .child(content_owned)
            })
            .into_any_element()
    }

    /// Render assistant message - left aligned, dark bubble with model label
    /// @plan:PLAN-20260402-MARKDOWN.P11
    /// @requirement:REQ-MD-INTEGRATE-010
    pub(super) fn render_assistant_message(
        msg: &ChatMessage,
        show_thinking: bool,
    ) -> gpui::AnyElement {
        let mut bubble = AssistantBubble::new(msg.content.clone());

        if let Some(ref model_label) = msg.model_label {
            bubble = bubble.model_id(model_label.clone());
        } else {
            bubble = bubble.model_id("Assistant");
        }

        if show_thinking {
            if let Some(ref thinking) = msg.thinking {
                bubble = bubble.thinking(thinking.clone()).show_thinking(true);
            }
        }

        bubble.into_any_element()
    }

    /// Render a single inline approval bubble with action button callbacks.
    ///
    /// A shared `AtomicBool` guard prevents duplicate responses from rapid
    /// clicks — once any button fires, all four become no-ops.
    fn render_approval_bubble(
        &self,
        bubble: &super::state::ToolApprovalBubble,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let request_id = bubble.request_id.clone();
        let state = bubble.state.clone();

        let mut approval = ApprovalBubble::new(
            &bubble.request_id,
            &bubble.tool_name,
            &bubble.tool_argument,
            state,
        );

        if matches!(bubble.state, ApprovalBubbleState::Pending) {
            let bridge = self.bridge.clone();
            let decided = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            let rid = request_id.clone();
            let b1 = bridge.clone();
            let d1 = decided.clone();
            approval = approval.on_yes(move || {
                if d1.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b1 {
                    bridge.emit(UserEvent::ToolApprovalResponse {
                        request_id: rid.clone(),
                        decision: ToolApprovalResponseAction::ProceedOnce,
                    });
                }
            });

            let rid = request_id.clone();
            let b2 = bridge.clone();
            let d2 = decided.clone();
            approval = approval.on_session(move || {
                if d2.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b2 {
                    bridge.emit(UserEvent::ToolApprovalResponse {
                        request_id: rid.clone(),
                        decision: ToolApprovalResponseAction::ProceedSession,
                    });
                }
            });

            let rid = request_id.clone();
            let b3 = bridge.clone();
            let d3 = decided.clone();
            approval = approval.on_always(move || {
                if d3.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b3 {
                    bridge.emit(UserEvent::ToolApprovalResponse {
                        request_id: rid.clone(),
                        decision: ToolApprovalResponseAction::ProceedAlways,
                    });
                }
            });

            let rid = request_id;
            let b4 = bridge;
            let d4 = decided;
            approval = approval.on_no(move || {
                if d4.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b4 {
                    bridge.emit(UserEvent::ToolApprovalResponse {
                        request_id: rid.clone(),
                        decision: ToolApprovalResponseAction::Denied,
                    });
                }
            });
        }

        // Use cx to mark the closure as capturing the context lifetime
        let _ = cx;
        approval
    }

    /// Render thinking block with blue tint
    #[allow(dead_code)]
    pub(super) fn render_thinking_block(content: &str) -> impl IntoElement {
        div()
            .max_w(px(300.0))
            .px(px(8.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .bg(Theme::thinking_bg())
            .border_l_2()
            .border_color(Theme::text_muted())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_muted())
                            .child("Thinking"),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .italic()
                            .child(content.to_string()),
                    ),
            )
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_input_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_streaming = matches!(self.state.streaming, StreamingState::Streaming { .. });
        let input_text = self.state.input_text.clone();
        let has_text = !input_text.trim().is_empty();
        let focus_handle = self.focus_handle.clone();
        let cursor_pos = self.state.cursor_position.min(input_text.len());

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
        let text_content = if input_text.is_empty() {
            "Type a message...".to_string()
        } else {
            let before = &input_text[..cursor_pos];
            let after = &input_text[cursor_pos..];
            format!("{before}|{after}")
        };

        div()
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
            .child(Self::render_composer_field(
                focus_handle,
                input_box_height,
                max_composer_height,
                line_height,
                &input_text,
                text_content,
            ))
            .child(self.render_send_stop_button(is_streaming, has_text, cx))
    }

    fn render_composer_field(
        focus_handle: gpui::FocusHandle,
        input_box_height: f32,
        max_composer_height: f32,
        line_height: f32,
        input_text: &str,
        text_content: String,
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
            .rounded(px(Theme::RADIUS_MD))
            .overflow_x_hidden()
            .overflow_y_scroll()
            .cursor_text()
            .on_mouse_down(MouseButton::Left, {
                move |_, window, cx| {
                    window.focus(&focus_handle, cx);
                }
            })
            .child(
                div()
                    .w_full()
                    .text_size(px(Theme::font_size_mono()))
                    .line_height(px(line_height))
                    .text_color(if input_text.is_empty() {
                        Theme::text_secondary()
                    } else {
                        Theme::text_primary()
                    })
                    .whitespace_normal()
                    .child(text_content),
            )
    }

    /// Send/Stop button with event emission.
    /// @plan PLAN-20250130-GPUIREDUX.P04
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    fn render_send_stop_button(
        &self,
        is_streaming: bool,
        has_text: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(if is_streaming { "stop-btn" } else { "send-btn" })
            .debug_selector(|| {
                if is_streaming {
                    "chat-stop-button".to_string()
                } else {
                    "chat-send-button".to_string()
                }
            })
            .flex_shrink_0()
            .min_h(px(36.0))
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_MD))
            .cursor_pointer()
            .when(is_streaming, |d| {
                d.bg(Theme::error())
                    .text_color(Theme::selection_fg())
                    .child("Stop")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            tracing::info!("Stop button clicked - emitting StopStreaming");
                            this.emit(UserEvent::StopStreaming);
                            this.state.streaming = StreamingState::Idle;
                            this.maybe_scroll_chat_to_bottom(cx);
                            cx.notify();
                        }),
                    )
            })
            .when(!is_streaming && has_text, |d| {
                d.bg(Theme::bg_dark())
                    .text_color(Theme::text_primary())
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .child("Send")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            if matches!(this.state.streaming, StreamingState::Streaming { .. }) {
                                tracing::info!("Send button ignored while stream is active");
                                return;
                            }
                            let text = this.state.input_text.clone();
                            if !text.trim().is_empty() {
                                tracing::info!(
                                    "Send button clicked - emitting SendMessage: {}",
                                    text
                                );
                                this.send_message_and_start_streaming(text, cx);
                            }
                        }),
                    )
            })
            .when(!is_streaming && !has_text, |d| {
                d.bg(Theme::bg_dark())
                    .text_color(Theme::text_secondary())
                    .child("Send")
            })
    }
}

impl ChatView {
    /// Read the current window mode from the global state.
    fn current_app_mode(cx: &gpui::Context<Self>) -> AppMode {
        cx.try_global::<MainPanelAppState>()
            .map(|s| s.app_mode)
            .unwrap_or_default()
    }

    /// Render the main chat content column (title bar + chat area + input bar).
    fn render_main_content(
        &self,
        app_mode: AppMode,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .min_w(px(0.0))
            .flex()
            .flex_col()
            // Title bar (32px)
            .child(self.render_title_bar(cx))
            // Export feedback row
            .when(self.state.export_feedback_message.is_some(), |d| {
                d.child(self.render_export_feedback_bar())
            })
            // Chat area (flex)
            .child(self.render_chat_area(cx))
            // Input bar (50px)
            .child(self.render_input_bar(cx))
            // Overlay dropdowns (only in popup mode; popout uses sidebar)
            .when(
                app_mode == AppMode::Popup && self.state.conversation_dropdown_open,
                |d| d.child(self.render_conversation_dropdown(cx)),
            )
            .when(self.state.profile_dropdown_open, |d| {
                d.child(self.render_profile_dropdown(window, cx))
            })
    }
}

impl gpui::Render for ChatView {
    #[allow(clippy::too_many_lines)]
    #[rustfmt::skip]
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let app_mode = Self::current_app_mode(cx);
        let show_sidebar = app_mode == AppMode::Popout && self.state.sidebar_visible;

        div()
            .id("chat-view")
            .debug_selector(|| "chat-view-root".to_string())
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .child(
                canvas(
                    |bounds, _window: &mut gpui::Window, _cx: &mut gpui::App| bounds,
                    {
                        let entity = cx.entity();
                        let focus = self.focus_handle.clone();
                        move |bounds: Bounds<Pixels>, _, window: &mut gpui::Window, cx: &mut gpui::App| {
                            window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                        }
                    },
                )
                .size_0(),
            )

            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    this.handle_key_down(event, cx);
                }),
            )
            .relative()
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Body: sidebar (optional) + main content
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    // Sidebar in popout mode
                    .when(show_sidebar, |d| {
                        d.child(self.render_sidebar(cx))
                    })
                    // Main content column
                    .child(self.render_main_content(app_mode, window, cx))
            )
    }
}
