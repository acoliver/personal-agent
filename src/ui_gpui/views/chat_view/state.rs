//! Chat state types: data model, enums, and pure state-transition helpers.
//!
//! This module owns `ChatMessage`, `MessageRole`, `StreamingState`, and `ChatState`.
//! No GPUI rendering, no bridge, no scroll handles.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use crate::models::ConversationExportFormat;
use crate::presentation::view_command::{
    ConversationSearchResult, ConversationSummary, ProfileSummary, ToolApprovalContext,
    ToolCategory,
};
use crate::ui_gpui::components::markdown_content::{parse_markdown_blocks, MarkdownBlock};
use std::cell::OnceCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use uuid::Uuid;

/// Represents a single message in the chat (for UI display)
///
/// Uses `Arc<String>` for content and thinking to avoid expensive heap
/// allocations when cloning messages during render. Finalized messages
/// also cache their parsed markdown blocks to avoid re-parsing on every
/// re-render.
///
/// @plan PLAN-20260407-ISSUE172.P01
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: MessageRole,
    /// Arc-wrapped content for cheap cloning during render.
    pub content: Arc<String>,
    /// Optional thinking content, also Arc-wrapped.
    pub thinking: Option<Arc<String>>,
    pub model_label: Option<String>,
    pub timestamp: Option<u64>,
    /// Cached parsed markdown blocks. Only set for finalized messages.
    /// Streaming messages should NOT cache since content changes.
    /// Uses `OnceCell` for lazy initialization with interior mutability.
    markdown_cache: OnceCell<Arc<Vec<MarkdownBlock>>>,
}

impl PartialEq for ChatMessage {
    fn eq(&self, other: &Self) -> bool {
        self.role == other.role
            && self.content == other.content
            && self.thinking == other.thinking
            && self.model_label == other.model_label
            && self.timestamp == other.timestamp
        // Intentionally exclude markdown_cache from equality check
        // since it's derived from content
    }
}

impl Eq for ChatMessage {}

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
            content: Arc::new(content.into()),
            thinking: None,
            model_label: None,
            timestamp: None,
            markdown_cache: OnceCell::new(),
        }
    }

    pub fn assistant(content: impl Into<String>, model_label: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Arc::new(content.into()),
            thinking: None,
            model_label: Some(model_label.into()),
            timestamp: None,
            markdown_cache: OnceCell::new(),
        }
    }

    #[must_use]
    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(Arc::new(thinking.into()));
        self
    }

    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Get or parse markdown blocks for this message.
    ///
    /// Finalized messages cache their parsed blocks on first access.
    /// This avoids re-parsing markdown on every re-render, which was
    /// a major source of sluggishness in long conversations.
    ///
    /// @plan PLAN-20260407-ISSUE172.P02
    #[must_use]
    pub fn get_or_parse_markdown(&self) -> Arc<Vec<MarkdownBlock>> {
        self.markdown_cache
            .get_or_init(|| Arc::new(parse_markdown_blocks(&self.content)))
            .clone()
    }

    /// Get the raw content string slice for this message.
    #[must_use]
    pub fn content_str(&self) -> &str {
        &self.content
    }

    /// Returns a reference to the Arc-wrapped content string.
    #[must_use]
    pub fn content_arc(&self) -> Arc<String> {
        Arc::clone(&self.content)
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
/// Supports grouping of related operations (same category + same primary target).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolApprovalBubble {
    /// Single request ID for non-grouped, first request ID for grouped
    pub request_id: String,
    /// All request IDs in this group (for resolving)
    pub request_ids: Vec<String>,
    /// The approval context (shared across grouped operations)
    pub context: ToolApprovalContext,
    /// Lifecycle state
    pub state: ApprovalBubbleState,
    /// Group key for matching incoming requests to existing bubbles
    pub group_key: (ToolCategory, String),
    /// Additional grouped operations (first is always in context)
    pub grouped_operations: Vec<GroupedOperation>,
    /// Whether the grouped operations list is expanded
    pub expanded: bool,
}

/// Represents a single operation within a grouped approval bubble.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupedOperation {
    pub request_id: String,
    /// Per-operation details (e.g., line range for edit, pattern for search)
    pub details: Vec<(String, String)>,
}

impl ToolApprovalBubble {
    /// Create a new single-operation approval bubble.
    pub fn new(request_id: impl Into<String>, context: ToolApprovalContext) -> Self {
        let group_key = (context.category, context.primary_target.clone());
        let request_id = request_id.into();
        Self {
            request_id: request_id.clone(),
            request_ids: vec![request_id],
            context,
            state: ApprovalBubbleState::Pending,
            group_key,
            grouped_operations: Vec::new(),
            expanded: false,
        }
    }

