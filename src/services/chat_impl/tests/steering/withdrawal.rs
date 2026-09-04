//! Withdrawing a steer whose turn ended while it was being accepted.
//!
//! `queue_steering` decides a conversation has a running turn and then puts
//! the steer on that conversation's queue. Those are two lock acquisitions,
//! one after the other, and nothing holds the turn still in between. A
//! teardown that lands there drains the queue before the entry is on it, so
//! the entry arrives on a queue nothing will ever come back for — already
//! announced to the view as waiting, and never resolved. That is the stuck
//! bubble `SteeringDiscarded` exists to prevent, reached through a narrower
//! window than the paths that already emit it.
//!
//! Reading the stream state once more, after the entry is queued, is what
//! closes it. These tests drive that re-check against the state the window
//! leaves behind: an announced entry on the queue, and no turn left to
//! deliver it.
//!
//! The window has a second half. The insert releases the queue lock before
//! the `SteeringQueued` is emitted, so a teardown can land there too, drain
//! the entry and announce the discard ahead of the announcement that the
//! entry exists. The re-check then has nothing left to remove, and says so
//! anyway: what the view needs is a terminal event after every
//! `SteeringQueued`, not a unique one.
//!
//! @plan PLAN-20260903-ISSUE222.P07
//! @plan PLAN-20260903-ISSUE222.P08
//! @requirement REQ-222-003
//! @requirement REQ-222-004

use super::*;
use crate::events::emit;
use crate::services::chat_impl::steering::QueuedSteering;
use crate::services::chat_impl::streaming::clear_streaming_state;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// What the view was told about one steer.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-003
#[derive(Debug, PartialEq, Eq)]
enum Announcement {
    Queued,
    Discarded,
}

/// Every announcement carrying `steer_id`, in the order it was emitted.
///
/// Order is the assertion, not a detail of it: the view has to be told the
/// entry exists before it is told to take it away, or the withdrawal lands
/// on an entry that has not been rendered yet and that entry then stays on
/// screen for the rest of the session.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-003
fn announcements(events: &[ChatEvent], steer_id: Uuid) -> Vec<Announcement> {
    events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::SteeringQueued { steer_id: id, .. } if *id == steer_id => {
                Some(Announcement::Queued)
            }
            ChatEvent::SteeringDiscarded { steer_id: id, .. } if *id == steer_id => {
                Some(Announcement::Discarded)
            }
            _ => None,
        })
        .collect()
}

/// The event that tells the collector it has seen everything the body did.
///
/// The bus delivers in emission order, so a collector holding this has
/// already drained every event emitted before it. That is a fact about the
/// bus rather than an interval these tests hope is long enough under load.
/// Nothing here cancels a stream, so this id cannot be confused with a real
/// one.
fn end_marker(conversation_id: Uuid) -> AppEvent {
    AppEvent::Chat(ChatEvent::StreamCancelled {
        conversation_id,
        message_id: Uuid::new_v4(),
        partial_content: String::new(),
    })
}

/// Run `body` while a task drains the event bus into this conversation's
/// chat events, and return both.
///
/// The bus is a shared 16-slot ring. A receiver that only drains once the
/// work is finished can be lagged past the events under test by whatever
/// else is running in parallel, which turns an exact-count assertion into a
/// statement about how busy the process was. Draining concurrently keeps
/// these counts about this conversation.
///
/// The collector signals `ready` the moment it holds a subscription and the
/// body does not start until that arrives: a broadcast receiver buffers from
/// the instant it exists, so a subscription in hand is exactly the guarantee
/// these counts need.
async fn events_during<T, F>(conversation_id: Uuid, body: F) -> (T, Vec<ChatEvent>)
where
    F: std::future::Future<Output = T>,
{
    /// Only bounds a hang; the collector stops on the marker, not on time.
    const GIVE_UP_AFTER: Duration = Duration::from_secs(10);

    let ready = Arc::new(Notify::new());
    let ready_for_collector = ready.clone();
    let subscribed = CancellationToken::new();
    let subscribed_for_collector = subscribed.clone();

    let collector = tokio::spawn(async move {
        let mut rx = subscribe();
        // Subscribed. `Notify` keeps the permit even with nobody waiting
        // yet, so this cannot be signalled into the void.
        ready_for_collector.notify_one();
        subscribed_for_collector.cancel();

        let mut matched = Vec::new();
        let deadline = tokio::time::Instant::now() + GIVE_UP_AFTER;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(AppEvent::Chat(event))) => {
                    if event_conversation_id(&event) != Some(conversation_id) {
                        continue;
                    }
                    if matches!(event, ChatEvent::StreamCancelled { .. }) {
                        return matched;
                    }
                    matched.push(event);
                }
                // Other subsystems' events, and lags caused by other tests'
                // traffic, are skipped; keep draining.
                Ok(Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    panic!("the event bus closed before {conversation_id} reached its end marker")
                }
                Err(elapsed) => panic!(
                    "waiting {GIVE_UP_AFTER:?} for {conversation_id} to reach its end marker: \
                     {elapsed}, saw {matched:?}"
                ),
            }
        }
    });

    ready.notified().await;
    subscribed.cancelled().await;

    let outcome = body.await;
    let _ = emit(end_marker(conversation_id));

    let events = collector.await.expect("event collector must not panic");
    (outcome, events)
}

