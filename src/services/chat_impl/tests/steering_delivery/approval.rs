//! A steer accepted while a tool approval is pending, and the chained
//! turn that delivers it once the turn reaches its boundary.
//!
//! @plan PLAN-20260905-STEERINT.P04
//! @requirement REQ-222-008
//! @requirement REQ-SI-002

use super::*;

/// A turn that stops at a real tool approval, and what it was told.
///
/// The first turn takes the waiter, announces that it has reached the gate,
/// and blocks until someone decides. `decided` therefore distinguishes "the
/// steer resolved the approval" from "the user did".
///
/// @plan PLAN-20260903-ISSUE222.P02
/// @plan PLAN-20260905-STEERINT.P04
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

    /// A turn runner whose first turn blocks on the approval decision, and
    /// whose chained turn delivers the queued steer at its boundary.
    fn runner(
        self: &Arc<Self>,
        log: &TurnLog,
        boundary_delivery: Option<(SharedSink, &'static str)>,
    ) -> impl FnMut(Vec<LlmMessage>) -> TurnFuture {
        let probe = self.clone();
        let log = log.clone();
        move |messages: Vec<LlmMessage>| {
            let index = {
                let mut turns = log.lock().expect("turn log poisoned");
                turns.push(messages);
                turns.len() - 1
            };
            let probe = probe.clone();
            let boundary_delivery = boundary_delivery.clone();
            Box::pin(async move {
                if index == 0 {
                    probe.await_decision().await;
                    completed_turn("ran the tool")
                } else {
                    if let Some((sink, text)) = &boundary_delivery {
                        sink.lock()
                            .await
                            .delivered(text)
                            .await
                            .expect("the boundary delivery must succeed");
                    }
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
/// @plan PLAN-20260905-STEERINT.P04
/// @requirement REQ-222-008
/// @requirement REQ-SI-002
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn steer_during_a_pending_approval_waits_for_the_decision() {
    let _steering_bus_guard = lock_steering_bus().await;
    let fixture = DeliveryFixture::new();
    let log = new_turn_log();
    let request_id = Uuid::new_v4().to_string();
    let probe = ApprovalProbe::new(fixture.service.approval_gate.wait_for_approval(
        request_id.clone(),
        "WriteFile".to_string(),
        fixture.conversation_id,
    ));
    let steer_id = StdMutex::new(None);
    let sink = shared_sink(sink_for(&fixture));

    let (((), ()), events) = events_during(fixture.conversation_id, async {
        tokio::join!(
            fixture.run_turns(probe.runner(&log, Some((sink, "write the other file instead")),)),
            async {
                probe.reached.notified().await;
                *steer_id.lock().expect("steer slot poisoned") =
                    Some(fixture.steer("write the other file instead").await);

                // Let the waiting turn observe a decision it must not have
                // been given.
                tokio::task::yield_now().await;
                assert!(
                    !probe.decided.load(Ordering::SeqCst),
                    "a steer must not resolve a pending tool approval"
                );

                fixture
                    .service
                    .resolve_tool_approval(
                        request_id.clone(),
                        ToolApprovalResponseAction::ProceedOnce,
                    )
                    .await
                    .expect("resolving the approval must succeed");
            }
        )
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
    assert!(
        !shape(&turns[1])
            .iter()
            .any(|(_, content)| content == "write the other file instead"),
        "the steer queued during the approval must be delivered by the transport, \
         never seeded by PA"
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
