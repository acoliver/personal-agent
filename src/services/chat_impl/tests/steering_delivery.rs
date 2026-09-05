//! Chaining of a send's follow-up turns while steering stays queued.
//!
//! Covers issue #222 and its mid-turn steering integration: the transport
//! delivers a queued steer itself, at a tool-call boundary inside a running
//! turn, and the sink records it. When a turn still ends with entries
//! queued, the send chains a follow-up turn so the transport gets a boundary
//! to deliver them at. A cancelled or failed turn chains nothing,
//! finalization still happens exactly once, and the chain is bounded.
//!
//! The turn itself is the only double here: each test hands the chaining loop
//! a runner that records the history it was seeded with and replies with a
//! scripted transcript, which is the same level the approval-gate tests in
//! `three_stream_concurrency.rs` operate at. Where a test simulates the
//! delivery side, it hands the text to the real mid-turn sink, so the row
//! and the resolved queue entry are the production ones.
//!
//! @plan PLAN-20260903-ISSUE222.P02
//! @plan PLAN-20260905-STEERINT.P04
//! @requirement REQ-222-005
//! @requirement REQ-222-006
//! @requirement REQ-222-007
//! @requirement REQ-222-008
//! @requirement REQ-SI-002
//! @requirement REQ-SI-006

use super::*;
use crate::events::subscribe;
use crate::events::types::ChatEvent;
use crate::llm::steering::SteeringDeliverySink;
use crate::llm::Role as LlmRole;
use crate::services::chat_impl::streaming::steering_delivery::{
    run_steered_turns_and_finalize, SteeringDeliveryContext, MAX_STEERING_TURNS,
};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
use tokio::time::Duration;

mod approval;
mod discard;
mod sink;

/// The future a scripted turn runner hands back.
///
/// Boxed because one runner is a constant reply while another awaits a tool
/// approval decision, and the delivery loop takes a single future type.
type TurnFuture = Pin<Box<dyn std::future::Future<Output = streaming::StreamTranscript> + Send>>;

/// The seed history handed to each turn, in order.
type TurnLog = Arc<StdMutex<Vec<Vec<LlmMessage>>>>;

/// A turn that produced `text` and ended cleanly.
fn completed_turn(text: &str) -> streaming::StreamTranscript {
    streaming::StreamTranscript {
        response_text: text.to_string(),
        completed: true,
        ..streaming::StreamTranscript::default()
    }
}

/// A `CompressionResult` that leaves the conversation's context untouched
/// apart from what finalization itself writes.
fn no_compression() -> CompressionResult {
    CompressionResult {
        messages: vec![],
        phase: crate::models::CompressionPhase::None,
        masked_tool_seqs: None,
        summary_range: None,
        preserved_facts: None,
        estimated_tokens: 0,
    }
}

/// `(role, content)` for every message in a turn's seed history.
///
/// `LlmMessage` has no `PartialEq`, and role plus content is what an
/// ordering assertion is actually about.
fn shape(messages: &[LlmMessage]) -> Vec<(LlmRole, String)> {
    messages
        .iter()
        .map(|message| (message.role, message.content.clone()))
        .collect()
}

/// `(role, content)` for every message persisted to the conversation.
async fn persisted_shape(
    conversations: &Arc<MockConversationService>,
) -> Vec<(MessageRole, String)> {
    conversations
        .messages
        .read()
        .await
        .iter()
        .map(|message| (message.role.clone(), message.content.clone()))
        .collect()
}

/// Conversation id carried by the chat events these tests assert on.
fn event_conversation_id(event: &ChatEvent) -> Option<Uuid> {
    match event {
        ChatEvent::SteeringQueued {
            conversation_id, ..
        }
        | ChatEvent::SteeringDelivered {
            conversation_id, ..
        }
        | ChatEvent::SteeringDiscarded {
            conversation_id, ..
        }
        | ChatEvent::StreamCompleted {
            conversation_id, ..
        }
        | ChatEvent::StreamCancelled {
            conversation_id, ..
        }
        | ChatEvent::StreamError {
            conversation_id, ..
        } => Some(*conversation_id),
        _ => None,
    }
}

/// The `steer_id`s carried by the `SteeringDelivered` events, in order.
fn delivered_ids(events: &[ChatEvent]) -> Vec<Uuid> {
    events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::SteeringDelivered { steer_id, .. } => Some(*steer_id),
            _ => None,
        })
        .collect()
}