/// Queue a steer through the real service path.
///
/// Going through `ChatService::steer` rather than the queue directly is what
/// makes the entry and its `SteeringQueued` the production ones: the state
/// these tests re-check is the state an accepted steer actually leaves.
async fn accept_steer(service: &ChatServiceImpl, conversation_id: Uuid, text: &str) -> Uuid {
    ChatService::steer(service, conversation_id, text.to_string())
        .await
        .expect("the turn is running here, so the steer must be accepted")
}

/// A steer whose turn is gone by the time its insert is re-checked is taken
/// back off the queue, announced as discarded, and refused.
///
/// The refusal has to be the one the precondition check gives, or a caller
/// can tell which side of the window it landed on and starts caring.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-003
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_whose_turn_ended_during_the_insert_is_withdrawn_and_announced() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let ((steer_id, error), events) = events_during(conversation_id, async {
        let steer_id = accept_steer(&service, conversation_id, "take the other branch").await;

        // The turn ends, taking the stream slot with it and leaving this
        // entry behind: the drain that would have collected it already ran.
        service.clear_all_streams_for_test();

        let error = service
            .confirm_or_withdraw_steering(conversation_id, steer_id)
            .expect_err("a steer with no turn left to deliver it must be refused");
        (steer_id, error)
    })
    .await;

    assert!(
        matches!(&error, ServiceError::Validation(message) if message.contains("No active turn to steer")),
        "the refusal must be the one the precondition check gives, got: {error}"
    );
    assert!(
        drained_pairs(&service, conversation_id).is_empty(),
        "a withdrawn steer must not stay on the queue"
    );
    assert_eq!(
        announcements(&events, steer_id),
        vec![Announcement::Queued, Announcement::Discarded],
        "the view must be told the entry exists exactly once and withdrawn exactly once, \
         in that order, got {events:?}"
    );
}

/// Withdrawing one steer takes only that steer.
///
/// The queue behind it may belong to a turn that is about to drain it, so
/// taking the whole queue would cancel instructions the user is still owed.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-003
#[tokio::test]
async fn withdrawing_one_steer_leaves_the_rest_of_the_queue_alone() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let ((first, withdrawn, third), events) = events_during(conversation_id, async {
        let first = accept_steer(&service, conversation_id, "first steer").await;
        let withdrawn = accept_steer(&service, conversation_id, "second steer").await;
        let third = accept_steer(&service, conversation_id, "third steer").await;

        service.clear_all_streams_for_test();
        service
            .confirm_or_withdraw_steering(conversation_id, withdrawn)
            .expect_err("a steer with no turn left to deliver it must be refused");
        (first, withdrawn, third)
    })
    .await;

    assert_eq!(
        drained_pairs(&service, conversation_id),
        vec![
            (first, "first steer".to_string()),
            (third, "third steer".to_string()),
        ],
        "only the withdrawn steer may be taken, and the rest must keep their order"
    );
    assert_eq!(
        discarded_ids(&events),
        vec![withdrawn],
        "only the withdrawn steer may be announced as discarded, got {events:?}"
    );
}

