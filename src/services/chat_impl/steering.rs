//! The per-conversation steering queue (issue #222).
//!
//! A message typed while a turn is running waits here until that turn reaches
//! a boundary. Accepting a steer never disturbs the generation it steers:
//! nothing in this module touches a cancellation token or an approval.
//!
//! @plan PLAN-20260903-ISSUE222.P01
//! @requirement REQ-222-004
//! @requirement REQ-222-006

use super::{ChatService, ChatServiceImpl, ServiceError, ServiceResult};
use crate::events::types::ChatEvent;
use crate::events::{emit, AppEvent};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};
use uuid::Uuid;

/// Upper bound on steering messages a single conversation may hold.
///
/// An unbounded queue lets a user stack arbitrary instructions that all flush
/// at one boundary, which is the unpredictable-flush failure mode this feature
/// exists to avoid.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
pub(super) const MAX_QUEUED_STEERING_MESSAGES: usize = 5;

/// A steering message waiting for its conversation's next turn boundary.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
pub(super) struct QueuedSteering {
    pub(super) id: Uuid,
    pub(super) text: String,
}

/// Per-conversation FIFO steering queues, keyed the same way as
/// `ChatServiceImpl::active_streams`.
///
/// Lock discipline: `active_streams` and `steering_queues` are never held at
/// the same time. Every path that needs both takes one, releases it, then
/// takes the other, so there is no acquisition order to invert and no
/// deadlock to construct.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
pub(super) type SteeringQueues = Arc<StdMutex<HashMap<Uuid, VecDeque<QueuedSteering>>>>;

/// Take every steering message queued for a conversation, in FIFO order,
/// leaving its queue empty.
///
/// A free function because the spawned stream task holds only the shared
/// `SteeringQueues` handle, never the service, and it is the task that drains
/// the queue at a turn boundary.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
pub(super) fn drain_steering_queue(
    queues: &SteeringQueues,
    conversation_id: Uuid,
) -> Vec<QueuedSteering> {
    queues
        .lock()
        .expect("steering_queues poisoned")
        .remove(&conversation_id)
        .map(Vec::from)
        .unwrap_or_default()
}

/// Announce that queued steering messages are never going to be delivered.
///
/// A queued entry is rendered as waiting until something reports what became
/// of it, and delivery is only one of the two answers. Every path that ends a
/// turn while entries are still queued owes the other one, or the user is
/// left watching an instruction that will never be acted on.
///
/// Callers pass entries they have already taken out of the queue, so this
/// holds no lock and can be called with none held.
///
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-003
pub(super) fn emit_steering_discarded(conversation_id: Uuid, discarded: &[QueuedSteering]) {
    for entry in discarded {
        let _ = emit(AppEvent::Chat(ChatEvent::SteeringDiscarded {
            conversation_id,
            steer_id: entry.id,
        }));
    }
}

impl ChatServiceImpl {
    /// Queue a steering message for a conversation whose turn is running.
    ///
    /// Steering is purely additive: this never cancels the in-flight
    /// generation, never touches the conversation's cancellation token, and
    /// never resolves a pending approval. `cancel` stays reachable only from
    /// an explicit user stop (REQ-222-006).
    ///
    /// Lock discipline: the `active_streams` guard taken by
    /// `is_streaming_for` is released before `steering_queues` is locked, so
    /// the two locks are never held at once. See [`SteeringQueues`].
    ///
    /// # Errors
    ///
    /// Returns `ServiceError::Validation` when the text is blank, when the
    /// conversation has no turn in `StreamLifecycle::Running`, or when the
    /// conversation already holds `MAX_QUEUED_STEERING_MESSAGES` entries.
    ///
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @requirement REQ-222-004
    /// @requirement REQ-222-006
    pub(super) fn queue_steering(&self, conversation_id: Uuid, text: &str) -> ServiceResult<Uuid> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(ServiceError::Validation(
                "Steering message is empty".to_string(),
            ));
        }

        if !ChatService::is_streaming_for(self, conversation_id) {
            return Err(ServiceError::Validation(
                "No active turn to steer".to_string(),
            ));
        }

        let entry = QueuedSteering {
            id: Uuid::new_v4(),
            text: trimmed.to_string(),
        };
        let steer_id = entry.id;
        let queued_text = entry.text.clone();

        if !self.push_steering(conversation_id, entry) {
            return Err(ServiceError::Validation(format!(
                "Steering queue is full ({MAX_QUEUED_STEERING_MESSAGES} messages already waiting)"
            )));
        }

        let _ = emit(AppEvent::Chat(ChatEvent::SteeringQueued {
            conversation_id,
            steer_id,
            text: queued_text,
        }));

        Ok(steer_id)
    }

    /// Append `entry` to a conversation's steering queue.
    ///
    /// Returns `false` without queuing when the conversation already holds
    /// `MAX_QUEUED_STEERING_MESSAGES` entries.
    ///
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @requirement REQ-222-004
    fn push_steering(&self, conversation_id: Uuid, entry: QueuedSteering) -> bool {
        let mut queues = self
            .steering_queues
            .lock()
            .expect("steering_queues poisoned");
        let depth = queues.get(&conversation_id).map_or(0, VecDeque::len);
        if depth >= MAX_QUEUED_STEERING_MESSAGES {
            return false;
        }
        queues.entry(conversation_id).or_default().push_back(entry);
        true
    }

    /// Take every steering message queued for a conversation, in FIFO order,
    /// leaving its queue empty.
    ///
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @requirement REQ-222-005
    pub(super) fn drain_steering(&self, conversation_id: Uuid) -> Vec<QueuedSteering> {
        drain_steering_queue(&self.steering_queues, conversation_id)
    }
}