/// The `steer_id`s carried by the `SteeringQueued` events, in order.
fn queued_ids(events: &[ChatEvent]) -> Vec<Uuid> {
    events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::SteeringQueued { steer_id, .. } => Some(*steer_id),
            _ => None,
        })
        .collect()
}

/// The `steer_id`s carried by the `SteeringDiscarded` events, in order.
///
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-003
fn discarded_ids(events: &[ChatEvent]) -> Vec<Uuid> {
    events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::SteeringDiscarded { steer_id, .. } => Some(*steer_id),
            _ => None,
        })
        .collect()
}

/// How many `StreamCompleted` events were seen.
fn completion_count(events: &[ChatEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, ChatEvent::StreamCompleted { .. }))
        .count()
}

/// Run `body` while a task drains the event bus into this conversation's
/// chat events.
///
/// The bus is a shared 16-slot ring, so a receiver that only drains after the
/// fact can be lagged past the events under test by other tests running in
/// parallel. Draining concurrently keeps the exact-count assertions honest.
///
/// The collector signals `ready` the moment it holds a subscription, and
/// `body` does not start until that signal arrives. A broadcast receiver
/// buffers from the instant it is created, so a subscription in hand is
/// exactly the guarantee the exact-count assertions need. It is also a fact
/// the collector reports, rather than an interval this test hopes is long
/// enough under load.
async fn events_during<T, F>(conversation_id: Uuid, body: F) -> (T, Vec<ChatEvent>)
where
    F: std::future::Future<Output = T>,
{
    let stop = CancellationToken::new();
    let stop_for_collector = stop.clone();
    let ready = Arc::new(Notify::new());
    let ready_for_collector = ready.clone();

    let collector = tokio::spawn(async move {
        let mut rx = subscribe();
        // Subscribed. `Notify` holds the permit even if nobody is waiting
        // yet, so this cannot be signalled into the void.
        ready_for_collector.notify_one();

        let mut matched = Vec::new();
        loop {
            tokio::select! {
                received = rx.recv() => match received {
                    Ok(AppEvent::Chat(event)) => {
                        if event_conversation_id(&event) == Some(conversation_id) {
                            matched.push(event);
                        }
                    }
                    // Other subsystems' events, and lags caused by other
                    // tests' traffic, are skipped; keep draining.
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
                () = stop_for_collector.cancelled() => break,
            }
        }
        matched
    });

    ready.notified().await;

    let outcome = body.await;

    tokio::time::sleep(Duration::from_millis(50)).await;
    stop.cancel();
    let events = collector.await.expect("event collector must not panic");
    (outcome, events)
}

/// A conversation with a running turn, wired to the registries the delivery
/// loop operates on.
struct DeliveryFixture {
    service: Arc<ChatServiceImpl>,
    conversations: Arc<MockConversationService>,
    conversation_service: Arc<dyn crate::services::ConversationService>,
    conversation_id: Uuid,
    stream_id: Uuid,
    active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
    steering_queues: SteeringQueues,
    cancel: CancellationToken,
}

impl DeliveryFixture {
    /// Model label recorded on every assistant message these tests persist.
    const MODEL_LABEL: &'static str = "Steered Model";

    fn new() -> Self {
        let conversations = Arc::new(MockConversationService::new(Uuid::new_v4()));
        let conversation_service =
            conversations.clone() as Arc<dyn crate::services::ConversationService>;
        Self::with_conversation_service(conversations, conversation_service)
    }

    /// A fixture whose turns persist through `conversation_service`, with
    /// `conversations` the mock underneath it that records what landed.
    ///
    /// The two are the same object for `new`. The discard tests interpose a
    /// double so a write can fail, or can steer the turn that is tearing
    /// down around it.
    ///
    /// @plan PLAN-20260903-ISSUE222.P06
    /// @requirement REQ-222-003
    fn with_conversation_service(
        conversations: Arc<MockConversationService>,
        conversation_service: Arc<dyn crate::services::ConversationService>,
    ) -> Self {
        let profile_service =
            Arc::new(MockProfileService::new()) as Arc<dyn crate::services::ProfileService>;
        let service = Arc::new(ChatServiceImpl::new_for_tests(
            conversation_service.clone(),
            profile_service,
        ));

        let conversation_id = Uuid::new_v4();
        service
            .begin_stream_for_test(conversation_id)
            .expect("begin_stream should succeed");
        let stream_id = service
            .stream_id_for_test(conversation_id)
            .expect("the reserved stream should be readable back");
        let (active_streams, steering_queues) = service.stream_registries_for_test();

        Self {
            service,
            conversations,
            conversation_service,
            conversation_id,
            stream_id,
            active_streams,
            steering_queues,
            cancel: CancellationToken::new(),
        }
    }

    /// The history the first turn is seeded with.
    fn initial_messages() -> Vec<LlmMessage> {
        vec![
            LlmMessage::system("be helpful"),
            LlmMessage::user("original prompt"),
        ]
    }

    /// Queue a steering message through the real service path.
    async fn steer(&self, text: &str) -> Uuid {
        ChatService::steer(
            self.service.as_ref(),
            self.conversation_id,
            text.to_string(),
        )
        .await
        .expect("steering a running turn must be accepted")
    }

    /// Run this send's turns through the delivery loop and finalize.
    async fn run_turns<R>(&self, run_turn: R)
    where
        R: FnMut(Vec<LlmMessage>) -> TurnFuture,
    {
        let ctx = SteeringDeliveryContext {
            finalize: streaming::StreamFinalizeContext {
                conversation_service: &self.conversation_service,
                conversation_id: self.conversation_id,
                stream_id: self.stream_id,
                active_streams: &self.active_streams,
                steering_queues: &self.steering_queues,
            },
            cancel: &self.cancel,
            model_label: Self::MODEL_LABEL,
        };
        run_steered_turns_and_finalize(&ctx, no_compression(), Self::initial_messages(), run_turn)
            .await;
    }

    fn is_streaming(&self) -> bool {
        ChatService::is_streaming_for(self.service.as_ref(), self.conversation_id)
    }
}

/// A runner that logs each turn's seed history and replies with the next
/// scripted completed transcript.
fn scripted_runner(
    log: &TurnLog,
    replies: &'static [&'static str],
) -> impl FnMut(Vec<LlmMessage>) -> TurnFuture {
    let log = log.clone();
    move |messages: Vec<LlmMessage>| {
        let index = {
            let mut turns = log.lock().expect("turn log poisoned");
            turns.push(messages);
            turns.len() - 1
        };
        let reply = replies
            .get(index)
            .copied()
            .expect("the turn runner ran out of scripted replies");
        Box::pin(std::future::ready(completed_turn(reply))) as TurnFuture
    }
}

