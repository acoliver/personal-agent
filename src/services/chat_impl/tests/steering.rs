//! Steering-queue behavior for `ChatServiceImpl`.
//!
//! Covers issue #222: a steering message submitted while a turn is running is
//! queued against that conversation, is rejected when there is no running
//! turn, is capped, and never disturbs the in-flight generation.
//!
//! @plan PLAN-20260903-ISSUE222.P01
//! @requirement REQ-222-004
//! @requirement REQ-222-006

use crate::events::types::ChatEvent;
use crate::events::{subscribe, AppEvent};
use crate::services::chat_impl::{ChatServiceImpl, MAX_QUEUED_STEERING_MESSAGES};
use crate::services::{ChatService, ConversationService, ProfileService, ServiceError};
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

// Import test support utilities from parent tests module (support.rs)
use super::chat_test_support::{MockConversationService, MockProfileService};

fn make_test_chat_service() -> ChatServiceImpl {
    let conversation_service: Arc<dyn ConversationService> =
        Arc::new(MockConversationService::new(Uuid::new_v4()));
    let profile_service: Arc<dyn ProfileService> = Arc::new(MockProfileService::new());
    ChatServiceImpl::new_for_tests(conversation_service, profile_service)
}

/// The queued entries for a conversation, as `(steer_id, text)` pairs.
///
/// Draining is the only way to observe the queue, so every caller that
/// inspects it also empties it.
fn drained_pairs(service: &ChatServiceImpl, conversation_id: Uuid) -> Vec<(Uuid, String)> {
    service
        .drain_steering(conversation_id)
        .into_iter()
        .map(|entry| (entry.id, entry.text))
        .collect()
}

/// Collect chat events addressed to `conversation_id` within a short window.
///
/// The global event bus is shared by concurrently running tests, so every
/// assertion filters by a conversation UUID unique to this test.
async fn collect_chat_events(
    rx: &mut Receiver<AppEvent>,
    conversation_id: Uuid,
    window: Duration,
) -> Vec<ChatEvent> {
    let mut matched = Vec::new();
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(AppEvent::Chat(event))) => {
                if event_conversation_id(&event) == Some(conversation_id) {
                    matched.push(event);
                }
            }
            // Other subsystems' events and lagged receivers (which just missed
            // other tests' events) are skipped; keep draining.
            Ok(Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
            Err(_elapsed) => break,
        }
    }
    matched
}

/// Conversation id carried by the chat events these tests care about.
fn event_conversation_id(event: &ChatEvent) -> Option<Uuid> {
    match event {
        ChatEvent::SteeringQueued {
            conversation_id, ..
        }
        | ChatEvent::SteeringDelivered {
            conversation_id, ..
        }
        | ChatEvent::StreamCancelled {
            conversation_id, ..
        } => Some(*conversation_id),
        _ => None,
    }
}

/// The `(steer_id, text)` payloads of the `SteeringQueued` events collected.
fn queued_payloads(events: &[ChatEvent]) -> Vec<(Uuid, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::SteeringQueued { steer_id, text, .. } => Some((*steer_id, text.clone())),
            _ => None,
        })
        .collect()
}

/// A conversation with no turn in flight has nothing to steer, so the steer is
/// rejected and nothing is queued.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_without_active_stream_is_rejected_and_queues_nothing() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();
    let mut event_rx = subscribe();

    let error = ChatService::steer(&service, conversation_id, "turn left".to_string())
        .await
        .expect_err("steering an idle conversation must be rejected");

    assert!(
        matches!(&error, ServiceError::Validation(message) if message.contains("No active turn to steer")),
        "expected a no-active-turn validation error, got: {error}"
    );
    assert!(
        drained_pairs(&service, conversation_id).is_empty(),
        "a rejected steer must not queue anything"
    );

    let events =
        collect_chat_events(&mut event_rx, conversation_id, Duration::from_millis(100)).await;
    assert!(
        queued_payloads(&events).is_empty(),
        "a rejected steer must not emit SteeringQueued, got {events:?}"
    );
}

/// A steer against a running turn is accepted, lands on that conversation's
/// queue, and announces itself with the id it returned.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_with_running_stream_queues_the_entry_and_emits_queued_event() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();
    let mut event_rx = subscribe();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let steer_id = ChatService::steer(
        &service,
        conversation_id,
        "  focus on the parser  ".to_string(),
    )
    .await
    .expect("steering a running turn must be accepted");

    assert_eq!(
        drained_pairs(&service, conversation_id),
        vec![(steer_id, "focus on the parser".to_string())],
        "the queue must hold exactly the accepted steer, trimmed"
    );

    let events =
        collect_chat_events(&mut event_rx, conversation_id, Duration::from_millis(100)).await;
    assert_eq!(
        queued_payloads(&events),
        vec![(steer_id, "focus on the parser".to_string())],
        "the accepted steer must emit exactly one SteeringQueued, got {events:?}"
    );

    service.clear_all_streams_for_test();
}

