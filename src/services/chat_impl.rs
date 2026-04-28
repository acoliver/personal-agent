use super::{AppSettingsService, ChatService, ChatStreamEvent, ServiceError, ServiceResult};
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
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

mod streaming;

use streaming::{clear_streaming_state, emit_stream_error, run_stream_task, STREAM_ERROR_MESSAGE};

// Re-export for tests
#[cfg(test)]
use streaming::persist_context_state;

const COMPRESSION_SETTINGS_KEY: &str = "compression";

/// Lifecycle state of an active stream.
/// @plan PLAN-20260416-ISSUE173.P14-CR3
pub(super) enum StreamLifecycle {
    /// Stream reservation held, but task not yet spawned.
    /// This prevents race conditions where concurrent sends could duplicate
    /// messages before the stream is tracked.
    Starting,
    /// Stream task is running and handling the LLM response.
    Running,
}

/// Active stream state for a single conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR3
/// @requirement REQ-173-001.1
pub(super) struct ActiveStream {
    /// Unique epoch identifier for this stream instance.
    /// Used to prevent stale task cleanup from evicting newer streams.
    pub(super) stream_id: Uuid,
    /// The spawned task handle. None while in Starting state.
    pub(super) task: Option<JoinHandle<()>>,
    /// Cancellation token for cooperative cancellation.
    pub(super) cancel: CancellationToken,
    /// Current lifecycle state of the stream.
    pub(super) state: StreamLifecycle,
}

pub struct ChatServiceImpl {
    conversation_service: Arc<dyn ConversationService>,
    profile_service: Arc<dyn super::ProfileService>,
    app_settings_service: Arc<dyn super::AppSettingsService>,
    skills_service: Arc<dyn SkillsService>,
    /// Per-conversation active streams. Replaces `is_streaming`/`current_conversation_id`/`stream_task`.
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @requirement REQ-173-001.1
    active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
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
            // @plan PLAN-20260416-ISSUE173.P03
            // @requirement REQ-173-001.1
            active_streams: Arc::new(StdMutex::new(HashMap::new())),
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

    /// Reserve a slot for a new stream for a specific conversation.
    ///
    /// Returns `(stream_id, cancel_token)` on success, or
    /// `ServiceError::Internal` when the conversation already has an entry.
    ///
    /// The check-and-insert is performed atomically under a single lock
    /// acquisition, so concurrent calls on the same `conversation_id` cannot
    /// both pass the guard (which previously allowed duplicate user messages
    /// to be written during `prepare_message_context` before either stream
    /// was tracked).
    ///
    /// The reservation is inserted in `StreamLifecycle::Starting` with
    /// `task = None`. `spawn_stream_task` later upgrades the entry to
    /// `StreamLifecycle::Running` with the task handle, matching by
    /// `stream_id` so a superseded reservation cannot overwrite a newer one.
    ///
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @plan PLAN-20260416-ISSUE173.P14-CR3
    /// @plan PLAN-20260416-ISSUE173.P14-CR4
    /// @requirement REQ-173-001.1
    /// @requirement REQ-173-001.2
    fn begin_stream(&self, conversation_id: Uuid) -> ServiceResult<(Uuid, CancellationToken)> {
        let stream_id = Uuid::new_v4();
        let cancel = CancellationToken::new();
        {
            let mut map = self.active_streams.lock().expect("active_streams poisoned");
            if map.contains_key(&conversation_id) {
                return Err(ServiceError::Internal(
                    "Stream already in progress for this conversation".to_string(),
                ));
            }
            map.insert(
                conversation_id,
                ActiveStream {
                    stream_id,
                    task: None,
                    cancel: cancel.clone(),
                    state: StreamLifecycle::Starting,
                },
            );
        }
        Ok((stream_id, cancel))
    }