fn new_turn_log() -> TurnLog {
    Arc::new(StdMutex::new(Vec::new()))
}

/// The seed histories the runner recorded, consuming the log.
fn recorded(log: &TurnLog) -> Vec<Vec<LlmMessage>> {
    log.lock().expect("turn log poisoned").clone()
}

/// The mid-turn sink of a fixture's send, shared behind a lock so a turn's
/// future can hand it texts.
///
/// @plan PLAN-20260905-STEERINT.P04
type SharedSink = Arc<tokio::sync::Mutex<streaming::steering_sink::MidTurnSteering>>;

/// A sink wired to the fixture's conversation service and steering queues.
fn sink_for(fixture: &DeliveryFixture) -> streaming::steering_sink::MidTurnSteering {
    streaming::steering_sink::MidTurnSteering::new(
        fixture.conversation_service.clone(),
        fixture.conversation_id,
        fixture.steering_queues.clone(),
    )
}

/// Wrap a sink for handoff into turn futures.
fn shared_sink(sink: streaming::steering_sink::MidTurnSteering) -> SharedSink {
    Arc::new(tokio::sync::Mutex::new(sink))
}

/// A runner that logs each turn's seed history, replies with the next
/// scripted completed transcript, and on turn `deliver_on` (1-based) first
/// hands `texts` to the sink — what the transport does with the queued steers
/// at that turn's tool boundary.
///
/// @plan PLAN-20260905-STEERINT.P04
fn boundary_runner(
    log: &TurnLog,
    replies: &'static [&'static str],
    sink: SharedSink,
    deliver_on: usize,
    texts: &'static [&'static str],
) -> impl FnMut(Vec<LlmMessage>) -> TurnFuture {
    let log = log.clone();
    move |messages: Vec<LlmMessage>| {
        let index = {
            let mut turns = log.lock().expect("turn log poisoned");
            turns.push(messages);
            turns.len() - 1
        };
        let sink = Arc::clone(&sink);
        Box::pin(async move {
            if index + 1 == deliver_on {
                for text in texts {
                    sink.lock()
                        .await
                        .delivered(text)
                        .await
                        .expect("the boundary delivery must persist and resolve its entry");
                }
            }
            completed_turn(
                replies
                    .get(index)
                    .copied()
                    .expect("the turn runner ran out of scripted replies"),
            )
        }) as TurnFuture
    }
}

