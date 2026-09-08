//! The services-layer sink for steering texts a turn delivered mid-run.
//!
//! The fork hands a delivered text to [`SteeringDeliverySink::delivered`] the
//! moment the model actually received it. This type is that sink: it persists
//! the text as a user row, resolves the conversation's queue entry by FIFO
//! position, and announces the resolution. Storage and the UI stay out of the
//! LLM layer; the trait object points the dependency this way only.
//!
//! @plan PLAN-20260905-STEERINT.P04
//! @requirement REQ-SI-003
//! @requirement REQ-SI-005

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::events::types::ChatEvent;
use crate::events::{emit, AppEvent};
use crate::llm::error::LlmError;
use crate::llm::steering::SteeringDeliverySink;
use crate::models::Message;
use crate::services::ConversationService;

use super::super::{emit_steering_discarded, pop_steering_head, SteeringQueues};

/// Persists and correlates the steering texts one send's turns delivered.
///
/// Correlation is by FIFO position, never by matching text: the fork drains
/// its transport in the order acceptance pushed, so the queue head is always
/// the entry whose text was just delivered, and two identical texts still
/// resolve to their own distinct entries.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-003
/// @requirement REQ-SI-005
pub(in crate::services::chat_impl) struct MidTurnSteering {
    conversation_service: Arc<dyn ConversationService>,
    conversation_id: Uuid,
    steering_queues: SteeringQueues,
    /// Number of delivered texts persisted during this send.
    pub(in crate::services::chat_impl) delivered: usize,
}

impl MidTurnSteering {
    /// A sink for one send, over its conversation's registries.
    pub(in crate::services::chat_impl) fn new(
        conversation_service: Arc<dyn ConversationService>,
        conversation_id: Uuid,
        steering_queues: SteeringQueues,
    ) -> Self {
        Self {
            conversation_service,
            conversation_id,
            steering_queues,
            delivered: 0,
        }
    }
}

#[async_trait]
impl SteeringDeliverySink for MidTurnSteering {
    async fn delivered(&mut self, text: &str) -> Result<(), LlmError> {
        if let Err(error) = self
            .conversation_service
            .add_message(self.conversation_id, Message::user(text.to_string()))
            .await
        {
            // ServiceError's Display never carries user content; the steering
            // text itself is intentionally not logged.
            tracing::warn!(
                conversation_id = %self.conversation_id,
                error = %error,
                "Failed to persist a steering text the model already received"
            );
            // The entry this delivery was for is the head by FIFO position.
            // The turn is about to fail, so it is taken and announced here
            // rather than left for teardown to announce a second time.
            if let Some(entry) = pop_steering_head(&self.steering_queues, self.conversation_id) {
                emit_steering_discarded(self.conversation_id, &[entry]);
            }
            return Err(LlmError::Stream(format!(
                "failed to persist delivered steering: {error}"
            )));
        }

        self.delivered += 1;
        // An empty deque means teardown raced the delivery and already
        // announced the discard: the row is persisted and recorded, but no
        // terminal event is emitted and nothing is treated as an error.
        if let Some(entry) = pop_steering_head(&self.steering_queues, self.conversation_id) {
            let _ = emit(AppEvent::Chat(ChatEvent::SteeringDelivered {
                conversation_id: self.conversation_id,
                steer_id: entry.id,
            }));
        }
        Ok(())
    }
}
