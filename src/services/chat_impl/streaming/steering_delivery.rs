//! Delivery of queued steering messages at the turn boundary (issue #222).
//!
//! A user send runs one `AgentStream` turn. When that turn finishes cleanly
//! and the conversation has steering messages waiting, this module runs
//! another turn seeded with the conversation so far plus the steering text —
//! no new user-initiated send, and nothing cancelled. The chain ends when the
//! queue is empty, the turn did not finish cleanly, or the turn cap is hit;
//! finalization then runs once, on the last turn.
//!
//! @plan PLAN-20260903-ISSUE222.P02
//! @requirement REQ-222-005
//! @requirement REQ-222-006
//! @requirement REQ-222-007

use super::{
    finalize_by_outcome, has_assistant_output, persist_assistant_response, StreamFinalizeContext,
    StreamTranscript,
};
use crate::compression::pipeline::CompressionResult;
use crate::events::types::ChatEvent;
use crate::events::{emit, AppEvent};
use crate::llm::Message as LlmMessage;
use crate::models::Message;
use crate::services::chat_impl::{drain_steering_queue, QueuedSteering};
use std::future::Future;
use tokio_util::sync::CancellationToken;

/// Upper bound on the turns one user send may chain through steering.
///
/// The queue itself is capped, but a user can refill it during every
/// follow-up turn, so the cap alone never terminates the chain. This does.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
pub(in crate::services::chat_impl) const MAX_STEERING_TURNS: usize = 10;

/// Everything a chained turn needs beyond the turn runner itself.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
pub(in crate::services::chat_impl) struct SteeringDeliveryContext<'a> {
    /// Coordinates and registries the single finalization operates on.
    pub(in crate::services::chat_impl) finalize: StreamFinalizeContext<'a>,
    /// The send's cancellation token. A cancelled turn chains nothing.
    pub(in crate::services::chat_impl) cancel: &'a CancellationToken,
    /// Profile name recorded on every assistant message this send persists.
    pub(in crate::services::chat_impl) model_label: &'a str,
}

/// Run a user send's turns — the first, plus any that steering chains onto
/// it — and finalize exactly once, on the last.
///
/// `run_turn` is handed the history for a turn and yields that turn's
/// transcript. Intermediate turns persist their own assistant output here so
/// it is ordered before the steering message that follows it; the last turn's
/// output is persisted by `finalize_by_outcome`, so nothing is written twice.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
/// @requirement REQ-222-007
pub(in crate::services::chat_impl) async fn run_steered_turns_and_finalize<R, Fut>(
    ctx: &SteeringDeliveryContext<'_>,
    compression_result: CompressionResult,
    messages: Vec<LlmMessage>,
    run_turn: R,
) where
    R: FnMut(Vec<LlmMessage>) -> Fut,
    Fut: Future<Output = StreamTranscript>,
{
    let transcript = run_steered_turns(ctx, messages, run_turn).await;
    finalize_by_outcome(
        &ctx.finalize,
        compression_result,
        transcript,
        ctx.model_label,
    )
    .await;
}

/// Run turns until the conversation stops steering, returning the last
/// turn's transcript for finalization.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
/// @requirement REQ-222-006
async fn run_steered_turns<R, Fut>(
    ctx: &SteeringDeliveryContext<'_>,
    mut messages: Vec<LlmMessage>,
    mut run_turn: R,
) -> StreamTranscript
where
    R: FnMut(Vec<LlmMessage>) -> Fut,
    Fut: Future<Output = StreamTranscript>,
{
    let conversation_id = ctx.finalize.conversation_id;
    let mut turns = 0_usize;

    loop {
        // The runner needs an owned history because its future outlives this
        // borrow of `messages`, which the next iteration extends.
        let transcript = run_turn(messages.clone()).await;
        turns += 1;

        if !reaches_delivery_boundary(&transcript, ctx.cancel) {
            return transcript;
        }

        let queued = drain_steering_queue(ctx.finalize.steering_queues, conversation_id);
        if queued.is_empty() {
            return transcript;
        }

        if turns >= MAX_STEERING_TURNS {
            tracing::warn!(
                conversation_id = %conversation_id,
                turns,
                dropped = queued.len(),
                "Steering chain reached its turn cap; dropping the messages still queued"
            );
            return transcript;
        }

        persist_assistant_response(
            ctx.finalize.conversation_service,
            conversation_id,
            &transcript,
            ctx.model_label,
            false,
        )
        .await;
        if has_assistant_output(&transcript) {
            messages.push(assistant_message(&transcript));
        }

        deliver_steering(ctx, queued, &mut messages).await;
    }
}

/// Whether a finished turn reached a boundary a steering message may be
/// delivered at.
///
/// A turn that failed, that never completed, or whose send has been stopped
/// is over: it drains nothing and chains nothing (REQ-222-006). This is also
/// the re-check that runs before every follow-up turn starts.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-006
fn reaches_delivery_boundary(transcript: &StreamTranscript, cancel: &CancellationToken) -> bool {
    transcript.completed && transcript.error.is_none() && !cancel.is_cancelled()
}

/// Persist, announce, and seed each drained steering message in FIFO order.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
/// @requirement REQ-222-007
async fn deliver_steering(
    ctx: &SteeringDeliveryContext<'_>,
    queued: Vec<QueuedSteering>,
    messages: &mut Vec<LlmMessage>,
) {
    let conversation_id = ctx.finalize.conversation_id;
    for entry in queued {
        if let Err(error) = ctx
            .finalize
            .conversation_service
            .add_message(conversation_id, Message::user(entry.text.clone()))
            .await
        {
            // ServiceError's Display never carries user content; the steering
            // text itself is intentionally not logged.
            tracing::warn!(
                conversation_id = %conversation_id,
                steer_id = %entry.id,
                error = %error,
                "Failed to persist a delivered steering message"
            );
        }

        let _ = emit(AppEvent::Chat(ChatEvent::SteeringDelivered {
            conversation_id,
            steer_id: entry.id,
        }));

        messages.push(LlmMessage::user(entry.text));
    }
}

/// The finished turn's assistant output, shaped the way
/// `ChatServiceImpl::build_llm_messages` shapes a persisted assistant
/// message, so the chained turn sees the history a reload would rebuild.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
fn assistant_message(transcript: &StreamTranscript) -> LlmMessage {
    let mut message = LlmMessage::assistant(transcript.response_text.clone());
    if !transcript.thinking_text.is_empty() {
        message = message.with_thinking(transcript.thinking_text.clone());
    }
    if !transcript.tool_calls.is_empty() {
        message = message.with_tool_uses(transcript.tool_calls.clone());
    }
    if !transcript.tool_results.is_empty() {
        message = message.with_tool_results(transcript.tool_results.clone());
    }
    message
}
