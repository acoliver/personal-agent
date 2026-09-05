//! Chaining of a send's follow-up turns while steering stays queued.
//!
//! A user send runs one `AgentStream` turn. The transport delivers queued
//! steering itself, mid-turn at each tool-call boundary, and PA's sink
//! (`steering_sink`) persists and resolves what was delivered. When a turn
//! still finishes cleanly with entries left queued — a turn that never
//! crossed a tool boundary, or steers that arrived after its last one — this
//! module chains another turn, so the transport gets a boundary to deliver
//! them at. The texts are never written into the history here: the transport
//! owns them, and taking them away from it would strand both copies. The
//! chain ends when the queue is empty, the turn did not finish cleanly, or
//! the turn cap is hit; finalization then runs once, on the last turn.
//!
//! Every ending that leaves messages queued announces them as discarded. A
//! queued entry is on screen until something says what became of it, and
//! delivery is only one of the two answers.
//!
//! @plan PLAN-20260903-ISSUE222.P02
//! @plan PLAN-20260903-ISSUE222.P06
//! @plan PLAN-20260905-STEERINT.P04
//! @requirement REQ-222-003
//! @requirement REQ-222-005
//! @requirement REQ-222-006
//! @requirement REQ-222-007
//! @requirement REQ-SI-002
//! @requirement REQ-SI-006

use super::{
    finalize_by_outcome, has_assistant_output, persist_assistant_response, StreamFinalizeContext,
    StreamTranscript,
};
use crate::compression::pipeline::CompressionResult;
use crate::llm::Message as LlmMessage;
use crate::services::chat_impl::{
    drain_steering_queue, emit_steering_discarded, has_queued_steering,
};
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
/// it is ordered before the steering text the transport delivers during the
/// turn that follows; the last turn's output is persisted by
/// `finalize_by_outcome`, so nothing is written twice. Queued steering texts
/// are never seeded into the history: the transport delivers them mid-turn
/// and the sink records them (REQ-SI-002).
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260903-ISSUE222.P06
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-222-005
/// @requirement REQ-222-007
/// @requirement REQ-SI-002
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
/// turn's transcript.
///
/// The leftover check between turns is a peek, not a drain: the queued texts
/// are the transport's to deliver at the next turn's first tool boundary,
/// and taking them here would strand a text the transport still holds
/// (REQ-SI-006). Hitting the turn cap is the one ending that does take the
/// entries — no turn of this send is going to deliver them — so it announces
/// them discarded.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-222-003
/// @requirement REQ-222-005
/// @requirement REQ-222-006
/// @requirement REQ-SI-006
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

        if !has_queued_steering(ctx.finalize.steering_queues, conversation_id) {
            return transcript;
        }

        if turns >= MAX_STEERING_TURNS {
            let discarded = drain_steering_queue(ctx.finalize.steering_queues, conversation_id);
            tracing::warn!(
                conversation_id = %conversation_id,
                turns,
                dropped = discarded.len(),
                "Steering chain reached its turn cap; dropping the messages still queued"
            );
            emit_steering_discarded(conversation_id, &discarded);
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
    }
}

/// Whether a finished turn reached a boundary a steering message may be
/// delivered at.
///
/// A turn that failed, that never completed, or whose send has been stopped
/// is over: it delivers nothing and chains nothing (REQ-222-006). This is
/// also the re-check that runs before every follow-up turn starts.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-006
fn reaches_delivery_boundary(transcript: &StreamTranscript, cancel: &CancellationToken) -> bool {
    transcript.completed && transcript.error.is_none() && !cancel.is_cancelled()
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
