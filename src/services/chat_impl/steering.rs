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
/// deadlock to construct. Nothing is emitted while either is held.
///
/// What that discipline costs is atomicity, and it is worth naming: a path
/// that reads one registry and then writes the other never sees a single
/// consistent state, so a turn can end in the gap. The path where that
/// matters is accepting a steer, and
/// [`ChatServiceImpl::confirm_or_withdraw_steering`] is what it does about
/// it — it reads the stream state again once the entry is queued, so
/// whichever of the two ran first, the second one observes it.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-004
pub(super) type SteeringQueues = Arc<StdMutex<HashMap<Uuid, VecDeque<QueuedSteering>>>>;

/// The refusal a steer gets when its conversation has no turn to steer.
///
/// Shared by the two places that reach that conclusion — the check before
/// the insert and the re-check after it — so a caller cannot tell which of
/// them refused and the two cannot drift apart.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-004
fn no_active_turn() -> ServiceError {
    ServiceError::Validation("No active turn to steer".to_string())
}

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
        emit_steering_discarded_id(conversation_id, entry.id);
    }
}

/// Announce that one steer, named by the id its `SteeringQueued` carried, is
/// never going to be delivered.
///
/// Takes an id rather than an entry because the entry may already be gone:
/// the caller that has to say so is not always the one that took it off the
/// queue. See [`ChatServiceImpl::confirm_or_withdraw_steering`].
///
/// @plan PLAN-20260903-ISSUE222.P08
/// @requirement REQ-222-003
pub(super) fn emit_steering_discarded_id(conversation_id: Uuid, steer_id: Uuid) {
    let _ = emit(AppEvent::Chat(ChatEvent::SteeringDiscarded {
        conversation_id,
        steer_id,
    }));
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
    /// the two locks are never held at once. See [`SteeringQueues`]. That
    /// also means the check and the insert are not one atomic step, which is
    /// why the entry is confirmed against the stream state again once it is
    /// queued.
    ///
    /// # Errors
    ///
    /// Returns `ServiceError::Validation` when the text is blank, when the
    /// conversation has no turn in `StreamLifecycle::Running` either before
    /// or after the entry is queued, or when the conversation already holds
    /// `MAX_QUEUED_STEERING_MESSAGES` entries.
    ///
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @plan PLAN-20260903-ISSUE222.P07
    /// @requirement REQ-222-003
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
            return Err(no_active_turn());
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

        // Announced before it is confirmed, so the view renders the entry
        // before it can be told to withdraw it. The other order would leave a
        // withdrawal landing on an entry that does not exist yet, and nothing
        // would come back for it afterwards.
        //
        // A teardown draining this queue can still announce its discard
        // ahead of this, because the insert above released the lock. What
        // follows this emit is therefore a terminal event for `steer_id`
        // whether or not that entry is still on the queue: at least one
        // discard always lands after this, never none.
        // @plan PLAN-20260903-ISSUE222.P08
        // @requirement REQ-222-003
        let _ = emit(AppEvent::Chat(ChatEvent::SteeringQueued {
            conversation_id,
            steer_id,
            text: queued_text,
        }));

        self.confirm_or_withdraw_steering(conversation_id, steer_id)
    }

    /// Confirm a steer that is now on the queue, or take it back.
    ///
    /// The precondition check and the insert are two separate lock
    /// acquisitions, so a turn can end between them. The entry then lands on
    /// a queue that turn's teardown has already drained, and nothing will
    /// ever come back for it: the view renders an instruction that never
    /// reaches a terminal state. Reading the stream state once more, with
    /// the entry already queued, is what makes that case observable —
    /// whichever of the two ran first, the second one sees it.
    ///
    /// The withdrawal is announced because the caller has already emitted
    /// `SteeringQueued` for this entry, and `SteeringDiscarded` is the only
    /// other way the view stops rendering it. It is announced whether or not
    /// this call is the one that took the entry off the queue: a teardown
    /// that drained it first may have announced its own discard *before* the
    /// `SteeringQueued` was emitted, in which case the view processed a
    /// withdrawal for an entry it had not rendered yet and then rendered it.
    /// Staying quiet here on the strength of the teardown's announcement
    /// would strand that entry on screen for the rest of the session.
    ///
    /// The guarantee is therefore at least one terminal event after every
    /// `SteeringQueued`, not at most one. A repeated discard costs nothing:
    /// the view removes by id, so the second one finds no entry and takes
    /// nothing, which `a_discard_of_an_unknown_id_withdraws_nothing` pins.
    ///
    /// Lock discipline: `is_streaming_for` releases `active_streams` before
    /// `remove_steering` locks `steering_queues`, and both are released
    /// before anything is emitted. See [`SteeringQueues`].
    ///
    /// # Errors
    ///
    /// Returns the same `ServiceError::Validation` the precondition check
    /// returns, so a caller cannot tell the two orderings apart.
    ///
    /// @plan PLAN-20260903-ISSUE222.P07
    /// @plan PLAN-20260903-ISSUE222.P08
    /// @requirement REQ-222-003
    /// @requirement REQ-222-004
    pub(super) fn confirm_or_withdraw_steering(
        &self,
        conversation_id: Uuid,
        steer_id: Uuid,
    ) -> ServiceResult<Uuid> {
        if ChatService::is_streaming_for(self, conversation_id) {
            return Ok(steer_id);
        }

        let _ = self.remove_steering(conversation_id, steer_id);
        emit_steering_discarded_id(conversation_id, steer_id);
        Err(no_active_turn())
    }

    /// Append `entry` to a conversation's steering queue.
    ///
    /// Returns `false` without queuing when the conversation already holds
    /// `MAX_QUEUED_STEERING_MESSAGES` entries.
    ///
    /// Reachable from the steering tests so they can put an entry on a queue
    /// without announcing it, which is the state `queue_steering` leaves
    /// behind between the insert and the emit.
    ///
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @plan PLAN-20260903-ISSUE222.P08
    /// @requirement REQ-222-004
    pub(super) fn push_steering(&self, conversation_id: Uuid, entry: QueuedSteering) -> bool {
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

    /// Take one steering message off a conversation's queue by id, leaving
    /// everything else queued in order.
    ///
    /// `drain_steering` is the wrong tool for withdrawing a single entry: the
    /// rest of the queue may belong to a turn that is about to deliver it,
    /// and taking it would cancel instructions the user is still owed.
    ///
    /// Returns `None` when the entry is no longer queued, which is what a
    /// teardown racing the withdrawal leaves behind — `clear_streaming_state`
    /// drains the whole queue, so it may already have taken and announced
    /// this entry. The caller announces the discard either way; see
    /// [`ChatServiceImpl::confirm_or_withdraw_steering`] for why a `None`
    /// here is not permission to stay quiet.
    ///
    /// @plan PLAN-20260903-ISSUE222.P07
    /// @plan PLAN-20260903-ISSUE222.P08
    /// @requirement REQ-222-003
    fn remove_steering(&self, conversation_id: Uuid, steer_id: Uuid) -> Option<QueuedSteering> {
        let mut queues = self
            .steering_queues
            .lock()
            .expect("steering_queues poisoned");
        let removed = queues.get_mut(&conversation_id).and_then(|queue| {
            let position = queue.iter().position(|entry| entry.id == steer_id)?;
            queue.remove(position)
        });
        drop(queues);
        removed
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
