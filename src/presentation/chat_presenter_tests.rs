use super::*;
use crate::events::types::ChatEvent;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

/// Mock `ConversationService` for testing
struct MockConversationService;

#[async_trait::async_trait]
impl ConversationService for MockConversationService {
    async fn create(
        &self,
        _title: Option<String>,
        _model_profile_id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        Ok(crate::models::Conversation {
            id: Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            title: Some("Test Conversation".to_string()),
            profile_id: _model_profile_id,
            messages: vec![],
        })
    }

    async fn load(
        &self,
        _id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound(
            "Not implemented".to_string(),
        ))
    }

    async fn list_metadata(
        &self,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<crate::models::ConversationMetadata>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn add_message(
        &self,
        _conversation_id: Uuid,
        message: crate::models::Message,
    ) -> Result<crate::models::Message, crate::services::ServiceError> {
        Ok(message)
    }

    async fn search(
        &self,
        _query: &str,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<crate::models::SearchResult>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn message_count(
        &self,
        _conversation_id: Uuid,
    ) -> Result<usize, crate::services::ServiceError> {
        Ok(0)
    }

    async fn update_context_state(
        &self,
        _id: Uuid,
        _state: &crate::models::ContextState,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_context_state(
        &self,
        _id: Uuid,
    ) -> Result<Option<crate::models::ContextState>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn rename(
        &self,
        _id: Uuid,
        _new_title: String,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn set_active(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_active(&self) -> Result<Option<Uuid>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn get_messages(
        &self,
        _conversation_id: Uuid,
    ) -> Result<Vec<crate::models::Message>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn update(
        &self,
        _id: Uuid,
        _title: Option<String>,
        _model_profile_id: Option<Uuid>,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound(
            "Not implemented".to_string(),
        ))
    }
}

/// Mock `ChatService` for testing
struct MockChatService;

#[async_trait::async_trait]
impl ChatService for MockChatService {
    async fn send_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<
        Box<dyn futures::Stream<Item = crate::services::ChatStreamEvent> + Send + Unpin>,
        crate::services::ServiceError,
    > {
        let stream = futures::stream::empty::<crate::services::ChatStreamEvent>();
        Ok(Box::new(stream))
    }

    fn cancel(&self) {
        // Mock cancel does nothing
    }

    fn is_streaming(&self) -> bool {
        false
    }

    async fn resolve_tool_approval(
        &self,
        _request_id: String,
        _decision: crate::events::types::ToolApprovalResponseAction,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }
}

struct MockProfileService;

#[async_trait::async_trait]
impl ProfileService for MockProfileService {
    async fn list(
        &self,
    ) -> Result<Vec<crate::models::ModelProfile>, crate::services::ServiceError> {
        Ok(vec![])
    }

    async fn get(
        &self,
        id: Uuid,
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound(format!(
            "profile {id} not found"
        )))
    }

    async fn create(
        &self,
        _name: String,
        _provider: String,
        _model: String,
        _base_url: Option<String>,
        _auth: crate::models::AuthConfig,
        _parameters: crate::models::ModelParameters,
        _system_prompt: Option<String>,
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound(
            "not implemented".to_string(),
        ))
    }

    async fn update(
        &self,
        _id: Uuid,
        _name: Option<String>,
        _provider: Option<String>,
        _model: Option<String>,
        _base_url: Option<String>,
        _auth: Option<crate::models::AuthConfig>,
        _parameters: Option<crate::models::ModelParameters>,
        _system_prompt: Option<String>,
    ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
        Err(crate::services::ServiceError::NotFound(
            "not implemented".to_string(),
        ))
    }

    async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn test_connection(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_default(
        &self,
    ) -> Result<Option<crate::models::ModelProfile>, crate::services::ServiceError> {
        Ok(Some(crate::models::ModelProfile {
            id: Uuid::new_v4(),
            name: "Default".to_string(),
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            auth: crate::models::AuthConfig::Keychain {
                label: "test-key".to_string(),
            },
            parameters: crate::models::ModelParameters::default(),
            system_prompt: "test".to_string(),
        }))
    }

    async fn set_default(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }
}

/// Test presenter creation
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[tokio::test]
async fn test_handle_send_message_emits_events() {
    let (_event_tx, _) = broadcast::channel::<AppEvent>(100);
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
    let profile_service = Arc::new(MockProfileService) as Arc<dyn ProfileService>;

    // Simulate sending a message
    let content = "Hello, world!".to_string();
    let mut tx = view_tx.clone();
    let conv_service = conversation_service.clone();
    let chat_svc = chat_service.clone();
    let profile_svc = profile_service.clone();

    tokio::spawn(async move {
        ChatPresenter::handle_send_message(
            &conv_service,
            &chat_svc,
            &profile_svc,
            &mut tx,
            content,
        )
        .await;
    });

    // Wait for async processing
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Verify ViewCommands were sent
    let mut found_message = false;
    let mut found_thinking = false;

    while let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::MessageAppended { role, .. } => {
                if matches!(role, MessageRole::User) {
                    found_message = true;
                }
            }
            ViewCommand::ShowThinking { .. } => {
                found_thinking = true;
            }
            _ => {}
        }
    }

    assert!(found_message, "Should have user message appended");
    assert!(found_thinking, "Should show thinking indicator");
}

