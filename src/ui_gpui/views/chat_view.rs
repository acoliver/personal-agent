//! Chat view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P04
//! @requirement REQ-GPUI-003

use crate::events::types::UserEvent;
use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, ProfileSummary, ViewCommand,
};
use crate::ui_gpui::app_store::{ChatStoreSnapshot, ConversationLoadState, StreamingStoreSnapshot};

use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::components::AssistantBubble;
use crate::ui_gpui::selection_intent_channel;
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, point, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight,
    MouseButton, Pixels, ScrollDelta, ScrollHandle, ScrollWheelEvent, SharedString, UTF16Selection,
};
use std::ops::Range;
use std::sync::Arc;
use uuid::Uuid;

/// Represents a single message in the chat (for UI display)
#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub model_id: Option<String>,
    pub timestamp: Option<u64>,
}

/// Message role enum
#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            thinking: None,
            model_id: None,
            timestamp: None,
        }
    }

    pub fn assistant(content: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            thinking: None,
            model_id: Some(model_id.into()),
            timestamp: None,
        }
    }

    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Streaming state for AI responses
#[derive(Clone, Debug, PartialEq)]
pub enum StreamingState {
    Idle,
    Streaming { content: String, done: bool },
    Error(String),
}

/// Main chat state container
#[derive(Clone)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub streaming: StreamingState,
    pub show_thinking: bool,
    pub thinking_content: Option<String>,
    pub input_text: String,
    pub cursor_position: usize,
    pub conversation_title: String,
    pub current_model: String,
    pub profiles: Vec<ProfileSummary>,
    pub selected_profile_id: Option<Uuid>,
    pub profile_dropdown_open: bool,
    pub profile_dropdown_index: usize,
    pub conversations: Vec<ConversationSummary>,
    pub active_conversation_id: Option<Uuid>,
    pub conversation_dropdown_open: bool,
    pub conversation_dropdown_index: usize,
    pub conversation_title_editing: bool,
    pub conversation_title_input: String,
    pub rename_replace_on_next_char: bool,
    pub chat_autoscroll_enabled: bool,
    /// IME marked (composing) text range in UTF-16 offsets, if any.
    pub marked_range: Option<Range<usize>>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            streaming: StreamingState::Idle,
            show_thinking: false,
            thinking_content: None,
            input_text: String::new(),
            cursor_position: 0,
            conversation_title: "New Conversation".to_string(),
            conversations: Vec::new(),
            active_conversation_id: None,
            conversation_dropdown_open: false,
            conversation_dropdown_index: 0,
            conversation_title_editing: false,
            conversation_title_input: String::new(),
            rename_replace_on_next_char: false,
            chat_autoscroll_enabled: true,
            marked_range: None,
            current_model: "No profile selected".to_string(),
            profiles: Vec::new(),
            selected_profile_id: None,
            profile_dropdown_open: false,
            profile_dropdown_index: 0,
        }
    }
}

impl ChatState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_streaming(mut self, state: StreamingState) -> Self {
        self.streaming = state;
        self
    }

    pub fn with_thinking(mut self, enabled: bool, content: Option<String>) -> Self {
        self.show_thinking = enabled;
        self.thinking_content = content;
        self
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    pub fn set_streaming(&mut self, state: StreamingState) {
        self.streaming = state;
    }

    pub fn set_thinking(&mut self, enabled: bool, content: Option<String>) {
        self.show_thinking = enabled;
        self.thinking_content = content;
    }

    fn selected_conversation(&self) -> Option<&ConversationSummary> {
        self.active_conversation_id
            .and_then(|id| {
                self.conversations
                    .iter()
                    .find(|conversation| conversation.id == id)
            })
            .or_else(|| self.conversations.first())
    }

    fn sync_conversation_title_from_active(&mut self) {
        self.conversation_title = self
            .selected_conversation()
            .map(|conversation| {
                if conversation.title.trim().is_empty() {
                    "Untitled Conversation".to_string()
                } else {
                    conversation.title.clone()
                }
            })
            .unwrap_or_else(|| "New Conversation".to_string());
    }

    fn sync_conversation_dropdown_index(&mut self) {
        self.conversation_dropdown_index = self
            .active_conversation_id
            .and_then(|id| {
                self.conversations
                    .iter()
                    .position(|conversation| conversation.id == id)
            })
            .unwrap_or(0)
            .min(self.conversations.len().saturating_sub(1));
    }

    fn selected_profile(&self) -> Option<&ProfileSummary> {
        self.selected_profile_id
            .and_then(|id| self.profiles.iter().find(|profile| profile.id == id))
            .or_else(|| self.profiles.iter().find(|profile| profile.is_default))
    }

    fn sync_current_model_from_profile(&mut self) {
        self.current_model = self
            .selected_profile()
            .map(|profile| profile.model_id.clone())
            .unwrap_or_else(|| "No profile selected".to_string());
    }

    fn sync_profile_dropdown_index(&mut self) {
        self.profile_dropdown_index = self
            .selected_profile_id
            .and_then(|id| self.profiles.iter().position(|profile| profile.id == id))
            .unwrap_or(0)
            .min(self.profiles.len().saturating_sub(1));
    }
}

/// Chat view component with event handling
///
/// @plan PLAN-20250130-GPUIREDUX.P04
pub struct ChatView {
    pub state: ChatState,
    focus_handle: FocusHandle,
    bridge: Option<Arc<GpuiBridge>>,
    conversation_id: Option<Uuid>,
    chat_scroll_handle: ScrollHandle,
}

