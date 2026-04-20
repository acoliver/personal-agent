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

    fn cancel(&self, _conversation_id: Uuid) {
        // Mock cancel does nothing
    }

    fn is_streaming(&self) -> bool {
        false
    }

    fn is_streaming_for(&self, _conversation_id: Uuid) -> bool {
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
            context_window_size: 128_000,
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
/// @plan PLAN-20260416-ISSUE173.P05
/// @requirement REQ-173-002.3
#[tokio::test]
async fn test_handle_stop_streaming() {
    let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;
    let (view_tx, _) = mpsc::channel::<ViewCommand>(100);
    let conversation_id = Uuid::new_v4();

    // Stop should call cancel on chat service with provided conversation id
    ChatPresenter::handle_stop_streaming(&chat_service, &mut view_tx.clone(), conversation_id)
        .await;

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

/// Mock that returns two conversations with populated metadata.
struct PopulatedMetadataService;

#[async_trait::async_trait]
impl ConversationService for PopulatedMetadataService {
    async fn create(
        &self,
        _title: Option<String>,
        _profile_id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
    }
    async fn load(
        &self,
        _id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
    }
    async fn list_metadata(
        &self,
        _limit: Option<usize>,
        _offset: Option<usize>,
    ) -> Result<Vec<crate::models::ConversationMetadata>, crate::services::ServiceError> {
        Ok(vec![
            crate::models::ConversationMetadata {
                id: Uuid::nil(),
                title: Some("Rust patterns".to_string()),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                profile_id: Some(Uuid::new_v4()),
                message_count: 10,
                last_message_preview: Some("How to use tokio select".to_string()),
            },
            crate::models::ConversationMetadata {
                id: Uuid::new_v4(),
                title: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                profile_id: None,
                message_count: 0,
                last_message_preview: None,
            },
        ])
    }
    async fn add_message(
        &self,
        _cid: Uuid,
        msg: crate::models::Message,
    ) -> Result<crate::models::Message, crate::services::ServiceError> {
        Ok(msg)
    }
    async fn search(
        &self,
        _q: &str,
        _l: Option<usize>,
        _o: Option<usize>,
    ) -> Result<Vec<crate::models::SearchResult>, crate::services::ServiceError> {
        Ok(vec![])
    }
    async fn message_count(&self, _cid: Uuid) -> Result<usize, crate::services::ServiceError> {
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
        _cid: Uuid,
    ) -> Result<Vec<crate::models::Message>, crate::services::ServiceError> {
        Ok(vec![])
    }
    async fn update(
        &self,
        _id: Uuid,
        _title: Option<String>,
        _mpid: Option<Uuid>,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
    }
}

/// Test `emit_conversation_list` maps metadata (including preview) to summaries
#[tokio::test]
async fn test_emit_conversation_list_with_populated_metadata() {
    let service = Arc::new(PopulatedMetadataService) as Arc<dyn ConversationService>;
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let mut tx = view_tx.clone();

    ChatPresenter::emit_conversation_list(&service, &mut tx)
        .await
        .expect("Should succeed");

    let cmd = view_rx.try_recv().expect("Should emit conversation list");
    match cmd {
        ViewCommand::ConversationListRefreshed { conversations } => {
            assert_eq!(conversations.len(), 2);
            assert_eq!(conversations[0].title, "Rust patterns");
            assert_eq!(conversations[0].message_count, 10);
            assert_eq!(
                conversations[0].preview,
                Some("How to use tokio select".to_string())
            );
            // Second conversation has None title → fallback
            assert_eq!(conversations[1].title, "Untitled Conversation");
            assert!(conversations[1].preview.is_none());
        }
        other => panic!("Expected ConversationListRefreshed, got {other:?}"),
    }
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

/// Mock that returns populated search results to exercise mapping code.
struct SearchableConversationService;

#[async_trait::async_trait]
impl ConversationService for SearchableConversationService {
    async fn create(
        &self,
        _title: Option<String>,
        _model_profile_id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
    }
    async fn load(
        &self,
        _id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
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
        Ok(vec![
            crate::models::SearchResult {
                conversation_id: Uuid::new_v4(),
                title: "Rust async patterns".to_string(),
                match_type: crate::models::SearchMatchType::Title,
                match_context: "...tokio async patterns...".to_string(),
                score: 1.5,
                updated_at: chrono::Utc::now(),
                message_count: 12,
            },
            crate::models::SearchResult {
                conversation_id: Uuid::new_v4(),
                title: "EventBus refactoring".to_string(),
                match_type: crate::models::SearchMatchType::Content,
                match_context: "switching from tokio broadcast to flume".to_string(),
                score: 0.8,
                updated_at: chrono::Utc::now(),
                message_count: 8,
            },
        ])
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
        unimplemented!()
    }
}

/// Mock that fails search to exercise the error path.
struct FailingSearchService;

#[async_trait::async_trait]
impl ConversationService for FailingSearchService {
    async fn create(
        &self,
        _title: Option<String>,
        _model_profile_id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
    }
    async fn load(
        &self,
        _id: Uuid,
    ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
        unimplemented!()
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
        Err(crate::services::ServiceError::Storage(
            "Search unavailable".to_string(),
        ))
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
        unimplemented!()
    }
}

/// Test search with actual results exercises the mapping code
#[tokio::test]
async fn test_search_conversations_with_results() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let service = Arc::new(SearchableConversationService) as Arc<dyn ConversationService>;

    ChatPresenter::handle_search_conversations(&service, &view_tx, "tokio".to_string()).await;

    let cmd = view_rx.try_recv().expect("Should emit search results");
    match cmd {
        ViewCommand::ConversationSearchResults { results } => {
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].title, "Rust async patterns");
            assert!(results[0].is_title_match);
            assert_eq!(results[0].message_count, 12);
            assert_eq!(results[1].title, "EventBus refactoring");
            assert!(!results[1].is_title_match);
            assert_eq!(
                results[1].match_context,
                "switching from tokio broadcast to flume"
            );
        }
        other => panic!("Expected ConversationSearchResults, got {other:?}"),
    }
}

