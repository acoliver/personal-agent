//! Delivery of queued steering messages at the turn boundary.
//!
//! Covers issue #222: when a turn finishes with steering messages waiting,
//! the send chains another turn seeded with the conversation so far plus the
//! steering text. A cancelled or failed turn chains nothing, finalization
//! still happens exactly once, and the chain is bounded.
//!
//! The turn itself is the only double here: each test hands the delivery loop
//! a runner that records the history it was seeded with and replies with a
//! scripted transcript, which is the same level the approval-gate tests in
//! `three_stream_concurrency.rs` operate at.
//!
//! @plan PLAN-20260903-ISSUE222.P02
//! @requirement REQ-222-005
//! @requirement REQ-222-006
//! @requirement REQ-222-007
//! @requirement REQ-222-008

use super::*;
use crate::events::subscribe;
use crate::events::types::ChatEvent;
use crate::llm::Role as LlmRole;
use crate::services::chat_impl::streaming::steering_delivery::{
    run_steered_turns_and_finalize, SteeringDeliveryContext, MAX_STEERING_TURNS,
};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
use tokio::time::Duration;

mod discard;

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

/// An empty queue must leave a send exactly as it is today: one turn, one
/// completion, and the assistant output persisted once.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_steering_queue_runs_a_single_turn_and_finalizes_once() {
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

/// A queued steer chains a second turn whose history carries the steer as a
/// user message after the first turn's assistant output.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_steer_chains_a_second_turn_carrying_the_steer_text() {
    let fixture = DeliveryFixture::new();
    fixture.steer("focus on the parser").await;
    let log = new_turn_log();

    fixture
        .run_turns(scripted_runner(&log, &["first answer", "second answer"]))
        .await;

    let turns = recorded(&log);
    assert_eq!(turns.len(), 2, "a queued steer must chain a second turn");
    assert_eq!(
        shape(&turns[1]),
        vec![
            (LlmRole::System, "be helpful".to_string()),
            (LlmRole::User, "original prompt".to_string()),
            (LlmRole::Assistant, "first answer".to_string()),
            (LlmRole::User, "focus on the parser".to_string()),
        ],
        "the second turn must be seeded with the finished turn plus the steer"
    );
}

/// Two steers queued against the same boundary arrive in submission order.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_queued_steers_are_delivered_in_fifo_order() {
    let fixture = DeliveryFixture::new();
    fixture.steer("first steer").await;
    fixture.steer("second steer").await;
    let log = new_turn_log();

    fixture
        .run_turns(scripted_runner(&log, &["first answer", "second answer"]))
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
            (LlmRole::User, "first steer".to_string()),
            (LlmRole::User, "second steer".to_string()),
        ],
        "both steers must be seeded in FIFO order"
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

/// Every delivered steer announces itself once, under the id its
/// `SteeringQueued` reported.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steering_delivered_is_emitted_once_per_steer_with_the_queued_id() {
    let fixture = DeliveryFixture::new();
    let log = new_turn_log();

    let ((first_id, second_id), events) = events_during(fixture.conversation_id, async {
        let first_id = fixture.steer("first steer").await;
        let second_id = fixture.steer("second steer").await;
        fixture
            .run_turns(scripted_runner(&log, &["first answer", "second answer"]))
            .await;
        (first_id, second_id)
    })
    .await;

    assert_eq!(
        delivered_ids(&events),
        vec![first_id, second_id],
        "each steer must be delivered once, under the id steer() returned, got {events:?}"
    );
}

