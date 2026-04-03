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
mod tests {
    use super::*;
    use crate::models::{AuthConfig, Message, ModelParameters};
    use std::sync::Arc;

    struct MockConversationService {
        profile_id: Uuid,
    }
    use crate::agent::McpApprovalMode;
    use crate::services::{AppSettingsService, ServiceError, ServiceResult};
    use std::collections::HashMap;
    use tokio::sync::Barrier;

    struct InMemoryAppSettingsService {
        settings: RwLock<HashMap<String, String>>,
    }

    impl InMemoryAppSettingsService {
        fn new() -> Self {
            Self {
                settings: RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl AppSettingsService for InMemoryAppSettingsService {
        async fn get_default_profile_id(&self) -> ServiceResult<Option<Uuid>> {
            Ok(None)
        }

        async fn set_default_profile_id(&self, _id: Uuid) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_current_conversation_id(&self) -> ServiceResult<Option<Uuid>> {
            Ok(None)
        }

        async fn set_current_conversation_id(&self, _id: Uuid) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_hotkey(&self) -> ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_hotkey(&self, _hotkey: String) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_theme(&self) -> ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_theme(&self, _theme: String) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_setting(&self, key: &str) -> ServiceResult<Option<String>> {
            Ok(self.settings.read().await.get(key).cloned())
        }

        async fn set_setting(&self, key: &str, value: String) -> ServiceResult<()> {
            self.settings.write().await.insert(key.to_string(), value);
            Ok(())
        }

        async fn reset_to_defaults(&self) -> ServiceResult<()> {
            self.settings.write().await.clear();
            Ok(())
        }
    }

    struct FailingAppSettingsService;

    #[async_trait::async_trait]
    impl AppSettingsService for FailingAppSettingsService {
        async fn get_default_profile_id(&self) -> ServiceResult<Option<Uuid>> {
            Ok(None)
        }

        async fn set_default_profile_id(&self, _id: Uuid) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_current_conversation_id(&self) -> ServiceResult<Option<Uuid>> {
            Ok(None)
        }

        async fn set_current_conversation_id(&self, _id: Uuid) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_hotkey(&self) -> ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_hotkey(&self, _hotkey: String) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_theme(&self) -> ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_theme(&self, _theme: String) -> ServiceResult<()> {
            Ok(())
        }

        async fn get_setting(&self, _key: &str) -> ServiceResult<Option<String>> {
            Ok(None)
        }

        async fn set_setting(&self, _key: &str, _value: String) -> ServiceResult<()> {
            Err(ServiceError::Storage(
                "simulated settings persistence failure".to_string(),
            ))
        }

        async fn reset_to_defaults(&self) -> ServiceResult<()> {
            Ok(())
        }
    }

    fn make_approval_test_chat_service(
        app_settings_service: Arc<dyn AppSettingsService>,
    ) -> (
        ChatServiceImpl,
        tokio::sync::mpsc::Receiver<ViewCommand>,
        Arc<ApprovalGate>,
    ) {
        let conversation_service = Arc::new(MockConversationService::new(Uuid::new_v4()))
            as Arc<dyn super::super::ConversationService>;
        let profile_service =
            Arc::new(MockProfileService::new()) as Arc<dyn crate::services::ProfileService>;
        let (view_tx, view_rx) = tokio::sync::mpsc::channel(8);
        let approval_gate = Arc::new(ApprovalGate::new());
        let policy = Arc::new(AsyncMutex::new(ToolApprovalPolicy {
            yolo_mode: false,
            auto_approve_reads: false,
            mcp_approval_mode: McpApprovalMode::PerTool,
            persistent_allowlist: Vec::new(),
            persistent_denylist: Vec::new(),
            session_allowlist: std::collections::HashSet::new(),
        }));

        let service = ChatServiceImpl::new(
            conversation_service,
            profile_service,
            app_settings_service,
            view_tx,
            approval_gate.clone(),
            policy,
        );

        (service, view_rx, approval_gate)
    }

    #[tokio::test]
    async fn resolve_tool_approval_denied_does_not_update_policy() {
        let app_settings =
            Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
        let (service, mut view_rx, approval_gate) =
            make_approval_test_chat_service(app_settings.clone());
        let request_id = Uuid::new_v4().to_string();
        let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        service
            .resolve_tool_approval(request_id.clone(), ToolApprovalResponseAction::Denied)
            .await
            .expect("denied resolution should succeed");

        let approved = waiter.wait().await.expect("waiter should receive decision");
        assert!(
            !approved,
            "denied decision should propagate false to waiter"
        );

        let resolved = view_rx
            .recv()
            .await
            .expect("view should receive ToolApprovalResolved");
        assert_eq!(
            resolved,
            ViewCommand::ToolApprovalResolved {
                request_id,
                approved: false,
            }
        );

        let policy_after = service.policy.lock().await.clone();
        assert_eq!(
            policy_after.evaluate("WriteFile"),
            crate::agent::tool_approval_policy::ToolApprovalDecision::AskUser,
            "Denied should not add session or persistent allow rules"
        );
    }

    #[tokio::test]
    async fn resolve_tool_approval_proceed_session_updates_session_policy() {
        let app_settings =
            Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
        let (service, mut view_rx, approval_gate) =
            make_approval_test_chat_service(app_settings.clone());
        let request_id = Uuid::new_v4().to_string();
        let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        service
            .resolve_tool_approval(
                request_id.clone(),
                ToolApprovalResponseAction::ProceedSession,
            )
            .await
            .expect("session resolution should succeed");

        let approved = waiter.wait().await.expect("waiter should receive decision");
        assert!(approved, "ProceedSession should propagate true to waiter");

        let resolved = view_rx
            .recv()
            .await
            .expect("view should receive ToolApprovalResolved");
        assert_eq!(
            resolved,
            ViewCommand::ToolApprovalResolved {
                request_id,
                approved: true,
            }
        );

        let policy_after = service.policy.lock().await.clone();
        assert_eq!(
            policy_after.evaluate("WriteFile"),
            crate::agent::tool_approval_policy::ToolApprovalDecision::Allow,
            "ProceedSession should add an in-memory session allow rule"
        );

        let persisted = app_settings
            .get_setting(crate::agent::tool_approval_policy::TOOL_APPROVAL_POLICY_SETTINGS_KEY)
            .await
            .expect("settings read should succeed");
        assert!(
            persisted.is_none(),
            "ProceedSession should not persist policy to settings"
        );
    }

    #[tokio::test]
    async fn resolve_tool_approval_proceed_always_persists_policy() {
        let app_settings =
            Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
        let (service, mut view_rx, approval_gate) =
            make_approval_test_chat_service(app_settings.clone());
        let request_id = Uuid::new_v4().to_string();
        let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        service
            .resolve_tool_approval(
                request_id.clone(),
                ToolApprovalResponseAction::ProceedAlways,
            )
            .await
            .expect("persistent resolution should succeed");

        let approved = waiter.wait().await.expect("waiter should receive decision");
        assert!(approved, "ProceedAlways should propagate true to waiter");

        let resolved = view_rx
            .recv()
            .await
            .expect("view should receive ToolApprovalResolved");
        assert_eq!(
            resolved,
            ViewCommand::ToolApprovalResolved {
                request_id,
                approved: true,
            }
        );

        let persisted = app_settings
            .get_setting(crate::agent::tool_approval_policy::TOOL_APPROVAL_POLICY_SETTINGS_KEY)
            .await
            .expect("settings read should succeed")
            .expect("ProceedAlways should persist policy payload");
        assert!(persisted.contains("WriteFile"));
    }

    #[tokio::test]
    async fn resolve_tool_approval_is_atomic_between_competing_decisions() {
        let app_settings =
            Arc::new(InMemoryAppSettingsService::new()) as Arc<dyn AppSettingsService>;
        let (service, _view_rx, approval_gate) =
            make_approval_test_chat_service(app_settings.clone());

        let request_id = Uuid::new_v4().to_string();
        let _waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        let barrier = Arc::new(Barrier::new(2));
        let service_denied = Arc::new(service);
        let service_always = service_denied.clone();

        let request_id_denied = request_id.clone();
        let barrier_denied = barrier.clone();
        let denied_handle = tokio::spawn(async move {
            barrier_denied.wait().await;
            service_denied
                .resolve_tool_approval(request_id_denied, ToolApprovalResponseAction::Denied)
                .await
        });

        let request_id_always = request_id.clone();
        let barrier_always = barrier.clone();
        let always_handle = tokio::spawn(async move {
            barrier_always.wait().await;
            service_always
                .resolve_tool_approval(request_id_always, ToolApprovalResponseAction::ProceedAlways)
                .await
        });

        let denied_result = denied_handle.await.expect("denied task should join");
        let always_result = always_handle.await.expect("always task should join");

        let winner_is_denied = denied_result.is_ok();
        let winner_is_always = always_result.is_ok();
        assert!(
            winner_is_denied ^ winner_is_always,
            "exactly one resolver should win the approval claim"
        );

        let persisted = app_settings
            .get_setting(crate::agent::tool_approval_policy::TOOL_APPROVAL_POLICY_SETTINGS_KEY)
            .await
            .expect("settings read should succeed");

        if winner_is_always {
            let payload = persisted.expect("ProceedAlways winner should persist policy");
            assert!(payload.contains("WriteFile"));
        } else {
            assert!(
                persisted.is_none(),
                "Denied winner must not persist allowlist entries"
            );
        }
    }

    #[tokio::test]
    async fn resolve_tool_approval_returns_error_when_persistence_fails() {
        let app_settings = Arc::new(FailingAppSettingsService) as Arc<dyn AppSettingsService>;
        let (service, _view_rx, approval_gate) = make_approval_test_chat_service(app_settings);
        let request_id = Uuid::new_v4().to_string();
        let waiter = approval_gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        let error = service
            .resolve_tool_approval(request_id, ToolApprovalResponseAction::ProceedAlways)
            .await
            .expect_err("ProceedAlways should fail when persistence fails");

        assert!(
            matches!(error, ServiceError::Storage(_)),
            "persistence failure should bubble up as storage error"
        );

        drop(waiter);
    }

    impl MockConversationService {
        fn new(profile_id: Uuid) -> Self {
            Self { profile_id }
        }
    }

    #[async_trait::async_trait]
    impl super::super::ConversationService for MockConversationService {
        async fn create(
            &self,
            _title: Option<String>,
            model_profile_id: Uuid,
        ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Ok(crate::models::Conversation::new(model_profile_id))
        }

        async fn load(
            &self,
            _id: Uuid,
        ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            // Return a valid conversation so the test can proceed
            Ok(crate::models::Conversation::new(self.profile_id))
        }

        async fn list(
            &self,
            _limit: Option<usize>,
            _offset: Option<usize>,
        ) -> Result<Vec<crate::models::Conversation>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn add_user_message(
            &self,
            _conversation_id: Uuid,
            content: String,
        ) -> Result<Message, crate::services::ServiceError> {
            Ok(Message::user(content))
        }

        async fn add_assistant_message(
            &self,
            _conversation_id: Uuid,
            content: String,
        ) -> Result<Message, crate::services::ServiceError> {
            Ok(Message::assistant(content))
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
        ) -> Result<Vec<Message>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn update(
            &self,
            _id: Uuid,
            _title: Option<String>,
            _model_profile_id: Option<Uuid>,
        ) -> Result<crate::models::Conversation, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }
    }

    struct MockProfileService {
        profile: Arc<RwLock<Option<crate::models::ModelProfile>>>,
        profiles_by_id: Arc<RwLock<std::collections::HashMap<Uuid, crate::models::ModelProfile>>>,
    }

    impl MockProfileService {
        fn new() -> Self {
            Self {
                profile: Arc::new(RwLock::new(None)),
                profiles_by_id: Arc::new(RwLock::new(std::collections::HashMap::new())),
            }
        }

        async fn set_default_profile(&self, profile: crate::models::ModelProfile) {
            *self.profile.write().await = Some(profile);
        }

        async fn add_profile(&self, profile: crate::models::ModelProfile) {
            self.profiles_by_id
                .write()
                .await
                .insert(profile.id, profile);
        }
    }

    #[async_trait::async_trait]
    impl crate::services::ProfileService for MockProfileService {
        async fn list(
            &self,
        ) -> Result<Vec<crate::models::ModelProfile>, crate::services::ServiceError> {
            Ok(vec![])
        }

        async fn get(
            &self,
            id: Uuid,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            self.profiles_by_id
                .read()
                .await
                .get(&id)
                .cloned()
                .ok_or_else(|| {
                    crate::services::ServiceError::NotFound(format!("profile {id} not found"))
                })
        }

        async fn create(
            &self,
            _name: String,
            _provider: String,
            _model: String,
            _base_url: Option<String>,
            _auth: AuthConfig,
            _parameters: ModelParameters,
            _system_prompt: Option<String>,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            // Return a dummy profile for testing
            Ok(crate::models::ModelProfile::new(
                _name,
                _provider,
                _model,
                _base_url.unwrap_or_else(|| "https://api.test.com/v1".to_string()),
                _auth,
            ))
        }

        async fn update(
            &self,
            _id: Uuid,
            _name: Option<String>,
            _provider: Option<String>,
            _model: Option<String>,
            _base_url: Option<String>,
            _auth: Option<AuthConfig>,
            _parameters: Option<ModelParameters>,
            _system_prompt: Option<String>,
        ) -> Result<crate::models::ModelProfile, crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }

        async fn delete(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Err(crate::services::ServiceError::NotFound("test".to_string()))
        }

        async fn test_connection(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }

        async fn get_default(
            &self,
        ) -> Result<Option<crate::models::ModelProfile>, crate::services::ServiceError> {
            Ok(self.profile.read().await.clone())
        }

        async fn set_default(&self, _id: Uuid) -> Result<(), crate::services::ServiceError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_send_message() {
        crate::services::secure_store::use_mock_backend();
        crate::services::secure_store::api_keys::store("_test_send_msg", "fake-key-for-test")
            .expect("store test key");

        // Set default profile
        let profile = crate::models::ModelProfile::new(
            "Test Profile".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Keychain {
                label: "_test_send_msg".to_string(),
            },
        );
        let profile_id = profile.id;

        let conversation_service = Arc::new(MockConversationService::new(profile_id))
            as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());

        // Set the default profile directly on the mock
        mock_profile_service.set_default_profile(profile).await;

        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

        let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

        let conversation_id = Uuid::new_v4();
        let result = chat_service
            .send_message(conversation_id, "Hello, world!".to_string())
            .await;

        // The send_message call should succeed in creating the stream
        // The actual LLM call happens asynchronously and will fail with invalid API key
        // but the important thing is we got a stream back (not a placeholder)
        assert!(
            result.is_ok(),
            "send_message should return Ok with a stream, got: {:?}",
            result.err()
        );

        // Clean up test key
        let _ = crate::services::secure_store::api_keys::delete("_test_send_msg");
    }