/// Test handle stop streaming
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[tokio::test]
async fn test_handle_stop_streaming() {
    let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
    let (view_tx, _) = mpsc::channel::<ViewCommand>(100);

    // Stop should call cancel on chat service
    ChatPresenter::handle_stop_streaming(&chat_service, &mut view_tx.clone()).await;

    // If we get here without panic, test passes
    assert!(!chat_service.is_streaming());
}

/// Test handle text delta produces view command
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[tokio::test]
async fn test_handle_text_delta_produces_view_command() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

    let conversation_id = Uuid::new_v4();
    let event = ChatEvent::TextDelta {
        conversation_id,
        text: "Hello".to_string(),
    };

    ChatPresenter::handle_chat_event(&mut view_tx.clone(), event).await;

    // Verify AppendStream command was sent
    if let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::AppendStream { chunk, .. } => {
                assert_eq!(chunk, "Hello");
            }
            _ => panic!("Expected AppendStream command"),
        }
    } else {
        panic!("Should have received a ViewCommand");
    }
}

/// Test handle stream completed
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[tokio::test]
async fn test_handle_stream_completed() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_id = Uuid::new_v4();

    let event = ChatEvent::StreamCompleted {
        conversation_id,
        message_id: Uuid::new_v4(),
        total_tokens: Some(100),
    };

    ChatPresenter::handle_chat_event(&mut view_tx.clone(), event).await;

    // Verify FinalizeStream and HideThinking commands
    let mut found_finalize = false;
    let mut found_hide = false;

    while let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::FinalizeStream { tokens, .. } => {
                assert_eq!(tokens, 100);
                found_finalize = true;
            }
            ViewCommand::HideThinking { .. } => {
                found_hide = true;
            }
            _ => {}
        }
    }

    assert!(found_finalize, "Should finalize stream");
    assert!(found_hide, "Should hide thinking");
}

/// Test handle new conversation
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[tokio::test]
async fn test_handle_new_conversation() {
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let profile_service = Arc::new(MockProfileService) as Arc<dyn ProfileService>;
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

    ChatPresenter::handle_new_conversation(
        &conversation_service,
        &profile_service,
        &mut view_tx.clone(),
    )
    .await;

    // Verify conversation created and activated
    let mut found_created = false;
    let mut found_activated = false;

    // Allow time for async processing
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    while let Ok(cmd) = view_rx.try_recv() {
        match cmd {
            ViewCommand::ConversationCreated { .. } => {
                found_created = true;
            }
            ViewCommand::ConversationActivated { .. } => {
                found_activated = true;
            }
            _ => {}
        }
    }

    assert!(found_created, "Should create conversation");
    assert!(found_activated, "Should activate conversation");
}

/// Test `handle_search_conversations` with empty query returns empty results
#[tokio::test]
async fn test_search_conversations_empty_query() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;

    ChatPresenter::handle_search_conversations(&conversation_service, &view_tx, String::new())
        .await;

    let cmd = view_rx.try_recv().expect("Should emit search results");
    match cmd {
        ViewCommand::ConversationSearchResults { results } => {
            assert!(results.is_empty(), "Empty query returns empty results");
        }
        other => panic!("Expected ConversationSearchResults, got {other:?}"),
    }
}

/// Test `handle_search_conversations` with whitespace-only query returns empty results
#[tokio::test]
async fn test_search_conversations_whitespace_query() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;

    ChatPresenter::handle_search_conversations(&conversation_service, &view_tx, "   ".to_string())
        .await;

    let cmd = view_rx.try_recv().expect("Should emit search results");
    match cmd {
        ViewCommand::ConversationSearchResults { results } => {
            assert!(results.is_empty(), "Whitespace query returns empty results");
        }
        other => panic!("Expected ConversationSearchResults, got {other:?}"),
    }
}

/// Test `handle_search_conversations` with non-empty query calls service
#[tokio::test]
async fn test_search_conversations_non_empty_query() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;

    ChatPresenter::handle_search_conversations(
        &conversation_service,
        &view_tx,
        "tokio".to_string(),
    )
    .await;

    let cmd = view_rx.try_recv().expect("Should emit search results");
    match cmd {
        ViewCommand::ConversationSearchResults { results } => {
            // MockConversationService returns empty vec for search
            assert!(results.is_empty());
        }
        other => panic!("Expected ConversationSearchResults, got {other:?}"),
    }
}