/// A cancelled turn never reaches a delivery boundary: it drains nothing and
/// chains nothing, even though it finished cleanly.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-006
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_turn_drains_nothing_and_chains_no_follow_up() {
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
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-007
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steering_message_persists_between_the_assistant_outputs() {
    let fixture = DeliveryFixture::new();
    fixture.steer("adjust course").await;
    let log = new_turn_log();
    let log_for_runner = log.clone();

    fixture
        .run_turns(move |messages: Vec<LlmMessage>| {
            let index = {
                let mut turns = log_for_runner.lock().expect("turn log poisoned");
                turns.push(messages);
                turns.len() - 1
            };
            let transcript = if index == 0 {
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
                completed_turn("after the steer")
            };
            Box::pin(std::future::ready(transcript)) as TurnFuture
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
    let preceding = &second[second.len() - 2];
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
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_completed_is_emitted_once_across_a_chained_send() {
    let fixture = DeliveryFixture::new();
    let log = new_turn_log();

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture.steer("keep going").await;
        fixture
            .run_turns(scripted_runner(&log, &["first answer", "second answer"]))
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
/// Every follow-up turn steers again through the real service path, which
/// only succeeds while the conversation still holds its stream slot — so this
/// also pins that the slot survives the chain.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-005
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chain_stops_at_the_turn_cap_when_steering_keeps_refilling() {
    let fixture = DeliveryFixture::new();
    fixture.steer("steer 0").await;
    let log = new_turn_log();
    let log_for_runner = log.clone();
    let service = fixture.service.clone();
    let conversation_id = fixture.conversation_id;

    let ((), events) = events_during(conversation_id, async {
        fixture
            .run_turns(move |messages: Vec<LlmMessage>| {
                let index = {
                    let mut turns = log_for_runner.lock().expect("turn log poisoned");
                    turns.push(messages);
                    turns.len()
                };
                let service = service.clone();
                Box::pin(async move {
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
}

/// A turn that stops at a real tool approval, and what it was told.
///
/// The first turn takes the waiter, announces that it has reached the gate,
/// and blocks until someone decides. `decided` therefore distinguishes "the
/// steer resolved the approval" from "the user did".
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-008
struct ApprovalProbe {
    waiter: StdMutex<Option<crate::llm::client_agent::ApprovalWaiter>>,
    reached: tokio::sync::Notify,
    decided: AtomicBool,
    approved: AtomicBool,
}

impl ApprovalProbe {
    fn new(waiter: crate::llm::client_agent::ApprovalWaiter) -> Arc<Self> {
        Arc::new(Self {
            waiter: StdMutex::new(Some(waiter)),
            reached: tokio::sync::Notify::new(),
            decided: AtomicBool::new(false),
            approved: AtomicBool::new(false),
        })
    }

    /// A turn runner whose first turn blocks on the approval decision.
    fn runner(self: &Arc<Self>, log: &TurnLog) -> impl FnMut(Vec<LlmMessage>) -> TurnFuture {
        let probe = self.clone();
        let log = log.clone();
        move |messages: Vec<LlmMessage>| {
            let index = {
                let mut turns = log.lock().expect("turn log poisoned");
                turns.push(messages);
                turns.len() - 1
            };
            let probe = probe.clone();
            Box::pin(async move {
                if index == 0 {
                    probe.await_decision().await;
                    completed_turn("ran the tool")
                } else {
                    completed_turn("followed the steer")
                }
            }) as TurnFuture
        }
    }

    async fn await_decision(&self) {
        let pending = self
            .waiter
            .lock()
            .expect("waiter slot poisoned")
            .take()
            .expect("the first turn owns the approval waiter");
        self.reached.notify_one();
        let decision = pending.wait().await.expect("the approval must resolve");
        self.approved.store(decision, Ordering::SeqCst);
        self.decided.store(true, Ordering::SeqCst);
    }
}

/// A steer submitted while a tool approval is pending neither resolves nor
/// cancels that approval, and lands only after the decision lets the turn
/// reach its boundary.
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @requirement REQ-222-008
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steer_during_a_pending_approval_waits_for_the_decision() {
    let fixture = DeliveryFixture::new();
    let log = new_turn_log();
    let request_id = Uuid::new_v4().to_string();
    let probe = ApprovalProbe::new(fixture.service.approval_gate.wait_for_approval(
        request_id.clone(),
        "WriteFile".to_string(),
        fixture.conversation_id,
    ));
    let steer_id = StdMutex::new(None);

    let (((), ()), events) = events_during(fixture.conversation_id, async {
        tokio::join!(fixture.run_turns(probe.runner(&log)), async {
            probe.reached.notified().await;
            *steer_id.lock().expect("steer slot poisoned") =
                Some(fixture.steer("write the other file instead").await);

            // Let the waiting turn observe a decision it must not have been
            // given.
            tokio::task::yield_now().await;
            assert!(
                !probe.decided.load(Ordering::SeqCst),
                "a steer must not resolve a pending tool approval"
            );

            fixture
                .service
                .resolve_tool_approval(request_id.clone(), ToolApprovalResponseAction::ProceedOnce)
                .await
                .expect("resolving the approval must succeed");
        })
    })
    .await;

    assert!(
        probe.approved.load(Ordering::SeqCst),
        "the approval must resolve from the user's decision, approved"
    );
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ChatEvent::StreamCancelled { .. })),
        "a steer must not cancel the turn holding the approval, got {events:?}"
    );

    let turns = recorded(&log);
    assert_eq!(
        turns.len(),
        2,
        "the steer must be delivered after the approved turn reaches its boundary"
    );
    assert_eq!(
        shape(&turns[1]).last().cloned(),
        Some((LlmRole::User, "write the other file instead".to_string())),
        "the chained turn must carry the steer queued during the approval"
    );
    assert_eq!(
        delivered_ids(&events),
        vec![steer_id
            .lock()
            .expect("steer slot poisoned")
            .expect("the driver must have queued a steer")],
        "the steer must be announced as delivered exactly once, got {events:?}"
    );
}
