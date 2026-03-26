//! Chat state types: data model, enums, and pure state-transition helpers.
//!
//! This module owns `ChatMessage`, `MessageRole`, `StreamingState`, and `ChatState`.
//! No GPUI rendering, no bridge, no scroll handles.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use crate::presentation::view_command::{ConversationSummary, ProfileSummary};
use std::ops::Range;
use uuid::Uuid;

/// Represents a single message in the chat (for UI display)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub model_id: Option<String>,
    pub timestamp: Option<u64>,
}

/// Message role enum
#[derive(Clone, Debug, PartialEq, Eq)]
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

    #[must_use]
    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }

    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Streaming state for AI responses
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamingState {
    Idle,
    Streaming { content: String, done: bool },
    Error(String),
}

/// Main chat state container
#[allow(clippy::struct_excessive_bools)]
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    #[must_use]
    pub fn with_streaming(mut self, state: StreamingState) -> Self {
        self.streaming = state;
        self
    }

    #[must_use]
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

    pub(super) fn selected_conversation(&self) -> Option<&ConversationSummary> {
        self.active_conversation_id
            .and_then(|id| {
                self.conversations
                    .iter()
                    .find(|conversation| conversation.id == id)
            })
            .or_else(|| self.conversations.first())
    }

    pub(super) fn sync_conversation_title_from_active(&mut self) {
        self.conversation_title = self.selected_conversation().map_or_else(
            || "New Conversation".to_string(),
            |conversation| {
                if conversation.title.trim().is_empty() {
                    "Untitled Conversation".to_string()
                } else {
                    conversation.title.clone()
                }
            },
        );
    }

    pub(super) fn sync_conversation_dropdown_index(&mut self) {
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

    pub(super) fn selected_profile(&self) -> Option<&ProfileSummary> {
        self.selected_profile_id
            .and_then(|id| self.profiles.iter().find(|profile| profile.id == id))
            .or_else(|| self.profiles.iter().find(|profile| profile.is_default))
    }

    pub(super) fn sync_current_model_from_profile(&mut self) {
        self.current_model = self.selected_profile().map_or_else(
            || "No profile selected".to_string(),
            |profile| profile.model_id.clone(),
        );
    }

    pub(super) fn sync_profile_dropdown_index(&mut self) {
        self.profile_dropdown_index = self
            .selected_profile_id
            .and_then(|id| self.profiles.iter().position(|profile| profile.id == id))
            .unwrap_or(0)
            .min(self.profiles.len().saturating_sub(1));
    }
}