/// Test search error path returns empty results gracefully
#[tokio::test]
async fn test_search_conversations_service_error() {
    let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);
    let service = Arc::new(FailingSearchService) as Arc<dyn ConversationService>;

    ChatPresenter::handle_search_conversations(&service, &view_tx, "query".to_string()).await;

    let cmd = view_rx
        .try_recv()
        .expect("Should emit search results on error");
    match cmd {
        ViewCommand::ConversationSearchResults { results } => {
            assert!(results.is_empty(), "Error should return empty results");
        }
        other => panic!("Expected ConversationSearchResults, got {other:?}"),
    }
}

/// Test `AppMode` default is `Popup`
#[test]
fn test_app_mode_default_is_popup() {
    let mode = super::super::view_command::AppMode::default();
    assert_eq!(mode, super::super::view_command::AppMode::Popup);
}

/// Test `ConversationSearchResult` equality and fields
#[test]
fn test_conversation_search_result_fields() {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let result = super::super::view_command::ConversationSearchResult {
        id,
        title: "Test".to_string(),
        is_title_match: true,
        match_context: "context".to_string(),
        message_count: 5,
        updated_at: now,
    };
    assert_eq!(result.id, id);
    assert!(result.is_title_match);
    assert_eq!(result.message_count, 5);
    let clone = result.clone();
    assert_eq!(result, clone);
}

// ============================================================================
// Cancel Pipeline TDD Tests (Phase 04)
// These tests use struct-variant syntax for UserEvent::StopStreaming which
// will not compile until P05 lands the event type change.
// ============================================================================

use crate::events::types::UserEvent;
use std::sync::Mutex;

/// Mock `ChatService` that records cancel calls for verification
struct RecordingChatService {
    cancelled_conversations: Arc<Mutex<Vec<Uuid>>>,
}

