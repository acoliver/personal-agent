//! Chat view content render subtrees.
//!
//! Contains `render_chat_area`, message rendering helpers, `render_input_bar`,
//! and the root `impl Render for ChatView`. These are the content-area methods
//! below the navigation bars.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use super::emoji::strip_emojis;
use super::state::{ApprovalBubbleState, ChatMessage, MessageRole, StreamingState};
use super::transcript::{derive_document_orders, transcript_row_leaf_count, TranscriptRow};
use super::ChatView;
use crate::events::types::{ToolApprovalResponseAction, UserEvent};
use crate::presentation::view_command::AppMode;
use crate::ui_gpui::components::markdown_content::blocks_to_elements_with_leaf_factory;
use crate::ui_gpui::components::transcript_selection::{
    TranscriptSelectionContext, TranscriptSelectionLeafFactory,
};
use crate::ui_gpui::components::{ApprovalBubble, AssistantBubble};
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::main_panel::MainPanelAppState;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, MouseButton, Pixels, SharedString,
};
use std::sync::Arc;

impl ChatView {
    /// Dispatch a `KeyDownEvent` from the root render node.
    ///
    /// Extracted from `render()` to keep the root Render impl under the
    /// lizard -L 100 length threshold.
    pub(super) fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.handle_platform_shortcut(event, window, cx) {
            return;
        }
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if self.sidebar_search_focused(cx) {
            match key.as_str() {
                "escape" => {
                    self.set_sidebar_search_focused(false, cx);
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
            "enter" => self.handle_composer_enter(*modifiers, cx),
            "escape" => self.handle_escape_key(window, cx),
            _ => {}
        }
    }

    fn handle_platform_shortcut(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let key = &event.keystroke.key;
        let should_handle = Self::routes_platform_shortcut(
            event.keystroke.modifiers,
            key,
            cfg!(not(target_os = "macos")),
        );
        if should_handle {
            self.handle_platform_key(key, window, cx);
        }
        should_handle
    }

    /// Handle Cmd+key shortcuts.
    fn handle_platform_key(
        &mut self,
        key: &str,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
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
                self.scroll_transcript_to_bottom();
                self.refresh_transcript_selection_revisions();
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
            "a" => self.select_all_for_focused_surface(window, cx),
            "c" => self.copy_selection_or_input(window, cx),
            "x" => {
                if self.sidebar_search_focused(cx) {
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

    /// Render the chat area with messages
    /// @plan PLAN-20250130-GPUIREDUX.P03
    pub(super) fn render_chat_area(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let rows: Arc<[TranscriptRow]> = self.transcript_rows().into();
        let row_count = rows.len();
        let show_thinking = self.state.show_thinking;
        let filter_emoji = self.state.filter_emoji;
        let row_leaf_counts: Vec<usize> = rows
            .iter()
            .map(|&row| {
                transcript_row_leaf_count(
                    row,
                    &self.state.messages,
                    &self.state.streaming,
                    filter_emoji,
                )
            })
            .collect();
        let document_orders: Arc<[u64]> = derive_document_orders(&row_leaf_counts).into();
        let copy_document = self.transcript_copy_document(&rows);
        let scroll_offset = self.transcript_list_state.scroll_px_offset_for_scrollbar();
        self.sync_transcript_list_item_count(row_count);

        div()
            .id("chat-area")
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_hidden()
            .flex()
            .flex_col()
            .when(row_count == 0, |d| {
                d.items_center().justify_center().child(
                    div()
                        .text_size(px(Theme::font_size_body()))
                        .text_color(Theme::text_secondary())
                        .child("No messages yet"),
                )
            })
            .when(row_count > 0, |d| {
                d.child(
                    gpui::list(
                        self.transcript_list_state.clone(),
                        cx.processor(move |this, index, _window, cx| {
                            let row = rows[index];
                            let document_order = document_orders[index];
                            let selection = this.transcript_selection_context(
                                row,
                                scroll_offset,
                                document_order,
                                Arc::clone(&copy_document),
                            );
                            div()
                                .w_full()
                                .when(index + 1 < row_count, |d| d.pb(px(8.0)))
                                .child(this.render_transcript_row(
                                    row,
                                    show_thinking,
                                    filter_emoji,
                                    selection,
                                    cx,
                                ))
                                .into_any_element()
                        }),
                    )
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .p(px(super::TRANSCRIPT_LIST_PADDING_VERTICAL)),
                )
            })
    }

    fn render_transcript_row(
        &self,
        row: TranscriptRow,
        show_thinking: bool,
        filter_emoji: bool,
        selection: Option<TranscriptSelectionContext>,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        match row {
            TranscriptRow::Message(index) => {
                let selection = selection.expect("message row requires selectable content");
                let id = SharedString::from(format!("msg-{index}"));
                div()
                    .id(id)
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(Self::render_message(
                        &self.state.messages[index],
                        show_thinking,
                        filter_emoji,
                        selection,
                    ))
                    .into_any_element()
            }
            TranscriptRow::Approval(index) => {
                let conversation_id = self
                    .state
                    .active_conversation_id
                    .expect("approval row requires an active conversation");
                let bubble = &self.state.approval_bubbles[&conversation_id][index];
                let id = SharedString::from(format!("approval-{index}"));
                div()
                    .id(id)
                    .w_full()
                    .flex()
                    .justify_start()
                    .child(self.render_approval_bubble(bubble, cx))
                    .into_any_element()
            }
            TranscriptRow::Streaming => self
                .render_streaming_message(
                    &self.state.streaming,
                    show_thinking,
                    filter_emoji,
                    selection.expect("streaming row requires selectable content"),
                )
                .into_any_element(),
        }
    }

    /// Render the streaming assistant message bubble.
    fn render_streaming_message(
        &self,
        streaming: &StreamingState,
        show_thinking: bool,
        filter_emoji: bool,
        selection: TranscriptSelectionContext,
    ) -> impl IntoElement {
        let (content, _done) = match streaming {
            StreamingState::Streaming { content, done } => {
                tracing::debug!(
                    stream_buffer_len = content.len(),
                    "rendering streaming assistant bubble"
                );
                (content.clone(), *done)
            }
            _ => (String::new(), false),
        };
        let display_content: Arc<String> = if filter_emoji {
            Arc::new(strip_emojis(&content))
        } else {
            Arc::new(content)
        };
        let mut bubble = AssistantBubble::new(display_content, selection)
            .model_id("streaming")
            .show_thinking(show_thinking)
            .streaming(true);
        if let Some(ref thinking) = self.state.thinking_content {
            if !thinking.is_empty() {
                bubble = bubble.thinking(thinking.clone());
            }
        }
        div().id("streaming-msg").child(bubble)
    }

    /// Render a single message
    /// @plan PLAN-20250130-GPUIREDUX.P03
    /// @plan PLAN-20260407-ISSUE172.P03 (markdown caching)
    pub(super) fn render_message(
        msg: &ChatMessage,
        show_thinking: bool,
        filter_emoji: bool,
        selection: TranscriptSelectionContext,
    ) -> impl IntoElement {
        match msg.role {
            MessageRole::User => Self::render_user_message(msg, &selection),
            MessageRole::Assistant => {
                Self::render_assistant_message(msg, show_thinking, filter_emoji, selection)
            }
        }
    }

    /// Render user message - right aligned, green bubble
    /// @plan:PLAN-20260402-ISSUE153.P02
    /// @plan:PLAN-20260407-ISSUE172.P04 (markdown caching)
    /// @requirement:REQ-MSG-LINK-001
    pub(super) fn render_user_message(
        msg: &ChatMessage,
        selection: &TranscriptSelectionContext,
    ) -> gpui::AnyElement {
        let blocks = msg.get_or_parse_markdown();
        let text_color = Theme::user_bubble_text();
        let mut factory = TranscriptSelectionLeafFactory::new(
            selection.scroll_offset,
            selection.content_key,
            Arc::clone(&selection.copy_document),
        );
        let mut document_order = selection.document_order;
        let rendered = blocks_to_elements_with_leaf_factory(
            &blocks,
            text_color,
            Theme::user_bubble_bg(),
            &mut factory,
            &mut document_order,
            selection.first_copy_separator,
        );

        let bubble = div()
            .max_w(px(300.0))
            .px(px(10.0))
            .py(px(10.0))
            .rounded(px(12.0))
            .text_size(px(Theme::font_size_mono()))
            .children(rendered);

        div()
            .w_full()
            .flex()
            .justify_end()
            .child(Theme::user_bubble(bubble))
            .into_any_element()
    }

    /// Render assistant message - left aligned, dark bubble with model label
    /// @plan:PLAN-20260402-MARKDOWN.P11
    /// @plan:PLAN-20260407-ISSUE172.P05 (markdown caching)
    /// @plan:PLAN-20260407-ISSUE172.P10 (Arc<String> + cached blocks)
    /// @requirement:REQ-MD-INTEGRATE-010
    pub(super) fn render_assistant_message(
        msg: &ChatMessage,
        show_thinking: bool,
        filter_emoji: bool,
        selection: TranscriptSelectionContext,
    ) -> gpui::AnyElement {
        let mut bubble = if filter_emoji {
            // When filtering emojis, we need a new string - no cache can be used
            AssistantBubble::new(strip_emojis(&msg.content), selection)
        } else {
            // Pass Arc clone directly - no heap allocation
            // Also pass cached markdown blocks for finalized messages
            AssistantBubble::new(Arc::clone(&msg.content), selection)
                .with_cached_blocks(msg.get_or_parse_markdown())
        };

        if let Some(ref model_label) = msg.model_label {
            bubble = bubble.model_id(model_label.clone());
        } else {
            bubble = bubble.model_id("Assistant");
        }

        if show_thinking {
            if let Some(ref thinking) = msg.thinking {
                bubble = bubble.thinking((**thinking).clone()).show_thinking(true);
            }
        }

        bubble.into_any_element()
    }

    /// Render a single inline approval bubble with action button callbacks.
    ///
    /// A shared `AtomicBool` guard prevents duplicate responses from rapid
    /// clicks — once any button fires, all four become no-ops.
    ///
    /// For grouped bubbles, all `request_ids` in the group are resolved with
    /// the same decision.
    fn render_approval_bubble(
        &self,
        bubble: &super::state::ToolApprovalBubble,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let request_ids = bubble.request_ids.clone();
        let state = bubble.state.clone();
        let operation_count = bubble.operation_count();
        let grouped_ops = bubble.grouped_operations.clone();
        let expanded = bubble.expanded;

        let mut approval = ApprovalBubble::new(&bubble.request_id, bubble.context.clone(), state)
            .operation_count(operation_count)
            .expanded(expanded)
            .grouped_operations(grouped_ops);

        if matches!(bubble.state, ApprovalBubbleState::Pending) {
            let bridge = self.bridge.clone();
            let decided = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

            let rids = request_ids.clone();
            let b1 = bridge.clone();
            let d1 = decided.clone();
            approval = approval.on_yes(move || {
                if d1.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b1 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::ProceedOnce,
                        });
                    }
                }
            });

            let rids = request_ids.clone();
            let b2 = bridge.clone();
            let d2 = decided.clone();
            approval = approval.on_session(move || {
                if d2.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b2 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::ProceedSession,
                        });
                    }
                }
            });

            let rids = request_ids.clone();
            let b3 = bridge.clone();
            let d3 = decided.clone();
            approval = approval.on_always(move || {
                if d3.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b3 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::ProceedAlways,
                        });
                    }
                }
            });

            let rids = request_ids;
            let b4 = bridge;
            let d4 = decided;
            approval = approval.on_no(move || {
                if d4.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    return;
                }
                if let Some(ref bridge) = b4 {
                    for rid in &rids {
                        bridge.emit(UserEvent::ToolApprovalResponse {
                            request_id: rid.clone(),
                            decision: ToolApprovalResponseAction::Denied,
                        });
                    }
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
            .child(self.render_send_stop_button(is_streaming, has_text, cx))
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
        _app_mode: AppMode,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            // Title bar (32px)
            .child(self.render_title_bar(cx))
            // Expired ChatGPT session, with a way back in
            .when(self.state.codex_reauth_account.is_some(), |d| {
                d.child(Self::render_codex_reauth_bar(cx))
            })
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

    /// The banner shown when a stored `ChatGPT` session expired mid-conversation.
    ///
    /// A refresh that fails permanently is not a provider error the user can
    /// act on, so the chat says what happened and offers the one thing that
    /// fixes it.
    fn render_codex_reauth_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("chat-codex-reauth-bar")
            .w_full()
            .px(px(12.0))
            .py(px(6.0))
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::error())
            .child("Your ChatGPT session expired.")
            .child(
                div()
                    .id("btn-chat-codex-reauth")
                    .px(px(10.0))
                    .py(px(2.0))
                    .bg(Theme::accent())
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_color(Theme::accent_fg())
                    .child("Sign in again")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.start_codex_reauth();
                            cx.notify();
                        }),
                    ),
            )
    }
}

impl gpui::Render for ChatView {
    #[allow(clippy::too_many_lines)]
    #[rustfmt::skip]
    fn render(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        self.ensure_selection_auto_scroll_subscription(window, cx);
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
                cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                    this.handle_key_down(event, window, cx);
                }),
            )
            .relative()
            // Top bar (44px) — absolutely positioned so its width cannot be
            // affected by chat content's intrinsic size. Spans full window
            // width regardless of what's below. See issue #171.
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .h(px(44.0 * Theme::ui_scale()))
                    .overflow_hidden()
                    .child(self.render_top_bar(cx)),
            )
            // Body: sidebar (optional) + main content. Absolutely positioned
            // below the top bar so body width is fixed by the window, not by
            // a flex sibling relationship with the bar.
            .child(
                div()
                    .absolute()
                    .top(px(44.0 * Theme::ui_scale()))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
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
