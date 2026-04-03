//! Chat service implementation

/// @plan PLAN-20250127-REMEDIATE.P02, PLAN-20250127-REMEDIATE.P03
/// @requirement REM-001, REM-002, REM-003, REM-004, REM-005, REM-006, REM-007
use super::{ChatService, ChatStreamEvent, ServiceError, ServiceResult};
use crate::agent::tool_approval_policy::ToolApprovalPolicy;
use crate::events::types::{ChatEvent, ToolApprovalResponseAction};
use crate::events::{emit, AppEvent};
use crate::llm::client_agent::ApprovalGate;
use crate::llm::AgentClientExt;
use crate::llm::{LlmClient, Message as LlmMessage, StreamEvent as LlmStreamEvent};
use crate::mcp::McpService;
use crate::models::MessageRole;
use crate::presentation::view_command::ViewCommand;
use crate::services::template::{expand_system_prompt, TemplateContext};
use crate::services::ConversationService;
use futures::{stream, Stream};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use uuid::Uuid;

/// Minimal implementation of `ChatService`
pub struct ChatServiceImpl {
    conversation_service: Arc<dyn ConversationService>,
    profile_service: Arc<dyn super::ProfileService>,
    app_settings_service: Arc<dyn super::AppSettingsService>,
    is_streaming: Arc<AtomicBool>,
    current_conversation_id: Arc<RwLock<Option<Uuid>>>,
    /// Channel for sending view commands (used for tool approval UI)
    view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    /// Approval gate for coordinating user approval of tool execution
    approval_gate: Arc<ApprovalGate>,
    /// Policy for evaluating tool approval requirements
    policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
}

impl ChatServiceImpl {
    /// Create a new `ChatServiceImpl`
    #[must_use]
    pub fn new(
        conversation_service: Arc<dyn ConversationService>,
        profile_service: Arc<dyn super::ProfileService>,
        app_settings_service: Arc<dyn super::AppSettingsService>,
        view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
        approval_gate: Arc<ApprovalGate>,
        policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
    ) -> Self {
        Self {
            conversation_service,
            profile_service,
            app_settings_service,
            is_streaming: Arc::new(AtomicBool::new(false)),
            current_conversation_id: Arc::new(RwLock::new(None)),
            view_tx,
            approval_gate,
            policy,
        }
    }

    /// Build a fully wired service using settings-backed approval policy state.
    pub async fn new_with_settings(
        conversation_service: Arc<dyn ConversationService>,
        profile_service: Arc<dyn super::ProfileService>,
        app_settings_service: Arc<dyn super::AppSettingsService>,
        view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
        approval_gate: Arc<ApprovalGate>,
    ) -> Self {
        let policy = ToolApprovalPolicy::load_from_settings(app_settings_service.as_ref())
            .await
            .unwrap_or_default();

        Self::new(
            conversation_service,
            profile_service,
            app_settings_service,
            view_tx,
            approval_gate,
            Arc::new(AsyncMutex::new(policy)),
        )
    }

    /// Create a test-scoped `ChatServiceImpl` with default approval wiring.
    ///
    /// # Panics
    ///
    /// Panics if test app settings initialization fails.
    pub fn new_for_tests(
        conversation_service: Arc<dyn ConversationService>,
        profile_service: Arc<dyn super::ProfileService>,
    ) -> Self {
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(100);
        let approval_gate = Arc::new(ApprovalGate::new());
        let settings_path = std::env::temp_dir().join(format!(
            "chat-service-test-app-settings-{}.json",
            uuid::Uuid::new_v4()
        ));

        Self::new(
            conversation_service,
            profile_service,
            Arc::new(
                super::AppSettingsServiceImpl::new(settings_path)
                    .expect("failed to create test app settings service"),
            ) as Arc<dyn super::AppSettingsService>,
            view_tx,
            approval_gate,
            Arc::new(AsyncMutex::new(ToolApprovalPolicy::default())),
        )
    }

    async fn begin_stream(&self, conversation_id: Uuid) -> ServiceResult<()> {
        if self
            .is_streaming
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_err()
        {
            return Err(ServiceError::Internal(
                "Stream already in progress".to_string(),
            ));
        }

        *self.current_conversation_id.write().await = Some(conversation_id);
        Ok(())
    }