/// An empty queue must leave a send exactly as it is today: one turn, one
/// completion, and the assistant output persisted once.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_steering_queue_runs_a_single_turn_and_finalizes_once() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let log = new_turn_log();

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture
            .run_turns(scripted_runner(&log, &["only answer"]))
            .await;
    })
    .await;

    let turns = recorded(&log);
    assert_eq!(turns.len(), 1, "an empty queue must run exactly one turn");
    assert_eq!(
        shape(&turns[0]),
        shape(&DeliveryFixture::initial_messages()),
        "the single turn must be seeded with the prepared history, unchanged"
    );

    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![(MessageRole::Assistant, "only answer".to_string())],
        "the assistant output must be persisted exactly once, with nothing else"
    );
    let persisted = fixture.conversations.messages.read().await.clone();
    assert!(
        !persisted[0].interrupted,
        "a cleanly finished turn must not be marked interrupted"
    );
    assert_eq!(
        persisted[0].model_id.as_deref(),
        Some(DeliveryFixture::MODEL_LABEL)
    );
    assert!(
        fixture.conversations.context_state.read().await.is_some(),
        "normal finalization must still persist context state"
    );

    assert_eq!(
        completion_count(&events),
        1,
        "an empty queue must emit exactly one StreamCompleted, got {events:?}"
    );
    assert!(
        delivered_ids(&events).is_empty(),
        "an empty queue must deliver no steering, got {events:?}"
    );
    assert!(
        !fixture.is_streaming(),
        "finalization must release the conversation's stream slot"
    );
}

/// A text-only turn never crosses a tool boundary, so its steer is still
/// queued when the turn ends; the chain peeks at it and runs a follow-up
/// turn, and the transport's boundary in that turn delivers it.
///
/// The steer is never seeded into the follow-up turn's history: the seed
/// carries only the finished turn's assistant output, and the text reaches
/// the model through the sink at the boundary, persisted after that output.
///
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-002
/// @requirement REQ-SI-006
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_text_only_turn_leaves_its_steer_queued_and_the_chained_turn_delivers_it() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let steer_id = fixture.steer("focus on the parser").await;
    let log = new_turn_log();
    let sink = shared_sink(sink_for(&fixture));

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture
            .run_turns(boundary_runner(
                &log,
                &["first answer", "second answer"],
                sink,
                2,
                &["focus on the parser"],
            ))
            .await;
    })
    .await;

    let turns = recorded(&log);
    assert_eq!(turns.len(), 2, "a queued steer must chain a second turn");
    assert_eq!(
        shape(&turns[1]),
        vec![
            (LlmRole::System, "be helpful".to_string()),
            (LlmRole::User, "original prompt".to_string()),
            (LlmRole::Assistant, "first answer".to_string()),
        ],
        "PA must never seed the steer into the chained turn's history"
    );
    assert_eq!(
        delivered_ids(&events),
        vec![steer_id],
        "the chained turn's boundary must deliver the leftover steer, got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![
            (MessageRole::Assistant, "first answer".to_string()),
            (MessageRole::User, "focus on the parser".to_string()),
            (MessageRole::Assistant, "second answer".to_string()),
        ],
        "the delivered steer must sit between the two assistant outputs"
    );
    assert_eq!(
        completion_count(&events),
        1,
        "the chained send must finalize once, got {events:?}"
    );
}

/// Two steers queued behind a text-only turn are delivered in submission
/// order at the chained turn's boundary, and neither is seeded into the
/// turn's history by PA.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-SI-002
/// @requirement REQ-SI-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_queued_steers_are_delivered_in_fifo_order() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let first_id = fixture.steer("first steer").await;
    let second_id = fixture.steer("second steer").await;
    let log = new_turn_log();
    let sink = shared_sink(sink_for(&fixture));

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture
            .run_turns(boundary_runner(
                &log,
                &["first answer", "second answer"],
                sink,
                2,
                &["first steer", "second steer"],
            ))
            .await;
    })
    .await;

    let turns = recorded(&log);
    assert_eq!(
        turns.len(),
        2,
        "queued steers must chain one follow-up turn"
    );
    assert_eq!(
        shape(&turns[1]),
        vec![
            (LlmRole::System, "be helpful".to_string()),
            (LlmRole::User, "original prompt".to_string()),
            (LlmRole::Assistant, "first answer".to_string()),
        ],
        "the queued steers must be delivered by the transport, never seeded by PA"
    );
    assert_eq!(
        delivered_ids(&events),
        vec![first_id, second_id],
        "both steers must resolve their own entries, in FIFO order, got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![
            (MessageRole::Assistant, "first answer".to_string()),
            (MessageRole::User, "first steer".to_string()),
            (MessageRole::User, "second steer".to_string()),
            (MessageRole::Assistant, "second answer".to_string()),
        ],
        "persistence must record the steers in FIFO order too"
    );
}

