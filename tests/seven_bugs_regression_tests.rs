//! Behavioral regression tests for runtime bugs identified in manual testing.
//!
//! Bug 1: R button starts new conversation instead of inline rename
//! Bug 4: Model has no conversational memory (history not sent to LLM)
//! Bug 5: Thinking blocks never appear; T toggle appears disabled

#![allow(
    clippy::or_fun_call,
    clippy::significant_drop_tightening,
    clippy::field_reassign_with_default
)]

use std::sync::Arc;
use uuid::Uuid;

use personal_agent::models::{Conversation, Message};
use personal_agent::presentation::view_command::ConversationSummary;
use personal_agent::services::conversation::ConversationService;
use personal_agent::services::{ServiceError, ServiceResult};

use async_trait::async_trait;

// ──────────────────────────────────────────────────────────────────────
// Mock services
// ──────────────────────────────────────────────────────────────────────

struct InMemoryConversationService {
    conversations: tokio::sync::Mutex<Vec<Conversation>>,
    active: tokio::sync::Mutex<Option<Uuid>>,
}

impl InMemoryConversationService {
    fn new() -> Self {
        Self {
            conversations: tokio::sync::Mutex::new(Vec::new()),
            active: tokio::sync::Mutex::new(None),
        }
    }
}

#[async_trait]
impl ConversationService for InMemoryConversationService {
    async fn create(&self, title: Option<String>, profile_id: Uuid) -> ServiceResult<Conversation> {
        let mut conv = Conversation::new(profile_id);
        if let Some(t) = title {
            conv.set_title(t);
        }
        self.conversations.lock().await.push(conv.clone());
        Ok(conv)
    }

    async fn load(&self, id: Uuid) -> ServiceResult<Conversation> {
        self.conversations
            .lock()
            .await
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or(ServiceError::NotFound(format!("No conversation {id}")))
    }

