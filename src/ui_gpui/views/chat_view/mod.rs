//! Chat view implementation
//!
//! This is the public entry point for the chat view family.
//! Sub-modules own:
//! - `chat_state`  — data types and pure state-transition helpers
//! - `chat_render` — all render_* methods and `impl Render`
//! - `chat_ime`    — `EntityInputHandler` + `Focusable` implementations
//!
//! @plan PLAN-20250130-GPUIREDUX.P04
//! @plan PLAN-20260325-ISSUE11B.P02
//! @requirement REQ-GPUI-003

mod command;
mod ime;
mod render;
mod render_bars;
mod render_sidebar;
mod state;

// ── Re-exports so downstream consumers (mod.rs, tests, main_panel.rs) ──
// see the same type paths as before extraction.
pub use state::{
    ApprovalBubbleState, ChatMessage, ChatState, GroupedOperation, MessageRole, StreamingState,
    ToolApprovalBubble,
};

use crate::events::types::UserEvent;
use crate::presentation::view_command::ConversationMessagePayload;
use crate::ui_gpui::app_store::{ChatStoreSnapshot, ConversationLoadState, StreamingStoreSnapshot};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::selection_intent_channel;
use crate::ui_gpui::theme::Theme;
use gpui::{point, px, FocusHandle, Pixels, ScrollDelta, ScrollHandle, ScrollWheelEvent};
#[cfg(test)]
use std::cell::Cell;
use std::sync::Arc;
use uuid::Uuid;

/// Chat view component with event handling
///
/// @plan PLAN-20250130-GPUIREDUX.P04
pub struct ChatView {
    pub state: ChatState,
    pub(super) focus_handle: FocusHandle,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) conversation_id: Option<Uuid>,
    pub(super) selection_generation: u64,
    pub(super) chat_scroll_handle: ScrollHandle,
    #[cfg(test)]
    pub(super) maybe_scroll_chat_to_bottom_invocations: Cell<usize>,
}