/// An entry a teardown already drained is still announced as discarded.
///
/// Finding the entry gone says nothing about whether the view has been told
/// what became of it: the teardown that took it may have announced its
/// discard before this steer's `SteeringQueued` was even emitted, in which
/// case the view withdrew an entry it had not rendered yet and then rendered
/// it. So the announcement is unconditional. Announcing twice costs nothing,
/// because the view removes by id and the second one finds nothing to take.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @plan PLAN-20260903-ISSUE222.P08
/// @requirement REQ-222-003
#[tokio::test]
async fn withdrawing_an_entry_a_teardown_already_drained_still_announces_a_discard() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let (steer_id, events) = events_during(conversation_id, async {
        let steer_id = accept_steer(&service, conversation_id, "already collected").await;

        // The teardown reached this queue first and took the entry with it.
        service.clear_all_streams_for_test();
        assert_eq!(
            drained_pairs(&service, conversation_id),
            vec![(steer_id, "already collected".to_string())],
            "the drain standing in for the teardown must be what took the entry"
        );

        service
            .confirm_or_withdraw_steering(conversation_id, steer_id)
            .expect_err("a steer with no turn left to deliver it must still be refused");
        steer_id
    })
    .await;

    assert_eq!(
        announcements(&events, steer_id),
        vec![Announcement::Queued, Announcement::Discarded],
        "an announced entry must reach a terminal event even when this call did not take it, \
         got {events:?}"
    );
}

/// A steer whose entry a teardown drains before its `SteeringQueued` is even
/// emitted still reaches a terminal event.
///
/// `queue_steering` inserts, releases the queue lock, and only then
/// announces the entry. A teardown that lands in that gap drains the queue
/// and announces the discard first, so the view processes a withdrawal for
/// an entry it has not rendered yet and then renders it. The re-check that
/// follows finds nothing left to remove, and if that silenced it the entry
/// would wait on screen for the rest of the session.
///
/// The window is driven by call order — the production insert, the
/// production teardown, the announcement `queue_steering` emits next, then
/// the re-check — so this is a fact about the sequence rather than a race
/// two threads have to be caught in.
///
/// @plan PLAN-20260903-ISSUE222.P08
/// @requirement REQ-222-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steer_drained_before_it_was_announced_still_reaches_a_terminal_event() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");
    let stream_id = service
        .stream_id_for_test(conversation_id)
        .expect("the running turn must have a stream id");
    let (active_streams, steering_queues) = service.stream_registries_for_test();
    let steer_id = Uuid::new_v4();

    let ((), events) = events_during(conversation_id, async {
        // queue_steering has decided the turn is running and put the entry on
        // the queue. Nothing has been announced yet.
        assert!(
            service.push_steering(
                conversation_id,
                QueuedSteering {
                    id: steer_id,
                    text: "take the other branch".to_string(),
                },
            ),
            "the queue has room, so the insert must take"
        );

        // The turn's teardown lands in the gap, takes the entry and reports
        // it gone before anyone has been told it exists.
        clear_streaming_state(
            &active_streams,
            &steering_queues,
            conversation_id,
            stream_id,
        );

        // queue_steering resumes and announces the entry it queued, knowing
        // nothing of the teardown.
        let _ = emit(AppEvent::Chat(ChatEvent::SteeringQueued {
            conversation_id,
            steer_id,
            text: "take the other branch".to_string(),
        }));

        service
            .confirm_or_withdraw_steering(conversation_id, steer_id)
            .expect_err("a steer with no turn left to deliver it must be refused");
    })
    .await;

    let announced = announcements(&events, steer_id);
    assert_eq!(
        announced,
        vec![
            Announcement::Discarded,
            Announcement::Queued,
            Announcement::Discarded,
        ],
        "the teardown's discard arrives before the entry is announced, so the re-check owes \
         the view another one, got {events:?}"
    );
    assert_eq!(
        announced.last(),
        Some(&Announcement::Discarded),
        "the last thing said about a steer nobody will deliver must be that it is gone, \
         got {events:?}"
    );
}

/// A steer whose turn is still running is confirmed, not withdrawn.
///
/// The re-check runs on every accepted steer, so the ordinary path is the
/// one it has to leave alone: the entry stays queued for its boundary, the
/// caller gets its id, and the view hears nothing beyond the announcement
/// that it is waiting.
///
/// @plan PLAN-20260903-ISSUE222.P07
/// @requirement REQ-222-003
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_against_a_running_turn_is_confirmed_and_never_discarded() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let (steer_id, events) = events_during(conversation_id, async {
        ChatService::steer(&service, conversation_id, "keep going".to_string())
            .await
            .expect("steering a running turn must be accepted")
    })
    .await;

    assert!(
        ChatService::is_streaming_for(&service, conversation_id),
        "the re-check must not disturb the turn it is checking"
    );
    assert_eq!(
        drained_pairs(&service, conversation_id),
        vec![(steer_id, "keep going".to_string())],
        "an accepted steer must stay queued for its delivery boundary"
    );
    assert_eq!(
        announcements(&events, steer_id),
        vec![Announcement::Queued],
        "a steer the turn can still deliver must be announced queued and nothing else, \
         got {events:?}"
    );

    service.clear_all_streams_for_test();
}