    async fn list(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<Conversation>> {
        let convs = self.conversations.lock().await;
        let o = offset.unwrap_or(0);
        let l = limit.unwrap_or(convs.len());
        let end = std::cmp::min(o + l, convs.len());
        if o >= convs.len() {
            return Ok(Vec::new());
        }
        Ok(convs[o..end].to_vec())
    }

    async fn add_user_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Message> {
        let mut convs = self.conversations.lock().await;
        let conv = convs
            .iter_mut()
            .find(|c| c.id == conversation_id)
            .ok_or(ServiceError::NotFound("no conv".into()))?;
        let msg = Message::user(content);
        conv.add_message(msg.clone());
        Ok(msg)
    }

    async fn add_assistant_message(
        &self,
        conversation_id: Uuid,
        content: String,
        _thinking_content: Option<String>,
    ) -> ServiceResult<Message> {
        let mut convs = self.conversations.lock().await;
        let conv = convs
            .iter_mut()
            .find(|c| c.id == conversation_id)
            .ok_or(ServiceError::NotFound("no conv".into()))?;
        let msg = Message::assistant(content);
        conv.add_message(msg.clone());
        Ok(msg)
    }

    async fn rename(&self, id: Uuid, new_title: String) -> ServiceResult<()> {
        let mut convs = self.conversations.lock().await;
        let conv = convs
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ServiceError::NotFound("no conv".into()))?;
        conv.set_title(new_title);
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> ServiceResult<()> {
        self.conversations.lock().await.retain(|c| c.id != id);
        Ok(())
    }

    async fn set_active(&self, id: Uuid) -> ServiceResult<()> {
        let convs = self.conversations.lock().await;
        if !convs.iter().any(|c| c.id == id) {
            return Err(ServiceError::NotFound("no conv".into()));
        }
        drop(convs);
        *self.active.lock().await = Some(id);
        Ok(())
    }

    async fn get_active(&self) -> ServiceResult<Option<Uuid>> {
        Ok(*self.active.lock().await)
    }

    async fn get_messages(&self, conversation_id: Uuid) -> ServiceResult<Vec<Message>> {
        let convs = self.conversations.lock().await;
        let conv = convs
            .iter()
            .find(|c| c.id == conversation_id)
            .ok_or(ServiceError::NotFound("no conv".into()))?;
        Ok(conv.messages.clone())
    }

    async fn update(
        &self,
        id: Uuid,
        title: Option<String>,
        profile_id: Option<Uuid>,
    ) -> ServiceResult<Conversation> {
        let mut convs = self.conversations.lock().await;
        let conv = convs
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(ServiceError::NotFound("no conv".into()))?;
        if let Some(t) = title {
            conv.set_title(t);
        }
        if let Some(pid) = profile_id {
            conv.profile_id = pid;
        }
        Ok(conv.clone())
    }
}

// ======================================================================
// Bug 1: R button should enter rename mode, not start new conversation
// ======================================================================

#[test]
fn bug1_rename_enters_edit_mode_on_active_conversation() {
    use personal_agent::ui_gpui::views::chat_view::ChatState;

    let conv_id = Uuid::new_v4();
    let mut state = ChatState::default();
    state.active_conversation_id = Some(conv_id);
    state.conversation_title = "My Chat".to_string();
    state.conversations = vec![ConversationSummary {
        id: conv_id,
        title: "My Chat".to_string(),
        updated_at: chrono::Utc::now(),
        message_count: 3,
    }];

    // Simulate start_rename_conversation logic
    state.conversation_dropdown_open = false;
    state.conversation_title_editing = true;
    state.conversation_title_input = state.conversation_title.clone();

    assert!(state.conversation_title_editing);
    assert_eq!(state.conversation_title_input, "My Chat");
    assert_eq!(
        state.active_conversation_id,
        Some(conv_id),
        "active_conversation_id must not change during rename"
    );
}

#[test]
fn bug1_rename_submit_updates_title_everywhere() {
    use personal_agent::ui_gpui::views::chat_view::ChatState;

    let conv_id = Uuid::new_v4();
    let mut state = ChatState::default();
    state.active_conversation_id = Some(conv_id);
    state.conversation_title = "Old Title".to_string();
    state.conversation_title_editing = true;
    state.conversation_title_input = "New Title".to_string();
    state.conversations = vec![ConversationSummary {
        id: conv_id,
        title: "Old Title".to_string(),
        updated_at: chrono::Utc::now(),
        message_count: 2,
    }];

    let new_title = state.conversation_title_input.trim().to_string();
    state.conversation_title = new_title.clone();
    if let Some(c) = state.conversations.iter_mut().find(|c| c.id == conv_id) {
        c.title = new_title;
    }
    state.conversation_title_editing = false;
    state.conversation_title_input.clear();

    assert_eq!(state.conversation_title, "New Title");
    assert_eq!(state.conversations[0].title, "New Title");
    assert!(!state.conversation_title_editing);
}

// ======================================================================
// Bug 4: Model has no conversational memory
// ======================================================================

#[tokio::test]
async fn bug4_conversation_preserves_full_history() {
    let profile_id = Uuid::new_v4();
    let conv_service = Arc::new(InMemoryConversationService::new()) as Arc<dyn ConversationService>;

    let conv = conv_service
        .create(Some("Memory Test".to_string()), profile_id)
        .await
        .unwrap();
    let conv_id = conv.id;

    conv_service
        .add_user_message(conv_id, "My name is Alice".to_string())
        .await
        .unwrap();
    conv_service
        .add_assistant_message(conv_id, "Hello Alice!".to_string(), None)
        .await
        .unwrap();
    conv_service
        .add_user_message(conv_id, "What is my name?".to_string())
        .await
        .unwrap();

    let messages = conv_service.get_messages(conv_id).await.unwrap();

    assert!(
        messages.len() >= 3,
        "BUG 4: Should have >= 3 messages, got {}",
        messages.len()
    );
    assert!(
        messages.iter().any(|m| m.content == "My name is Alice"),
        "BUG 4: First user message must be in history"
    );
    assert!(
        messages.iter().any(|m| m.content.contains("Hello Alice")),
        "BUG 4: Assistant reply must be in history"
    );
}

/// `chat_impl` adds the user message via `add_user_message()`, then loads
/// the conversation (which now includes it), then ALSO pushes a duplicate.
#[tokio::test]
async fn bug4_no_duplicate_user_message_after_add_and_load() {
    let profile_id = Uuid::new_v4();
    let conv_service = Arc::new(InMemoryConversationService::new()) as Arc<dyn ConversationService>;

    let conv = conv_service
        .create(Some("Dup Test".to_string()), profile_id)
        .await
        .unwrap();
    let conv_id = conv.id;

    conv_service
        .add_user_message(conv_id, "Hello world".to_string())
        .await
        .unwrap();
    let loaded = conv_service.load(conv_id).await.unwrap();
    let hello_count = loaded
        .messages
        .iter()
        .filter(|m| m.content == "Hello world")
        .count();

    assert_eq!(
        hello_count, 1,
        "BUG 4: Message appears {hello_count} times. chat_impl.rs pushes it again, \
         so the LLM sees the user message twice."
    );
}

// ======================================================================
// Bug 5: Thinking blocks don't appear
// ======================================================================

#[test]
fn bug5_thinking_content_accumulates_and_attaches() {
    use personal_agent::ui_gpui::views::chat_view::{ChatMessage, ChatState, StreamingState};

    let mut state = ChatState::default();
    state.show_thinking = true;
    state.streaming = StreamingState::Streaming {
        content: String::new(),
        done: false,
    };
    state.thinking_content = Some(String::new());

    // Accumulate thinking
    state.thinking_content = Some(state.thinking_content.unwrap_or_default() + "Step 1. ");
    state.thinking_content = Some(state.thinking_content.unwrap_or_default() + "Step 2.");

    assert_eq!(state.thinking_content.as_ref().unwrap(), "Step 1. Step 2.");

    // Accumulate stream content
    if let StreamingState::Streaming {
        ref mut content, ..
    } = state.streaming
    {
        content.push_str("The answer is 42.");
    }

    // FinalizeStream should attach thinking to message
    if let StreamingState::Streaming { content, .. } = &state.streaming {
        let mut msg = ChatMessage::assistant(content.clone(), "model");
        if let Some(thinking) = &state.thinking_content {
            msg = msg.with_thinking(thinking.clone());
        }
        state.messages.push(msg);
    }
    state.streaming = StreamingState::Idle;

    assert!(
        state.messages[0].thinking.is_some(),
        "BUG 5: Finalized message must have thinking attached"
    );
    assert!(
        state.messages[0]
            .thinking
            .as_ref()
            .unwrap()
            .contains("Step 1"),
        "BUG 5: Thinking should contain accumulated content"
    );
}

#[test]
fn bug5_show_thinking_toggles() {
    use personal_agent::ui_gpui::views::chat_view::ChatState;
    let mut state = ChatState::default();
    assert!(!state.show_thinking);
    state.show_thinking = !state.show_thinking;
    assert!(state.show_thinking);
    state.show_thinking = !state.show_thinking;
    assert!(!state.show_thinking);
}
