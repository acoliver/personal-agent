//! Delivery of queued steering messages at the turn boundary (issue #222).
//!
//! A user send runs one `AgentStream` turn. When that turn finishes cleanly
//! and the conversation has steering messages waiting, this module runs
//! another turn seeded with the conversation so far plus the steering text —
//! no new user-initiated send, and nothing cancelled. The chain ends when the
//! queue is empty, the turn did not finish cleanly, the turn cap is hit, or a
//! steering message cannot be recorded; finalization then runs once, on the
//! last turn.
//!
//! Every ending that leaves messages queued announces them as discarded. A
//! queued entry is on screen until something says what became of it, and
//! delivery is only one of the two answers.
//!
//! @plan PLAN-20260903-ISSUE222.P02
//! @plan PLAN-20260903-ISSUE222.P06
//! @requirement REQ-222-003
//! @requirement REQ-222-005
//! @requirement REQ-222-006
//! @requirement REQ-222-007

use super::{
    finalize_by_outcome, finalize_completed_turn, has_assistant_output, persist_assistant_response,
    StreamFinalizeContext, StreamTranscript,
};
use crate::compression::pipeline::CompressionResult;
use crate::events::types::ChatEvent;
use crate::events::{emit, AppEvent};
use crate::llm::Message as LlmMessage;
use crate::models::Message;
use crate::services::chat_impl::{drain_steering_queue, emit_steering_discarded, QueuedSteering};
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
/// output is normally persisted by `finalize_by_outcome`, so nothing is
/// written twice. The one chain that ends between those two writes reports
/// itself with [`ChainOutcome::OutputPersisted`] and finalizes without the
/// second, which is what keeps that guarantee true.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260903-ISSUE222.P06
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
    let (transcript, outcome) = run_steered_turns(ctx, messages, run_turn).await;
    match outcome {
        ChainOutcome::Unfinalized => {
            finalize_by_outcome(
                &ctx.finalize,
                compression_result,
                transcript,
                ctx.model_label,
            )
            .await;
        }
        ChainOutcome::OutputPersisted => {
            finalize_completed_turn(&ctx.finalize, compression_result, &transcript).await;
        }
    }
}

/// What the chain has left for finalization to do with the last turn.
///
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-007
enum ChainOutcome {
    /// Nothing about the last turn has been recorded, so finalization decides
    /// what to persist from the transcript itself. This is every ordinary
    /// ending: the queue ran dry, the turn did not finish cleanly, or the
    /// chain hit its turn cap.
    Unfinalized,
    /// The last turn's assistant output is already recorded, because the
    /// chain stopped after writing it and before the follow-up turn it was
    /// written for could start. The rest of finalization still applies;
    /// writing that output again would record it twice.
    OutputPersisted,
}

/// Run turns until the conversation stops steering, returning the last
/// turn's transcript and what finalization still owes it.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
/// @requirement REQ-222-006
async fn run_steered_turns<R, Fut>(
    ctx: &SteeringDeliveryContext<'_>,
    mut messages: Vec<LlmMessage>,
    mut run_turn: R,
) -> (StreamTranscript, ChainOutcome)
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
            return (transcript, ChainOutcome::Unfinalized);
        }

        let queued = drain_steering_queue(ctx.finalize.steering_queues, conversation_id);
        if queued.is_empty() {
            return (transcript, ChainOutcome::Unfinalized);
        }

        if turns >= MAX_STEERING_TURNS {
            tracing::warn!(
                conversation_id = %conversation_id,
                turns,
                dropped = queued.len(),
                "Steering chain reached its turn cap; dropping the messages still queued"
            );
            emit_steering_discarded(conversation_id, &queued);
            return (transcript, ChainOutcome::Unfinalized);
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

        if !deliver_steering(ctx, &queued, &mut messages).await {
            return (transcript, ChainOutcome::OutputPersisted);
        }
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
/// Returns `false` when a steering message could not be persisted, which
/// stops the chain. Recording the steer is what keeps the history a chained
/// turn is seeded with equal to the history a reload rebuilds; a turn run
/// over text the store rejected would break that for the rest of the
/// conversation. So the failed message is not seeded, is not announced as
/// delivered, and neither is anything behind it: they are announced as
/// discarded instead, because no later turn is going to pick them up.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-003
/// @requirement REQ-222-005
/// @requirement REQ-222-007
async fn deliver_steering(
    ctx: &SteeringDeliveryContext<'_>,
    queued: &[QueuedSteering],
    messages: &mut Vec<LlmMessage>,
) -> bool {
    let conversation_id = ctx.finalize.conversation_id;
    for (index, entry) in queued.iter().enumerate() {
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
            emit_steering_discarded(conversation_id, &queued[index..]);
            return false;
        }

        let _ = emit(AppEvent::Chat(ChatEvent::SteeringDelivered {
            conversation_id,
            steer_id: entry.id,
        }));

        messages.push(LlmMessage::user(entry.text.clone()));
    }
    true
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