    /// Check if this bubble can group with the given context.
    #[must_use]
    pub fn can_group_with(&self, context: &ToolApprovalContext) -> bool {
        self.state == ApprovalBubbleState::Pending
            && self.group_key.0 == context.category
            && self.group_key.1 == context.primary_target
    }

    /// Add a grouped operation to this bubble.
    pub fn add_operation(&mut self, request_id: impl Into<String>, details: Vec<(String, String)>) {
        let request_id = request_id.into();
        self.request_ids.push(request_id.clone());
        self.grouped_operations.push(GroupedOperation {
            request_id,
            details,
        });
    }

    /// Get the total number of operations in this bubble.
    #[must_use]
    pub const fn operation_count(&self) -> usize {
        1 + self.grouped_operations.len()
    }
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
    /// Inline tool approval bubbles pending or resolved in this session, keyed by conversation.
    pub approval_bubbles: HashMap<Uuid, Vec<ToolApprovalBubble>>,
    /// Whether YOLO mode (auto-approve all) is currently active.
    pub yolo_mode: bool,
    /// Whether the sidebar is visible (popout mode only).
    pub sidebar_visible: bool,
    /// Current search query typed in the sidebar search box.
    pub sidebar_search_query: String,
    /// Whether the sidebar search box currently has input focus.
    pub sidebar_search_focused: bool,
    /// Search results from the backend, if a search is active.
    pub sidebar_search_results: Option<Vec<ConversationSearchResult>>,
    /// Conversation ID pending delete confirmation in the sidebar.
    pub delete_confirming_id: Option<Uuid>,
    /// When true, emojis are stripped from assistant message display.
    pub filter_emoji: bool,
    /// Conversation ids currently streaming in the background (for sidebar indicator).
    ///
    /// @plan PLAN-20260416-ISSUE173.P11
    /// @requirement REQ-173-004.3
    pub streaming_conversation_ids: std::collections::HashSet<Uuid>,
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
            approval_bubbles: HashMap::new(),
            yolo_mode: false,
            sidebar_visible: true,
            sidebar_search_query: String::new(),
            sidebar_search_focused: false,
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
            filter_emoji: false,
            streaming_conversation_ids: std::collections::HashSet::new(),
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
        // Prime the markdown cache on each message so that
        // clones produced during render share the cached Arc.
        for msg in &messages {
            let _ = msg.get_or_parse_markdown();
        }
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
        // Prime the markdown cache on the original message before storage.
        // This ensures that clones produced during render share the cached Arc.
        // @plan PLAN-20260407-ISSUE172.P06
        let _ = message.get_or_parse_markdown();
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
        assert_eq!(&*msg.content, "hello");
        assert!(msg.thinking.is_none());
        assert!(msg.model_label.is_none());
    }

    #[test]
    fn chat_message_assistant_sets_model_label() {
        let msg = ChatMessage::assistant("response", "gpt-4o");
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(&*msg.content, "response");
        assert_eq!(msg.model_label.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn chat_message_with_thinking_attaches_thinking_content() {
        let msg = ChatMessage::assistant("answer", "model").with_thinking("step 1");
        assert_eq!(msg.thinking.as_deref().map(String::as_str), Some("step 1"));
    }

    #[test]
    fn chat_message_with_timestamp() {
        let msg = ChatMessage::user("hello").with_timestamp(1_234_567_890);
        assert_eq!(msg.timestamp, Some(1_234_567_890));
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
    fn streaming_state_error_variant() {
        let state = StreamingState::Error("test error".to_string());
        assert!(matches!(state, StreamingState::Error(_)));
    }

    #[test]
    fn streaming_state_streaming_variant() {
        let state = StreamingState::Streaming {
            content: "test".to_string(),
            done: true,
        };
        assert!(matches!(state, StreamingState::Streaming { .. }));
    }

    #[test]
    fn chat_state_set_thinking() {
        let mut state = ChatState::new();
        state.set_thinking(true, Some("thinking content".to_string()));
        assert!(state.show_thinking);
        assert_eq!(state.thinking_content, Some("thinking content".to_string()));
    }

    #[test]
    fn chat_state_default_timestamp_is_none() {
        let msg = ChatMessage::user("hello");
        assert!(msg.timestamp.is_none());
    }

    #[test]
    fn chat_state_new_creates_empty_state() {
        let state = ChatState::new();
        assert!(state.messages.is_empty());
        assert_eq!(state.input_text, "");
        assert_eq!(state.conversation_title, "New Conversation");
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
        let bubble = ToolApprovalBubble::new(
            "req-1",
            ToolApprovalContext::new("shell", ToolCategory::Shell, "git push"),
        );
        assert_eq!(bubble.state, ApprovalBubbleState::Pending);
        assert_eq!(bubble.request_id, "req-1");
        assert_eq!(bubble.request_ids, vec!["req-1".to_string()]);
        assert_eq!(bubble.context.tool_name, "shell");
        assert_eq!(bubble.context.primary_target, "git push");
        assert_eq!(bubble.operation_count(), 1);
    }

    #[test]
    fn tool_approval_bubble_transitions_to_approved() {
        let mut bubble = ToolApprovalBubble::new(
            "req-2",
            ToolApprovalContext::new("write", ToolCategory::FileWrite, "/tmp/f.txt"),
        );
        bubble.state = ApprovalBubbleState::Approved;
        assert_eq!(bubble.state, ApprovalBubbleState::Approved);
    }

    #[test]
    fn tool_approval_bubble_transitions_to_denied() {
        let mut bubble = ToolApprovalBubble::new(
            "req-3",
            ToolApprovalContext::new("shell", ToolCategory::Shell, "rm -rf /"),
        );
        bubble.state = ApprovalBubbleState::Denied;
        assert_eq!(bubble.state, ApprovalBubbleState::Denied);
    }

    #[test]
    fn approval_bubbles_can_be_pushed_to_state() {
        let mut state = ChatState::default();
        let conversation_id = Uuid::new_v4();
        let pending = ToolApprovalBubble::new(
            "r1",
            ToolApprovalContext::new("shell", ToolCategory::Shell, "ls"),
        );
        let mut approved = ToolApprovalBubble::new(
            "r2",
            ToolApprovalContext::new("write", ToolCategory::FileWrite, "/tmp/a.txt"),
        );
        approved.state = ApprovalBubbleState::Approved;
        state
            .approval_bubbles
            .insert(conversation_id, vec![pending, approved]);
        let bubbles = state
            .approval_bubbles
            .get(&conversation_id)
            .expect("conversation bucket should exist");
        assert_eq!(bubbles.len(), 2);
        assert_eq!(bubbles[0].state, ApprovalBubbleState::Pending);
        assert_eq!(bubbles[1].state, ApprovalBubbleState::Approved);
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

    #[test]
    fn tool_approval_bubble_can_group_with_same_category_and_target() {
        let bubble = ToolApprovalBubble::new(
            "req-1",
            ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/main.rs"),
        );
        let context = ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/main.rs");
        assert!(bubble.can_group_with(&context));
    }

    #[test]
    fn tool_approval_bubble_cannot_group_with_different_category() {
        let bubble = ToolApprovalBubble::new(
            "req-1",
            ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/main.rs"),
        );
        let context =
            ToolApprovalContext::new("WriteFile", ToolCategory::FileWrite, "/tmp/main.rs");
        assert!(!bubble.can_group_with(&context));
    }

    #[test]
    fn tool_approval_bubble_cannot_group_with_different_target() {
        let bubble = ToolApprovalBubble::new(
            "req-1",
            ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/a.rs"),
        );
        let context = ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/b.rs");
        assert!(!bubble.can_group_with(&context));
    }

    #[test]
    fn tool_approval_bubble_cannot_group_when_not_pending() {
        let mut bubble = ToolApprovalBubble::new(
            "req-1",
            ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/main.rs"),
        );
        bubble.state = ApprovalBubbleState::Approved;
        let context = ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/main.rs");
        assert!(!bubble.can_group_with(&context));
    }

    #[test]
    fn tool_approval_bubble_add_operation() {
        let mut bubble = ToolApprovalBubble::new(
            "req-1",
            ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/main.rs"),
        );
        assert_eq!(bubble.operation_count(), 1);
        assert_eq!(bubble.request_ids.len(), 1);

        bubble.add_operation("req-2", vec![("line".to_string(), "10".to_string())]);
        assert_eq!(bubble.operation_count(), 2);
        assert_eq!(bubble.request_ids.len(), 2);
        assert_eq!(bubble.grouped_operations.len(), 1);
    }

    #[test]
    fn grouped_operation_creation() {
        let op = GroupedOperation {
            request_id: "req-1".to_string(),
            details: vec![("key".to_string(), "value".to_string())],
        };
        assert_eq!(op.request_id, "req-1");
        assert_eq!(op.details.len(), 1);
    }
}
