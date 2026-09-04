//! Streaming helper functions for `ChatServiceImpl`.

use super::{
    drain_steering_queue, emit_steering_discarded, ActiveStream, AgentClientExt, ApprovalGate,
    AsyncMutex, ChatEvent, ChatStreamEvent, CompressionResult, LlmMessage, PreparedMessageContext,
    ServiceError, StdMutex, SteeringQueues, ToolApprovalPolicy, ViewCommand,
};
use crate::events::{emit, AppEvent};
use crate::llm::error::{debug_error_message, LlmError};
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

pub(super) mod steering_delivery;

use steering_delivery::{run_steered_turns_and_finalize, SteeringDeliveryContext};

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
    /// Set when the stream failed (returned `Err` or an `Error` event).
    /// A later `Complete` event clears it so normal finalization can run.
    pub(super) error: Option<String>,
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
                &mut transcript,
            );
        })
        .await
    {
        record_stream_run_failure(
            &mut transcript,
            &error,
            conversation_id,
            diagnostics_context,
            tx,
        );
    }

    transcript
}

/// Record a returned `Err` from `run_agent_stream` on the transcript and
/// report it, unless the failure already reported itself (issue #193).
///
/// `do_run_agent_stream` reports common mid-stream failures twice: an
/// `Error` event callback first, then a returned `Err` carrying the same
/// message. The event path has already logged the failure and emitted
/// diagnostics on the bus and stream channel, so the first report wins.
/// Failures that return `Err` without an `Error` event — e.g. `AgentStream`
/// construction failure — are still reported here.
pub(super) fn record_stream_run_failure(
    transcript: &mut StreamTranscript,
    error: &LlmError,
    conversation_id: Uuid,
    diagnostics_context: &StreamDiagnosticContext,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
) {
    if transcript.error.is_some() {
        return;
    }
    let err_msg = debug_error_message(error);
    transcript.error = Some(err_msg.clone());
    handle_stream_run_failure(
        conversation_id,
        diagnostics_context,
        transcript,
        &err_msg,
        tx,
    );
}

