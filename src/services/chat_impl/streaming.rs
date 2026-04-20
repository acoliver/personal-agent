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
}

pub(super) async fn stream_agent_response(
    client: &LlmClient,
    agent: &serdes_ai_agent::Agent<crate::llm::client_agent::McpToolContext>,
    messages: &[LlmMessage],
    context: crate::llm::client_agent::McpToolContext,
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
) -> StreamTranscript {
    let mut transcript = StreamTranscript::default();

    if let Err(error) = client
        .run_agent_stream(agent, messages, context, |event| {
            handle_llm_stream_event(
                event,
                conversation_id,
                tx,
                &mut transcript.response_text,
                &mut transcript.thinking_text,
                &mut transcript.tool_calls,
                &mut transcript.tool_results,
                &mut transcript.input_tokens,
                &mut transcript.output_tokens,
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
        emit_stream_error(conversation_id, STREAM_ERROR_MESSAGE.to_string(), false, tx);
    }

    transcript
}

/// Finalize a stream task and clean up state for the conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @requirement REQ-173-001.3
#[allow(clippy::too_many_arguments)]
pub(super) async fn finalize_stream_task(
    conversation_service: &Arc<dyn ConversationService>,
    conversation_id: Uuid,
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
    clear_streaming_state(active_streams, conversation_id);
}

/// Run a stream task for a conversation.
/// @plan PLAN-20260416-ISSUE173.P03
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

    let Some(agent) = create_stream_agent(
        &client,
        mcp_tools,
        &system_prompt,
        conversation_id,
        &tx,
        &active_streams,
        &cancel,
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

    let transcript =
        stream_agent_response(&client, &agent, &messages, context, conversation_id, &tx).await;

    finalize_stream_task(
        &conversation_service,
        conversation_id,
        compression_result,
        transcript,
        &active_streams,
        &profile.name,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_llm_stream_event(
    event: LlmStreamEvent,
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    response_text: &mut String,
    thinking_text: &mut String,
    tool_calls: &mut Vec<crate::llm::tools::ToolUse>,
    tool_results: &mut Vec<crate::llm::tools::ToolResult>,
    input_tokens: &mut Option<u32>,
    output_tokens: &mut Option<u32>,
) {
    match event {
        LlmStreamEvent::TextDelta(text) => {
            tracing::info!("ChatService emitting TextDelta: '{}'", text);
            let _ = emit(AppEvent::Chat(ChatEvent::TextDelta {
                conversation_id,
                text: text.clone(),
            }));
            let _ = tx.send(ChatStreamEvent::Token(text.clone()));
            response_text.push_str(&text);
        }
        LlmStreamEvent::ThinkingDelta(text) => {
            let _ = emit(AppEvent::Chat(ChatEvent::ThinkingDelta {
                conversation_id,
                text: text.clone(),
            }));
            thinking_text.push_str(&text);
        }
        LlmStreamEvent::ToolCallStarted { tool_name, call_id } => {
            let _ = emit(AppEvent::Chat(ChatEvent::ToolCallStarted {
                conversation_id,
                tool_call_id: call_id,
                tool_name,
            }));
        }
        LlmStreamEvent::ToolCallCompleted {
            tool_name,
            call_id,
            success,
            result,
            error,
        } => {
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
            *input_tokens = completed_input_tokens;
            *output_tokens = completed_output_tokens;
            let _ = tx.send(ChatStreamEvent::Complete {
                input_tokens: completed_input_tokens,
                output_tokens: completed_output_tokens,
            });
        }
        LlmStreamEvent::Error(err) => {
            tracing::error!(
                conversation_id = %conversation_id,
                error = %err,
                response_chars = response_text.len(),
                thinking_chars = thinking_text.len(),
                "LLM stream event error"
            );
            emit_stream_error(conversation_id, STREAM_ERROR_MESSAGE.to_string(), false, tx);
        }
        LlmStreamEvent::ToolUse(_tool_use) => {}
    }
}

pub(super) fn emit_stream_error(
    conversation_id: Uuid,
    error: String,
    recoverable: bool,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
) {
    let _ = emit(AppEvent::Chat(ChatEvent::StreamError {
        conversation_id,
        error: error.clone(),
        recoverable,
    }));
    let _ = tx.send(ChatStreamEvent::Error(ServiceError::Internal(error)));
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
    if response_text.is_empty()
        && thinking_text.is_empty()
        && tool_calls.is_empty()
        && tool_results.is_empty()
    {
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

/// Clear streaming state for a specific conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @requirement REQ-173-001.3
pub(super) fn clear_streaming_state(
    active_streams: &Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    conversation_id: Uuid,
) {
    let mut map = active_streams.lock().expect("active_streams poisoned");
    map.remove(&conversation_id);
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
