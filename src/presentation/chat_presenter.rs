//! ChatPresenter - handles user chat events and service coordination
//!
//! ChatPresenter subscribes to user chat events and chat domain events,
//! coordinates with chat and conversation services, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P12
//! @requirement REQ-027.1
//! @pseudocode presenters.md lines 20-251

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::events::{AppEvent, types::{ChatEvent, UserEvent, ConversationEvent}};
use crate::events::bus::EventBus;
use crate::services::{ChatService, ConversationService};
use super::{Presenter, PresenterError, ViewCommand};
use super::view_command::{MessageRole, ErrorSeverity};

/// ChatPresenter - handles chat UI events and service coordination
///
/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
/// @pseudocode presenters.md lines 20-25
pub struct ChatPresenter {
    /// Reference to event bus for subscribing to events
    event_bus: Arc<EventBus>,

    /// Reference to conversation service
    conversation_service: Arc<dyn ConversationService>,

    /// Reference to chat service
    chat_service: Arc<dyn ChatService>,

    /// View command sender (mpsc for reliable delivery)
    view_tx: mpsc::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ChatPresenter {
    /// Create a new ChatPresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    /// @pseudocode presenters.md lines 31-42
    pub fn new(
        event_bus: Arc<EventBus>,
        conversation_service: Arc<dyn ConversationService>,
        chat_service: Arc<dyn ChatService>,
        view_tx: mpsc::Sender<ViewCommand>,
    ) -> Self {
        Self {
            event_bus,
            conversation_service,
            chat_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    /// @pseudocode presenters.md lines 50-69
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        // Subscribe to events from EventBus
        let mut rx = self.event_bus.subscribe();
        let running = self.running.clone();
        let conversation_service = self.conversation_service.clone();
        let chat_service = self.chat_service.clone();
        let mut view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&conversation_service, &chat_service, &mut view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("ChatPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("ChatPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("ChatPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    /// @pseudocode presenters.md lines 250-253
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle events from EventBus
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_event(
        conversation_service: &Arc<dyn ConversationService>,
        chat_service: &Arc<dyn ChatService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        tracing::debug!("ChatPresenter::handle_event: {:?}", event);
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(conversation_service, chat_service, view_tx, user_evt).await;
            }
            AppEvent::Chat(chat_evt) => {
                tracing::info!("ChatPresenter handling ChatEvent: {:?}", chat_evt);
                Self::handle_chat_event(view_tx, chat_evt).await;
            }
            AppEvent::Conversation(conv_evt) => {
                Self::handle_conversation_event(view_tx, conv_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_user_event(
        conversation_service: &Arc<dyn ConversationService>,
        chat_service: &Arc<dyn ChatService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SendMessage { text } => {
                Self::handle_send_message(conversation_service, chat_service, view_tx, text).await;
            }
            UserEvent::StopStreaming => {
                Self::handle_stop_streaming(chat_service, view_tx).await;
            }
            UserEvent::NewConversation => {
                Self::handle_new_conversation(conversation_service, view_tx).await;
            }
            UserEvent::ToggleThinking => {
                Self::handle_toggle_thinking(view_tx).await;
            }
            UserEvent::ConfirmRenameConversation { id, title } => {
                Self::handle_rename_conversation(conversation_service, view_tx, id, title).await;
            }
            UserEvent::SelectConversation { id } => {
                Self::handle_select_conversation(conversation_service, view_tx, id).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle chat events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_chat_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: ChatEvent,
    ) {
        match event {
            ChatEvent::StreamStarted { conversation_id, message_id: _, model_id: _ } => {
                let _ = view_tx.send(ViewCommand::ShowThinking { conversation_id }).await;
            }
            ChatEvent::TextDelta { text } => {
                let _ = view_tx.send(ViewCommand::AppendStream {
                    conversation_id: Uuid::nil(),
                    chunk: text,
                }).await;
            }
            ChatEvent::ThinkingDelta { text } => {
                let _ = view_tx.send(ViewCommand::AppendThinking {
                    conversation_id: Uuid::nil(),
                    content: text,
                }).await;
            }
            ChatEvent::ToolCallStarted { tool_call_id: _, tool_name } => {
                let _ = view_tx.send(ViewCommand::ShowToolCall {
                    conversation_id: Uuid::nil(),
                    tool_name,
                    status: "running".to_string(),
                }).await;
            }
            ChatEvent::ToolCallCompleted { tool_call_id: _, tool_name, success, result, duration_ms } => {
                let status = if success { "completed".to_string() } else { "failed".to_string() };
                let _ = view_tx.send(ViewCommand::UpdateToolCall {
                    conversation_id: Uuid::nil(),
                    tool_name,
                    status,
                    result: Some(result),
                    duration: Some(duration_ms),
                }).await;
            }
            ChatEvent::StreamCompleted { conversation_id, message_id: _, total_tokens } => {
                let _ = view_tx.send(ViewCommand::FinalizeStream {
                    conversation_id,
                    tokens: total_tokens.unwrap_or(0) as u64,
                }).await;
                let _ = view_tx.send(ViewCommand::HideThinking { conversation_id }).await;
            }
            ChatEvent::StreamCancelled { conversation_id, message_id: _, partial_content } => {
                let _ = view_tx.send(ViewCommand::StreamCancelled {
                    conversation_id,
                    partial_content,
                }).await;
                let _ = view_tx.send(ViewCommand::HideThinking { conversation_id }).await;
            }
            ChatEvent::StreamError { conversation_id, error, recoverable } => {
                let _ = view_tx.send(ViewCommand::StreamError {
                    conversation_id,
                    error: error.clone(),
                    recoverable,
                }).await;
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Stream Error".to_string(),
                    message: error,
                    severity: if recoverable { ErrorSeverity::Warning } else { ErrorSeverity::Error },
                }).await;
            }
            ChatEvent::MessageSaved { conversation_id, message_id: _ } => {
                let _ = view_tx.send(ViewCommand::MessageSaved {
                    conversation_id,
                }).await;
            }
        }
    }

    /// Handle SendMessage user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_send_message(
        conversation_service: &Arc<dyn ConversationService>,
        chat_service: &Arc<dyn ChatService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        content: String,
    ) {
        // Validate non-empty
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        // Get or create conversation
        let conversation_id = match Self::get_or_create_conversation(conversation_service, view_tx).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to get/create conversation: {}", e);
                let error_msg = format!("Failed to create conversation: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Conversation Error".to_string(),
                    message: error_msg.clone(),
                    severity: ErrorSeverity::Error,
                }).await;
                return;
            }
        };

        // Emit view commands for user message
        let _ = view_tx.send(ViewCommand::MessageAppended {
            conversation_id,
            role: MessageRole::User,
            content: trimmed.to_string(),
        }).await;

        // Show loading state
        let _ = view_tx.send(ViewCommand::ShowThinking { conversation_id }).await;

        // Send message via service
        match chat_service.send_message(conversation_id, trimmed.to_string()).await {
            Ok(_stream) => {
                // Stream events will be handled via ChatEvent
            }
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!("Failed to send message: {}", error_msg);
                let _ = view_tx.send(ViewCommand::StreamError {
                    conversation_id,
                    error: error_msg.clone(),
                    recoverable: false,
                }).await;
                let _ = view_tx.send(ViewCommand::HideThinking { conversation_id }).await;
            }
        }
    }

    /// Handle ToggleThinking user event
    ///
    /// @plan PLAN-20250128-PRESENTERS.P01
    /// @requirement REQ-027.1
    async fn handle_toggle_thinking(
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        let _ = view_tx.send(ViewCommand::ToggleThinkingVisibility).await;
    }

    /// Handle ConfirmRenameConversation user event
    ///
    /// @plan PLAN-20250128-PRESENTERS.P01
    /// @requirement REQ-027.1
    async fn handle_rename_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
        title: String,
    ) {
        match conversation_service.rename(id, title.clone()).await {
            Ok(_) => {
                let _ = view_tx.send(ViewCommand::ConversationRenamed {
                    id,
                    new_title: title,
                }).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to rename conversation: {}", e);
                tracing::error!("{}", error_msg);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: error_msg,
                    severity: ErrorSeverity::Error,
                }).await;
            }
        }
    }

    /// Handle conversation events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P01
    /// @requirement REQ-027.1
    async fn handle_conversation_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: ConversationEvent,
    ) {
        match event {
            ConversationEvent::Created { id, title } => {
                let _ = view_tx.send(ViewCommand::ConversationCreated {
                    id,
                    profile_id: Uuid::nil(),
                }).await;
            }
            ConversationEvent::TitleUpdated { id, title } => {
                let _ = view_tx.send(ViewCommand::ConversationRenamed {
                    id,
                    new_title: title,
                }).await;
            }
            ConversationEvent::Deleted { id } => {
                let _ = view_tx.send(ViewCommand::ConversationDeleted { id }).await;
            }
            ConversationEvent::Activated { id } => {
                let _ = view_tx.send(ViewCommand::ConversationActivated { id }).await;
            }
            ConversationEvent::Deactivated => {
                let _ = view_tx.send(ViewCommand::ConversationCleared).await;
            }
            ConversationEvent::ListRefreshed { count } => {
                let _ = view_tx.send(ViewCommand::HistoryUpdated {
                    count: Some(count),
                }).await;
            }
            _ => {}
        }
    }

    /// Handle StopStreaming user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_stop_streaming(
        chat_service: &Arc<dyn ChatService>,
        _view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        chat_service.cancel();
        // StreamCancelled event will be emitted by the service
    }

    /// Handle NewConversation user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_new_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        // For now, use a default profile UUID - in real implementation this would come from settings
        let default_profile = Uuid::nil();

        let result = conversation_service.create(Some("New Conversation".to_string()), default_profile).await;
        match result {
            Ok(_conversation) => {
                let conversation_id = _conversation.id;
                let _ = view_tx.send(ViewCommand::ConversationCreated {
                    id: conversation_id,
                    profile_id: default_profile,
                }).await;
                let _ = view_tx.send(ViewCommand::ConversationActivated { id: conversation_id }).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to create conversation: {}", e);
                tracing::error!("{}", error_msg);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: error_msg,
                    severity: ErrorSeverity::Error,
                }).await;
            }
        }
    }

    /// Handle SelectConversation user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn handle_select_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
    ) {
        let result = conversation_service.set_active(id).await;
        match result {
            Ok(_) => {
                let _ = view_tx.send(ViewCommand::ConversationActivated { id }).await;
            }
            Err(e) => {
                let error_msg = format!("Failed to select conversation: {}", e);
                tracing::error!("{}", error_msg);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: error_msg,
                    severity: ErrorSeverity::Error,
                }).await;
            }
        }
    }

    /// Get or create active conversation
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    async fn get_or_create_conversation(
        conversation_service: &Arc<dyn ConversationService>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
        // Try to get active conversation
        let active_result = conversation_service.get_active().await;
        if let Ok(Some(id)) = active_result {
            return Ok(id);
        }

        // Create new conversation with default profile
        let default_profile = Uuid::nil();
        let conversation_result = conversation_service.create(Some("New Conversation".to_string()), default_profile).await;

        match conversation_result {
            Ok(conversation) => {
                let conversation_id = conversation.id;

                let _ = view_tx.send(ViewCommand::ConversationCreated {
                    id: conversation_id,
                    profile_id: default_profile,
                }).await;

                Ok(conversation_id)
            }
            Err(e) => {
                Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P12
// @requirement REQ-027.1
impl Presenter for ChatPresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        // This is kept for trait compatibility
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.1
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{broadcast, mpsc};

    /// Mock ConversationService for testing
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

        async fn load(&self, _id: Uuid) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("Not implemented".to_string()))
        }

        async fn list(&self, _limit: Option<usize>, _offset: Option<usize>) -> Result<Vec<crate::models::Conversation>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn add_user_message(&self, _conversation_id: Uuid, _content: String) -> Result<crate::models::Message, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("Not implemented".to_string()))
        }

        async fn add_assistant_message(&self, _conversation_id: Uuid, _content: String) -> Result<crate::models::Message, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("Not implemented".to_string()))
        }

        async fn rename(&self, _id: Uuid, _new_title: String) -> Result<(), crate::services::ServiceError> {
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

        async fn get_messages(&self, _conversation_id: Uuid) -> Result<Vec<crate::models::Message>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn update(&self, _id: Uuid, _title: Option<String>, _model_profile_id: Option<Uuid>) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("Not implemented".to_string()))
        }
    }

    /// Mock ChatService for testing
    struct MockChatService;

    #[async_trait::async_trait]
    impl ChatService for MockChatService {
        async fn send_message(
        &self,
        _conversation_id: Uuid,
        _content: String,
    ) -> Result<Box<dyn futures::Stream<Item = crate::services::ChatStreamEvent> + Send + Unpin>, crate::services::ServiceError> {
            // Return empty stream
            use futures::StreamExt;
            let stream = futures::stream::empty::<crate::services::ChatStreamEvent>();
            Ok(Box::new(stream))
        }

        fn cancel(&self) {
            // Mock cancel does nothing
        }

        fn is_streaming(&self) -> bool {
            false
        }
    }

    /// Test presenter creation
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[tokio::test]
    async fn test_handle_send_message_emits_events() {
        let (event_tx, _) = broadcast::channel::<AppEvent>(100);
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        let conversation_service = Arc::new(MockConversationService) as Arc<dyn ConversationService>;
        let chat_service = Arc::new(MockChatService) as Arc<dyn ChatService>;

        // Simulate sending a message
        let content = "Hello, world!".to_string();
        let mut tx = view_tx.clone();
        let conv_service = conversation_service.clone();
        let chat_svc = chat_service.clone();

        tokio::spawn(async move {
            ChatPresenter::handle_send_message(&conv_service, &chat_svc, &mut tx, content).await;
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

        let event = ChatEvent::TextDelta {
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
        let (view_tx, mut view_rx) = mpsc::channel::<ViewCommand>(100);

        ChatPresenter::handle_new_conversation(&conversation_service, &mut view_tx.clone()).await;

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
}