/// Report a failed agent run: log it, emit sanitized diagnostics on the event
/// bus, and surface the generic error to the stream channel. Shared by the
/// returned-`Err` path of `run_agent_stream` (issue #193 keeps its
/// diagnostics actionable instead of discarding partial output).
pub(super) fn handle_stream_run_failure(
    conversation_id: Uuid,
    diagnostics_context: &StreamDiagnosticContext,
    transcript: &StreamTranscript,
    err_msg: &str,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
) {
    // Raw provider errors can embed credentials; only the sanitized form is
    // safe to log. The diagnostics builder applies the same redaction.
    let sanitized_error = sanitize_text(err_msg);
    tracing::error!(
        conversation_id = %conversation_id,
        error = %sanitized_error,
        response_chars = transcript.response_text.len(),
        thinking_chars = transcript.thinking_text.len(),
        "LLM stream task failed"
    );
    let diagnostics = build_stream_error_diagnostics(
        Some(err_msg),
        diagnostics_context,
        transcript,
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

/// Finalize a stream task and clean up state for the conversation.
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @requirement REQ-173-001.3
pub(super) async fn finalize_stream_task(
    ctx: &StreamFinalizeContext<'_>,
    compression_result: CompressionResult,
    transcript: StreamTranscript,
    model_label: &str,
) {
    persist_assistant_response(
        ctx.conversation_service,
        ctx.conversation_id,
        &transcript,
        model_label,
        false,
    )
    .await;

    finalize_completed_turn(ctx, compression_result, &transcript).await;
}

/// Everything finalizing a cleanly finished turn does apart from writing its
/// assistant output: record context state, announce the completion, and
/// release the conversation's stream slot.
///
/// Split out for the steering chain, which persists each intermediate turn's
/// output itself so the steering message that follows is ordered after it. A
/// chain that stops right after one of those writes still has to finalize,
/// but writing that output again would record it twice.
///
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-007
pub(super) async fn finalize_completed_turn(
    ctx: &StreamFinalizeContext<'_>,
    compression_result: CompressionResult,
    transcript: &StreamTranscript,
) {
    persist_context_state(
        ctx.conversation_service,
        ctx.conversation_id,
        compression_result,
        transcript.input_tokens,
        transcript.output_tokens,
    )
    .await;

    let _ = emit(AppEvent::Chat(ChatEvent::StreamCompleted {
        conversation_id: ctx.conversation_id,
        message_id: Uuid::new_v4(),
        total_tokens: transcript
            .input_tokens
            .and_then(|input| transcript.output_tokens.map(|output| input + output)),
    }));
    ctx.clear_streaming_state();
}

/// Finalize a stream that failed partway: persist whatever partial text and/or
/// thinking was produced, marked as interrupted, without touching context
/// state or emitting a completion (issue #193).
pub(super) async fn finalize_interrupted_stream(
    ctx: &StreamFinalizeContext<'_>,
    transcript: &StreamTranscript,
    model_label: &str,
) {
    // Raw provider errors can embed credentials; only the sanitized form is
    // safe to log.
    let sanitized_error = sanitize_text(transcript.error.as_deref().unwrap_or("unknown"));
    tracing::warn!(
        conversation_id = %ctx.conversation_id,
        error = %sanitized_error,
        response_chars = transcript.response_text.len(),
        thinking_chars = transcript.thinking_text.len(),
        "Finalizing interrupted chat stream with partial output"
    );
    persist_assistant_response(
        ctx.conversation_service,
        ctx.conversation_id,
        transcript,
        model_label,
        true,
    )
    .await;
    ctx.clear_streaming_state();
}

/// Finalize a stream according to its recorded outcome (issue #193):
/// - completed: normal finalization — persist the message, update context
///   state, and emit `StreamCompleted`. A stream that errored and then
///   completed lands here because `Complete` clears the error.
/// - failed without completing: persist partial output marked interrupted,
///   leave context state untouched, emit no completion.
/// - neither (e.g. cancelled): persist nothing.
///
/// Every branch clears the conversation's active-stream entry.
pub(super) async fn finalize_by_outcome(
    ctx: &StreamFinalizeContext<'_>,
    compression_result: CompressionResult,
    transcript: StreamTranscript,
    model_label: &str,
) {
    if transcript.completed {
        finalize_stream_task(ctx, compression_result, transcript, model_label).await;
    } else if transcript.error.is_some() {
        finalize_interrupted_stream(ctx, &transcript, model_label).await;
    } else {
        // Neither completed nor errored (e.g. cancelled): nothing to persist.
        ctx.clear_streaming_state();
    }
}

/// The stream-scoped state every outcome of `finalize_by_outcome` operates
/// on: the conversation service, the (conversation, stream) coordinates, and
/// the shared per-conversation registries.
pub(super) struct StreamFinalizeContext<'a> {
    pub(super) conversation_service: &'a Arc<dyn ConversationService>,
    pub(super) conversation_id: Uuid,
    pub(super) stream_id: Uuid,
    pub(super) active_streams: &'a Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @requirement REQ-222-004
    pub(super) steering_queues: &'a SteeringQueues,
}

impl StreamFinalizeContext<'_> {
    /// Release this stream's slot and its conversation's steering queue.
    fn clear_streaming_state(&self) {
        clear_streaming_state(
            self.active_streams,
            self.steering_queues,
            self.conversation_id,
            self.stream_id,
        );
    }
}