/// Steering reaches only the conversation that owns the running stream: a
/// steer for A is rejected while only B is running, and B's queue stays empty.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_for_one_conversation_does_not_land_on_another() {
    let service = make_test_chat_service();
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed");

    let error = ChatService::steer(&service, conversation_a, "reroute".to_string())
        .await
        .expect_err("steering A while only B is running must be rejected");

    assert!(
        matches!(&error, ServiceError::Validation(message) if message.contains("No active turn to steer")),
        "expected a no-active-turn validation error for A, got: {error}"
    );
    assert!(
        drained_pairs(&service, conversation_a).is_empty(),
        "A must have nothing queued"
    );
    assert!(
        drained_pairs(&service, conversation_b).is_empty(),
        "a steer addressed to A must never land on B's queue"
    );

    service.clear_all_streams_for_test();
}

/// The queue is capped: the steer past the cap is rejected with an error that
/// names the cap, and the already-queued entries survive in submission order.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
#[tokio::test]
async fn steer_past_the_cap_is_rejected_and_preserves_the_full_queue() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let mut accepted = Vec::new();
    for index in 0..MAX_QUEUED_STEERING_MESSAGES {
        let text = format!("steer {index}");
        let steer_id = ChatService::steer(&service, conversation_id, text.clone())
            .await
            .expect("steers up to the cap must be accepted");
        accepted.push((steer_id, text));
    }

    let error = ChatService::steer(&service, conversation_id, "one too many".to_string())
        .await
        .expect_err("a steer past the cap must be rejected");

    let message = error.to_string();
    assert!(
        message.contains(&MAX_QUEUED_STEERING_MESSAGES.to_string()),
        "the rejection must name the cap of {MAX_QUEUED_STEERING_MESSAGES}, got: {message}"
    );
    assert_eq!(
        drained_pairs(&service, conversation_id),
        accepted,
        "the rejected steer must leave the full queue untouched and in order"
    );

    service.clear_all_streams_for_test();
}

/// Steering text that carries no instruction is rejected rather than queued.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
#[tokio::test]
async fn whitespace_only_steering_text_is_rejected() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let error = ChatService::steer(&service, conversation_id, "  \n\t  ".to_string())
        .await
        .expect_err("whitespace-only steering text must be rejected");

    assert!(
        matches!(error, ServiceError::Validation(_)),
        "expected a validation error, got: {error}"
    );
    assert!(
        drained_pairs(&service, conversation_id).is_empty(),
        "blank steering text must not queue anything"
    );

    service.clear_all_streams_for_test();
}

/// Draining hands back every queued steer in submission order and leaves the
/// conversation with an empty queue.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-005
#[tokio::test]
async fn drain_steering_returns_fifo_order_and_empties_the_queue() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    let mut submitted = Vec::new();
    for text in ["first", "second", "third"] {
        let steer_id = ChatService::steer(&service, conversation_id, text.to_string())
            .await
            .expect("steering a running turn must be accepted");
        submitted.push((steer_id, text.to_string()));
    }

    assert_eq!(
        drained_pairs(&service, conversation_id),
        submitted,
        "drain must return queued steers in FIFO order"
    );
    assert!(
        drained_pairs(&service, conversation_id).is_empty(),
        "drain must leave the queue empty"
    );

    service.clear_all_streams_for_test();
}

/// Steering is additive: it never aborts the generation it is steering.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-006
#[tokio::test]
async fn steering_does_not_cancel_the_active_turn() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();
    let mut event_rx = subscribe();

    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");

    ChatService::steer(
        &service,
        conversation_id,
        "keep going, but add tests".to_string(),
    )
    .await
    .expect("steering a running turn must be accepted");

    assert!(
        ChatService::is_streaming_for(&service, conversation_id),
        "the turn must still be running after a steer"
    );

    let events =
        collect_chat_events(&mut event_rx, conversation_id, Duration::from_millis(100)).await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ChatEvent::StreamCancelled { .. })),
        "steering must not cancel the turn, got {events:?}"
    );

    service.clear_all_streams_for_test();
}

/// A cancelled turn will never reach a delivery boundary, so its queued steers
/// are discarded — and only its own.
///
/// @plan PLAN-20260903-ISSUE222.P01
/// @requirement REQ-222-004
#[tokio::test]
async fn cancelling_a_stream_discards_only_that_conversations_queue() {
    let service = make_test_chat_service();
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();

    service
        .begin_stream_for_test(conversation_a)
        .expect("begin_stream(A) should succeed");
    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed");

    ChatService::steer(&service, conversation_a, "steer A".to_string())
        .await
        .expect("steering A must be accepted");
    let b_steer_id = ChatService::steer(&service, conversation_b, "steer B".to_string())
        .await
        .expect("steering B must be accepted");

    ChatService::cancel(&service, conversation_a);
    sleep(Duration::from_millis(20)).await;

    assert!(
        drained_pairs(&service, conversation_a).is_empty(),
        "cancelling A must discard A's queued steering"
    );
    assert_eq!(
        drained_pairs(&service, conversation_b),
        vec![(b_steer_id, "steer B".to_string())],
        "cancelling A must leave B's queue untouched"
    );

    service.clear_all_streams_for_test();
}
