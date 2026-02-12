//! Chat service implementation

/// @plan PLAN-20250127-REMEDIATE.P02, PLAN-20250127-REMEDIATE.P03
/// @requirement REM-001, REM-002, REM-003, REM-004, REM-005, REM-006, REM-007

use super::{ChatService, ChatStreamEvent, ServiceError, ServiceResult};
use crate::events::{emit, AppEvent};
use crate::events::types::ChatEvent;
use crate::llm::{LlmClient, Message as LlmMessage, StreamEvent as LlmStreamEvent};
use crate::llm::AgentClientExt;
use crate::mcp::McpService;
use crate::models::{Message, MessageRole};
use crate::services::ConversationService;
use futures::{stream, Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Minimal implementation of ChatService
pub struct ChatServiceImpl {
    conversation_service: Arc<dyn ConversationService>,
    profile_service: Arc<dyn super::ProfileService>,
    is_streaming: Arc<AtomicBool>,
    current_conversation_id: Arc<RwLock<Option<Uuid>>>,
}

impl ChatServiceImpl {
    /// Create a new ChatServiceImpl
    #[must_use]
    pub fn new(
        conversation_service: Arc<dyn ConversationService>,
        profile_service: Arc<dyn super::ProfileService>,
    ) -> Self {
        Self {
            conversation_service,
            profile_service,
            is_streaming: Arc::new(AtomicBool::new(false)),
            current_conversation_id: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait::async_trait]
impl ChatService for ChatServiceImpl {
    /// Send a message and get a streaming response
    async fn send_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>> {
        // Check if already streaming
        if self
            .is_streaming
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return Err(ServiceError::Internal("Stream already in progress".to_string()));
        }

        // Store conversation ID
        *self.current_conversation_id.write().await = Some(conversation_id);

        // Get or create conversation
        let _conversation = match self
            .conversation_service
            .load(conversation_id)
            .await
        {
            Ok(conv) => conv,
            Err(_) => {
                // Create new conversation if it doesn't exist
                // Use default profile for now
                let default_profile = self.profile_service.get_default().await
                    .map_err(|_| ServiceError::Internal("No default profile available".to_string()))?
                    .ok_or_else(|| ServiceError::Internal("No default profile available".to_string()))?;

                self.conversation_service
                    .create(None, default_profile.id)
                    .await?
            }
        };

        // Add user message
        self.conversation_service
            .add_user_message(conversation_id, content.clone())
            .await?;

        // Get default profile
        let profile = self
            .profile_service
            .get_default()
            .await
            .map_err(|_| ServiceError::Internal("No active profile".to_string()))?
            .ok_or_else(|| ServiceError::Internal("No active profile".to_string()))?;

        // Load conversation to get message history
        let conversation = self
            .conversation_service
            .load(conversation_id)
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to load conversation: {}", e)))?;

        // Create LLM client from profile
        let client = LlmClient::from_profile(&profile)
            .map_err(|e| ServiceError::Internal(format!("Failed to create LLM client: {}", e)))?;

        // Convert conversation messages to LlmClient Message format
        let mut messages: Vec<LlmMessage> = conversation
            .messages
            .iter()
            .map(|msg| match msg.role {
                MessageRole::System => LlmMessage::system(msg.content.clone()),
                MessageRole::User => LlmMessage::user(msg.content.clone()),
                MessageRole::Assistant => LlmMessage::assistant(msg.content.clone()),
            })
            .collect();

        // Add the new user message (already added to conversation above)
        messages.push(LlmMessage::user(content.clone()));

        // Generate message ID for this response
        let message_id = Uuid::new_v4();
        let model_id = profile.model_id.clone();

        // Emit stream started event
        emit(AppEvent::Chat(ChatEvent::StreamStarted {
            conversation_id,
            message_id,
            model_id: model_id.clone(),
        }));

        // Get MCP tools for tool use support (REM-004)
        // @requirement REM-004, REM-007
        let mcp_tools = {
            let mcp_service = McpService::global();
            let mcp_guard = mcp_service.lock().await;
            mcp_guard.get_llm_tools()
        };
        
        // Track the assistant response as it streams
        let is_streaming = self.is_streaming.clone();
        let conversation_service = self.conversation_service.clone();
        let event_conversation_id = conversation_id;

        // Use a channel to bridge the callback-based API to a stream
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ChatStreamEvent>();

        // Spawn a task to call the LLM with MCP tools using Agent
        tokio::spawn(async move {
            let mut response_text = String::new();
            let mut thinking_text = String::new();

            // Get system prompt from conversation
            let system_prompt = conversation.messages.iter()
                .find(|m| m.role == MessageRole::System)
                .map(|m| m.content.as_str())
                .unwrap_or("");

            // Create Agent with MCP tools (uses existing AgentClientExt)
            // @requirement AGENT-001, AGENT-003
            let agent = match client.create_agent(mcp_tools, system_prompt).await {
                Ok(a) => a,
                Err(e) => {
                    let err_msg = format!("Failed to create agent: {}", e);
                    emit(AppEvent::Chat(ChatEvent::StreamError {
                        conversation_id: event_conversation_id,
                        error: err_msg.clone(),
                        recoverable: false,
                    }));
                    let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err_msg)));
                    is_streaming.store(false, Ordering::Release);
                    return;
                }
            };

            // Run Agent stream (Agent executes tools internally)
            // @requirement AGENT-005, AGENT-006
            if let Err(e) = client.run_agent_stream(&agent, &messages, |event| {
                match event {
                    LlmStreamEvent::TextDelta(text) => {
                        // Emit ChatEvent via EventBus for real-time UI updates
                        tracing::info!("ChatService emitting TextDelta: '{}'", text);
                        emit(AppEvent::Chat(ChatEvent::TextDelta { text: text.clone() }));
                        // Also send to stream for caller
                        let _ = tx.send(ChatStreamEvent::Token(text.clone()));
                        response_text.push_str(&text);
                    }
                    LlmStreamEvent::ThinkingDelta(text) => {
                        emit(AppEvent::Chat(ChatEvent::ThinkingDelta { text: text.clone() }));
                        thinking_text.push_str(&text);
                    }
                    LlmStreamEvent::Complete => {
                        let _ = tx.send(ChatStreamEvent::Complete);
                    }
                    LlmStreamEvent::Error(err) => {
                        emit(AppEvent::Chat(ChatEvent::StreamError {
                            conversation_id: event_conversation_id,
                            error: err.clone(),
                            recoverable: false,
                        }));
                        let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err)));
                    }
                    // Tool events are handled INSIDE run_agent_stream
                    // Agent automatically executes tools and continues
                    _ => {}
                }
            }).await {
                let err_msg = e.to_string();
                emit(AppEvent::Chat(ChatEvent::StreamError {
                    conversation_id: event_conversation_id,
                    error: err_msg.clone(),
                    recoverable: false,
                }));
                let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err_msg)));
            }

            // Save the assistant message when complete
            if !response_text.is_empty() || !thinking_text.is_empty() {
                let content = if !thinking_text.is_empty() {
                    // For now, just save the text content (thinking could be stored separately)
                    response_text.clone()
                } else {
                    response_text.clone()
                };

                let _ = conversation_service.add_assistant_message(event_conversation_id, content).await;
            }

            // Emit StreamCompleted event
            emit(AppEvent::Chat(ChatEvent::StreamCompleted {
                conversation_id: event_conversation_id,
                message_id: Uuid::new_v4(),
                total_tokens: None,
            }));

            // Reset streaming flag
            is_streaming.store(false, Ordering::Release);
        });

        // Convert the channel receiver to a stream
        let message_stream: Pin<Box<dyn Stream<Item = ChatStreamEvent> + Send>> = Box::pin(stream::unfold(rx, move |mut rx| async move {
            match rx.recv().await {
                Some(event) => Some((event, rx)),
                None => None,
            }
        }));

        Ok(Box::new(message_stream))
    }

    /// Cancel the current streaming operation
    fn cancel(&self) {
        // Reset streaming flag to allow new messages
        self.is_streaming.store(false, Ordering::Release);
    }

    /// Check if currently streaming
    fn is_streaming(&self) -> bool {
        self.is_streaming.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, ModelParameters, Message, MessageRole};
    use std::sync::Arc;

    struct MockConversationService {
        profile_id: Uuid,
    }

    impl MockConversationService {
        fn new(profile_id: Uuid) -> Self {
            Self { profile_id }
        }
    }

    #[async_trait::async_trait]
    impl super::super::ConversationService for MockConversationService {
        async fn create(&self, _title: Option<String>, model_profile_id: Uuid) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Ok(crate::models::Conversation::new(model_profile_id))
        }

        async fn load(&self, _id: Uuid) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            // Return a valid conversation so the test can proceed
            Ok(crate::models::Conversation::new(self.profile_id))
        }

        async fn list(&self, _limit: Option<usize>, _offset: Option<usize>) -> Result<Vec<crate::models::Conversation>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn add_user_message(&self, _conversation_id: Uuid, content: String) -> Result<Message, crate::services::ServiceError> {
            Ok(Message::user(content))
        }

        async fn add_assistant_message(&self, _conversation_id: Uuid, content: String) -> Result<Message, crate::services::ServiceError> {
            Ok(Message::assistant(content))
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

        async fn get_messages(&self, _conversation_id: Uuid) -> Result<Vec<Message>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn update(&self, _id: Uuid, _title: Option<String>, _model_profile_id: Option<Uuid>) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }
    }

    struct MockProfileService {
        profile: Arc<RwLock<Option<crate::models::ModelProfile>>>,
    }

    impl MockProfileService {
        fn new() -> Self {
            Self {
                profile: Arc::new(RwLock::new(None)),
            }
        }

        async fn set_default_profile(&self, profile: crate::models::ModelProfile) {
            *self.profile.write().await = Some(profile);
        }
    }

    #[async_trait::async_trait]
    impl crate::services::ProfileService for MockProfileService {
        async fn list(&self) -> Result<Vec<crate::models::ModelProfile>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn get(&self, _id: Uuid) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }

        async fn create(
            &self,
            _name: String,
            _provider: String,
            _model: String,
            _auth: AuthConfig,
            _parameters: ModelParameters,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            // Return a dummy profile for testing
            Ok(crate::models::ModelProfile::new(
                _name,
                _provider,
                _model,
                "https://api.test.com/v1".to_string(),
                _auth,
            ))
        }

        async fn update(
            &self,
            _id: Uuid,
            _name: Option<String>,
            _model: Option<String>,
            _auth: Option<AuthConfig>,
            _parameters: Option<ModelParameters>,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }

        async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }

        async fn test_connection(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn get_default(&self) -> Result<Option<crate::models::ModelProfile>, crate::services::ServiceError> {
            Ok(self.profile.read().await.clone())
        }

        async fn set_default(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_send_message() {
        // Set default profile
        let profile = crate::models::ModelProfile::new(
            "Test Profile".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Key { value: "test-key".to_string() },
        );
        let profile_id = profile.id;

        let conversation_service = Arc::new(MockConversationService::new(profile_id)) as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());

        // Set the default profile directly on the mock
        mock_profile_service.set_default_profile(profile).await;
        
        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

        let chat_service = ChatServiceImpl::new(conversation_service, profile_service);

        let conversation_id = Uuid::new_v4();
        let result = chat_service.send_message(conversation_id, "Hello, world!".to_string()).await;

        // The send_message call should succeed in creating the stream
        // The actual LLM call happens asynchronously and will fail with invalid API key
        // but the important thing is we got a stream back (not a placeholder)
        assert!(result.is_ok(), "send_message should return Ok with a stream, got: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_cancel() {
        let profile = crate::models::ModelProfile::new(
            "Test Profile".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Key { value: "test-key".to_string() },
        );
        let profile_id = profile.id;

        let conversation_service = Arc::new(MockConversationService::new(profile_id)) as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());

        // Set the default profile directly on the mock
        mock_profile_service.set_default_profile(profile).await;
        
        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

        let chat_service = ChatServiceImpl::new(conversation_service, profile_service);

        // Cancel should work even without streaming
        chat_service.cancel();
        assert!(!chat_service.is_streaming());
    }

    #[tokio::test]
    async fn test_is_streaming() {
        let profile = crate::models::ModelProfile::new(
            "Test Profile".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Key { value: "test-key".to_string() },
        );
        let profile_id = profile.id;

        let conversation_service = Arc::new(MockConversationService::new(profile_id)) as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());

        // Set the default profile directly on the mock
        mock_profile_service.set_default_profile(profile).await;
        
        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

        let chat_service = ChatServiceImpl::new(conversation_service, profile_service);

        // Initially not streaming
        assert!(!chat_service.is_streaming());
    }
}