/// Run a stream task for a conversation.
///
/// One user send runs one turn, plus any further turns queued steering
/// messages chain onto it (issue #222). The agent is built once and reused;
/// each turn gets a freshly built tool context. Finalization happens once,
/// on the last turn.
///
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-173-001.1, REQ-173-001.3
/// @requirement REQ-222-005
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_stream_task(
    prepared: PreparedMessageContext,
    mcp_tools: Vec<crate::llm::tools::Tool>,
    tx: tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    steering_queues: SteeringQueues,
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
        &steering_queues,
        &cancel,
        &diagnostics_context,
    )
    .await
    else {
        return;
    };

    let delivery_ctx = SteeringDeliveryContext {
        finalize: StreamFinalizeContext {
            conversation_service: &conversation_service,
            conversation_id,
            stream_id,
            active_streams: &active_streams,
            steering_queues: &steering_queues,
        },
        cancel: &cancel,
        model_label: &profile.name,
    };

    // Bound outside the turn runner so the future it returns has one type for
    // every turn, independent of the call that produced it.
    let client = &client;
    let agent = &agent;
    let diagnostics_context = &diagnostics_context;
    let tx = &tx;

    run_steered_turns_and_finalize(
        &delivery_ctx,
        compression_result,
        messages,
        move |turn_messages: Vec<LlmMessage>| {
            let context = build_stream_context(
                conversation_id,
                view_tx.clone(),
                approval_gate.clone(),
                policy.clone(),
                skills_service.clone(),
                filter_emoji,
            );
            async move {
                stream_agent_response(
                    client,
                    agent,
                    &turn_messages,
                    context,
                    diagnostics_context,
                    conversation_id,
                    tx,
                )
                .await
            }
        },
    )
    .await;
}

