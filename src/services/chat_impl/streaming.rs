//! Streaming helper functions for `ChatServiceImpl`.

use super::{
    ActiveStream, AgentClientExt, ApprovalGate, AsyncMutex, ChatEvent, ChatStreamEvent,
    CompressionResult, LlmMessage, PreparedMessageContext, ServiceError, StdMutex,
    ToolApprovalPolicy, ViewCommand,
};
use crate::events::{emit, AppEvent};
use crate::llm::error::debug_error_message;
use crate::llm::{LlmClient, StreamEvent as LlmStreamEvent};
use crate::models::{ContextState, Message};
use crate::services::ConversationService;
use crate::ui_gpui::error_log::{
    base_url_host, sanitize_text, ErrorLogDiagnosticContext, ErrorLogRunStatus,
    ErrorLogStreamLifecycle, ErrorLogToolContext,
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{build_stream_context, create_stream_agent};

pub(super) const STREAM_ERROR_MESSAGE: &str = "An error interrupted the chat stream.";

#[derive(Default)]
pub(super) struct StreamTranscript {
    pub(super) response_text: String,
    pub(super) thinking_text: String,
    pub(super) tool_calls: Vec<crate::llm::tools::ToolUse>,
    pub(super) tool_results: Vec<crate::llm::tools::ToolResult>,
    pub(super) input_tokens: Option<u32>,
    pub(super) output_tokens: Option<u32>,
    pub(super) completed: bool,
}

#[derive(Clone, Default)]
pub(super) struct StreamDiagnosticContext {
    pub(super) profile_id: Uuid,
    pub(super) profile_name: String,
    pub(super) provider_id: String,
    pub(super) model_id: String,
    pub(super) base_url_host: Option<String>,
}

impl StreamDiagnosticContext {
    pub(super) fn from_profile(profile: &crate::models::ModelProfile) -> Self {
        Self {
            profile_id: profile.id,
            profile_name: profile.name.clone(),
            provider_id: profile.provider_id.clone(),
            model_id: profile.model_id.clone(),
            base_url_host: base_url_host(&profile.base_url),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn stream_agent_response(
    client: &LlmClient,
    agent: &serdes_ai_agent::Agent<crate::llm::client_agent::McpToolContext>,
    messages: &[LlmMessage],
    context: crate::llm::client_agent::McpToolContext,
    diagnostics_context: &StreamDiagnosticContext,
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
) -> StreamTranscript {
    let mut transcript = StreamTranscript::default();

    if let Err(error) = client
        .run_agent_stream(agent, messages, context, |event| {
            handle_llm_stream_event(
                diagnostics_context,
                event,
                conversation_id,
                tx,
                &mut transcript.response_text,
                &mut transcript.thinking_text,
                &mut transcript.tool_calls,
                &mut transcript.tool_results,
                &mut transcript.input_tokens,
                &mut transcript.output_tokens,
                &mut transcript.completed,
            );
        })
        .await
    {
        let err_msg = debug_error_message(&error);
        tracing::error!(
            conversation_id = %conversation_id,
            error = %err_msg,
            response_chars = transcript.response_text.len(),
            thinking_chars = transcript.thinking_text.len(),
            "LLM stream task failed"
        );
        let diagnostics = build_stream_error_diagnostics(
            Some(&err_msg),
            diagnostics_context,
            &transcript,
            ErrorLogStreamLifecycle::Failed,
        );
        emit_stream_error(
            conversation_id,
            STREAM_ERROR_MESSAGE.to_string(),
            false,
            Some(Box::new(diagnostics)),
            tx,
        );
    }

    transcript
}

/// Finalize a stream task and clean up state for the conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @requirement REQ-173-001.3
#[allow(clippy::too_many_arguments)]
pub(super) async fn finalize_stream_task(
    conversation_service: &Arc<dyn ConversationService>,
    conversation_id: Uuid,
    stream_id: Uuid,
    compression_result: CompressionResult,
    transcript: StreamTranscript,
    active_streams: &Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    model_label: &str,
) {
    persist_assistant_response(
        conversation_service,
        conversation_id,
        &transcript.response_text,
        &transcript.thinking_text,
        &transcript.tool_calls,
        &transcript.tool_results,
        model_label,
    )
    .await;

    persist_context_state(
        conversation_service,
        conversation_id,
        compression_result,
        transcript.input_tokens,
        transcript.output_tokens,
    )
    .await;

    let _ = emit(AppEvent::Chat(ChatEvent::StreamCompleted {
        conversation_id,
        message_id: Uuid::new_v4(),
        total_tokens: transcript
            .input_tokens
            .and_then(|input| transcript.output_tokens.map(|output| input + output)),
    }));
    clear_streaming_state(active_streams, conversation_id, stream_id);
}

/// Run a stream task for a conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @requirement REQ-173-001.1, REQ-173-001.3
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_stream_task(
    prepared: PreparedMessageContext,
    mcp_tools: Vec<crate::llm::tools::Tool>,
    tx: tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    cancel: CancellationToken,
    conversation_service: Arc<dyn ConversationService>,
    conversation_id: Uuid,
    stream_id: Uuid,
    view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    approval_gate: Arc<ApprovalGate>,
    policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
) {
    let PreparedMessageContext {
        profile,
        client,
        messages,
        system_prompt,
        skills_service,
        compression_result,
        filter_emoji,
    } = prepared;

    let diagnostics_context = StreamDiagnosticContext::from_profile(&profile);

    let Some(agent) = create_stream_agent(
        &client,
        mcp_tools,
        &system_prompt,
        conversation_id,
        stream_id,
        &tx,
        &active_streams,
        &cancel,
        &diagnostics_context,
    )
    .await
    else {
        return;
    };

    let context = build_stream_context(
        conversation_id,
        view_tx.clone(),
        approval_gate.clone(),
        policy.clone(),
        skills_service,
        filter_emoji,
    );

    let transcript = stream_agent_response(
        &client,
        &agent,
        &messages,
        context,
        &diagnostics_context,
        conversation_id,
        &tx,
    )
    .await;

    if !transcript.completed {
        clear_streaming_state(&active_streams, conversation_id, stream_id);
        return;
    }

    finalize_stream_task(
        &conversation_service,
        conversation_id,
        stream_id,
        compression_result,
        transcript,
        &active_streams,
        &profile.name,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_llm_stream_event(
    diagnostics_context: &StreamDiagnosticContext,
    event: LlmStreamEvent,
    conversation_id: Uuid,

    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    response_text: &mut String,
    thinking_text: &mut String,
    tool_calls: &mut Vec<crate::llm::tools::ToolUse>,
    tool_results: &mut Vec<crate::llm::tools::ToolResult>,
    input_tokens: &mut Option<u32>,
    output_tokens: &mut Option<u32>,
    completed: &mut bool,
) {
    match event {
        LlmStreamEvent::TextDelta(text) => {
            handle_text_delta(conversation_id, tx, response_text, &text);
        }
        LlmStreamEvent::ThinkingDelta(text) => {
            handle_thinking_delta(conversation_id, thinking_text, &text);
        }
        LlmStreamEvent::ToolCallStarted { tool_name, call_id } => {
            handle_tool_call_started(conversation_id, tool_name, call_id);
        }
        LlmStreamEvent::ToolCallCompleted {
            tool_name,
            call_id,
            success,
            result,
            error,
        } => {
            handle_tool_call_completed(conversation_id, tool_name, call_id, success, result, error);
        }
        LlmStreamEvent::ToolTranscript {
            tool_calls: completed_tool_calls,
            tool_results: completed_tool_results,
        } => {
            *tool_calls = completed_tool_calls;
            *tool_results = completed_tool_results;
        }
        LlmStreamEvent::Complete {
            input_tokens: completed_input_tokens,
            output_tokens: completed_output_tokens,
        } => {
            handle_stream_complete(
                tx,
                input_tokens,
                output_tokens,
                completed,
                completed_input_tokens,
                completed_output_tokens,
            );
        }
        LlmStreamEvent::Error(err) => {
            let snapshot = ErrorSnapshot {
                response_text,
                thinking_text,
                tool_calls,
                tool_results,
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
            };
            handle_stream_error_event(conversation_id, tx, diagnostics_context, &snapshot, &err);
        }

        LlmStreamEvent::ToolUse(_tool_use) => {}
    }
}

fn handle_text_delta(
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    response_text: &mut String,
    text: &str,
) {
    tracing::info!("ChatService emitting TextDelta: '{}'", text);
    let _ = emit(AppEvent::Chat(ChatEvent::TextDelta {
        conversation_id,
        text: text.to_string(),
    }));
    let _ = tx.send(ChatStreamEvent::Token(text.to_string()));
    response_text.push_str(text);
}

fn handle_thinking_delta(conversation_id: Uuid, thinking_text: &mut String, text: &str) {
    let _ = emit(AppEvent::Chat(ChatEvent::ThinkingDelta {
        conversation_id,
        text: text.to_string(),
    }));
    thinking_text.push_str(text);
}

fn handle_tool_call_started(conversation_id: Uuid, tool_name: String, call_id: String) {
    let _ = emit(AppEvent::Chat(ChatEvent::ToolCallStarted {
        conversation_id,
        tool_call_id: call_id,
        tool_name,
    }));
}

fn handle_tool_call_completed(
    conversation_id: Uuid,
    tool_name: String,
    call_id: String,
    success: bool,
    result: Option<String>,
    error: Option<String>,
) {
    let payload = result.or(error).unwrap_or_default();
    let _ = emit(AppEvent::Chat(ChatEvent::ToolCallCompleted {
        conversation_id,
        tool_call_id: call_id,
        tool_name,
        success,
        result: payload,
        duration_ms: 0,
    }));
}

fn handle_stream_complete(
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    input_tokens: &mut Option<u32>,
    output_tokens: &mut Option<u32>,
    completed: &mut bool,
    completed_input_tokens: Option<u32>,
    completed_output_tokens: Option<u32>,
) {
    *input_tokens = completed_input_tokens;
    *output_tokens = completed_output_tokens;
    *completed = true;
    let _ = tx.send(ChatStreamEvent::Complete {
        input_tokens: completed_input_tokens,
        output_tokens: completed_output_tokens,
    });
}

struct ErrorSnapshot<'a> {
    response_text: &'a str,
    thinking_text: &'a str,
    tool_calls: &'a [crate::llm::tools::ToolUse],
    tool_results: &'a [crate::llm::tools::ToolResult],
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

fn handle_stream_error_event(
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    diagnostics_context: &StreamDiagnosticContext,
    snapshot: &ErrorSnapshot<'_>,
    err: &str,
) {
    tracing::error!(
        conversation_id = %conversation_id,
        error = %err,
        response_chars = snapshot.response_text.len(),
        thinking_chars = snapshot.thinking_text.len(),
        "LLM stream event error"
    );

    let transcript = StreamTranscript {
        response_text: snapshot.response_text.to_string(),
        thinking_text: snapshot.thinking_text.to_string(),
        tool_calls: snapshot.tool_calls.to_vec(),
        tool_results: snapshot.tool_results.to_vec(),
        input_tokens: snapshot.input_tokens,
        output_tokens: snapshot.output_tokens,
        completed: false,
    };
    let mut diagnostics = build_stream_error_diagnostics(
        Some(err),
        diagnostics_context,
        &transcript,
        ErrorLogStreamLifecycle::Failed,
    );
    diagnostics.code_path =
        Some("services::chat_impl::streaming::handle_llm_stream_event".to_string());

    emit_stream_error(
        conversation_id,
        STREAM_ERROR_MESSAGE.to_string(),
        false,
        Some(Box::new(diagnostics)),
        tx,
    );
}

pub(super) fn emit_stream_error(
    conversation_id: Uuid,
    error: String,
    recoverable: bool,
    diagnostics: Option<Box<ErrorLogDiagnosticContext>>,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
) {
    let _ = emit(AppEvent::Chat(ChatEvent::StreamError {
        conversation_id,
        error: error.clone(),
        recoverable,
        diagnostics,
    }));
    let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(error)));
}

pub(super) fn build_stream_error_diagnostics(
    underlying_error: Option<&str>,
    context: &StreamDiagnosticContext,
    transcript: &StreamTranscript,
    lifecycle: ErrorLogStreamLifecycle,
) -> ErrorLogDiagnosticContext {
    let tool_calls = transcript
        .tool_calls
        .iter()
        .map(|tool| {
            let result = transcript
                .tool_results
                .iter()
                .find(|result| result.tool_use_id == tool.id);
            ErrorLogToolContext {
                tool_name: sanitize_text(&tool.name),
                tool_call_id: Some(sanitize_text(&tool.id)),
                success: result.map(|result| !result.is_error),
                summary: result.map(|result| summarize_tool_output(&result.content)),
            }
        })
        .collect();

    ErrorLogDiagnosticContext {
        underlying_error: underlying_error.map(sanitize_text),
        subsystem: Some("chat stream".to_string()),
        code_path: Some("services::chat_impl::streaming".to_string()),
        profile_id: Some(context.profile_id),
        profile_name: Some(sanitize_text(&context.profile_name)),
        provider_id: Some(sanitize_text(&context.provider_id)),
        model_id: Some(sanitize_text(&context.model_id)),
        base_url_host: context.base_url_host.clone(),
        run_status: Some(ErrorLogRunStatus::Failed),
        stream_lifecycle: Some(lifecycle),
        input_tokens: transcript.input_tokens,
        output_tokens: transcript.output_tokens,
        partial_assistant_response_len: Some(transcript.response_text.len()),
        thinking_len: Some(transcript.thinking_text.len()),
        tool_calls,
        recent_events: vec!["stream error emitted".to_string()],
        ..ErrorLogDiagnosticContext::default()
    }
}

fn summarize_tool_output(content: &str) -> String {
    const MAX_SUMMARY_CHARS: usize = 240;
    let sanitized = sanitize_text(content.trim());
    let char_count = sanitized.chars().count();
    if char_count <= MAX_SUMMARY_CHARS {
        sanitized
    } else {
        format!(
            "{}… ({char_count} chars total)",
            sanitized
                .chars()
                .take(MAX_SUMMARY_CHARS)
                .collect::<String>()
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn persist_assistant_response(
    conversation_service: &Arc<dyn ConversationService>,
    conversation_id: Uuid,
    response_text: &str,
    thinking_text: &str,
    tool_calls: &[crate::llm::tools::ToolUse],
    tool_results: &[crate::llm::tools::ToolResult],
    model_label: &str,
) {
    if response_text.is_empty() && thinking_text.is_empty() {
        if !tool_calls.is_empty() || !tool_results.is_empty() {
            tracing::warn!(
                conversation_id = %conversation_id,
                tool_calls = tool_calls.len(),
                tool_results = tool_results.len(),
                "Skipping assistant response with tool transcript but no assistant-visible output"
            );
        }
        return;
    }

    let mut msg = if thinking_text.is_empty() {
        Message::assistant(response_text.to_string())
    } else {
        Message::assistant_with_thinking(response_text.to_string(), thinking_text.to_string())
    };

    // Set the model_id to preserve which profile generated this response
    msg.model_id = Some(model_label.to_string());

    if !tool_calls.is_empty() {
        msg.tool_calls = Some(serde_json::to_string(tool_calls).unwrap_or_else(|error| {
            tracing::warn!("Failed to serialize tool calls: {error}");
            "[]".to_string()
        }));
    }

    if !tool_results.is_empty() {
        msg.tool_results = Some(serde_json::to_string(tool_results).unwrap_or_else(|error| {
            tracing::warn!("Failed to serialize tool results: {error}");
            "[]".to_string()
        }));
    }

    let _ = conversation_service.add_message(conversation_id, msg).await;
}

/// Clear streaming state for a specific conversation, but only if the
/// stored entry still corresponds to the caller's `stream_id`.
///
/// This guards against a stale spawned task (e.g. one whose `cancel()` has
/// already fired and which is now unwinding) removing the entry for a
/// brand-new stream that a later `begin_stream` call has reserved for the
/// same conversation id. Without this epoch check the old task would evict
/// the new reservation as soon as it finished its own cleanup.
///
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @requirement REQ-173-001.3
pub(super) fn clear_streaming_state(
    active_streams: &Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    conversation_id: Uuid,
    stream_id: Uuid,
) {
    let mut map = active_streams.lock().expect("active_streams poisoned");
    if let Some(entry) = map.get(&conversation_id) {
        if entry.stream_id == stream_id {
            map.remove(&conversation_id);
        }
    }
}

pub(super) async fn persist_context_state(
    conversation_service: &Arc<dyn ConversationService>,
    conversation_id: Uuid,
    compression_result: CompressionResult,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
) {
    let mut state = match conversation_service
        .get_context_state(conversation_id)
        .await
    {
        Ok(Some(existing_state)) => existing_state,
        Ok(None) => ContextState::default(),
        Err(error) => {
            tracing::warn!(
                conversation_id = %conversation_id,
                error = %error,
                "Failed to load existing compression context state; creating a new state"
            );
            ContextState::default()
        }
    };

    state.compression_phase = Some(compression_result.phase);
    state.masked_tool_seqs = compression_result.masked_tool_seqs;
    state.summary_range = compression_result.summary_range;
    state.compressed_at = Some(chrono::Utc::now());
    state.preserved_facts = compression_result.preserved_facts;
    state.last_input_tokens = input_tokens;
    state.last_output_tokens = output_tokens;

    tracing::debug!(
        conversation_id = %conversation_id,
        ?state.compression_phase,
        estimated_tokens = compression_result.estimated_tokens,
        "Persisting compression context state"
    );

    if let Err(error) = conversation_service
        .update_context_state(conversation_id, &state)
        .await
    {
        tracing::warn!(
            conversation_id = %conversation_id,
            error = %error,
            "Failed to persist compression context state"
        );
    }
}