/// A cancelled turn never reaches a delivery boundary: it drains nothing and
/// chains nothing, even though it finished cleanly.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-006
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_turn_drains_nothing_and_chains_no_follow_up() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    fixture.steer("too late").await;
    let log = new_turn_log();
    let log_for_runner = log.clone();
    let cancel = fixture.cancel.clone();

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture
            .run_turns(move |messages: Vec<LlmMessage>| {
                log_for_runner
                    .lock()
                    .expect("turn log poisoned")
                    .push(messages);
                // The user stopped the turn while it was finishing, so the
                // transcript is clean but the token has fired.
                cancel.cancel();
                Box::pin(std::future::ready(completed_turn("interrupted answer"))) as TurnFuture
            })
            .await;
    })
    .await;

    assert_eq!(
        recorded(&log).len(),
        1,
        "a cancelled turn must not chain a follow-up"
    );
    assert!(
        delivered_ids(&events).is_empty(),
        "a cancelled turn must deliver no steering, got {events:?}"
    );
    assert!(
        !persisted_shape(&fixture.conversations)
            .await
            .iter()
            .any(|(role, content)| *role == MessageRole::User && content == "too late"),
        "a cancelled turn must not persist the queued steer"
    );
}

/// A failed turn drains nothing and chains nothing; its partial output is
/// still finalized as interrupted.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-006
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_turn_drains_nothing_and_chains_no_follow_up() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    fixture.steer("never delivered").await;
    let log = new_turn_log();
    let log_for_runner = log.clone();

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture
            .run_turns(move |messages: Vec<LlmMessage>| {
                log_for_runner
                    .lock()
                    .expect("turn log poisoned")
                    .push(messages);
                Box::pin(std::future::ready(streaming::StreamTranscript {
                    response_text: "partial answer".to_string(),
                    error: Some("provider reset the connection".to_string()),
                    ..streaming::StreamTranscript::default()
                })) as TurnFuture
            })
            .await;
    })
    .await;

    assert_eq!(
        recorded(&log).len(),
        1,
        "a failed turn must not chain a follow-up"
    );
    assert!(
        delivered_ids(&events).is_empty(),
        "a failed turn must deliver no steering, got {events:?}"
    );
    assert_eq!(
        completion_count(&events),
        0,
        "a failed turn must not emit StreamCompleted, got {events:?}"
    );
    let persisted = fixture.conversations.messages.read().await.clone();
    assert_eq!(
        persisted.len(),
        1,
        "a failed turn must persist only its own partial output"
    );
    assert!(
        persisted[0].interrupted,
        "a failed turn's output must carry the interrupted marker"
    );
}