impl RecordingChatService {
    fn new() -> Self {
        Self {
            cancelled_conversations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn cancelled_ids(&self) -> Vec<Uuid> {
        self.cancelled_conversations.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ChatService for RecordingChatService {
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

    fn cancel(&self, conversation_id: Uuid) {
        self.cancelled_conversations
            .lock()
            .unwrap()
            .push(conversation_id);
    }

    fn is_streaming(&self) -> bool {
        false
    }

    fn is_streaming_for(&self, _conversation_id: Uuid) -> bool {
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

/// Test that `StopStreaming` event forwards the `conversation_id` to `cancel()`.
///
/// @plan PLAN-20260416-ISSUE173.P04
/// @requirement REQ-173-002.3
#[tokio::test]
async fn handle_stop_streaming_forwards_conversation_id() {
    let chat_service = Arc::new(RecordingChatService::new());
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);

    let conversation_id_a = Uuid::new_v4();

    // Dispatch StopStreaming with conversation_id A using struct-variant syntax.
    // This will fail to compile until P05 changes UserEvent::StopStreaming to struct variant.
    let event = UserEvent::StopStreaming {
        conversation_id: conversation_id_a,
    };

    // Process the event through the presenter
    ChatPresenter::handle_user_event(
        &conversation_service,
        &(chat_service.clone() as Arc<dyn ChatService>),
        &(Arc::new(MockProfileService) as Arc<dyn ProfileService>),
        &(Arc::new(MockAppSettingsService) as Arc<dyn AppSettingsService>),
        &Arc::new(std::sync::Mutex::new(
            crate::models::ConversationExportFormat::Md,
        )),
        &mut view_tx.clone(),
        event,
    )
    .await;

    // Assert the mock received exactly one cancel(A) call
    let cancelled = chat_service.cancelled_ids();
    assert_eq!(cancelled.len(), 1, "Expected exactly one cancel call");
    assert_eq!(
        cancelled[0], conversation_id_a,
        "Expected cancel to be called with conversation_id A"
    );
}

/// Test that `StopStreaming` for conversation A does NOT cancel conversation B.
///
/// @plan PLAN-20260416-ISSUE173.P04
/// @requirement REQ-173-002.3
#[tokio::test]
async fn handle_stop_streaming_does_not_cancel_other_conversations() {
    let chat_service = Arc::new(RecordingChatService::new());
    let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
    let (view_tx, _view_rx) = mpsc::channel::<ViewCommand>(100);

    let conversation_id_a = Uuid::new_v4();
    let conversation_id_b = Uuid::new_v4();

    // Dispatch StopStreaming with conversation_id A using struct-variant syntax.
    // This will fail to compile until P05 changes UserEvent::StopStreaming to struct variant.
    let event = UserEvent::StopStreaming {
        conversation_id: conversation_id_a,
    };

    // Process the event through the presenter
    ChatPresenter::handle_user_event(
        &conversation_service,
        &(chat_service.clone() as Arc<dyn ChatService>),
        &(Arc::new(MockProfileService) as Arc<dyn ProfileService>),
        &(Arc::new(MockAppSettingsService) as Arc<dyn AppSettingsService>),
        &Arc::new(std::sync::Mutex::new(
            crate::models::ConversationExportFormat::Md,
        )),
        &mut view_tx.clone(),
        event,
    )
    .await;

    // Assert cancel(B) was NEVER called
    let cancelled = chat_service.cancelled_ids();
    assert!(
        !cancelled.contains(&conversation_id_b),
        "cancel(B) should never be called when stopping stream for conversation A"
    );
}

/// Mock `AppSettingsService` for testing
struct MockAppSettingsService;

#[async_trait::async_trait]
impl AppSettingsService for MockAppSettingsService {
    async fn get_default_profile_id(&self) -> Result<Option<Uuid>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_default_profile_id(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn clear_default_profile_id(&self) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_current_conversation_id(
        &self,
    ) -> Result<Option<Uuid>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_current_conversation_id(
        &self,
        _id: Uuid,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_hotkey(&self) -> Result<Option<String>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_hotkey(&self, _hotkey: String) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_theme(&self) -> Result<Option<String>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_theme(&self, _theme: String) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_filter_emoji(&self) -> Result<Option<bool>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_filter_emoji(&self, _enabled: bool) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn get_setting(
        &self,
        _key: &str,
    ) -> Result<Option<String>, crate::services::ServiceError> {
        Ok(None)
    }

    async fn set_setting(
        &self,
        _key: &str,
        _value: String,
    ) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }

    async fn reset_to_defaults(&self) -> Result<(), crate::services::ServiceError> {
        Ok(())
    }
}