pub(super) fn handle_llm_stream_event(
    diagnostics_context: &StreamDiagnosticContext,
    event: LlmStreamEvent,
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    transcript: &mut StreamTranscript,
) {
    match event {
        LlmStreamEvent::TextDelta(text) => {
            handle_text_delta(conversation_id, tx, &mut transcript.response_text, &text);
        }
        LlmStreamEvent::ThinkingDelta(text) => {
            handle_thinking_delta(conversation_id, &mut transcript.thinking_text, &text);
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
            transcript.tool_calls = completed_tool_calls;
            transcript.tool_results = completed_tool_results;
        }
        LlmStreamEvent::Complete {
            input_tokens: completed_input_tokens,
            output_tokens: completed_output_tokens,
        } => {
            handle_stream_complete(
                tx,
                transcript,
                completed_input_tokens,
                completed_output_tokens,
            );
        }
        LlmStreamEvent::Error(err) => {
            transcript.error = Some(err.clone());
            handle_stream_error_event(conversation_id, tx, diagnostics_context, transcript, &err);
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
    transcript: &mut StreamTranscript,
    completed_input_tokens: Option<u32>,
    completed_output_tokens: Option<u32>,
) {
    transcript.input_tokens = completed_input_tokens;
    transcript.output_tokens = completed_output_tokens;
    transcript.completed = true;
    // A stream that errored and then completed uses normal finalization.
    transcript.error = None;
    let _ = tx.send(ChatStreamEvent::Complete {
        input_tokens: completed_input_tokens,
        output_tokens: completed_output_tokens,
    });
}

fn handle_stream_error_event(
    conversation_id: Uuid,
    tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    diagnostics_context: &StreamDiagnosticContext,
    transcript: &StreamTranscript,
    err: &str,
) {
    // Raw provider errors can embed credentials; only the sanitized form is
    // safe to log. The diagnostics builder applies the same redaction for
    // storage/export, so it still receives the raw value.
    let sanitized_error = sanitize_text(err);
    tracing::error!(
        conversation_id = %conversation_id,
        error = %sanitized_error,
        response_chars = transcript.response_text.len(),
        thinking_chars = transcript.thinking_text.len(),
        "LLM stream event error"
    );

    let mut diagnostics = build_stream_error_diagnostics(
        Some(err),
        diagnostics_context,
        transcript,
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

/// Whether a finished turn produced assistant-visible output.
///
/// A transcript with neither text nor thinking is not persisted (#187), so
/// the steering delivery loop leaves it out of the history it chains onward
/// and that history keeps matching what a reload rebuilds.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-007
pub(super) const fn has_assistant_output(transcript: &StreamTranscript) -> bool {
    !transcript.response_text.is_empty() || !transcript.thinking_text.is_empty()
}

/// Persist the assistant output for a finished turn.
///
/// `interrupted` marks output persisted after a stream failure (issue #193);
/// normal completion passes `false`. Turns with neither text nor thinking are
/// skipped, preserving the #187 behavior for tool-only transcripts.
pub(super) async fn persist_assistant_response(
    conversation_service: &Arc<dyn ConversationService>,
    conversation_id: Uuid,
    transcript: &StreamTranscript,
    model_label: &str,
    interrupted: bool,
) {
    if !has_assistant_output(transcript) {
        if !transcript.tool_calls.is_empty() || !transcript.tool_results.is_empty() {
            tracing::warn!(
                conversation_id = %conversation_id,
                tool_calls = transcript.tool_calls.len(),
                tool_results = transcript.tool_results.len(),
                interrupted,
                "Skipping assistant response with tool transcript but no assistant-visible output"
            );
        }
        return;
    }

    let mut msg = if transcript.thinking_text.is_empty() {
        Message::assistant(transcript.response_text.clone())
    } else {
        Message::assistant_with_thinking(
            transcript.response_text.clone(),
            transcript.thinking_text.clone(),
        )
    };

    // Set the model_id to preserve which profile generated this response
    msg.model_id = Some(model_label.to_string());
    msg.interrupted = interrupted;

    if !transcript.tool_calls.is_empty() {
        msg.tool_calls = Some(
            serde_json::to_string(&transcript.tool_calls).unwrap_or_else(|error| {
                tracing::warn!("Failed to serialize tool calls: {error}");
                "[]".to_string()
            }),
        );
    }

    if !transcript.tool_results.is_empty() {
        msg.tool_results = Some(
            serde_json::to_string(&transcript.tool_results).unwrap_or_else(|error| {
                tracing::warn!("Failed to serialize tool results: {error}");
                "[]".to_string()
            }),
        );
    }

    if let Err(error) = conversation_service.add_message(conversation_id, msg).await {
        // ServiceError's Display never carries user or provider content;
        // the message body itself is intentionally not logged.
        tracing::warn!(
            conversation_id = %conversation_id,
            error = %error,
            interrupted,
            "Failed to persist assistant response"
        );
    }
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
/// When the entry is removed the conversation's steering queue goes with it:
/// that turn is definitively over, so anything still queued for it would only
/// leak into the next turn.
///
/// A steer accepted between the delivery loop's last drain and this removal
/// passes `is_streaming_for` — the entry still reads `Running` — and is
/// announced to the view as queued. This is where that entry's turn ends, so
/// this is where it is announced as discarded. Taking the queue rather than
/// dropping it is what makes that possible.
///
/// Lock discipline: the `active_streams` guard is released before
/// `steering_queues` is locked, so the two are never held at once, and both
/// are released before anything is emitted. See [`SteeringQueues`].
///
/// @plan PLAN-20260416-ISSUE173.P03
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @plan PLAN-20260903-ISSUE222.P01
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-173-001.3
/// @requirement REQ-222-003
/// @requirement REQ-222-004
pub(super) fn clear_streaming_state(
    active_streams: &Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    steering_queues: &SteeringQueues,
    conversation_id: Uuid,
    stream_id: Uuid,
) {
    let cleared = {
        let mut map = active_streams.lock().expect("active_streams poisoned");
        match map.get(&conversation_id) {
            Some(entry) if entry.stream_id == stream_id => {
                map.remove(&conversation_id);
                true
            }
            _ => false,
        }
    };

    if cleared {
        let discarded = drain_steering_queue(steering_queues, conversation_id);
        emit_steering_discarded(conversation_id, &discarded);
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