/// The steering user message lands between the assistant output that
/// preceded it and the assistant output it produced, so a reload replays the
/// turn in the order the user experienced it.
///
/// The chained turn's boundary delivers the steer through the sink, the loop
/// persists the finished turn's output before it, and finalization persists
/// the last turn's output after it.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-222-007
/// @requirement REQ-SI-002
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steering_message_persists_between_the_assistant_outputs() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    fixture.steer("adjust course").await;
    let log = new_turn_log();
    let sink = shared_sink(sink_for(&fixture));
    let log_for_runner = log.clone();

    fixture
        .run_turns(move |messages: Vec<LlmMessage>| {
            let index = {
                let mut turns = log_for_runner.lock().expect("turn log poisoned");
                turns.push(messages);
                turns.len() - 1
            };
            let sink = Arc::clone(&sink);
            Box::pin(async move {
                if index == 0 {
                    streaming::StreamTranscript {
                        response_text: "before the steer".to_string(),
                        thinking_text: "weighing options".to_string(),
                        tool_calls: vec![crate::llm::tools::ToolUse::new(
                            "call-1",
                            "web_search",
                            serde_json::json!({"query": "rust"}),
                        )],
                        tool_results: vec![crate::llm::tools::ToolResult::success("call-1", "ok")],
                        completed: true,
                        ..streaming::StreamTranscript::default()
                    }
                } else {
                    // The chained turn's boundary delivers the leftover steer
                    // through the sink, exactly as the transport would.
                    sink.lock()
                        .await
                        .delivered("adjust course")
                        .await
                        .expect("the boundary delivery must succeed");
                    completed_turn("after the steer")
                }
            }) as TurnFuture
        })
        .await;

    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![
            (MessageRole::Assistant, "before the steer".to_string()),
            (MessageRole::User, "adjust course".to_string()),
            (MessageRole::Assistant, "after the steer".to_string()),
        ],
        "the steer must be ordered after the preceding output and before the following one"
    );

    let persisted = fixture.conversations.messages.read().await.clone();
    assert!(
        persisted[0].tool_calls.is_some() && persisted[0].tool_results.is_some(),
        "the preceding turn's tool transcript must survive the chained turn"
    );

    let turns = recorded(&log);
    let second = &turns[1];
    assert!(
        !shape(second)
            .iter()
            .any(|(_, content)| content == "adjust course"),
        "the steer must be delivered by the transport, never seeded by PA"
    );
    let preceding = second.last().expect("the chained seed is never empty");
    assert_eq!(
        preceding.thinking_content.as_deref(),
        Some("weighing options"),
        "the chained history must carry the finished turn's thinking"
    );
    assert_eq!(
        preceding.tool_uses.len(),
        1,
        "the chained history must carry the finished turn's tool calls"
    );
    assert_eq!(
        preceding.tool_results.len(),
        1,
        "the chained history must carry the finished turn's tool results"
    );
}

/// A chained send completes once, not once per turn.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_completed_is_emitted_once_across_a_chained_send() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let log = new_turn_log();
    let sink = shared_sink(sink_for(&fixture));

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture.steer("keep going").await;
        fixture
            .run_turns(boundary_runner(
                &log,
                &["first answer", "second answer"],
                sink,
                2,
                &["keep going"],
            ))
            .await;
    })
    .await;

    assert_eq!(recorded(&log).len(), 2, "the send must run two turns");
    assert_eq!(
        completion_count(&events),
        1,
        "a chained send must emit exactly one StreamCompleted, got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![
            (MessageRole::Assistant, "first answer".to_string()),
            (MessageRole::User, "keep going".to_string()),
            (MessageRole::Assistant, "second answer".to_string()),
        ],
        "each turn's output must be persisted exactly once"
    );
}

/// A user who keeps steering cannot chain turns forever: the send stops at
/// `MAX_STEERING_TURNS`.
///
/// Every turn delivers the steer queued before it at its boundary — what the
/// transport does — and then the user steers again through the real service
/// path, which only succeeds while the conversation still holds its stream
/// slot. So this also pins that the slot survives the chain, and that only
/// the entry still queued at the cap is announced discarded.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-222-005
/// @requirement REQ-SI-006
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chain_stops_at_the_turn_cap_when_steering_keeps_refilling() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    fixture.steer("steer 0").await;
    let log = new_turn_log();
    let log_for_runner = log.clone();
    let service = fixture.service.clone();
    let conversation_id = fixture.conversation_id;
    let sink = shared_sink(sink_for(&fixture));

    let ((), events) = events_during(conversation_id, async {
        fixture
            .run_turns(move |messages: Vec<LlmMessage>| {
                let index = {
                    let mut turns = log_for_runner.lock().expect("turn log poisoned");
                    turns.push(messages);
                    turns.len()
                };
                let service = service.clone();
                let sink = Arc::clone(&sink);
                Box::pin(async move {
                    // The boundary delivers the steer the previous turn left
                    // queued, then the user steers again mid-turn.
                    sink.lock()
                        .await
                        .delivered("the steer the previous turn left queued")
                        .await
                        .expect("the boundary delivery must succeed");
                    ChatService::steer(service.as_ref(), conversation_id, format!("steer {index}"))
                        .await
                        .expect("the conversation must still be steerable mid-chain");
                    completed_turn("answer")
                }) as TurnFuture
            })
            .await;
    })
    .await;

    assert_eq!(
        recorded(&log).len(),
        MAX_STEERING_TURNS,
        "a refilling queue must stop the chain at the turn cap"
    );
    assert_eq!(
        completion_count(&events),
        1,
        "a capped chain must still emit exactly one StreamCompleted, got {events:?}"
    );
    assert_eq!(
        discarded_ids(&events).len(),
        1,
        "only the entry still queued at the cap must be announced discarded, got {events:?}"
    );
}
