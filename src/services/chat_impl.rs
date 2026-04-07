use super::{ChatService, ChatStreamEvent, ServiceError, ServiceResult};
use crate::agent::tool_approval_policy::ToolApprovalPolicy;
use crate::compression::pipeline::{CompressionPipeline, CompressionResult};
use crate::compression::thinking_stripper::strip_thinking_from_previous_turns;
use crate::config::CompressionConfig;
use crate::events::types::{ChatEvent, ToolApprovalResponseAction};
use crate::events::{emit, AppEvent};
use crate::llm::client_agent::ApprovalGate;
use crate::llm::AgentClientExt;
use crate::llm::{LlmClient, Message as LlmMessage};
use crate::mcp::McpService;
use crate::models::{Message, MessageRole};
use crate::presentation::view_command::ViewCommand;
use crate::services::template::{build_skills_prompt_block, expand_system_prompt, TemplateContext};
use crate::services::{ConversationService, SkillsService};
use futures::{stream, Stream};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

mod streaming;

use streaming::{clear_streaming_state, emit_stream_error, run_stream_task, STREAM_ERROR_MESSAGE};

// Re-export for tests
#[cfg(test)]
use streaming::persist_context_state;

const COMPRESSION_SETTINGS_KEY: &str = "compression";
pub struct ChatServiceImpl {
    conversation_service: Arc<dyn ConversationService>,
    profile_service: Arc<dyn super::ProfileService>,
    app_settings_service: Arc<dyn super::AppSettingsService>,
    skills_service: Arc<dyn SkillsService>,
    is_streaming: Arc<AtomicBool>,
    current_conversation_id: Arc<StdMutex<Option<Uuid>>>,
    stream_task: Arc<StdMutex<Option<JoinHandle<()>>>>,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversation_service: Arc<dyn ConversationService>,
        profile_service: Arc<dyn super::ProfileService>,
        app_settings_service: Arc<dyn super::AppSettingsService>,
        skills_service: Arc<dyn SkillsService>,
        view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
        approval_gate: Arc<ApprovalGate>,
        policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
    ) -> Self {
        Self {
            conversation_service,
            profile_service,
            app_settings_service,
            skills_service,
            is_streaming: Arc::new(AtomicBool::new(false)),
            current_conversation_id: Arc::new(StdMutex::new(None)),
            stream_task: Arc::new(StdMutex::new(None)),
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
        skills_service: Arc<dyn SkillsService>,
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
            skills_service,
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

        let app_settings = Arc::new(
            super::AppSettingsServiceImpl::new(settings_path)
                .expect("failed to create test app settings service"),
        ) as Arc<dyn super::AppSettingsService>;
        let skills_service = Arc::new(
            super::SkillsServiceImpl::new(app_settings.clone())
                .expect("failed to create test skills service"),
        ) as Arc<dyn SkillsService>;

        Self::new(
            conversation_service,
            profile_service,
            app_settings,
            skills_service,
            view_tx,
            approval_gate,
            Arc::new(AsyncMutex::new(ToolApprovalPolicy::default())),
        )
    }

    async fn refresh_tool_approval_policy_from_settings(&self) {
        match ToolApprovalPolicy::load_from_settings(self.app_settings_service.as_ref()).await {
            Ok(mut loaded_policy) => {
                let mut policy = self.policy.lock().await;
                let should_clear_session_allowlist = policy.yolo_mode && !loaded_policy.yolo_mode;

                if should_clear_session_allowlist {
                    loaded_policy.clear_session_allowlist();
                } else {
                    loaded_policy
                        .session_allowlist
                        .clone_from(&policy.session_allowlist);
                }

                *policy = loaded_policy;
            }
            Err(error) => {
                tracing::warn!("Failed to refresh tool approval policy before send: {error}");
            }
        }
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

        {
            let mut current = self
                .current_conversation_id
                .lock()
                .expect("current conversation mutex poisoned");
            *current = Some(conversation_id);
        }
        Ok(())
    }

    async fn load_compression_config(&self) -> CompressionConfig {
        match self
            .app_settings_service
            .get_setting(COMPRESSION_SETTINGS_KEY)
            .await
        {
            Ok(Some(raw_config)) => serde_json::from_str::<CompressionConfig>(&raw_config)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        error = %error,
                        "Failed to parse persisted compression config; using defaults"
                    );
                    CompressionConfig::default()
                }),
            Ok(None) => CompressionConfig::default(),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Failed to load persisted compression config; using defaults"
                );
                CompressionConfig::default()
            }
        }
    }

    fn cancel_active_stream(&self) {
        self.is_streaming.store(false, Ordering::Release);

        let conversation_id = *self
            .current_conversation_id
            .lock()
            .expect("current conversation mutex poisoned");

        let task_to_abort = self
            .stream_task
            .lock()
            .expect("stream task mutex poisoned")
            .take();
        if let Some(task) = task_to_abort {
            task.abort();
        }

        let resolved_requests = self.approval_gate.resolve_all(false);
        for request_id in resolved_requests {
            let _ = self.view_tx.try_send(ViewCommand::ToolApprovalResolved {
                request_id,
                approved: false,
            });
        }

        if let Some(conversation_id) = conversation_id {
            let _ = emit(AppEvent::Chat(ChatEvent::StreamCancelled {
                conversation_id,
                message_id: Uuid::new_v4(),
                partial_content: String::new(),
            }));
        }

        {
            let mut current = self
                .current_conversation_id
                .lock()
                .expect("current conversation mutex poisoned");
            *current = None;
        }
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
            .add_message(conversation_id, Message::user(content))
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
        let mut messages = Self::build_llm_messages(&conversation, &profile);
        strip_thinking_from_previous_turns(&mut messages);
        let compression_config = self.load_compression_config().await;
        let compression_result = CompressionPipeline::new().compress(
            messages,
            profile.context_window_size,
            &compression_config,
        );
        let raw_system_prompt =
            Self::system_prompt_for_conversation(&conversation, &profile).to_string();

        // Expand template variables in the system prompt
        let template_ctx =
            TemplateContext::new(conversation.created_at, &profile.name, &profile.model_id);
        let mut system_prompt = expand_system_prompt(&raw_system_prompt, &template_ctx);
        let enabled_skills = match self.skills_service.get_enabled_skills().await {
            Ok(skills) => skills,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Failed to fetch enabled skills; continuing without skills prompt block"
                );
                Vec::new()
            }
        };
        let skills_prompt_block = build_skills_prompt_block(&enabled_skills);
        if !skills_prompt_block.is_empty() {
            if !system_prompt.trim().is_empty() {
                system_prompt.push_str("\n\n");
            }
            system_prompt.push_str(&skills_prompt_block);
        }

        Ok(PreparedMessageContext {
            profile,
            client,
            messages: compression_result.messages.clone(),
            system_prompt,
            skills_service: self.skills_service.clone(),
            compression_result,
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
                MessageRole::User => {
                    let mut llm_message = LlmMessage::user(msg.content.clone());
                    if let Some(tool_results_raw) = msg.tool_results.as_deref() {
                        let parsed = serde_json::from_str::<Vec<crate::llm::tools::ToolResult>>(
                            tool_results_raw,
                        )
                        .unwrap_or_else(|error| {
                            tracing::warn!("Failed to parse persisted tool results: {error}");
                            Vec::new()
                        });
                        if !parsed.is_empty() {
                            llm_message = llm_message.with_tool_results(parsed);
                        }
                    }
                    llm_message
                }
                MessageRole::Assistant => {
                    let mut llm_message = LlmMessage::assistant(msg.content.clone());
                    if let Some(thinking) = msg.thinking_content.as_deref() {
                        if !thinking.is_empty() {
                            llm_message = llm_message.with_thinking(thinking.to_owned());
                        }
                    }
                    if let Some(tool_calls_raw) = msg.tool_calls.as_deref() {
                        let parsed =
                            serde_json::from_str::<Vec<crate::llm::tools::ToolUse>>(tool_calls_raw)
                                .unwrap_or_else(|error| {
                                    tracing::warn!("Failed to parse persisted tool calls: {error}");
                                    Vec::new()
                                });
                        if !parsed.is_empty() {
                            llm_message = llm_message.with_tool_uses(parsed);
                        }
                    }
                    if let Some(tool_results_raw) = msg.tool_results.as_deref() {
                        let parsed = serde_json::from_str::<Vec<crate::llm::tools::ToolResult>>(
                            tool_results_raw,
                        )
                        .unwrap_or_else(|error| {
                            tracing::warn!(
                                "Failed to parse persisted assistant tool results: {error}"
                            );
                            Vec::new()
                        });
                        if !parsed.is_empty() {
                            llm_message = llm_message.with_tool_results(parsed);
                        }
                    }
                    llm_message
                }
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

    async fn spawn_stream_task(
        &self,
        conversation_id: Uuid,
        prepared: PreparedMessageContext,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        tx: tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    ) {
        let is_streaming = self.is_streaming.clone();
        let current_conversation_id = self.current_conversation_id.clone();
        let conversation_service = self.conversation_service.clone();
        let view_tx = self.view_tx.clone();
        let approval_gate = self.approval_gate.clone();
        let policy = self.policy.clone();

        let handle = tokio::spawn(async move {
            run_stream_task(
                prepared,
                mcp_tools,
                tx,
                is_streaming,
                current_conversation_id,
                conversation_service,
                conversation_id,
                view_tx,
                approval_gate,
                policy,
            )
            .await;
        });

        let mut stream_task = self.stream_task.lock().expect("stream task mutex poisoned");
        if let Some(previous_task) = stream_task.replace(handle) {
            previous_task.abort();
        }
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
        let tool_identifiers = self
            .approval_gate
            .resolve_and_take_identifiers(&request_id, approved)
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Tool approval request {request_id} not found"))
            })?;

        let mut emit_policy_snapshot = false;
        match decision {
            ToolApprovalResponseAction::ProceedSession => {
                let mut policy = self.policy.lock().await;
                for tool_identifier in tool_identifiers {
                    policy.allow_for_session(tool_identifier);
                }
            }
            ToolApprovalResponseAction::ProceedAlways => {
                let mut updated_policy = self.policy.lock().await.clone();
                updated_policy
                    .allow_persistently_batch(tool_identifiers, self.app_settings_service.as_ref())
                    .await?;

                self.policy.lock().await.persistent_allowlist = updated_policy.persistent_allowlist;
                emit_policy_snapshot = true;
            }
            ToolApprovalResponseAction::ProceedOnce => {}
            ToolApprovalResponseAction::Denied => {
                self.cancel_active_stream();
            }
        }

        let _ = self.view_tx.try_send(ViewCommand::ToolApprovalResolved {
            request_id,
            approved,
        });

        if emit_policy_snapshot {
            let policy = self.policy.lock().await.clone();
            let _ = self
                .view_tx
                .try_send(ViewCommand::ToolApprovalPolicyUpdated {
                    yolo_mode: policy.yolo_mode,
                    auto_approve_reads: policy.auto_approve_reads,
                    skills_auto_approve: policy.skills_auto_approve,
                    mcp_approval_mode: policy.mcp_approval_mode,
                    persistent_allowlist: policy.persistent_allowlist,
                    persistent_denylist: policy.persistent_denylist,
                });
            let _ = self.view_tx.try_send(ViewCommand::YoloModeChanged {
                active: policy.yolo_mode,
            });
        }

        Ok(())
    }
}

