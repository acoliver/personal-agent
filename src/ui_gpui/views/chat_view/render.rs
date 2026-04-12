//! Chat view content render subtrees.
//!
//! Contains `render_chat_area`, message rendering helpers, `render_input_bar`,
//! and the root `impl Render for ChatView`. These are the content-area methods
//! below the navigation bars.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use super::state::{ChatMessage, StreamingState, TextSelection};
use super::ChatView;
use crate::events::types::UserEvent;
use crate::presentation::view_command::AppMode;
use crate::ui_gpui::components::TextLayoutSink;
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

        if self.state.sidebar_search_focused {
            match key.as_str() {
                "escape" => {
                    self.state.sidebar_search_focused = false;
                    if self.state.sidebar_search_query.is_empty() {
                        self.state.sidebar_search_results = None;
                    }
                    cx.notify();
                }
                "backspace" => {
                    self.state.sidebar_search_query.pop();
                    self.trigger_sidebar_search(cx);
                    cx.notify();
                }
                _ => {}
            }
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
    #[allow(clippy::too_many_lines)]
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
                self.clear_transcript_selection();
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
            "a" => {
                if self.state.sidebar_search_focused {
                    // select-all is a no-op for sidebar search (single-line)
                } else {
                    self.handle_select_all(cx);
                }
            }
            "c" => self.handle_copy(cx),
            "x" => {
                if self.state.sidebar_search_focused {
                    let text = self.state.sidebar_search_query.clone();
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                    self.state.sidebar_search_query.clear();
                    self.state.sidebar_search_results = None;
                } else {
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
                }
                cx.notify();
            }
            "left" => self.move_cursor_home(cx),
            "right" => self.scroll_chat_to_end(cx),
            _ => {}
        }
    }

    /// Render the chat area with messages.
    ///
    /// Build the flattened transcript backing buffer in visual order so a
    /// single selection range can span user/assistant/thinking blocks.
    ///
    /// When `filter_emoji` is on the buffer is left empty (display text
    /// diverges from source) and selection is disabled.
    pub(super) fn build_transcript_buffer(
        messages: &[ChatMessage],
        thinking_content: Option<&str>,
        show_thinking: bool,
        filter_emoji: bool,
    ) -> (String, Vec<std::ops::Range<usize>>) {
        let mut transcript_text = String::new();
        let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();

        if filter_emoji {
            return (transcript_text, ranges);
        }

        for msg in messages {
            let content_start = transcript_text.len();
            transcript_text.push_str(&msg.content);
            let content_end = transcript_text.len();
            ranges.push(content_start..content_end);

            if show_thinking {
                if let Some(thinking) = msg.thinking.as_ref() {
                    if !thinking.is_empty() {
                        transcript_text.push('\n');
                        let thinking_start = transcript_text.len();
                        transcript_text.push_str(thinking);
                        let thinking_end = transcript_text.len();
                        ranges.push(thinking_start..thinking_end);
                    }
                }
            }

            transcript_text.push('\n');
        }

        if let Some(thinking) = thinking_content {
            if !thinking.is_empty() {
                let thinking_start = transcript_text.len();
                transcript_text.push_str(thinking);
                let thinking_end = transcript_text.len();
                ranges.push(thinking_start..thinking_end);
                transcript_text.push('\n');
            }
        }

        (transcript_text, ranges)
    }

    /// Compute the per-block sub-range that lies inside the given selection.
    fn block_sub_range(
        block: &std::ops::Range<usize>,
        selection: Option<&std::ops::Range<usize>>,
    ) -> Option<std::ops::Range<usize>> {
        let range = selection?;
        let start = range.start.max(block.start);
        let end = range.end.min(block.end);
        (start < end).then_some(start - block.start..end - block.start)
    }

    /// Build the message-row elements that go inside the chat area, also
    /// returning per-block layout sinks for hit-testing. Each message body and
    /// optional thinking block gets a `TextLayoutSink` that the bubble will
    /// populate during `into_element` when `selectable` is true.
    fn build_message_rows(
        messages: &[ChatMessage],
        block_ranges: &[std::ops::Range<usize>],
        selection_range: Option<&std::ops::Range<usize>>,
        show_thinking: bool,
        filter_emoji: bool,
        selectable: bool,
    ) -> (Vec<gpui::AnyElement>, Vec<TextLayoutSink>) {
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        let mut layouts: Vec<TextLayoutSink> = Vec::with_capacity(block_ranges.len());

        if filter_emoji {
            for (i, msg) in messages.iter().enumerate() {
                let id = SharedString::from(format!("msg-{i}"));
                let element = Self::render_message(
                    msg,
                    show_thinking,
                    filter_emoji,
                    i,
                    None,
                    false,
                    None,
                    None,
                );
                rows.push(
                    div()
                        .id(id)
                        .w_full()
                        .flex()
                        .justify_start()
                        .child(element)
                        .into_any_element(),
                );
            }
            return (rows, layouts);
        }

        let mut block_cursor: usize = 0;
        #[allow(clippy::explicit_counter_loop)]
        for (i, msg) in messages.iter().enumerate() {
            let msg_block = block_ranges.get(block_cursor);
            let msg_range = msg_block.and_then(|b| Self::block_sub_range(b, selection_range));
            let body_sink: TextLayoutSink = std::rc::Rc::new(std::cell::RefCell::new(None));
            layouts.push(body_sink.clone());
            block_cursor += 1;

            // Create thinking sink if this message has thinking content
            let thinking_sink: Option<TextLayoutSink> = if show_thinking {
                msg.thinking.as_ref().filter(|t| !t.is_empty()).map(|_| {
                    let sink: TextLayoutSink = std::rc::Rc::new(std::cell::RefCell::new(None));
                    layouts.push(sink.clone());
                    block_cursor += 1;
                    sink
                })
            } else {
                None
            };

            let id = SharedString::from(format!("msg-{i}"));
            let element = Self::render_message(
                msg,
                show_thinking,
                filter_emoji,
                i,
                msg_range,
                selectable,
                Some(body_sink),
                thinking_sink,
            );
            rows.push(
                div()
                    .id(id)
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(element)
                    .into_any_element(),
            );
        }

        (rows, layouts)
    }

    /// Pointer handler body: left mouse down on the chat area.
    fn on_chat_pointer_down_left(
        &mut self,
        event: &gpui::MouseDownEvent,
        cx: &mut gpui::Context<Self>,
    ) {
        self.refresh_autoscroll_state_from_handle();

        if self.state.filter_emoji {
            self.clear_transcript_selection();
            cx.notify();
            return;
        }

        let Some((block_index, block_offset)) =
            self.transcript_block_index_at_point(event.position)
        else {
            // Sinks not yet populated — arm flat mode for the next frame.
            if !self.transcript_selection_armed {
                self.transcript_selection_armed = true;
                // For multi-click, stash the event for deferred replay.
                if event.click_count >= 2 {
                    self.transcript_pending_click = Some(super::PendingClick {
                        position: event.position,
                        click_count: event.click_count,
                    });
                    let entity = cx.entity();
                    cx.defer(move |cx| {
                        entity.update(cx, |this, cx| {
                            this.replay_pending_click(cx);
                        });
                    });
                }
                cx.notify();
                return;
            }
            self.clear_transcript_selection();
            cx.notify();
            return;
        };

        let Some(abs_offset) = self.transcript_offset_for_block_index(block_index, block_offset)
        else {
            self.clear_transcript_selection();
            cx.notify();
            return;
        };

        match event.click_count {
            2 => {
                self.transcript_drag_anchor = None;
                self.select_word_at_offset(abs_offset, cx);
            }
            n if n >= 3 => {
                self.transcript_drag_anchor = None;
                self.select_paragraph_at_offset(abs_offset, cx);
            }
            _ => {
                self.transcript_drag_anchor = Some(abs_offset);
                self.set_text_selection(abs_offset, abs_offset, true);
            }
        }

        cx.notify();
    }

    /// Replay a stashed multi-click (double/triple) after sinks become available.
    fn replay_pending_click(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(pending) = self.transcript_pending_click.take() else {
            return;
        };
        let Some((block_index, block_offset)) =
            self.transcript_block_index_at_point(pending.position)
        else {
            return;
        };
        let Some(abs_offset) = self.transcript_offset_for_block_index(block_index, block_offset)
        else {
            return;
        };
        match pending.click_count {
            2 => self.select_word_at_offset(abs_offset, cx),
            _ => self.select_paragraph_at_offset(abs_offset, cx),
        }
        cx.notify();
    }

    /// Pointer handler body: mouse move while dragging a transcript selection.
    fn on_chat_pointer_move(&mut self, event: &gpui::MouseMoveEvent, cx: &mut gpui::Context<Self>) {
        if self.state.filter_emoji || !event.dragging() {
            return;
        }
        let Some(anchor) = self.transcript_drag_anchor else {
            return;
        };
        let Some((block_index, block_offset)) =
            self.transcript_block_index_at_point(event.position)
        else {
            return;
        };
        let Some(current_offset) =
            self.transcript_offset_for_block_index(block_index, block_offset)
        else {
            return;
        };
        self.set_text_selection(anchor, current_offset, true);
        cx.notify();
    }

    /// Attach the full set of chat-area pointer handlers (drag select,
    /// double/triple-click word/paragraph, right-click copy, click-out clear)
    /// to a chat-area div builder.
    fn attach_chat_pointer_handlers(
        cx: &mut gpui::Context<Self>,
        d: gpui::Stateful<gpui::Div>,
    ) -> gpui::Stateful<gpui::Div> {
        d.on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
            this.refresh_autoscroll_state_after_wheel(event);
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                this.on_chat_pointer_down_left(event, cx);
            }),
        )
        .on_mouse_move(
            cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                this.on_chat_pointer_move(event, cx);
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _event: &gpui::MouseUpEvent, _window, cx| {
                this.on_chat_pointer_up_left(cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                this.handle_copy(cx);
            }),
        )
        .on_mouse_down_out(cx.listener(|this, _event, _window, cx| {
            if this.state.text_selection.is_some() || this.transcript_selection_armed {
                this.clear_transcript_selection();
                cx.notify();
            }
        }))
    }

    /// Build the "No messages yet" empty-state child element.
    fn build_empty_state_child() -> gpui::AnyElement {
        div()
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_secondary())
            .child("No messages yet")
            .into_any_element()
    }

    /// Pointer handler body: left mouse up on the chat area.
    fn on_chat_pointer_up_left(&mut self, cx: &mut gpui::Context<Self>) {
        self.transcript_drag_anchor = None;
        if let Some(selection) = self.state.text_selection.as_ref() {
            if selection.is_dragging {
                let range = selection.range.clone();
                self.state.text_selection = Some(TextSelection {
                    range,
                    is_dragging: false,
                });
                cx.notify();
            }
        } else if self.transcript_selection_armed {
            // User clicked and released without selecting — clear armed state.
            self.transcript_selection_armed = false;
            self.transcript_pending_click = None;
            cx.notify();
        }
    }

    /// Render the chat area with messages.
    ///
    /// Builds the flattened transcript backing buffer and per-block text
    /// layouts up front, stashes them on `self` for hit-testing handlers, then
    /// attaches pointer handlers (single drag, double-click word, triple-click
    /// paragraph, right-click copy).
    ///
    /// @plan PLAN-20250130-GPUIREDUX.P03
    /// @plan PLAN-20260406-ISSUE151.P01 - transcript selection + copy
    #[allow(clippy::too_many_lines)]
    pub(super) fn render_chat_area(&mut self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let messages = self.state.messages.clone();
        let streaming = self.state.streaming.clone();
        let show_thinking = self.state.show_thinking;
        let filter_emoji = self.state.filter_emoji;

        let (transcript_text, transcript_block_ranges) = Self::build_transcript_buffer(
            &messages,
            self.state.thinking_content.as_deref(),
            show_thinking,
            filter_emoji,
        );

        let selection_range = if filter_emoji {
            None
        } else {
            self.state.text_selection.as_ref().map(|s| s.range.clone())
        };

        let selectable = !filter_emoji
            && (self.state.text_selection.is_some() || self.transcript_selection_armed);

        let (message_rows, transcript_block_layouts) = Self::build_message_rows(
            &messages,
            &transcript_block_ranges,
            selection_range.as_ref(),
            show_thinking,
            filter_emoji,
            selectable,
        );

        // Stash transcript state on self so the mouse handlers can hit-test.
        self.transcript_text = transcript_text;
        self.transcript_block_ranges = transcript_block_ranges;
        self.transcript_block_layouts = transcript_block_layouts;

        let messages_empty = messages.is_empty();
        let is_streaming = matches!(streaming, StreamingState::Streaming { .. });

        let approval_rows: Vec<gpui::AnyElement> = self
            .state
            .approval_bubbles
            .iter()
            .enumerate()
            .filter(|(_, bubble)| {
                matches!(bubble.state, super::state::ApprovalBubbleState::Pending)
            })
            .take(1)
            .map(|(i, bubble)| {
                let id = SharedString::from(format!("approval-{i}"));
                div()
                    .id(id)
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(self.render_approval_bubble(bubble, cx))
                    .into_any_element()
            })
            .collect();

        let streaming_element = if is_streaming {
            Some(self.render_streaming_message(&streaming, show_thinking, filter_emoji))
        } else {
            None
        };

        let base = div()
            .id("chat-area")
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_y_scroll()
            .track_scroll(&self.chat_scroll_handle);

        Self::attach_chat_pointer_handlers(cx, base)
            .p(px(12.0))
            .flex()
            .flex_col()
            .items_stretch()
            .justify_start()
            .gap(px(8.0))
            .when(messages_empty && !is_streaming, |d| {
                d.items_center()
                    .justify_center()
                    .child(Self::build_empty_state_child())
            })
            .when(!messages_empty, |d| d.children(message_rows))
            .children(approval_rows)
            .when_some(streaming_element, gpui::ParentElement::child)
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
                    if this.state.sidebar_search_focused {
                        this.state.sidebar_search_focused = false;
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
        &mut self,
        _app_mode: AppMode,
        _window: &mut gpui::Window,
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
        // Note: Dropdown overlays are now rendered at root level in render()
        // to avoid being clipped by the flex container
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
            .overflow_hidden()
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
            // Dropdown overlays - rendered at root level so they don't get clipped by flex containers
            .when(
                self.state.conversation_dropdown_open
                    && (app_mode == AppMode::Popup || !self.state.sidebar_visible),
                |d| d.child(self.render_conversation_dropdown(cx)),
            )
            .when(self.state.profile_dropdown_open, |d| {
                d.child(self.render_profile_dropdown(window, cx))
            })
    }
}