    /// Clear a reservation made by `begin_stream` if it has not been upgraded
    /// to `Running` and the `stream_id` still matches. Used when prep-work
    /// fails between `begin_stream` and `spawn_stream_task`, so the slot
    /// does not stay held by a stream that never spawned.
    ///
    /// @plan PLAN-20260416-ISSUE173.P14-CR3
    /// @requirement REQ-173-001.2
    fn clear_reservation(&self, conversation_id: Uuid, stream_id: Uuid) {
        let mut map = self.active_streams.lock().expect("active_streams poisoned");
        if let Some(entry) = map.get(&conversation_id) {
            if entry.stream_id == stream_id && matches!(entry.state, StreamLifecycle::Starting) {
                map.remove(&conversation_id);
            }
        }
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

    /// Cancel the active stream for a specific conversation.
    ///
    /// Always resolves any pending approvals for the target conversation and emits
    /// a scoped `StreamCancelled` event. If a spawned task exists for the
    /// conversation it is aborted and its cancellation token is fired.
    /// Phase 7 replaces `resolve_all(false)` with a conversation-scoped variant;
    /// until then, pending approvals for other conversations are still drained
    /// here to preserve prior semantics — the narrower scope is the explicit
    /// deliverable of P07.
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @requirement REQ-173-002.1, REQ-173-002.2
    fn cancel_active_stream(&self, conversation_id: Uuid) {
        let removed = {
            let mut map = self.active_streams.lock().expect("active_streams poisoned");
            map.remove(&conversation_id)
        };

        if let Some(active) = removed {
            active.cancel.cancel();
            if let Some(handle) = active.task {
                handle.abort();
            }
        }

        // @plan PLAN-20260416-ISSUE173.P07
        // @requirement REQ-173-003.3
        let resolved = self
            .approval_gate
            .resolve_all_for_conversation(conversation_id, false);
        for (resolved_conversation_id, request_id) in resolved {
            let _ = self.view_tx.try_send(ViewCommand::ToolApprovalResolved {
                conversation_id: resolved_conversation_id,
                request_id,
                approved: false,
            });
        }

        let _ = emit(AppEvent::Chat(ChatEvent::StreamCancelled {
            conversation_id,
            message_id: Uuid::new_v4(),
            partial_content: String::new(),
        }));
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

        Self::append_emoji_avoidance(&self.app_settings_service, &mut system_prompt).await;

        // Get filter_emoji setting for tool output filtering
        let filter_emoji = match self.app_settings_service.get_filter_emoji().await {
            Ok(Some(enabled)) => enabled,
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Failed to read emoji filter setting; defaulting to disabled"
                );
                false
            }
        };