struct PreparedMessageContext {
    profile: crate::models::ModelProfile,
    client: LlmClient,
    messages: Vec<LlmMessage>,
    system_prompt: String,
    skills_service: Arc<dyn SkillsService>,
    compression_result: CompressionResult,
}

#[allow(clippy::missing_const_for_fn)]
fn build_stream_context(
    view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    approval_gate: Arc<ApprovalGate>,
    policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
    skills_service: Arc<dyn SkillsService>,
) -> crate::llm::client_agent::McpToolContext {
    crate::llm::client_agent::McpToolContext {
        view_tx,
        approval_gate,
        policy,
        skills_service,
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_stream_agent(
    client: &LlmClient,
    mcp_tools: Vec<crate::llm::tools::Tool>,
    system_prompt: &str,
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    is_streaming: &Arc<AtomicBool>,
    current_conversation_id: &Arc<StdMutex<Option<Uuid>>>,
) -> Option<serdes_ai_agent::Agent<crate::llm::client_agent::McpToolContext>> {
    match client.create_agent(mcp_tools, system_prompt).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            tracing::error!(
                conversation_id = %conversation_id,
                error = %e,
                "Failed to create agent for chat stream"
            );
            emit_stream_error(conversation_id, STREAM_ERROR_MESSAGE.to_string(), false, tx);
            clear_streaming_state(is_streaming, current_conversation_id, conversation_id);
            None
        }
    }
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
        self.refresh_tool_approval_policy_from_settings().await;

        let prepared = self
            .prepare_message_context(conversation_id, content)
            .await?;
        Self::emit_stream_started(conversation_id, prepared.profile.model_id.clone());

        let mcp_tools = self.load_mcp_tools().await;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ChatStreamEvent>();
        self.spawn_stream_task(conversation_id, prepared, mcp_tools, tx)
            .await;

        let message_stream: Pin<Box<dyn Stream<Item = ChatStreamEvent> + Send>> =
            Box::pin(stream::unfold(rx, move |mut rx| async move {
                rx.recv().await.map(|event| (event, rx))
            }));

        Ok(Box::new(message_stream))
    }

    /// Cancel the current streaming operation
    fn cancel(&self) {
        // Force-cancel the active stream task so tool loops stop immediately.
        self.cancel_active_stream();
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
