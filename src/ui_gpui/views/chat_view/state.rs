//! Chat state types: data model, enums, and pure state-transition helpers.
//!
//! This module owns `ChatMessage`, `MessageRole`, `StreamingState`, and `ChatState`.
//! No GPUI rendering, no bridge, no scroll handles.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use crate::models::ConversationExportFormat;
use crate::presentation::view_command::{
    ConversationSearchResult, ConversationSummary, ProfileSummary,
};
use std::ops::Range;
use uuid::Uuid;

/// Represents a single message in the chat (for UI display)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub model_label: Option<String>,
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
            model_label: None,
            timestamp: None,
        }
    }

    pub fn assistant(content: impl Into<String>, model_label: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            thinking: None,
            model_label: Some(model_label.into()),
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

/// Lifecycle state of a tool approval request bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalBubbleState {
    /// Waiting for the user to respond.
    Pending,
    /// User approved the tool call.
    Approved,
    /// User denied the tool call.
    Denied,
}

/// A single inline tool approval request displayed in the chat stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolApprovalBubble {
    pub request_id: String,
    pub tool_name: String,
    pub tool_argument: String,
    pub state: ApprovalBubbleState,
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
    pub conversation_export_format: ConversationExportFormat,
    pub export_feedback_message: Option<String>,
    pub export_feedback_is_error: bool,
    pub export_feedback_path: Option<String>,
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
    /// Inline tool approval bubbles pending or resolved in this session.
    pub approval_bubbles: Vec<ToolApprovalBubble>,
    /// Whether YOLO mode (auto-approve all) is currently active.
    pub yolo_mode: bool,
    /// Whether the sidebar is visible (popout mode only).
    pub sidebar_visible: bool,
    /// Current search query typed in the sidebar search box.
    pub sidebar_search_query: String,
    /// Search results from the backend, if a search is active.
    pub sidebar_search_results: Option<Vec<ConversationSearchResult>>,
    /// Conversation ID pending delete confirmation in the sidebar.
    pub delete_confirming_id: Option<Uuid>,
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
            approval_bubbles: Vec::new(),
            yolo_mode: false,
            sidebar_visible: true,
            sidebar_search_query: String::new(),
            sidebar_search_results: None,
            delete_confirming_id: None,
            current_model: "No profile selected".to_string(),
            profiles: Vec::new(),
            selected_profile_id: None,
            profile_dropdown_open: false,
            profile_dropdown_index: 0,
            conversation_export_format: ConversationExportFormat::Md,
            export_feedback_message: None,
            export_feedback_is_error: false,
            export_feedback_path: None,
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
            |profile| profile.name.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_user_sets_role_and_content() {
        let msg = ChatMessage::user("hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "hello");
        assert!(msg.thinking.is_none());
        assert!(msg.model_label.is_none());
    }

    #[test]
    fn chat_message_assistant_sets_model_label() {
        let msg = ChatMessage::assistant("response", "gpt-4o");
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content, "response");
        assert_eq!(msg.model_label.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn chat_message_with_thinking_attaches_thinking_content() {
        let msg = ChatMessage::assistant("answer", "model").with_thinking("step 1");
        assert_eq!(msg.thinking.as_deref(), Some("step 1"));
    }

    #[test]
    fn chat_state_default_is_idle_with_empty_messages() {
        let state = ChatState::default();
        assert!(state.messages.is_empty());
        assert!(matches!(state.streaming, StreamingState::Idle));
        assert!(!state.show_thinking);
        assert!(state.thinking_content.is_none());
        assert!(state.export_feedback_path.is_none());
    }

    #[test]
    fn chat_state_builder_chains() {
        let state = ChatState::new()
            .with_messages(vec![ChatMessage::user("hi")])
            .with_streaming(StreamingState::Streaming {
                content: "partial".into(),
                done: false,
            })
            .with_thinking(true, Some("chain".into()));

        assert_eq!(state.messages.len(), 1);
        assert!(matches!(state.streaming, StreamingState::Streaming { .. }));
        assert!(state.show_thinking);
        assert_eq!(state.thinking_content.as_deref(), Some("chain"));
    }

    #[test]
    fn add_message_appends_and_set_streaming_transitions() {
        let mut state = ChatState::new();
        state.add_message(ChatMessage::user("first"));
        state.add_message(ChatMessage::assistant("second", "m"));
        assert_eq!(state.messages.len(), 2);

        state.set_streaming(StreamingState::Streaming {
            content: String::new(),
            done: false,
        });
        assert!(matches!(state.streaming, StreamingState::Streaming { .. }));

        state.set_streaming(StreamingState::Idle);
        assert!(matches!(state.streaming, StreamingState::Idle));
    }

    #[test]
    fn selected_profile_prefers_explicit_then_default() {
        let p1 = ProfileSummary {
            id: Uuid::new_v4(),
            name: "A".into(),
            provider_id: "openai".into(),
            model_id: "m1".into(),
            is_default: false,
        };
        let p2 = ProfileSummary {
            id: Uuid::new_v4(),
            name: "B".into(),
            provider_id: "openai".into(),
            model_id: "m2".into(),
            is_default: true,
        };

        let mut state = ChatState::new();
        state.profiles = vec![p1.clone(), p2.clone()];

        // No explicit selection -> falls back to default
        assert_eq!(state.selected_profile().unwrap().id, p2.id);

        // Explicit selection wins
        state.selected_profile_id = Some(p1.id);
        assert_eq!(state.selected_profile().unwrap().id, p1.id);
    }

    #[test]
    fn sync_current_model_from_profile_uses_selected_or_fallback() {
        let profile = ProfileSummary {
            id: Uuid::new_v4(),
            name: "Test".into(),
            provider_id: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            is_default: true,
        };

        let mut state = ChatState::new();
        state.profiles = vec![profile];

        state.sync_current_model_from_profile();
        assert_eq!(state.current_model, "Test");

        state.profiles.clear();
        state.sync_current_model_from_profile();
        assert_eq!(state.current_model, "No profile selected");
    }

    #[test]
    fn sync_profile_dropdown_index_clamps_to_valid_range() {
        let p1 = ProfileSummary {
            id: Uuid::new_v4(),
            name: "A".into(),
            provider_id: "openai".into(),
            model_id: "m1".into(),
            is_default: false,
        };

        let mut state = ChatState::new();
        state.profiles = vec![p1.clone()];
        state.selected_profile_id = Some(p1.id);
        state.sync_profile_dropdown_index();
        assert_eq!(state.profile_dropdown_index, 0);

        // Unknown profile ID -> clamps to 0
        state.selected_profile_id = Some(Uuid::new_v4());
        state.sync_profile_dropdown_index();
        assert_eq!(state.profile_dropdown_index, 0);

        // Empty profiles -> clamps to 0
        state.profiles.clear();
        state.sync_profile_dropdown_index();
        assert_eq!(state.profile_dropdown_index, 0);
    }

    // ── Tool Approval State Tests ────────────────────────────────────────

    #[test]
    fn chat_state_default_has_empty_approval_bubbles_and_yolo_off() {
        let state = ChatState::default();
        assert!(state.approval_bubbles.is_empty());
        assert!(!state.yolo_mode);
    }

    #[test]
    fn tool_approval_bubble_pending_state() {
        let bubble = ToolApprovalBubble {
            request_id: "req-1".into(),
            tool_name: "shell".into(),
            tool_argument: "git push".into(),
            state: ApprovalBubbleState::Pending,
        };
        assert_eq!(bubble.state, ApprovalBubbleState::Pending);
        assert_eq!(bubble.request_id, "req-1");
        assert_eq!(bubble.tool_name, "shell");
        assert_eq!(bubble.tool_argument, "git push");
    }

    #[test]
    fn tool_approval_bubble_transitions_to_approved() {
        let mut bubble = ToolApprovalBubble {
            request_id: "req-2".into(),
            tool_name: "write".into(),
            tool_argument: "/tmp/f.txt".into(),
            state: ApprovalBubbleState::Pending,
        };
        bubble.state = ApprovalBubbleState::Approved;
        assert_eq!(bubble.state, ApprovalBubbleState::Approved);
    }

    #[test]
    fn tool_approval_bubble_transitions_to_denied() {
        let mut bubble = ToolApprovalBubble {
            request_id: "req-3".into(),
            tool_name: "shell".into(),
            tool_argument: "rm -rf /".into(),
            state: ApprovalBubbleState::Pending,
        };
        bubble.state = ApprovalBubbleState::Denied;
        assert_eq!(bubble.state, ApprovalBubbleState::Denied);
    }

    #[test]
    fn approval_bubbles_can_be_pushed_to_state() {
        let mut state = ChatState::default();
        state.approval_bubbles.push(ToolApprovalBubble {
            request_id: "r1".into(),
            tool_name: "shell".into(),
            tool_argument: "ls".into(),
            state: ApprovalBubbleState::Pending,
        });
        state.approval_bubbles.push(ToolApprovalBubble {
            request_id: "r2".into(),
            tool_name: "write".into(),
            tool_argument: "/tmp/a.txt".into(),
            state: ApprovalBubbleState::Approved,
        });
        assert_eq!(state.approval_bubbles.len(), 2);
        assert_eq!(
            state.approval_bubbles[0].state,
            ApprovalBubbleState::Pending
        );
        assert_eq!(
            state.approval_bubbles[1].state,
            ApprovalBubbleState::Approved
        );
    }

    #[test]
    fn approval_bubble_state_equality() {
        assert_eq!(ApprovalBubbleState::Pending, ApprovalBubbleState::Pending);
        assert_ne!(ApprovalBubbleState::Pending, ApprovalBubbleState::Approved);
        assert_ne!(ApprovalBubbleState::Approved, ApprovalBubbleState::Denied);
    }

    #[test]
    fn yolo_mode_toggling() {
        let mut state = ChatState::default();
        assert!(!state.yolo_mode);
        state.yolo_mode = true;
        assert!(state.yolo_mode);
        state.yolo_mode = false;
        assert!(!state.yolo_mode);
    }
}
