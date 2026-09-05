//! A steer's acceptance commits through the send's transport queue.
//!
//! The service-level queue stays the state machine for steering: cap,
//! announcements, withdrawal, teardown. The fork's `SteeringQueue` is the
//! transport, and the only thing that moves text into a model request. The
//! two are written in a fixed order — PA queue, `SteeringQueued`, the
//! confirm re-check, and only then the transport commit — because the
//! commit is the point of no return: once the transport holds the text, a
//! later run on the same queue will deliver it, so anything that can still
//! refuse the steer must happen first.
//!
//! @plan PLAN-20260905-STEERINT.P03
//! @requirement REQ-SI-002
//! @requirement REQ-SI-007

use super::*;
use crate::services::chat_impl::steering::QueuedSteering;

/// The transport queue a running send's slot holds.
///
/// @plan PLAN-20260905-STEERINT.P03
/// @requirement REQ-SI-002
fn transport_of(
    service: &ChatServiceImpl,
    conversation_id: Uuid,
) -> serdes_ai_agent::SteeringQueue {
    service
        .steering_transport_for_test(conversation_id)
        .expect("a running send must hold a transport queue")
}

/// An accepted steer lands on both sides of the two-queue relationship: the
/// state machine keeps the entry, and the transport holds the text a run
/// will deliver at its next tool boundary.
///
/// @plan PLAN-20260905-STEERINT.P03
/// @requirement REQ-SI-002
#[tokio::test]
async fn an_accepted_steer_is_committed_to_the_send_transport() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");
    let transport = transport_of(&service, conversation_id);

    let steer_id = ChatService::steer(
        &service,
        conversation_id,
        "  mid-turn correction  ".to_string(),
    )
    .await
    .expect("steering a running turn must be accepted");

    assert_eq!(
        transport.pending_len(),
        1,
        "the transport must hold the committed text for the next run to deliver"
    );
    assert_eq!(
        drained_pairs(&service, conversation_id),
        vec![(steer_id, "mid-turn correction".to_string())],
        "the state machine must keep the accepted entry, trimmed, until delivery"
    );

    service.clear_all_streams_for_test();
}

/// The steer past the queue cap is refused before any commit: the transport
/// holds exactly the accepted entries, nothing more.
///
/// @plan PLAN-20260905-STEERINT.P03
/// @requirement REQ-SI-002
#[tokio::test]
async fn steer_past_the_cap_never_reaches_the_transport() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");
    let transport = transport_of(&service, conversation_id);

    for index in 0..MAX_QUEUED_STEERING_MESSAGES {
        ChatService::steer(&service, conversation_id, format!("steer {index}"))
            .await
            .expect("steers up to the cap must be accepted");
    }

    ChatService::steer(&service, conversation_id, "one too many".to_string())
        .await
        .expect_err("a steer past the cap must be rejected");

    assert_eq!(
        transport.pending_len(),
        MAX_QUEUED_STEERING_MESSAGES,
        "the refused steer must never reach the transport"
    );

    service.clear_all_streams_for_test();
}

/// A steer withdrawn by the confirm re-check never reaches the transport.
///
/// The window the re-check exists for: the entry was inserted, the view was
/// told, and then the turn ended before the re-check ran. If acceptance
/// committed before that re-check, the text would sit in a transport a
/// later run of the same send could deliver — a ghost instruction the user
/// withdrew. The transport clone here is captured before the teardown so
/// the assertion observes the real queue object rather than its absence.
///
/// @plan PLAN-20260905-STEERINT.P03
/// @requirement REQ-SI-002
#[tokio::test]
async fn a_withdrawn_steer_is_never_committed_to_the_transport() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");
    let transport = transport_of(&service, conversation_id);

    // queue_steering has inserted the entry; nothing has been committed yet.
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

    // The turn ends before the re-check runs.
    service.clear_all_streams_for_test();
    service
        .confirm_or_withdraw_steering(conversation_id, steer_id)
        .expect_err("a steer with no turn left to deliver it must be refused");

    assert_eq!(
        transport.pending_len(),
        0,
        "the withdrawal must never commit the text to the transport"
    );
}

/// A commit whose send is gone between the re-check and the commit is
/// refused, takes its entry back off the queue, and announces the discard.
///
/// The re-check passed a moment earlier; by the time the commit reads the
/// slot, the teardown has removed it and its transport with it. The model
/// can never be handed the text, so the entry must not stay queued, and the
/// view must be told what became of it. The refusal is the same
/// `no_active_turn` the re-check gives, so a caller cannot tell which side
/// of the window it landed on.
///
/// @plan PLAN-20260905-STEERINT.P03
/// @requirement REQ-SI-002
/// @requirement REQ-SI-007
#[tokio::test]
async fn a_commit_after_the_transport_is_gone_is_withdrawn_and_announced() {
    let _steering_bus_guard = lock_steering_bus().await;
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();
    let mut event_rx = subscribe();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    // The state the commit is called in: entry queued, re-check passed.
    assert!(
        service.push_steering(
            conversation_id,
            QueuedSteering {
                id: steer_id,
                text: "too late".to_string(),
            },
        ),
        "the queue has room, so the insert must take"
    );

    // The send tears down before the commit runs, taking the slot and its
    // transport with it.
    service.clear_all_streams_for_test();

    let error = service
        .commit_steering_to_transport(conversation_id, steer_id, "too late")
        .expect_err("a commit whose transport is gone must be refused");

    assert!(
        matches!(&error, ServiceError::Validation(message)
            if message.contains("No active turn to steer")),
        "the refusal must be the one the re-check gives, got: {error}"
    );
    assert!(
        drained_pairs(&service, conversation_id).is_empty(),
        "the refused commit must take the entry back off the queue"
    );

    let events =
        collect_chat_events(&mut event_rx, conversation_id, Duration::from_millis(100)).await;
    assert_eq!(
        discarded_ids(&events),
        vec![steer_id],
        "the view must be told the entry will never be delivered, got {events:?}"
    );
}