impl ChatView {
    pub fn new(state: ChatState, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
            bridge: None,
            conversation_id: None,
            selection_generation: 0,
            chat_scroll_handle: ScrollHandle::new(),
            #[cfg(test)]
            maybe_scroll_chat_to_bottom_invocations: Cell::new(0),
        }
    }

    pub(super) fn refresh_autoscroll_state_from_handle(&mut self) {
        let offset = self.chat_scroll_handle.offset();
        let max_offset = self.chat_scroll_handle.max_offset();
        let distance_from_bottom = (max_offset.height + offset.y).abs();
        self.state.chat_autoscroll_enabled = distance_from_bottom <= px(8.0);
    }

    pub(super) fn refresh_autoscroll_state_after_wheel(&mut self, event: &ScrollWheelEvent) {
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta,
            ScrollDelta::Lines(delta) => point(px(delta.x * 16.0), px(delta.y * 16.0)),
        };

        // Positive Y moves the viewport upward (away from bottom) in GPUI's scroll model.
        // Negative Y moves toward bottom.
        if delta.y > px(0.0) {
            self.state.chat_autoscroll_enabled = false;
            return;
        }

        // Re-enable sticky-follow only once the viewport is actually near the bottom.
        if delta.y < px(0.0) {
            self.refresh_autoscroll_state_from_handle();
            return;
        }

        self.refresh_autoscroll_state_from_handle();
    }

    pub(super) fn maybe_scroll_chat_to_bottom(&self, cx: &mut gpui::Context<Self>) {
        if self.state.chat_autoscroll_enabled {
            #[cfg(test)]
            self.maybe_scroll_chat_to_bottom_invocations
                .set(self.maybe_scroll_chat_to_bottom_invocations.get() + 1);

            self.chat_scroll_handle.scroll_to_bottom();
            let entity = cx.entity();
            cx.defer(move |cx| {
                entity.update(cx, |this, cx| {
                    this.chat_scroll_handle.scroll_to_bottom();
                    cx.notify();
                });
                let entity = entity.clone();
                cx.defer(move |cx| {
                    entity.update(cx, |this, cx| {
                        this.chat_scroll_handle.scroll_to_bottom();
                        cx.notify();
                    });
                    let entity = entity.clone();
                    cx.defer(move |cx| {
                        entity.update(cx, |this, cx| {
                            this.chat_scroll_handle.scroll_to_bottom();
                            cx.notify();
                        });
                    });
                });
            });
        }
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    pub(super) fn messages_from_payload(
        messages: Vec<ConversationMessagePayload>,
        current_model: &str,
    ) -> Vec<ChatMessage> {
        messages
            .into_iter()
            .map(|message| {
                let mut chat_message = match message.role {
                    crate::presentation::view_command::MessageRole::User => {
                        ChatMessage::user(message.content)
                    }
                    crate::presentation::view_command::MessageRole::Assistant => {
                        // Use the per-message model_id if available, otherwise fall back to current_model
                        let model_label = message
                            .model_id
                            .unwrap_or_else(|| current_model.to_string());
                        ChatMessage::assistant(message.content, model_label)
                    }
                    crate::presentation::view_command::MessageRole::System
                    | crate::presentation::view_command::MessageRole::Tool => {
                        unreachable!(
                            "conversation replay payload excludes non-rendered message roles"
                        )
                    }
                };

                if let Some(thinking) = message.thinking_content {
                    chat_message = chat_message.with_thinking(thinking);
                }
                if let Some(timestamp) = message.timestamp {
                    chat_message = chat_message.with_timestamp(timestamp);
                }

                chat_message
            })
            .collect()
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    pub(super) fn streaming_state_from_snapshot(
        streaming: &StreamingStoreSnapshot,
        load_state: &ConversationLoadState,
    ) -> StreamingState {
        if let Some(error) = &streaming.last_error {
            return StreamingState::Error(error.clone());
        }

        if let ConversationLoadState::Error { message, .. } = load_state {
            return StreamingState::Error(message.clone());
        }

        if streaming.active_target.is_some() || !streaming.stream_buffer.is_empty() {
            return StreamingState::Streaming {
                content: streaming.stream_buffer.clone(),
                done: false,
            };
        }

        StreamingState::Idle
    }

    /// Set the bridge for event communication
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P04
    /// @requirement REQ-ARCH-001.1
    /// @requirement REQ-ARCH-004.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:022-035
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    pub fn apply_store_snapshot(
        &mut self,
        snapshot: ChatStoreSnapshot,
        cx: &mut gpui::Context<Self>,
    ) {
        let ChatStoreSnapshot {
            selected_conversation_id,
            selected_conversation_title,
            load_state,
            transcript,
            streaming,
            conversations,
            ..
        } = snapshot;

        let previous_conversation_id = self.conversation_id;
        let previous_selection_generation = self.selection_generation;

        // Clear approval bubbles when switching to a different conversation.
        if previous_conversation_id != selected_conversation_id {
            self.state.approval_bubbles.clear();
        }

        self.state.conversations = conversations;
        self.state.active_conversation_id = selected_conversation_id;
        self.conversation_id = selected_conversation_id;
        self.state.conversation_title = selected_conversation_title;

        let should_reset_autoscroll = match &load_state {
            ConversationLoadState::Loading {
                conversation_id,
                generation,
            }
            | ConversationLoadState::Ready {
                conversation_id,
                generation,
            }
            | ConversationLoadState::Error {
                conversation_id,
                generation,
                ..
            } => {
                let changed = previous_conversation_id != Some(*conversation_id)
                    || previous_selection_generation != *generation;
                self.selection_generation = *generation;
                changed
            }
            ConversationLoadState::Idle => {
                let changed =
                    previous_conversation_id.is_some() || previous_selection_generation != 0;
                self.selection_generation = 0;
                changed
            }
        };

        if should_reset_autoscroll {
            self.state.chat_autoscroll_enabled = true;
            self.chat_scroll_handle.scroll_to_bottom();
            self.maybe_scroll_chat_to_bottom(cx);
        }

        match &load_state {
            ConversationLoadState::Ready { .. } => {
                let current_model = self.state.current_model.clone();
                self.state.messages = Self::messages_from_payload(transcript, &current_model);
            }
            ConversationLoadState::Loading { .. } | ConversationLoadState::Error { .. } => {}
            ConversationLoadState::Idle => {
                if selected_conversation_id.is_none() {
                    self.state.messages.clear();
                }
            }
        }

        let was_streaming = matches!(self.state.streaming, StreamingState::Streaming { .. });
        let was_thinking = self
            .state
            .thinking_content
            .as_ref()
            .is_some_and(|content| !content.is_empty());
        self.state.streaming = Self::streaming_state_from_snapshot(&streaming, &load_state);
        // show_thinking is view-local and sticky — do NOT overwrite from store snapshot
        let has_thinking = !streaming.thinking_buffer.is_empty();
        self.state.thinking_content = has_thinking.then_some(streaming.thinking_buffer);
        self.state.sync_conversation_dropdown_index();

        if !should_reset_autoscroll
            && (was_streaming
                || was_thinking
                || has_thinking
                || matches!(self.state.streaming, StreamingState::Streaming { .. }))
        {
            self.maybe_scroll_chat_to_bottom(cx);
        }

        cx.notify();
    }

    /// Apply settings/profile data from the store snapshot so profiles are
    /// available on first render without waiting for the async presenter.
    pub fn apply_settings_snapshot(
        &mut self,
        settings: crate::ui_gpui::app_store::SettingsStoreSnapshot,
    ) {
        self.state.profiles = settings.profiles;
        self.state.selected_profile_id = settings.selected_profile_id.or_else(|| {
            self.state
                .profiles
                .iter()
                .find(|p| p.is_default)
                .map(|p| p.id)
        });
        self.state.sync_current_model_from_profile();
        self.state.sync_profile_dropdown_index();
    }

    /// Set the current conversation ID
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn set_conversation_id(&mut self, id: Uuid) {
        self.conversation_id = Some(id);
        self.selection_generation = 0;
        self.state.active_conversation_id = Some(id);
        self.state.sync_conversation_dropdown_index();
        self.state.sync_conversation_title_from_active();
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub(super) fn emit(&self, event: UserEvent) {
        tracing::info!("ChatView::emit called with event: {:?}", event);
        if let Some(bridge) = &self.bridge {
            tracing::info!("ChatView::emit - bridge is Some, calling bridge.emit");
            bridge.emit(event);
        } else {
            tracing::warn!("ChatView: No bridge set, cannot emit event: {:?}", event);
        }
    }

    pub(super) fn current_or_active_conversation_id(&self) -> Option<Uuid> {
        self.state
            .active_conversation_id
            .or(self.conversation_id)
            .or_else(|| {
                self.state
                    .conversations
                    .first()
                    .map(|conversation| conversation.id)
            })
    }

    /// @plan PLAN-20260304-GPUIREMEDIATE.P08
    /// @requirement REQ-ARCH-005.1
    /// @pseudocode analysis/pseudocode/03-main-panel-integration.md:014-127
    pub(super) fn select_conversation_at_index(
        &mut self,
        index: usize,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.conversations.is_empty() {
            return;
        }

        let bounded = index.min(self.state.conversations.len() - 1);
        let conversation_id = self.state.conversations[bounded].id;
        tracing::info!(
            conversation_id = %conversation_id,
            index = bounded,
            total = self.state.conversations.len(),
            "ChatView: selecting conversation from dropdown"
        );
        let switching_conversation = self.state.active_conversation_id != Some(conversation_id);
        self.state.conversation_dropdown_index = bounded;
        self.state.conversation_dropdown_open = false;
        self.state.conversation_title_editing = false;
        if switching_conversation {
            self.state.chat_autoscroll_enabled = true;
            self.chat_scroll_handle.scroll_to_bottom();
        }
        selection_intent_channel().request_select(conversation_id);
        cx.notify();
    }

    pub fn toggle_conversation_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.conversation_dropdown_open = !self.state.conversation_dropdown_open;
        if self.state.conversation_dropdown_open {
            self.state.profile_dropdown_open = false;
            self.state.conversation_title_editing = false;
            self.state.sync_conversation_dropdown_index();
        }
        tracing::info!(
            open = self.state.conversation_dropdown_open,
            count = self.state.conversations.len(),
            selected_index = self.state.conversation_dropdown_index,
            "ChatView: toggled conversation dropdown"
        );
        cx.notify();
    }

    #[must_use]
    pub const fn conversation_dropdown_open(&self) -> bool {
        self.state.conversation_dropdown_open
    }

    pub fn move_conversation_dropdown_selection(
        &mut self,
        delta: isize,
        cx: &mut gpui::Context<Self>,
    ) {
        if !self.state.conversation_dropdown_open || self.state.conversations.is_empty() {
            return;
        }

        let len = self.state.conversations.len().cast_signed();
        let current = self.state.conversation_dropdown_index.cast_signed();
        let next = (current + delta).clamp(0, len - 1).cast_unsigned();
        if next != self.state.conversation_dropdown_index {
            self.state.conversation_dropdown_index = next;
            cx.notify();
        }
    }

    pub fn confirm_conversation_dropdown_selection(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_dropdown_open {
            return;
        }
        self.select_conversation_at_index(self.state.conversation_dropdown_index, cx);
    }

    pub fn select_conversation_by_id(
        &mut self,
        conversation_id: Uuid,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(index) = self
            .state
            .conversations
            .iter()
            .position(|conversation| conversation.id == conversation_id)
        {
            self.select_conversation_at_index(index, cx);
        }
    }

    pub fn start_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(id) = self.current_or_active_conversation_id() {
            self.state.conversation_dropdown_open = false;
            self.state.conversation_title_editing = true;
            self.state.conversation_title_input = self.state.conversation_title.clone();
            self.state.rename_replace_on_next_char = true;
            self.state.active_conversation_id = Some(id);
            self.conversation_id = Some(id);
            // Rename mode is local UI state; persistence happens on ConfirmRenameConversation.
            // Emitting StartRenameConversation here causes presenter-driven re-activation cycles.
            cx.notify();
        }
    }

    pub fn submit_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }

        if let Some(id) = self.current_or_active_conversation_id() {
            let title = self.state.conversation_title_input.trim().to_string();
            if title.is_empty() {
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.rename_replace_on_next_char = false;
                self.state.sync_conversation_title_from_active();
                cx.notify();
                return;
            }

            self.state.conversation_title.clone_from(&title);
            if let Some(conversation) = self
                .state
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
            {
                conversation.title.clone_from(&title);
            }

            self.state.conversation_title_editing = false;
            self.state.conversation_title_input.clear();
            self.state.rename_replace_on_next_char = false;
            self.emit(UserEvent::ConfirmRenameConversation { id, title });
            cx.notify();
        }
    }

    pub fn cancel_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        self.state.conversation_title_editing = false;
        self.state.conversation_title_input.clear();
        self.state.rename_replace_on_next_char = false;
        self.state.sync_conversation_title_from_active();
        self.emit(UserEvent::CancelRenameConversation);
        cx.notify();
    }

    pub fn handle_rename_backspace(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        if self.state.rename_replace_on_next_char {
            self.state.conversation_title_input.clear();
            self.state.rename_replace_on_next_char = false;
        } else {
            self.state.conversation_title_input.pop();
        }
        cx.notify();
    }

    #[must_use]
    pub const fn conversation_title_editing(&self) -> bool {
        self.state.conversation_title_editing
    }

    pub(super) fn select_profile_at_index(&mut self, index: usize, cx: &mut gpui::Context<Self>) {
        if self.state.profiles.is_empty() {
            return;
        }

        let bounded = index.min(self.state.profiles.len() - 1);
        let profile_id = self.state.profiles[bounded].id;
        self.state.profile_dropdown_index = bounded;
        self.state.selected_profile_id = Some(profile_id);
        self.state.profile_dropdown_open = false;
        self.state.sync_current_model_from_profile();
        self.emit(UserEvent::SelectChatProfile { id: profile_id });
        cx.notify();
    }

    pub fn toggle_profile_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.profile_dropdown_open = !self.state.profile_dropdown_open;
        if self.state.profile_dropdown_open {
            self.state.conversation_dropdown_open = false;
            self.state.sync_profile_dropdown_index();
        }
        tracing::info!(
            open = self.state.profile_dropdown_open,
            count = self.state.profiles.len(),
            selected_index = self.state.profile_dropdown_index,
            "ChatView: toggled profile dropdown"
        );
        cx.notify();
    }

    #[must_use]
    pub const fn profile_dropdown_open(&self) -> bool {
        self.state.profile_dropdown_open
    }

    pub(super) fn active_input_text(&self) -> &str {
        if self.state.sidebar_search_focused {
            &self.state.sidebar_search_query
        } else if self.state.conversation_title_editing {
            &self.state.conversation_title_input
        } else {
            &self.state.input_text
        }
    }

    pub(super) const fn active_cursor_position(&self) -> usize {
        if self.state.sidebar_search_focused {
            self.state.sidebar_search_query.len()
        } else if self.state.conversation_title_editing {
            self.state.conversation_title_input.len()
        } else {
            self.state.cursor_position
        }
    }

    pub fn move_profile_dropdown_selection(&mut self, delta: isize, cx: &mut gpui::Context<Self>) {
        if !self.state.profile_dropdown_open || self.state.profiles.is_empty() {
            return;
        }

        let len = self.state.profiles.len().cast_signed();
        let current = self.state.profile_dropdown_index.cast_signed();
        let next = (current + delta).clamp(0, len - 1).cast_unsigned();
        if next != self.state.profile_dropdown_index {
            self.state.profile_dropdown_index = next;
            cx.notify();
        }
    }

    pub fn confirm_profile_dropdown_selection(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.profile_dropdown_open {
            return;
        }
        self.select_profile_at_index(self.state.profile_dropdown_index, cx);
    }

    /// Compute absolute left position for the profile dropdown menu.
    ///
    /// `sidebar_toggle_offset` is 36px in popout+sidebar-hidden mode (28px
    /// toggle button + 8px gap), 0 otherwise.
    pub(super) fn compute_profile_dropdown_left(
        window_width: Pixels,
        sidebar_toggle_offset: f32,
    ) -> Pixels {
        let min_left = px(12.0);
        let dropdown_width = px(260.0 * Theme::ui_scale());
        // chat-title-bar left padding (12) + conversation selector width (220)
        // + gap (8) + new button width (28) + gap (8) + sidebar toggle offset
        let preferred = px(276.0 + sidebar_toggle_offset);
        let max_left = (window_width - dropdown_width - min_left).max(min_left);
        preferred.max(min_left).min(max_left)
    }

    pub fn handle_paste(&mut self, text: &str, cx: &mut gpui::Context<Self>) {
        if self.state.sidebar_search_focused {
            self.state.sidebar_search_query.push_str(text);
            self.trigger_sidebar_search(cx);
            cx.notify();
            return;
        }
        if self.state.conversation_title_editing {
            if self.state.rename_replace_on_next_char {
                self.state.conversation_title_input.clear();
                self.state.rename_replace_on_next_char = false;
            }
            self.state.conversation_title_input.push_str(text);
            cx.notify();
            return;
        }
        if self.state.conversation_dropdown_open || self.state.profile_dropdown_open {
            return;
        }
        let pos = self.state.cursor_position.min(self.state.input_text.len());
        self.state.input_text.insert_str(pos, text);
        self.state.cursor_position = pos + text.len();
        cx.notify();
    }

    /// Handle select-all (Cmd+A)
    pub const fn handle_select_all(&mut self, _cx: &mut gpui::Context<Self>) {
        // No visual selection yet, but move cursor to end
        self.state.cursor_position = self.state.input_text.len();
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.cursor_position > 0 {
            let prev = self.state.input_text[..self.state.cursor_position]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.state.cursor_position = prev;
            cx.notify();
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.cursor_position < self.state.input_text.len() {
            let next = self.state.input_text[self.state.cursor_position..]
                .char_indices()
                .nth(1)
                .map_or(self.state.input_text.len(), |(i, _)| {
                    self.state.cursor_position + i
                });
            self.state.cursor_position = next;
            cx.notify();
        }
    }

    pub fn scroll_chat_page_up(&mut self, cx: &mut gpui::Context<Self>) {
        let current_offset = self.chat_scroll_handle.offset();
        let viewport_height = self.chat_scroll_handle.bounds().size.height;
        let page_delta = viewport_height.max(px(80.0));
        let target_offset = (current_offset.y + page_delta).min(Pixels::ZERO);

        self.chat_scroll_handle
            .set_offset(point(current_offset.x, target_offset));
        self.state.chat_autoscroll_enabled = false;
        cx.notify();
    }

    pub fn scroll_chat_page_down(&mut self, cx: &mut gpui::Context<Self>) {
        let current_offset = self.chat_scroll_handle.offset();
        let viewport_height = self.chat_scroll_handle.bounds().size.height;
        let page_delta = viewport_height.max(px(80.0));
        let max_offset = self.chat_scroll_handle.max_offset();
        let target_offset = (current_offset.y - page_delta).max(-max_offset.height);

        self.chat_scroll_handle
            .set_offset(point(current_offset.x, target_offset));
        self.state.chat_autoscroll_enabled =
            max_offset.height <= Pixels::ZERO || target_offset <= -max_offset.height + px(8.0);
        cx.notify();
    }

    pub fn scroll_chat_to_top(&mut self, cx: &mut gpui::Context<Self>) {
        let current_offset = self.chat_scroll_handle.offset();
        self.chat_scroll_handle
            .set_offset(point(current_offset.x, Pixels::ZERO));
        self.state.chat_autoscroll_enabled = false;
        cx.notify();
    }

    pub fn scroll_chat_to_end(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.chat_autoscroll_enabled = true;
        self.chat_scroll_handle.scroll_to_bottom();
        self.maybe_scroll_chat_to_bottom(cx);
        cx.notify();
    }

    /// Move cursor to start of line
    pub fn move_cursor_home(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.cursor_position = 0;
        cx.notify();
    }

    /// Move cursor to end of line
    pub fn move_cursor_end(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.cursor_position = self.state.input_text.len();
        cx.notify();
    }

    /// Handle backspace key (called from `MainPanel`)
    pub fn handle_backspace(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_title_editing {
            self.handle_rename_backspace(cx);
            return;
        }

        if self.state.conversation_dropdown_open {
            return;
        }

        if self.state.profile_dropdown_open {
            return;
        }
        if self.state.cursor_position > 0 && !self.state.input_text.is_empty() {
            let pos = self.state.cursor_position.min(self.state.input_text.len());
            // Find the previous char boundary so we delete a whole character,
            // not a single byte inside a multi-byte character like ´ or é.
            let prev = self.state.input_text[..pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.state.input_text.drain(prev..pos);
            self.state.cursor_position = prev;
        }
        cx.notify();
    }

    /// Handle enter key (called from `MainPanel`)
    pub fn handle_enter(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_title_editing {
            self.submit_rename_conversation(cx);
            return;
        }

        if self.state.conversation_dropdown_open {
            self.confirm_conversation_dropdown_selection(cx);
            return;
        }

        if self.state.profile_dropdown_open {
            self.confirm_profile_dropdown_selection(cx);
            return;
        }

        if matches!(self.state.streaming, StreamingState::Streaming { .. }) {
            tracing::info!("ChatView::handle_enter ignored while stream is active");
            return;
        }

        if !self.state.input_text.trim().is_empty() {
            let text = self.state.input_text.clone();
            tracing::info!("ChatView::handle_enter - emitting SendMessage: {}", text);
            self.send_message_and_start_streaming(text, cx);
        }
    }

    pub(super) fn send_message_and_start_streaming(
        &mut self,
        text: String,
        cx: &mut gpui::Context<Self>,
    ) {
        self.emit(UserEvent::SendMessage { text });
        self.state.input_text.clear();
        self.state.cursor_position = 0;
        self.state.chat_autoscroll_enabled = true;
        self.state.conversation_dropdown_open = false;
        self.state.profile_dropdown_open = false;
        self.state.conversation_title_editing = false;
        self.state.streaming = StreamingState::Streaming {
            content: String::new(),
            done: false,
        };
        self.maybe_scroll_chat_to_bottom(cx);
        cx.notify();
    }
}

#[cfg(test)]
#[path = "approval_tests.rs"]
mod approval_tests;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod mod_tests;
