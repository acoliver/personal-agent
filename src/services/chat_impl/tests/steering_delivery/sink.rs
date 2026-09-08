//! The `MidTurnSteering` sink: what happens to a text the model received.
//!
//! A delivery is a promise the view is already rendering: the steer leaves the
//! queue only when the row is persisted, and the `SteeringDelivered` it emits
//! names the entry resolved by FIFO position, never by matching text. The
//! duplicate-text test is the one that guards that rule: under any correlation
//! that looks entries up by text it passes, if at all, only by luck.
//!
//! @plan PLAN-20260905-STEERINT.P04
//! @requirement REQ-SI-003
//! @requirement REQ-SI-005

use super::*;

use crate::llm::steering::SteeringDeliverySink;

/// Queue an entry directly, without announcing it, under a caller-chosen id.
fn queue_entry(fixture: &DeliveryFixture, id: Uuid, text: &str) {
    assert!(
        fixture.service.push_steering(
            fixture.conversation_id,
            QueuedSteering {
                id,
                stream_id: fixture.stream_id,
                text: text.to_string(),
            },
        ),
        "the fixture's queue must accept the entry"
    );
}

/// Two deliveries of byte-identical text resolve the two distinct entries they
/// were queued under, in push order.
///
/// The ids are the assertion: only position-based correlation can return them
/// in the order the entries were pushed, because the texts cannot tell the
/// entries apart.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_texts_resolve_to_distinct_entries_in_push_order() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    queue_entry(&fixture, first, "same text");
    queue_entry(&fixture, second, "same text");
    let mut sink = sink_for(&fixture);

    let ((), events) = events_during(fixture.conversation_id, async {
        sink.delivered("same text")
            .await
            .expect("the first delivery must persist and resolve the head");
        sink.delivered("same text")
            .await
            .expect("the second delivery must persist and resolve the next entry");
    })
    .await;

    assert_eq!(
        delivered_ids(&events),
        vec![first, second],
        "each delivery must resolve its own queue entry, in push order, got {events:?}"
    );
    assert!(
        discarded_ids(&events).is_empty(),
        "a successful delivery must discard nothing, got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![
            (MessageRole::User, "same text".to_string()),
            (MessageRole::User, "same text".to_string()),
        ],
        "both delivered texts must be persisted as user rows, in delivery order"
    );
}

/// A delivery the store refuses returns an error, announces that entry as
/// discarded, writes no row, and never tries the write again.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_delivery_that_cannot_be_persisted_fails_and_discards_without_retry() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let steer_id = Uuid::new_v4();
    queue_entry(&fixture, steer_id, "do the thing");
    fixture.conversations.set_add_message_failure(true).await;
    let mut sink = sink_for(&fixture);

    let (result, events) = events_during(fixture.conversation_id, async {
        sink.delivered("do the thing").await
    })
    .await;

    assert!(
        result.is_err(),
        "a refused persistence must fail the delivery, and with it the turn"
    );
    assert_eq!(
        discarded_ids(&events),
        vec![steer_id],
        "the refused entry must be announced as never-to-deliver, got {events:?}"
    );
    assert!(
        delivered_ids(&events).is_empty(),
        "a steer no store ever recorded must not be reported delivered, got {events:?}"
    );
    assert!(
        fixture.conversations.messages.read().await.is_empty(),
        "the failed write must not have recorded a row"
    );
    assert_eq!(
        fixture.conversations.add_message_attempts(),
        1,
        "the sink must give up after the one failed write, not retry"
    );
}

/// An ordinary delivery persists the text and announces the entry it popped.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_delivery_persists_the_text_and_announces_the_popped_entry() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let steer_id = Uuid::new_v4();
    queue_entry(&fixture, steer_id, "focus on the parser");
    let mut sink = sink_for(&fixture);

    let ((), events) = events_during(fixture.conversation_id, async {
        sink.delivered("focus on the parser")
            .await
            .expect("the delivery must succeed");
    })
    .await;

    assert_eq!(
        delivered_ids(&events),
        vec![steer_id],
        "the delivery must announce the queued entry's id, got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![(MessageRole::User, "focus on the parser".to_string())],
        "the delivered text must be persisted as a user row"
    );
}

/// A delivery whose queue entry is already gone (teardown raced it) still
/// persists the text, records it, and stays silent: the discard already fired.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_delivery_with_no_queue_entry_persists_without_a_terminal_event() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let mut sink = sink_for(&fixture);

    let ((), events) = events_during(fixture.conversation_id, async {
        sink.delivered("landed during teardown")
            .await
            .expect("an empty queue is not an error; the row is still persisted");
    })
    .await;

    assert!(
        events.is_empty(),
        "the teardown race already announced the discard, so the sink must emit nothing, \
         got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![(MessageRole::User, "landed during teardown".to_string())],
        "the text must be persisted even though its queue entry is gone"
    );
    assert_eq!(
        sink.delivered, 1,
        "the delivery must be recorded for the turn that delivered it"
    );
}

/// Correlation must survive a delivery whose text matches a non-head entry:
/// the head is resolved whatever the delivered text says.
///
/// The mutation experiment showed the duplicate-text test alone passes under
/// a text-matcher that removes the entry it matched, because a FIFO fork
/// happens to feed texts back in queue order. This test removes that luck:
/// a text naming the second entry must still resolve the first, which is
/// what "never match by text" means as an implementation rule. The fork's
/// FIFO discipline means production never feeds texts out of order; this
/// pin exists so that a future correlation change fails here first.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn correlation_is_positional_even_when_the_text_matches_a_later_entry() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let head = Uuid::new_v4();
    let later = Uuid::new_v4();
    queue_entry(&fixture, head, "first text");
    queue_entry(&fixture, later, "second text");
    let mut sink = sink_for(&fixture);

    let ((), events) = events_during(fixture.conversation_id, async {
        sink.delivered("second text")
            .await
            .expect("the delivery must succeed");
    })
    .await;

    assert_eq!(
        delivered_ids(&events),
        vec![head],
        "the delivery must resolve the queue head, never the entry its text matches, \
         got {events:?}"
    );
    let remaining = fixture.service.drain_steering(fixture.conversation_id);
    assert_eq!(
        remaining.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        vec![later],
        "the entry the text matched must still be queued, waiting for its own delivery"
    );
}