impl ChatView {
    pub fn new(state: ChatState, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
            bridge: None,
            conversation_id: None,
            chat_scroll_handle: ScrollHandle::new(),
        }
    }

    fn refresh_autoscroll_state_from_handle(&mut self) {
        let offset = self.chat_scroll_handle.offset();
        let max_offset = self.chat_scroll_handle.max_offset();
        let distance_from_bottom = (max_offset.height + offset.y).abs();
        self.state.chat_autoscroll_enabled = distance_from_bottom <= px(8.0);
    }

    fn refresh_autoscroll_state_after_wheel(&mut self, event: &ScrollWheelEvent) {
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

        // Any downward wheel movement while sticky-follow is disabled should re-enable follow.
        // This avoids depending on offset timing when content height changes in the same frame.
        if delta.y < px(0.0) {
            self.state.chat_autoscroll_enabled = true;
            return;
        }

        self.refresh_autoscroll_state_from_handle();
    }

    fn maybe_scroll_chat_to_bottom(&self, cx: &mut gpui::Context<Self>) {
        if self.state.chat_autoscroll_enabled {
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
    fn messages_from_payload(
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
                        ChatMessage::assistant(message.content, current_model.to_string())
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
    fn streaming_state_from_snapshot(
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

        self.state.conversations = conversations;
        self.state.active_conversation_id = selected_conversation_id;
        self.conversation_id = selected_conversation_id;
        self.state.conversation_title = selected_conversation_title;

        match &load_state {
            ConversationLoadState::Ready { .. } => {
                let current_model = self.state.current_model.clone();
                self.state.messages = Self::messages_from_payload(transcript, &current_model);
            }
            ConversationLoadState::Loading { .. } => {}
            ConversationLoadState::Error { .. } => {}
            ConversationLoadState::Idle => {
                if selected_conversation_id.is_none() {
                    self.state.messages.clear();
                }
            }
        }

        self.state.streaming = Self::streaming_state_from_snapshot(&streaming, &load_state);
        // show_thinking is view-local and sticky — do NOT overwrite from store snapshot
        self.state.thinking_content =
            (!streaming.thinking_buffer.is_empty()).then_some(streaming.thinking_buffer);
        self.state.sync_conversation_dropdown_index();
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
        self.state.active_conversation_id = Some(id);
        self.state.sync_conversation_dropdown_index();
        self.state.sync_conversation_title_from_active();
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P04
    fn emit(&self, event: UserEvent) {
        tracing::info!("ChatView::emit called with event: {:?}", event);
        if let Some(bridge) = &self.bridge {
            tracing::info!("ChatView::emit - bridge is Some, calling bridge.emit");
            bridge.emit(event);
        } else {
            tracing::warn!("ChatView: No bridge set, cannot emit event: {:?}", event);
        }
    }

    fn current_or_active_conversation_id(&self) -> Option<Uuid> {
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
    fn select_conversation_at_index(&mut self, index: usize, cx: &mut gpui::Context<Self>) {
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
            if matches!(self.state.streaming, StreamingState::Streaming { .. }) {
                tracing::info!("ChatView: stopping active stream before conversation switch");
                self.emit(UserEvent::StopStreaming);
            }
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

    pub fn conversation_dropdown_open(&self) -> bool {
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

        let len = self.state.conversations.len() as isize;
        let current = self.state.conversation_dropdown_index as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
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

            self.state.conversation_title = title.clone();
            if let Some(conversation) = self
                .state
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
            {
                conversation.title = title.clone();
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


    pub fn conversation_title_editing(&self) -> bool {
        self.state.conversation_title_editing
    }

    fn select_profile_at_index(&mut self, index: usize, cx: &mut gpui::Context<Self>) {
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

    pub fn profile_dropdown_open(&self) -> bool {
        self.state.profile_dropdown_open
    }

    fn active_input_text(&self) -> &str {
        if self.state.conversation_title_editing {
            &self.state.conversation_title_input
        } else {
            &self.state.input_text
        }
    }

    fn active_cursor_position(&self) -> usize {
        if self.state.conversation_title_editing {
            self.state.conversation_title_input.len()
        } else {
            self.state.cursor_position
        }
    }

    pub fn move_profile_dropdown_selection(&mut self, delta: isize, cx: &mut gpui::Context<Self>) {
        if !self.state.profile_dropdown_open || self.state.profiles.is_empty() {
            return;
        }

        let len = self.state.profiles.len() as isize;
        let current = self.state.profile_dropdown_index as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
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

    /// Handle paste (Cmd+V) - insert clipboard text at cursor
    pub fn handle_paste(&mut self, text: &str, cx: &mut gpui::Context<Self>) {
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
    pub fn handle_select_all(&mut self, _cx: &mut gpui::Context<Self>) {
        // No visual selection yet, but move cursor to end
        self.state.cursor_position = self.state.input_text.len();
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.cursor_position > 0 {
            let prev = self.state.input_text[..self.state.cursor_position]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
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
                .map(|(i, _)| self.state.cursor_position + i)
                .unwrap_or(self.state.input_text.len());
            self.state.cursor_position = next;
            cx.notify();
        }
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

    /// Handle backspace key (called from MainPanel)
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
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.state.input_text.drain(prev..pos);
            self.state.cursor_position = prev;
        }
        cx.notify();
    }

    /// Handle enter key (called from MainPanel)
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
            self.emit(UserEvent::SendMessage { text });
            self.state.input_text.clear();
            self.state.cursor_position = 0;
            self.state.streaming = StreamingState::Streaming {
                content: String::new(),
                done: false,
            };
            self.maybe_scroll_chat_to_bottom(cx);
            cx.notify();
        }
    }

    /// Handle incoming ViewCommands
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ConversationMessagesLoaded {
                conversation_id,
                selection_generation: _,
                messages,
            } => {
                if self.state.active_conversation_id != Some(conversation_id) {
                    tracing::info!(%conversation_id, "ChatView: ignoring ConversationMessagesLoaded for inactive conversation");
                    return;
                }
                let message_count = messages.len();
                let current_model = self.state.current_model.clone();
                self.state.messages = Self::messages_from_payload(messages, &current_model);
                tracing::info!(%conversation_id, message_count, "ChatView: applied ConversationMessagesLoaded");
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::MessageAppended {
                conversation_id,
                role,
                content,
            } => {
                if self.state.active_conversation_id != Some(conversation_id) {
                    return;
                }
                let chat_msg = match role {
                    crate::presentation::view_command::MessageRole::User => {
                        ChatMessage::user(content)
                    }
                    crate::presentation::view_command::MessageRole::Assistant => {
                        ChatMessage::assistant(content, self.state.current_model.clone())
                    }
                    _ => return,
                };
                self.state.messages.push(chat_msg);
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }



            ViewCommand::ShowThinking { conversation_id } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                if !matches!(self.state.streaming, StreamingState::Streaming { .. }) {
                    self.state.streaming = StreamingState::Streaming {
                        content: String::new(),
                        done: false,
                    };
                }
                self.state.thinking_content = Some(String::new());
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }

            ViewCommand::HideThinking { conversation_id } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                self.state.thinking_content = None;
                cx.notify();
            }
            ViewCommand::AppendStream {
                conversation_id,
                chunk,
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                match &mut self.state.streaming {
                    StreamingState::Streaming { content, .. } => {
                        content.push_str(&chunk);
                    }
                    StreamingState::Idle => {
                        self.state.streaming = StreamingState::Streaming {
                            content: chunk,
                            done: false,
                        };
                    }
                    _ => {}
                }
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::FinalizeStream {
                conversation_id, ..
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }

                let thinking_content = self
                    .state
                    .thinking_content
                    .take()
                    .filter(|thinking| !thinking.is_empty());

                if let StreamingState::Streaming { content, .. } = &self.state.streaming {
                    let mut msg =
                        ChatMessage::assistant(content.clone(), self.state.current_model.clone());
                    if let Some(thinking) = thinking_content {
                        msg = msg.with_thinking(thinking);
                    }
                    self.state.messages.push(msg);
                }
                self.state.streaming = StreamingState::Idle;
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::StreamCancelled {
                conversation_id,
                partial_content,
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }

                if !partial_content.is_empty() {
                    let mut msg =
                        ChatMessage::assistant(partial_content, self.state.current_model.clone());
                    msg.content.push_str(" [cancelled]");
                    self.state.messages.push(msg);
                }
                self.state.streaming = StreamingState::Idle;
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::StreamError {
                conversation_id,
                error,
                ..
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }

                self.state.streaming = StreamingState::Error(error);
                cx.notify();
            }
            ViewCommand::AppendThinking {
                conversation_id,
                content,
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }

                self.state.thinking_content =
                    Some(self.state.thinking_content.clone().unwrap_or_default() + &content);
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::ToggleThinkingVisibility => {
                self.state.show_thinking = !self.state.show_thinking;
                cx.notify();
            }
            ViewCommand::ConversationRenamed { id, new_title } => {
                if let Some(conversation) = self
                    .state
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == id)
                {
                    conversation.title = new_title.clone();
                    conversation.updated_at = chrono::Utc::now();
                }

                if self.state.active_conversation_id == Some(id) {
                    self.state.conversation_title = new_title;
                }
                cx.notify();
            }
            ViewCommand::ConversationListRefreshed { conversations } => {
                let conversation_count = conversations.len();
                tracing::info!(
                    count = conversation_count,
                    "ChatView: received ConversationListRefreshed"
                );

                let previous_active = self.state.active_conversation_id.or(self.conversation_id);
                self.state.conversations = conversations;

                if self.state.conversations.is_empty() {
                    self.state.active_conversation_id = None;
                    self.conversation_id = None;
                    self.state.messages.clear();
                    self.state.streaming = StreamingState::Idle;
                    self.state.thinking_content = None;
                    self.state.conversation_dropdown_open = false;
                    self.state.conversation_dropdown_index = 0;
                    if !self.state.conversation_title_editing {
                        self.state.conversation_title = "New Conversation".to_string();
                    }
                } else {
                    let active_exists = previous_active
                        .map(|id| {
                            self.state
                                .conversations
                                .iter()
                                .any(|conversation| conversation.id == id)
                        })
                        .unwrap_or(false);

                    if active_exists {
                        self.state.active_conversation_id = previous_active;
                        self.conversation_id = previous_active;
                    } else {
                        let fallback = self.state.conversations[0].id;
                        self.state.active_conversation_id = Some(fallback);
                        self.conversation_id = Some(fallback);
                        self.state.messages.clear();
                        self.state.streaming = StreamingState::Idle;
                        self.state.thinking_content = None;
                        self.state.chat_autoscroll_enabled = true;
                    }

                    self.state.sync_conversation_dropdown_index();
                    if !self.state.conversation_title_editing {
                        self.state.sync_conversation_title_from_active();
                    }
                }

                cx.notify();
            }
            ViewCommand::ConversationActivated {
                id,
                selection_generation: _,
            } => {
                self.state.active_conversation_id = Some(id);
                self.conversation_id = Some(id);
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();

                if !self.state.conversation_title_editing {
                    self.state.conversation_title_input.clear();
                    if let Some(conversation) = self
                        .state
                        .conversations
                        .iter()
                        .find(|conversation| conversation.id == id)
                    {
                        self.state.conversation_title = if conversation.title.trim().is_empty() {
                            "Untitled Conversation".to_string()
                        } else {
                            conversation.title.clone()
                        };
                    }
                }

                self.state.sync_conversation_dropdown_index();
                cx.notify();
            }
            ViewCommand::ConversationCreated { id, .. } => {
                self.state.active_conversation_id = Some(id);
                self.conversation_id = Some(id);
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.state.conversation_title = "New Conversation".to_string();

                if !self
                    .state
                    .conversations
                    .iter()
                    .any(|conversation| conversation.id == id)
                {
                    let now = chrono::Utc::now();
                    self.state.conversations.insert(
                        0,
                        ConversationSummary {
                            id,
                            title: "New Conversation".to_string(),
                            updated_at: now,
                            message_count: 0,
                        },
                    );
                }

                self.state.sync_conversation_dropdown_index();
                cx.notify();
            }
            ViewCommand::ConversationTitleUpdated { id, title } => {
                if let Some(conversation) = self
                    .state
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == id)
                {
                    conversation.title = title.clone();
                    conversation.updated_at = chrono::Utc::now();
                }

                if self.state.active_conversation_id == Some(id) {
                    self.state.conversation_title = title;
                }

                cx.notify();
            }
            ViewCommand::ConversationDeleted { id } => {
                self.state
                    .conversations
                    .retain(|conversation| conversation.id != id);
                if self.state.active_conversation_id == Some(id) {
                    self.state.active_conversation_id = self
                        .state
                        .conversations
                        .first()
                        .map(|conversation| conversation.id);
                    self.conversation_id = self.state.active_conversation_id;
                    self.state.messages.clear();
                    self.state.streaming = StreamingState::Idle;
                    self.state.thinking_content = None;
                }
                self.state.sync_conversation_dropdown_index();
                self.state.sync_conversation_title_from_active();
                cx.notify();
            }
            ViewCommand::ConversationCleared => {
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.state.sync_conversation_title_from_active();
                cx.notify();
            }
            ViewCommand::ChatProfilesUpdated {
                profiles,
                selected_profile_id,
            }
            | ViewCommand::ShowSettings {
                profiles,
                selected_profile_id,
            } => {
                let profile_count = profiles.len();
                self.state.profiles = profiles;
                self.state.selected_profile_id = selected_profile_id.or_else(|| {
                    self.state
                        .profiles
                        .iter()
                        .find(|profile| profile.is_default)
                        .map(|profile| profile.id)
                });
                tracing::info!(
                    count = profile_count,
                    selected = ?self.state.selected_profile_id,
                    "ChatView: received profile snapshot"
                );
                self.state.sync_current_model_from_profile();
                self.state.sync_profile_dropdown_index();
                cx.notify();
            }
            ViewCommand::DefaultProfileChanged { profile_id } => {
                self.state.selected_profile_id = profile_id;
                for profile in &mut self.state.profiles {
                    profile.is_default = Some(profile.id) == profile_id;
                }
                self.state.sync_current_model_from_profile();
                self.state.sync_profile_dropdown_index();
                cx.notify();
            }
            _ => {
                // Other commands handled elsewhere
            }
        }
    }

    /// Render the top bar with icon, title, and toolbar buttons
    /// @plan PLAN-20250130-GPUIREDUX.P04
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let show_thinking = self.state.show_thinking;

        div()
            .id("chat-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            // Left: title
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("PersonalAgent")
                    )
            )
            // Right: buttons [T][S][H][+][Settings] with event emission
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // [T] Toggle thinking button
                    .child(
                        div()
                            .id("btn-thinking")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(show_thinking, |d| d.bg(Theme::bg_dark()))
                            .when(!show_thinking, |d| d.hover(|s| s.bg(Theme::bg_dark())))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("T")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                                tracing::info!("Toggle thinking clicked - emitting UserEvent");
                                this.emit(UserEvent::ToggleThinking);
                                // State update comes back via ToggleThinkingVisibility
                            }))
                    )
                    // [R] Rename conversation button
                    .child(
                        div()
                            .id("btn-rename")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("R")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                this.start_rename_conversation(cx);
                            }))
                    )
                    // [H] History button - navigate to history view
                    .child(
                        div()
                            .id("btn-history")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(Theme::bg_darker())
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .active(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("H")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                println!(">>> HISTORY BUTTON CLICKED - using navigation_channel <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::History
                                );
                            }))
                    )
                    // Settings button (gear icon) - navigate to settings view
                    .child(
                        div()
                            .id("btn-settings")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(Theme::bg_darker())
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .active(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("\u{2699}")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                println!(">>> SETTINGS BUTTON CLICKED - using navigation_channel <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::Settings
                                );
                            }))
                    )
                    // Exit/quit button (power icon)
                    .child(
                        div()
                            .id("btn-exit")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(Theme::bg_darker())
                            .hover(|s| s.bg(gpui::rgb(0x8B0000)))
                            .active(|s| s.bg(gpui::rgb(0x5C0000)))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("\u{23FB}")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                std::process::exit(0);
                            }))
                    )
            )
    }

    /// Render the title bar with conversation dropdown and model label
    /// @plan PLAN-20250130-GPUIREDUX.P03
    fn render_title_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let conversation_title = self.state.conversation_title.clone();
        let conversation_title_input = self.state.conversation_title_input.clone();
        let conversation_dropdown_open = self.state.conversation_dropdown_open;
        let conversation_title_editing = self.state.conversation_title_editing;
        let selected_profile = self
            .state
            .selected_profile()
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| "Select profile".to_string());
        let profile_dropdown_open = self.state.profile_dropdown_open;

        div()
            .id("chat-title-bar")
            .h(px(32.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(if conversation_title_editing {
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
                                    .text_size(px(13.0))
                                    .text_color(Theme::text_primary())
                                    .child(if conversation_title_input.is_empty() {
                                        "Enter conversation name".to_string()
                                    } else {
                                        conversation_title_input
                                    }),
                            )
                    } else {
                        div()
                            .id("conversation-dropdown")
                            .min_w(px(220.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(if conversation_dropdown_open {
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
                                    .text_size(px(13.0))
                                    .text_color(Theme::text_primary())
                                    .child(conversation_title),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(10.0))
                                    .text_color(Theme::text_primary())
                                    .child(if conversation_dropdown_open {
                                        "▲"
                                    } else {
                                        "▼"
                                    }),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.toggle_conversation_dropdown(cx);
                                }),
                            )
                    })
                    .child(
                        div()
                            .id("btn-new")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("+")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    tracing::info!("New conversation clicked - emitting UserEvent");
                                    this.emit(UserEvent::NewConversation);
                                    // Clear local state immediately
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
                                    this.chat_scroll_handle.scroll_to_bottom();
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("chat-profile-dropdown")
                            .max_w(px(225.0))
                            .min_w(px(100.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(if profile_dropdown_open {
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
                                            .text_size(px(11.0))
                                            .text_color(Theme::text_primary())
                                            .child(selected_profile),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(px(9.0))
                                            .text_color(Theme::text_secondary())
                                            .child(if profile_dropdown_open {
                                                "▲"
                                            } else {
                                                "▼"
                                            }),
                                    ),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.toggle_profile_dropdown(cx);
                                }),
                            ),
                    ),
            )
    }

    /// Render conversation dropdown overlay at root level so it can float over chat area
    fn render_conversation_dropdown(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active_conversation_id = self.state.active_conversation_id;
        let conversation_dropdown_index = self.state.conversation_dropdown_index;

        div()
            .id("chat-conversation-dropdown-overlay")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    if this.state.conversation_dropdown_open {
                        this.state.conversation_dropdown_open = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("chat-conversation-dropdown-menu")
                    .absolute()
                    .top(px(74.0))
                    .left(px(12.0))
                    .min_w(px(320.0))
                    .max_w(px(520.0))
                    .max_h(px(220.0))
                    .overflow_y_scroll()
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, _| {
                            // Keep clicks inside menu from falling through to overlay close handler.
                        }),
                    )
                    .children(self.state.conversations.iter().enumerate().map(
                        |(index, conversation)| {
                            let conversation_id = conversation.id;
                            let selected = active_conversation_id == Some(conversation_id);
                            let highlighted = conversation_dropdown_index == index;
                            let title = if conversation.title.trim().is_empty() {
                                "Untitled Conversation".to_string()
                            } else {
                                conversation.title.clone()
                            };
                            let count_label = if conversation.message_count == 1 {
                                "1 message".to_string()
                            } else {
                                format!("{} messages", conversation.message_count)
                            };

                            div()
                                .id(SharedString::from(format!(
                                    "chat-conversation-item-{}",
                                    conversation_id
                                )))
                                .w_full()
                                .px(px(8.0))
                                .py(px(6.0))
                                .cursor_pointer()
                                .when(selected, |row| {
                                    row.bg(Theme::accent()).text_color(gpui::white())
                                })
                                .when(!selected && highlighted, |row| {
                                    row.bg(Theme::accent_hover()).text_color(gpui::white())
                                })
                                .when(!selected && !highlighted, |row| {
                                    row.hover(|s| s.bg(Theme::bg_darker()))
                                        .text_color(Theme::text_primary())
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(div().text_size(px(11.0)).child(title))
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(Theme::text_secondary())
                                                .child(count_label),
                                        ),
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _window, cx| {
                                        this.select_conversation_at_index(index, cx);
                                        cx.stop_propagation();
                                    }),
                                )
                        },
                    )),
            )
    }

    /// Render profile dropdown overlay at root level so it is not clipped by chat scroll area
    fn render_profile_dropdown(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("chat-profile-dropdown-overlay")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    if this.state.profile_dropdown_open {
                        this.state.profile_dropdown_open = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("chat-profile-dropdown-menu")
                    .absolute()
                    .top(px(74.0))
                    .right(px(12.0))
                    .w(px(260.0))
                    .max_w(px(300.0))
                    .max_h(px(220.0))
                    .overflow_y_scroll()
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .shadow_lg()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, _| {
                            // Keep clicks inside menu from falling through to overlay close handler.
                        }),
                    )
                    .children(
                        self.state
                            .profiles
                            .iter()
                            .enumerate()
                            .map(|(index, profile)| {
                                let is_selected =
                                    self.state.selected_profile_id == Some(profile.id);
                                let is_highlighted = self.state.profile_dropdown_index == index;
                                let label = if profile.is_default {
                                    format!("{} (default)", profile.name)
                                } else {
                                    profile.name.clone()
                                };

                                div()
                                    .id(SharedString::from(format!(
                                        "chat-profile-item-{}",
                                        profile.id
                                    )))
                                    .w_full()
                                    .px(px(8.0))
                                    .py(px(6.0))
                                    .cursor_pointer()
                                    .when(is_selected, |row| {
                                        row.bg(Theme::accent()).text_color(gpui::white())
                                    })
                                    .when(!is_selected && is_highlighted, |row| {
                                        row.bg(Theme::accent_hover()).text_color(gpui::white())
                                    })
                                    .when(!is_selected && !is_highlighted, |row| {
                                        row.hover(|s| s.bg(Theme::bg_darker()))
                                            .text_color(Theme::text_primary())
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(div().text_size(px(11.0)).child(label))
                                            .child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .text_color(Theme::text_secondary())
                                                    .child(profile.model_id.clone()),
                                            ),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _window, cx| {
                                            this.select_profile_at_index(index, cx);
                                        }),
                                    )
                            }),
                    ),
            )
    }

    /// Render the chat area with messages
    /// @plan PLAN-20250130-GPUIREDUX.P03
    fn render_chat_area(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
                            .text_size(px(14.0))
                            .text_color(Theme::text_secondary())
                            .child("No messages yet"),
                    )
                },
            )
            // Messages
            .when(!messages.is_empty(), |d| {
                d.children(messages.into_iter().enumerate().map(|(i, msg)| {
                    let id = SharedString::from(format!("msg-{}", i));
                    div()
                        .id(id)
                        .w_full()
                        .flex()
                        .justify_start()
                        .child(self.render_message(&msg, show_thinking))
                }))
            })
            // Streaming message
            .when(matches!(streaming, StreamingState::Streaming { .. }), |d| {
                let (content, _done) = match &streaming {
                    StreamingState::Streaming { content, done } => (content.clone(), *done),
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
    fn render_message(&self, msg: &ChatMessage, show_thinking: bool) -> impl IntoElement {
        match msg.role {
            MessageRole::User => self.render_user_message(&msg.content),
            MessageRole::Assistant => self.render_assistant_message(msg, show_thinking),
        }
    }

    /// Render user message - right aligned, green bubble
    fn render_user_message(&self, content: &str) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .justify_end()
            .child(
                div()
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::user_bubble())
                    .text_size(px(13.0))
                    .text_color(Theme::text_primary())
                    .child(content.to_string()),
            )
            .into_any_element()
    }

    /// Render assistant message - left aligned, dark bubble with model label
    fn render_assistant_message(&self, msg: &ChatMessage, show_thinking: bool) -> gpui::AnyElement {
        let model_id = msg
            .model_id
            .clone()
            .unwrap_or_else(|| "Assistant".to_string());

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(2.0))
            // Model label
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(Theme::text_muted())
                    .child(model_id),
            )
            // Thinking block (if present and visible)
            .when(msg.thinking.is_some() && show_thinking, |d| {
                d.child(self.render_thinking_block(msg.thinking.as_ref().unwrap()))
            })
            // Response bubble
            .child(
                div()
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::assistant_bubble())
                    .border_1()
                    .border_color(Theme::border())
                    .text_size(px(13.0))
                    .text_color(Theme::text_primary())
                    .child(msg.content.clone()),
            )
            .into_any_element()
    }

    /// Render thinking block with blue tint
    fn render_thinking_block(&self, content: &str) -> impl IntoElement {
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
                            .text_size(px(9.0))
                            .text_color(Theme::text_muted())
                            .child("Thinking"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_muted())
                            .italic()
                            .child(content.to_string()),
                    ),
            )
    }

    fn render_input_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
                        // Keep line growth aligned with the visible composer width and font metrics.
                        // A conservative width avoids premature height jumps while still wrapping
                        // before the send/stop button area.
                        let approx_chars_per_line = 65usize;
                        (len + (approx_chars_per_line - 1)) / approx_chars_per_line
                    }
                })
                .sum::<usize>()
                .max(1)
        };

        // Keep composer capped to about 25% of a typical 600px chat panel.
        let max_composer_height = 150.0;
        let min_composer_height = 44.0;
        let line_height = 18.0;
        let computed_height = wrapped_line_count as f32 * line_height + 14.0;
        let input_box_height = computed_height.clamp(min_composer_height, max_composer_height);
        let text_content = if input_text.is_empty() {
            "Type a message...".to_string()
        } else {
            let before = &input_text[..cursor_pos];
            let after = &input_text[cursor_pos..];
            format!("{}|{}", before, after)
        };

        div()
            .w_full()
            .flex()
            .items_end()
            .min_h(px(56.0))
            .gap(px(Theme::SPACING_SM))
            .p(px(Theme::SPACING_MD))
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::bg_dark())
            .overflow_hidden()
            // Text input field
            .child(
                div()
                    .id("input-field")
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
                        let focus_handle = focus_handle.clone();
                        move |_, window, cx| {
                            window.focus(&focus_handle, cx);
                        }
                    })
                    .child(
                        div()
                            .w_full()
                            .text_size(px(13.0))
                            .line_height(px(line_height))
                            .text_color(if input_text.is_empty() {
                                Theme::text_secondary()
                            } else {
                                Theme::text_primary()
                            })
                            .whitespace_normal()
                            .child(text_content),
                    ),
            )
            // Send/Stop button with event emission
            // @plan PLAN-20250130-GPUIREDUX.P04
            .child(
                div()
                    .id(if is_streaming { "stop-btn" } else { "send-btn" })
                    .flex_shrink_0()
                    .min_h(px(36.0))
                    .px(px(Theme::SPACING_MD))
                    .py(px(Theme::SPACING_SM))
                    .rounded(px(Theme::RADIUS_MD))
                    .cursor_pointer()
                    .when(is_streaming, |d| {
                        d.bg(Theme::error())
                            .text_color(gpui::white())
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
                                    if matches!(
                                        this.state.streaming,
                                        StreamingState::Streaming { .. }
                                    ) {
                                        tracing::info!(
                                            "Send button ignored while stream is active"
                                        );
                                        return;
                                    }

                                    let text = this.state.input_text.clone();
                                    if !text.trim().is_empty() {
                                        tracing::info!(
                                            "Send button clicked - emitting SendMessage: {}",
                                            text
                                        );
                                        this.emit(UserEvent::SendMessage { text });
                                        this.state.input_text.clear();
                                        this.state.cursor_position = 0;
                                        this.state.streaming = StreamingState::Streaming {
                                            content: String::new(),
                                            done: false,
                                        };
                                        this.maybe_scroll_chat_to_bottom(cx);
                                        cx.notify();
                                    }
                                }),
                            )
                    })
                    .when(!is_streaming && !has_text, |d| {
                        d.bg(Theme::bg_dark())
                            .text_color(Theme::text_secondary())
                            .child("Send")
                    }),
            )
    }
}