        Ok(PreparedMessageContext {
            profile,
            client,
            messages: compression_result.messages.clone(),
            system_prompt,
            skills_service: self.skills_service.clone(),
            compression_result,
            filter_emoji,
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

    async fn append_emoji_avoidance(
        app_settings_service: &Arc<dyn AppSettingsService>,
        system_prompt: &mut String,
    ) {
        let filter_emoji = match app_settings_service.get_filter_emoji().await {
            Ok(Some(enabled)) => enabled,
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "Failed to read emoji filter setting; defaulting to disabled"
                );
                false
            }
        };
        if filter_emoji {
            if !system_prompt.trim().is_empty() {
                system_prompt.push_str(
                    "

",
                );
            }
            system_prompt.push_str("Please avoid using emojis in your responses.");
        }
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

    /// Spawn a stream task for a conversation and upgrade its reservation
    /// from `Starting` to `Running` in `active_streams`.
    ///
    /// The reservation (created by `begin_stream`) already holds the cancel
    /// token that was handed to this method. The upgrade is guarded by
    /// `stream_id`: if the entry in the map belongs to a newer stream
    /// (superseded reservation), the spawned task is aborted and no change
    /// is made. If the entry has been removed (e.g. by `cancel_active_stream`
    /// between `begin_stream` and spawn), the spawned task is aborted.
    ///
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @plan PLAN-20260416-ISSUE173.P14-CR3
    /// @plan PLAN-20260416-ISSUE173.P14-CR4
    /// @requirement REQ-173-001.1
    #[allow(clippy::too_many_arguments)]
    async fn spawn_stream_task(
        &self,
        conversation_id: Uuid,
        stream_id: Uuid,
        cancel: CancellationToken,
        prepared: PreparedMessageContext,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        tx: tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    ) {
        let active_streams = self.active_streams.clone();
        let conversation_service = self.conversation_service.clone();
        let view_tx = self.view_tx.clone();
        let approval_gate = self.approval_gate.clone();
        let policy = self.policy.clone();

        let handle = tokio::spawn(async move {
            run_stream_task(
                prepared,
                mcp_tools,
                tx,
                active_streams,
                cancel,
                conversation_service,
                conversation_id,
                stream_id,
                view_tx,
                approval_gate,
                policy,
            )
            .await;
        });

        let mut map = self.active_streams.lock().expect("active_streams poisoned");
        match map.get_mut(&conversation_id) {
            Some(entry) if entry.stream_id == stream_id => {
                entry.task = Some(handle);
                entry.state = StreamLifecycle::Running;
            }
            // Entry was superseded by a newer reservation, or was removed
            // (e.g. by a concurrent cancel). Abort the spawned task so it
            // does not keep running untracked.
            _ => {
                handle.abort();
            }
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
        let (conversation_id, tool_identifiers) = self
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
                self.cancel_active_stream(conversation_id);
            }
        }

        let _ = self.view_tx.try_send(ViewCommand::ToolApprovalResolved {
            conversation_id,
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
    filter_emoji: bool,
}

#[allow(clippy::missing_const_for_fn)]
fn build_stream_context(
    conversation_id: Uuid,
    view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    approval_gate: Arc<ApprovalGate>,
    policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
    skills_service: Arc<dyn SkillsService>,
    filter_emoji: bool,
) -> crate::llm::client_agent::McpToolContext {
    crate::llm::client_agent::McpToolContext {
        conversation_id,
        view_tx,
        approval_gate,
        policy,
        skills_service,
        filter_emoji,
    }
}

/// Create a stream agent for a conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @requirement REQ-173-001.1
#[allow(clippy::too_many_arguments)]
async fn create_stream_agent(
    client: &LlmClient,
    mcp_tools: Vec<crate::llm::tools::Tool>,
    system_prompt: &str,
    conversation_id: Uuid,
    stream_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    active_streams: &Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    _cancel: &tokio_util::sync::CancellationToken,
) -> Option<serdes_ai_agent::Agent<crate::llm::client_agent::McpToolContext>> {
    match client.create_agent(mcp_tools, system_prompt).await {
        Ok(agent) => Some(agent),
        Err(e) => {
            tracing::error!(
                conversation_id = %conversation_id,
                error = %e,
                "Failed to create agent for chat stream"
            );
            emit_stream_error(
                conversation_id,
                STREAM_ERROR_MESSAGE.to_string(),
                false,
                None,
                tx,
            );
            clear_streaming_state(active_streams, conversation_id, stream_id);
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
        // Reserve the slot atomically BEFORE any await, so two concurrent
        // sends on the same conversation cannot both pass the guard and
        // both write a user message via prepare_message_context.
        //
        // @plan PLAN-20260416-ISSUE173.P14-CR3
        // @requirement REQ-173-001.2
        let (stream_id, cancel) = self.begin_stream(conversation_id)?;

        self.refresh_tool_approval_policy_from_settings().await;

        let prepared = match self.prepare_message_context(conversation_id, content).await {
            Ok(prepared) => prepared,
            Err(error) => {
                self.clear_reservation(conversation_id, stream_id);
                return Err(error);
            }
        };
        Self::emit_stream_started(conversation_id, prepared.profile.model_id.clone());

        let mcp_tools = self.load_mcp_tools().await;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ChatStreamEvent>();
        self.spawn_stream_task(conversation_id, stream_id, cancel, prepared, mcp_tools, tx)
            .await;

        let message_stream: Pin<Box<dyn Stream<Item = ChatStreamEvent> + Send>> =
            Box::pin(stream::unfold(rx, move |mut rx| async move {
                rx.recv().await.map(|event| (event, rx))
            }));

        Ok(Box::new(message_stream))
    }

    /// Cancel the streaming operation for a specific conversation.
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @requirement REQ-173-002.1
    fn cancel(&self, conversation_id: Uuid) {
        self.cancel_active_stream(conversation_id);
    }

    /// Check if any stream is currently active.
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @requirement REQ-173-001.1
    fn is_streaming(&self) -> bool {
        let map = self.active_streams.lock().expect("active_streams poisoned");
        !map.is_empty()
    }

    /// Check if a specific conversation has an active stream.
    /// Only considers streams in `Running` state as "streaming" (not `Starting` reservations).
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @plan PLAN-20260416-ISSUE173.P14-CR3
    /// @requirement REQ-173-001.1
    fn is_streaming_for(&self, conversation_id: Uuid) -> bool {
        let map = self.active_streams.lock().expect("active_streams poisoned");
        map.get(&conversation_id)
            .is_some_and(|a| matches!(a.state, StreamLifecycle::Running))
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
impl ChatServiceImpl {
    /// Test-only shim to begin a stream for a specific conversation.
    ///
    /// Exercises the real `begin_stream` guard path (same synchronous
    /// reservation used in production `send_message`), then promotes the
    /// reservation from `Starting` to `Running` by attaching a long-sleeping
    /// mock task. This matches what `spawn_stream_task` does for real
    /// streams, so tests see `is_streaming_for` return `true` without
    /// driving an actual LLM stream.
    ///
    /// Returns `Err` with the real error message if `begin_stream`
    /// rejects the reservation (e.g. duplicate conversation).
    ///
    /// @plan PLAN-20260416-ISSUE173.P14-CR5
    /// @requirement REQ-173-001.1
    /// @requirement REQ-173-001.2
    pub(crate) fn begin_stream_for_test(&self, conversation_id: Uuid) -> ServiceResult<()> {
        let (stream_id, cancel) = self.begin_stream(conversation_id)?;

        // Mock long-running task. When the stream is cancelled or cleared,
        // the `CancellationToken` is cancelled (and `task.abort()` is called
        // by `cancel_active_stream` / `clear_all_streams_for_test`), which
        // wakes this sleep and lets the task exit cleanly.
        let mut task = Some(tokio::spawn(async move {
            tokio::select! {
                () = cancel.cancelled() => {}
                () = tokio::time::sleep(tokio::time::Duration::from_hours(1)) => {}
            }
        }));

        let promoted = {
            let mut map = self.active_streams.lock().expect("active_streams poisoned");
            if let Some(entry) = map.get_mut(&conversation_id) {
                if entry.stream_id == stream_id {
                    entry.task = task.take();
                    entry.state = StreamLifecycle::Running;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if promoted {
            Ok(())
        } else {
            // Entry was replaced or removed between reserve and promotion —
            // drop the spawned task to avoid leaking it.
            if let Some(handle) = task {
                handle.abort();
            }
            Err(ServiceError::Internal(
                "Stream reservation lost before promotion".to_string(),
            ))
        }
    }

    /// Test-only helper to read back the `stream_id` of the active entry for
    /// a conversation, if any. Used by CR #4 regression tests that verify
    /// a stale task cannot evict a newer stream.
    ///
    /// @plan PLAN-20260416-ISSUE173.P14-CR4
    /// @requirement REQ-173-001.3
    pub(crate) fn stream_id_for_test(&self, conversation_id: Uuid) -> Option<Uuid> {
        let map = self.active_streams.lock().expect("active_streams poisoned");
        map.get(&conversation_id).map(|a| a.stream_id)
    }

    /// Test-only helper to clear all mock streams.
    /// @plan PLAN-20260416-ISSUE173.P03
    /// @requirement REQ-173-001.3
    pub(crate) fn clear_all_streams_for_test(&self) {
        let mut map = self.active_streams.lock().expect("active_streams poisoned");
        for (_, active) in map.drain() {
            active.cancel.cancel();
            if let Some(handle) = active.task {
                handle.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests;