    #[tokio::test]
    async fn test_cancel() {
        let profile = crate::models::ModelProfile::new(
            "Test Profile".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Keychain {
                label: "test-key".to_string(),
            },
        );
        let profile_id = profile.id;

        let conversation_service = Arc::new(MockConversationService::new(profile_id))
            as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());

        // Set the default profile directly on the mock
        mock_profile_service.set_default_profile(profile).await;

        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

        let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

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
            AuthConfig::Keychain {
                label: "test-key".to_string(),
            },
        );
        let profile_id = profile.id;

        let conversation_service = Arc::new(MockConversationService::new(profile_id))
            as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());

        // Set the default profile directly on the mock
        mock_profile_service.set_default_profile(profile).await;

        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;

        let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

        // Initially not streaming
        assert!(!chat_service.is_streaming());
    }

    /// Proves that `prepare_message_context` resolves the profile via the
    /// conversation's `profile_id` rather than always using the global default.
    #[tokio::test]
    async fn prepare_message_context_uses_conversation_profile_id() {
        crate::services::secure_store::use_mock_backend();
        crate::services::secure_store::api_keys::store(
            "_test_conv_prof",
            "fake-key-for-conv-profile-test",
        )
        .expect("store test key");

        // Create a "kimi" profile that we want the conversation to use
        let kimi_profile = crate::models::ModelProfile::new(
            "Kimi Test".to_string(),
            "kimi-for-coding".to_string(),
            "kimi-k2-0711-preview".to_string(),
            String::new(),
            AuthConfig::Keychain {
                label: "_test_conv_prof".to_string(),
            },
        );
        let kimi_profile_id = kimi_profile.id;

        // Default profile is OpenAI — should NOT be used
        let default_profile = crate::models::ModelProfile::new(
            "Default".to_string(),
            "openai".to_string(),
            "gpt-4o".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Keychain {
                label: "_test_conv_prof".to_string(),
            },
        );

        // Conversation is bound to the kimi profile
        let conversation_service = Arc::new(MockConversationService::new(kimi_profile_id))
            as Arc<dyn super::super::ConversationService>;
        let mock_profile_service = Arc::new(MockProfileService::new());
        mock_profile_service
            .set_default_profile(default_profile)
            .await;
        mock_profile_service.add_profile(kimi_profile).await;

        let profile_service: Arc<dyn crate::services::ProfileService> = mock_profile_service;
        let chat_service = ChatServiceImpl::new_for_tests(conversation_service, profile_service);

        let prepared = chat_service
            .prepare_message_context(Uuid::new_v4(), "hello".to_string())
            .await
            .expect("prepare_message_context should succeed");

        assert_eq!(
            prepared.profile.id, kimi_profile_id,
            "prepared context should use the conversation's profile, not the default"
        );
        assert_eq!(prepared.profile.provider_id, "kimi-for-coding");

        let _ = crate::services::secure_store::api_keys::delete("_test_conv_prof");
    }
}