impl gpui::Focusable for ChatView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ── UTF-8 ↔ UTF-16 helpers for InputHandler ──────────────────────────
fn utf8_offset_to_utf16(text: &str, utf8_offset: usize) -> usize {
    text[..utf8_offset.min(text.len())]
        .encode_utf16()
        .count()
}

fn utf16_offset_to_utf8(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (byte_idx, ch) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_idx;
        }
        utf16_count += ch.len_utf16();
    }
    text.len()
}

impl gpui::EntityInputHandler for ChatView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = self.active_input_text();
        let start = utf16_offset_to_utf8(text, range.start);
        let end = utf16_offset_to_utf8(text, range.end);
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.active_input_text();
        let cursor_utf8 = self.active_cursor_position().min(text.len());
        let cursor_utf16 = utf8_offset_to_utf16(text, cursor_utf8);
        Some(UTF16Selection {
            range: cursor_utf16..cursor_utf16,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        self.state.marked_range.clone()
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        self.state.marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state.marked_range = None;

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

        let input = &mut self.state.input_text;
        let (start_utf8, end_utf8) = if let Some(r) = range {
            (
                utf16_offset_to_utf8(input, r.start),
                utf16_offset_to_utf8(input, r.end),
            )
        } else {
            let pos = self.state.cursor_position.min(input.len());
            (pos, pos)
        };

        input.replace_range(start_utf8..end_utf8, text);
        self.state.cursor_position = start_utf8 + text.len();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.conversation_dropdown_open || self.state.profile_dropdown_open {
            return;
        }
        if self.state.conversation_title_editing {
            self.state.conversation_title_input.push_str(new_text);
            cx.notify();
            return;
        }

        let input = &mut self.state.input_text;
        let (start_utf8, end_utf8) = if let Some(r) = range {
            (
                utf16_offset_to_utf8(input, r.start),
                utf16_offset_to_utf8(input, r.end),
            )
        } else if let Some(ref mr) = self.state.marked_range {
            (
                utf16_offset_to_utf8(input, mr.start),
                utf16_offset_to_utf8(input, mr.end),
            )
        } else {
            let pos = self.state.cursor_position.min(input.len());
            (pos, pos)
        };

        input.replace_range(start_utf8..end_utf8, new_text);
        self.state.cursor_position = start_utf8 + new_text.len();

        // Compute marked range in UTF-16 over the newly inserted text
        let mark_start_utf16 = utf8_offset_to_utf16(input, start_utf8);
        let mark_end_utf16 = mark_start_utf16
            + new_text.encode_utf16().count();
        self.state.marked_range = Some(mark_start_utf16..mark_end_utf16);

        if let Some(sel) = new_selected_range {
            let sel_utf8 = utf16_offset_to_utf8(input, mark_start_utf16 + sel.start);
            self.state.cursor_position = sel_utf8;
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Return the element bounds so the IME candidate window appears near the input area
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        let text = self.active_input_text();
        Some(utf8_offset_to_utf16(text, self.active_cursor_position()))
    }
}

impl gpui::Render for ChatView {
    #[rustfmt::skip]
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        tracing::info!(
            chat_view_entity_id = ?cx.entity_id(),
            conversation_title = %self.state.conversation_title,
            active_conversation_id = ?self.state.active_conversation_id,
            message_count = self.state.messages.len(),
            profile_count = self.state.profiles.len(),
            selected_profile_id = ?self.state.selected_profile_id,
            "ChatView::render state snapshot"
        );

        div()
            .id("chat-view")
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .child(
                canvas(
                    |bounds, _window: &mut gpui::Window, _cx: &mut gpui::App| bounds,
                    {
                        let entity = cx.entity().clone();
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
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;

                    // === KEYBOARD SHORTCUTS (Cmd+key) ===
                    if modifiers.platform {
                        match key.as_str() {
                            "h" => {
                                println!(">>> Cmd+H pressed - navigating to History <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::History,
                                );
                                return;
                            }
                            "," => {
                                println!(">>> Cmd+, pressed - navigating to Settings <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::Settings,
                                );
                                return;
                            }
                            "n" => {
                                println!(">>> Cmd+N pressed - new conversation <<<");
                                this.emit(UserEvent::NewConversation);
                                this.state.messages.clear();
                                this.state.input_text.clear();
                                this.state.cursor_position = 0;
                                this.state.streaming = StreamingState::Idle;
                                this.state.thinking_content = None;
                                this.state.conversation_title_editing = false;
                                this.state.conversation_title_input.clear();
                                this.state.chat_autoscroll_enabled = true;
                                this.chat_scroll_handle.scroll_to_bottom();
                                this.state.conversation_title = "New Conversation".to_string();
                                cx.notify();
                                return;
                            }
                            "t" => {
                                println!(">>> Cmd+T pressed - toggle thinking <<<");
                                this.emit(UserEvent::ToggleThinking);
                                return;
                            }
                            "p" => {
                                this.toggle_profile_dropdown(cx);
                                return;
                            }
                            "k" => {
                                this.toggle_conversation_dropdown(cx);
                                return;
                            }
                            "r" => {
                                this.start_rename_conversation(cx);
                                return;
                            }
                            "v" => {
                                if let Some(item) = cx.read_from_clipboard() {
                                    if let Some(text) = item.text() {
                                        this.handle_paste(&text, cx);
                                    }
                                }
                                return;
                            }
                            "a" => {
                                this.handle_select_all(cx);
                                return;
                            }
                            "x" => {
                                this.handle_select_all(cx);
                                let text = if this.state.conversation_title_editing {
                                    this.state.conversation_title_input.clone()
                                } else {
                                    this.state.input_text.clone()
                                };
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                                if this.state.conversation_title_editing {
                                    this.state.conversation_title_input.clear();
                                    this.state.rename_replace_on_next_char = false;
                                } else if !this.state.conversation_dropdown_open
                                    && !this.state.profile_dropdown_open
                                {
                                    this.state.input_text.clear();
                                    this.state.cursor_position = 0;
                                    this.state.marked_range = None;
                                }
                                cx.notify();
                                return;
                            }
                            "left" => {
                                this.move_cursor_home(cx);
                                return;
                            }
                            "right" => {
                                this.move_cursor_end(cx);
                                return;
                            }
                            _ => {}
                        }
                    }

                    if this.state.conversation_title_editing {
                        match key.as_str() {
                            "escape" => this.cancel_rename_conversation(cx),
                            "backspace" => this.handle_rename_backspace(cx),
                            "enter" => this.submit_rename_conversation(cx),
                            _ => {}
                        }
                        return;
                    }

                    if this.state.conversation_dropdown_open {
                        match key.as_str() {
                            "escape" => {
                                this.state.conversation_dropdown_open = false;
                                cx.notify();
                            }
                            "up" => {
                                this.move_conversation_dropdown_selection(-1, cx);
                            }
                            "down" => {
                                this.move_conversation_dropdown_selection(1, cx);
                            }
                            "enter" => {
                                this.confirm_conversation_dropdown_selection(cx);
                            }
                            _ => {}
                        }
                        return;
                    }

                    if this.state.profile_dropdown_open {
                        match key.as_str() {
                            "escape" => {
                                this.state.profile_dropdown_open = false;
                                cx.notify();
                            }
                            "up" => {
                                this.move_profile_dropdown_selection(-1, cx);
                            }
                            "down" => {
                                this.move_profile_dropdown_selection(1, cx);
                            }
                            "enter" => {
                                this.confirm_profile_dropdown_selection(cx);
                            }
                            _ => {}
                        }
                        return;
                    }

                    match key.as_str() {
                        "left" => this.move_cursor_left(cx),
                        "right" => this.move_cursor_right(cx),
                        "home" => this.move_cursor_home(cx),
                        "end" => this.move_cursor_end(cx),
                        "backspace" => this.handle_backspace(cx),
                        "enter" => this.handle_enter(cx),
                        "escape" => {
                            if matches!(this.state.streaming, StreamingState::Streaming { .. }) {
                                println!(">>> Escape pressed - stopping stream <<<");
                                this.emit(UserEvent::StopStreaming);
                                this.state.streaming = StreamingState::Idle;
                                cx.notify();
                            }
                        }
                        _ => {}
                    }

                }),
            )
            .relative()
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Title bar (32px)
            .child(self.render_title_bar(cx))
            // Chat area (flex)
            .child(self.render_chat_area(cx))
            // Input bar (50px)
            .child(self.render_input_bar(cx))
            // Overlay dropdowns (rendered at root level to avoid clipping)
            .when(self.state.conversation_dropdown_open, |d| {
                d.child(self.render_conversation_dropdown(cx))
            })
            .when(self.state.profile_dropdown_open, |d| {
                d.child(self.render_profile_dropdown(cx))
            })
    }
}