    async fn prepare_message_context(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<PreparedMessageContext> {
        let _conversation =
            if let Ok(conversation) = self.conversation_service.load(conversation_id).await {
                conversation
            } else {
                let default_profile = self.default_profile("No default profile available").await?;
                self.conversation_service
                    .create(None, default_profile.id)
                    .await?
            };

        self.conversation_service
            .add_user_message(conversation_id, content)
            .await?;

        let conversation = self
            .conversation_service
            .load(conversation_id)
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to load conversation: {e}")))?;

        // Use the conversation's stored profile_id as the authoritative profile
        // for this send. This ensures that when the user selects a chat profile
        // (e.g. Kimi), the conversation is updated and subsequent sends use
        // that profile — not a potentially stale global default.
        let profile = if let Ok(p) = self.profile_service.get(conversation.profile_id).await {
            p
        } else {
            tracing::warn!(
                conversation_profile_id = %conversation.profile_id,
                "Conversation profile not found; falling back to default"
            );
            self.default_profile("No active profile").await?
        };

        let client = LlmClient::from_profile(&profile)
            .map_err(|e| ServiceError::Internal(format!("Failed to create LLM client: {e}")))?;
        let messages = Self::build_llm_messages(&conversation, &profile);
        let raw_system_prompt =
            Self::system_prompt_for_conversation(&conversation, &profile).to_string();

        // Expand template variables in the system prompt
        let template_ctx =
            TemplateContext::new(conversation.created_at, &profile.name, &profile.model_id);
        let system_prompt = expand_system_prompt(&raw_system_prompt, &template_ctx);

        Ok(PreparedMessageContext {
            profile,
            client,
            messages,
            system_prompt,
        })
    }

    async fn default_profile(
        &self,
        missing_message: &str,
    ) -> ServiceResult<crate::models::ModelProfile> {
        self.profile_service
            .get_default()
            .await
            .map_err(|_| ServiceError::Internal(missing_message.to_string()))?
            .ok_or_else(|| ServiceError::Internal(missing_message.to_string()))
    }

    fn build_llm_messages(
        conversation: &crate::models::Conversation,
        profile: &crate::models::ModelProfile,
    ) -> Vec<LlmMessage> {
        // Create template context for expanding system messages
        let template_ctx =
            TemplateContext::new(conversation.created_at, &profile.name, &profile.model_id);

        let has_system_message = conversation
            .messages
            .iter()
            .any(|msg| msg.role == MessageRole::System && !msg.content.trim().is_empty());

        let mut messages: Vec<LlmMessage> = conversation
            .messages
            .iter()
            .map(|msg| match msg.role {
                MessageRole::System => {
                    // Expand template variables in conversation system messages
                    let expanded = expand_system_prompt(&msg.content, &template_ctx);
                    LlmMessage::system(expanded)
                }
                MessageRole::User => LlmMessage::user(msg.content.clone()),
                MessageRole::Assistant => LlmMessage::assistant(msg.content.clone()),
            })
            .collect();

        if !has_system_message && !profile.system_prompt.trim().is_empty() {
            // Expand template variables in the profile system prompt fallback
            let expanded = expand_system_prompt(&profile.system_prompt, &template_ctx);
            messages.insert(0, LlmMessage::system(expanded));
        }

        messages
    }

    fn system_prompt_for_conversation<'a>(
        conversation: &'a crate::models::Conversation,
        profile: &'a crate::models::ModelProfile,
    ) -> &'a str {
        conversation
            .messages
            .iter()
            .find(|message| {
                message.role == MessageRole::System && !message.content.trim().is_empty()
            })
            .map(|message| message.content.as_str())
            .filter(|prompt| !prompt.trim().is_empty())
            .unwrap_or(profile.system_prompt.as_str())
    }

    async fn load_mcp_tools(&self) -> Vec<crate::llm::tools::Tool> {
        let mcp_service = McpService::global();
        let mcp_guard = mcp_service.lock().await;
        mcp_guard.get_llm_tools()
    }

    fn emit_stream_started(conversation_id: Uuid, model_id: String) {
        let _ = emit(AppEvent::Chat(ChatEvent::StreamStarted {
            conversation_id,
            message_id: Uuid::new_v4(),
            model_id,
        }));
    }

    fn spawn_stream_task(
        &self,
        conversation_id: Uuid,
        prepared: PreparedMessageContext,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        tx: tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    ) {
        let is_streaming = self.is_streaming.clone();
        let conversation_service = self.conversation_service.clone();
        let view_tx = self.view_tx.clone();
        let approval_gate = self.approval_gate.clone();
        let policy = self.policy.clone();

        tokio::spawn(async move {
            run_stream_task(
                prepared,
                mcp_tools,
                tx,
                is_streaming,
                conversation_service,
                conversation_id,
                view_tx,
                approval_gate,
                policy,
            )
            .await;
        });
    }

    /// Resolve an in-flight tool approval request from UI input.
    ///
    /// # Errors
    ///
    /// Returns `ServiceError::NotFound` when `request_id` is unknown or already consumed,
    /// or persistence-related errors for `ProceedAlways` decisions.
    pub async fn resolve_tool_approval(
        &self,
        request_id: String,
        decision: ToolApprovalResponseAction,
    ) -> ServiceResult<()> {
        let approved = !matches!(decision, ToolApprovalResponseAction::Denied);
        let tool_identifier = self
            .approval_gate
            .resolve_and_take_identifier(&request_id, approved)
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Tool approval request {request_id} not found"))
            })?;

        match decision {
            ToolApprovalResponseAction::ProceedSession => {
                self.policy
                    .lock()
                    .await
                    .allow_for_session(tool_identifier.clone());
            }
            ToolApprovalResponseAction::ProceedAlways => {
                self.policy
                    .lock()
                    .await
                    .allow_persistently(tool_identifier, self.app_settings_service.as_ref())
                    .await?;
            }
            ToolApprovalResponseAction::ProceedOnce | ToolApprovalResponseAction::Denied => {}
        }

        let _ = self.view_tx.try_send(ViewCommand::ToolApprovalResolved {
            request_id,
            approved,
        });

        Ok(())
    }
}

