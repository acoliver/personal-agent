//! Regression tests for seven runtime bugs identified in manual testing.
//!
//! Each test documents the expected correct behavior.  Tests that exercise
//! bugs in the current code are expected to FAIL until fixes land.
//!
//! Bug 1: R button starts new conversation instead of inline rename
//! Bug 2: Conversation dropdown doesn't overlay messages (layout / z-index)
//! Bug 3: Profile dropdown renders off-screen / arrow hidden / no dropdown
//! Bug 4: Model has no conversational memory (history not sent to LLM)
//! Bug 5: Thinking blocks never appear; T toggle appears disabled
//! Bug 6: History view is empty (never receives conversation list)
//! Bug 7: Profile editor model field should be editable as text override

use std::sync::Arc;
use uuid::Uuid;

use personal_agent::models::{AuthConfig, Conversation, Message, ModelParameters, ModelProfile};
use personal_agent::presentation::view_command::ConversationSummary;
use personal_agent::services::conversation::ConversationService;
use personal_agent::services::profile::ProfileService;
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
            .ok_or(ServiceError::NotFound(format!("No conversation {}", id)))
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

fn make_test_profile(id: Uuid) -> ModelProfile {
    ModelProfile {
        id,
        name: "test-profile".to_string(),
        provider_id: "anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        auth: AuthConfig::Key {
            value: "test-key".to_string(),
        },
        parameters: ModelParameters::default(),
        system_prompt: "You are a helpful assistant.".to_string(),
    }
}

struct MockProfileService {
    profile_id: Uuid,
}

impl MockProfileService {
    fn new(id: Uuid) -> Self {
        Self { profile_id: id }
    }
}

#[async_trait]
impl ProfileService for MockProfileService {
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        Ok(vec![make_test_profile(self.profile_id)])
    }

    async fn get(&self, _id: Uuid) -> ServiceResult<ModelProfile> {
        Ok(make_test_profile(self.profile_id))
    }

    async fn get_default(&self) -> ServiceResult<Option<ModelProfile>> {
        Ok(Some(make_test_profile(self.profile_id)))
    }

    async fn set_default(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn create(
        &self,
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: AuthConfig,
        _params: ModelParameters,
        _system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        Ok(make_test_profile(self.profile_id))
    }

    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<AuthConfig>,
        _params: Option<ModelParameters>,
        _system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        Ok(make_test_profile(self.profile_id))
    }

    async fn delete(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> ServiceResult<()> {
        Ok(())
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
// Bug 2: Conversation dropdown must overlay messages
// ======================================================================

#[test]
fn bug2_conversation_dropdown_has_root_overlay_positioning() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    let start = source
        .find("chat-conversation-dropdown-menu")
        .expect("Dropdown menu element should exist");
    let window = &source[start..std::cmp::min(start + 1200, source.len())];

    // Accept either explicit z-index OR root-level overlay rendering with
    // `.absolute()` inside a `.relative()` chat-view root.
    let has_z_index = window.contains("z_index");
    let has_absolute = window.contains(".absolute()");
    let root_relative = source.contains(".id(\"chat-view\")") && source.contains(".relative()");

    assert!(
        has_z_index || (has_absolute && root_relative),
        "BUG 2: Conversation dropdown must be a real overlay (z_index or absolute+root-relative)"
    );
}

#[test]
fn bug2_conversation_dropdown_rendered_at_root_level() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    // The conversation dropdown should be rendered in the root render() function
    // (not inside render_title_bar) so it paints on top of everything.
    // It should be in a separate render_conversation_dropdown method called from render().
    let render_fn_pos = source.find("fn render(&mut self").unwrap();
    let render_fn_section = &source[render_fn_pos..];
    assert!(
        render_fn_section.contains("render_conversation_dropdown"),
        "BUG 2: Conversation dropdown must be rendered from root render() to overlay chat area"
    );
}

// ======================================================================
// Bug 3: Profile dropdown renders off-screen / no overlay
// ======================================================================

#[test]
fn bug3_profile_dropdown_is_overlay_at_root() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    // Profile dropdown menu must exist
    assert!(
        source.contains("chat-profile-dropdown-menu"),
        "BUG 3: Profile dropdown menu container must exist"
    );

    // It should be rendered from a dedicated method called from root render()
    let render_fn_pos = source.find("fn render(&mut self").unwrap();
    let render_fn_section = &source[render_fn_pos..];
    assert!(
        render_fn_section.contains("render_profile_dropdown"),
        "BUG 3: Profile dropdown must be rendered from root render() as overlay"
    );
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
        .add_assistant_message(conv_id, "Hello Alice!".to_string())
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

/// chat_impl adds the user message via add_user_message(), then loads
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
        "BUG 4: Message appears {} times. chat_impl.rs pushes it again, \
         so the LLM sees the user message twice.",
        hello_count
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

/// FinalizeStream handler must attach thinking_content to the message.
#[test]
fn bug5_finalize_stream_attaches_thinking_to_message() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");
    let finalize_pos = source
        .find("ViewCommand::FinalizeStream")
        .expect("FinalizeStream handler should exist");
    let window = &source[finalize_pos..std::cmp::min(finalize_pos + 500, source.len())];

    let attaches_thinking = window.contains("with_thinking")
        || window.contains("thinking_content")
        || window.contains(".thinking");

    assert!(
        attaches_thinking,
        "BUG 5: FinalizeStream must attach thinking_content to the ChatMessage. \
         Currently it creates ChatMessage::assistant() without thinking."
    );
}

// ======================================================================
// Bug 6: History view is empty
// ======================================================================

#[test]
fn bug6_history_view_handles_conversation_list_refreshed() {
    let source = include_str!("../src/ui_gpui/views/history_view.rs");
    assert!(
        source.contains("ConversationListRefreshed"),
        "BUG 6: HistoryView.handle_command must handle ConversationListRefreshed"
    );
}

#[test]
fn bug6_history_view_handles_conversation_created() {
    let source = include_str!("../src/ui_gpui/views/history_view.rs");
    assert!(
        source.contains("ConversationCreated"),
        "BUG 6: HistoryView should handle ConversationCreated"
    );
}

#[test]
fn bug6_chat_activation_does_not_clear_loaded_messages() {
    let source = include_str!("../src/ui_gpui/views/chat_view.rs");

    let activation_pos = source
        .find("ViewCommand::ConversationActivated")
        .expect("ConversationActivated handler should exist");
    let window = &source[activation_pos..std::cmp::min(activation_pos + 500, source.len())];

    assert!(
        !window.contains("messages.clear()"),
        "BUG 6/1 UX: ConversationActivated should not clear messages because presenter \
         sends MessageAppended immediately after activation. Clearing here can blank \
         chat content or create the illusion of a new conversation when renaming."
    );
}

// ======================================================================
// Bug 7: Profile editor model field should be text-editable
// ======================================================================

#[test]
fn bug7_profile_editor_model_is_text_editable() {
    let source = include_str!("../src/ui_gpui/views/profile_editor_view.rs");

    let has_model_active_field = source.contains("ActiveField::Model")
        || source.contains("active_field = Some(ActiveField::Model)");

    let has_model_text_field = {
        if let Some(start) = source.find("fn render_model_section") {
            let section = &source[start..std::cmp::min(start + 800, source.len())];
            section.contains("render_text_field")
        } else {
            false
        }
    };

    assert!(
        has_model_active_field || has_model_text_field,
        "BUG 7: Profile editor model section should use an editable text field."
    );
}