struct PreparedMessageContext {
    profile: crate::models::ModelProfile,
    client: LlmClient,
    messages: Vec<LlmMessage>,
    system_prompt: String,
}

#[allow(clippy::too_many_arguments)]
async fn run_stream_task(
    prepared: PreparedMessageContext,
    mcp_tools: Vec<crate::llm::tools::Tool>,
    tx: tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    is_streaming: Arc<AtomicBool>,
    conversation_service: Arc<dyn ConversationService>,
    conversation_id: Uuid,
    view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    approval_gate: Arc<ApprovalGate>,
    policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
) {
    let PreparedMessageContext {
        profile: _,
        client,
        messages,
        system_prompt,
    } = prepared;
    let mut response_text = String::new();
    let mut thinking_text = String::new();

    let agent = match client.create_agent(mcp_tools, &system_prompt).await {
        Ok(agent) => agent,
        Err(e) => {
            let err_msg = format!("Failed to create agent: {e}");
            let _ = emit(AppEvent::Chat(ChatEvent::StreamError {
                conversation_id,
                error: err_msg.clone(),
                recoverable: false,
            }));
            let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err_msg)));
            is_streaming.store(false, Ordering::Release);
            return;
        }
    };

    let context = crate::llm::client_agent::McpToolContext {
        view_tx: view_tx.clone(),
        approval_gate: approval_gate.clone(),
        policy: policy.clone(),
    };

    if let Err(e) = client
        .run_agent_stream(&agent, &messages, context, |event| match event {
            LlmStreamEvent::TextDelta(text) => {
                tracing::info!("ChatService emitting TextDelta: '{}'", text);
                let _ = emit(AppEvent::Chat(ChatEvent::TextDelta { text: text.clone() }));
                let _ = tx.send(ChatStreamEvent::Token(text.clone()));
                response_text.push_str(&text);
            }
            LlmStreamEvent::ThinkingDelta(text) => {
                let _ = emit(AppEvent::Chat(ChatEvent::ThinkingDelta {
                    text: text.clone(),
                }));
                thinking_text.push_str(&text);
            }
            LlmStreamEvent::Complete => {
                let _ = tx.send(ChatStreamEvent::Complete);
            }
            LlmStreamEvent::Error(err) => {
                let _ = emit(AppEvent::Chat(ChatEvent::StreamError {
                    conversation_id,
                    error: err.clone(),
                    recoverable: false,
                }));
                let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err)));
            }
            _ => {}
        })
        .await
    {
        let err_msg = e.to_string();
        let _ = emit(AppEvent::Chat(ChatEvent::StreamError {
            conversation_id,
            error: err_msg.clone(),
            recoverable: false,
        }));
        let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(err_msg)));
    }

    if !response_text.is_empty() || !thinking_text.is_empty() {
        let _ = conversation_service
            .add_assistant_message(conversation_id, response_text.clone())
            .await;
    }

    let _ = emit(AppEvent::Chat(ChatEvent::StreamCompleted {
        conversation_id,
        message_id: Uuid::new_v4(),
        total_tokens: None,
    }));
    is_streaming.store(false, Ordering::Release);
}

#[allow(clippy::too_many_lines)]
#[async_trait::async_trait]
impl ChatService for ChatServiceImpl {
    /// Send a message and get a streaming response
    async fn send_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>> {
        self.begin_stream(conversation_id).await?;

        let prepared = self
            .prepare_message_context(conversation_id, content)
            .await?;
        Self::emit_stream_started(conversation_id, prepared.profile.model_id.clone());
        let mcp_tools = self.load_mcp_tools().await;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ChatStreamEvent>();
        self.spawn_stream_task(conversation_id, prepared, mcp_tools, tx);

        let message_stream: Pin<Box<dyn Stream<Item = ChatStreamEvent> + Send>> =
            Box::pin(stream::unfold(rx, move |mut rx| async move {
                rx.recv().await.map(|event| (event, rx))
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

    async fn resolve_tool_approval(
        &self,
        request_id: String,
        decision: ToolApprovalResponseAction,
    ) -> ServiceResult<()> {
        Self::resolve_tool_approval(self, request_id, decision).await
    }
}

#[cfg(test)]
mod tests;
