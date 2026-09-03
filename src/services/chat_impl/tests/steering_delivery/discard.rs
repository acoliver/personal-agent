//! The terminal state of a queued steering message that is never delivered.
//!
//! A steer the service accepted is rendered as a waiting entry, and the view
//! only stops rendering it when something says what became of it. Delivery
//! says that. So must every path that ends a turn with entries still queued,
//! or the user is left watching an instruction that will never be acted on.
//!
//! Two of those paths are driven here: a steer accepted in the window between
//! the delivery loop's last drain and the release of the stream slot, and a
//! steer whose own persistence fails. Both are driven from inside a
//! conversation-service write, so the window is a fact of the call order
//! rather than something a sleep hopes to hit.
//!
//! @plan PLAN-20260903-ISSUE222.P06
//! @requirement REQ-222-003
//! @requirement REQ-222-007

use super::*;
use std::sync::{OnceLock, Weak};

/// A steer submitted from inside a write the turn makes while tearing down.
///
/// Holds the service weakly: the service owns the conversation service that
/// owns this, so an owning handle would close a cycle that never drops.
struct LateSteer {
    service: OnceLock<Weak<ChatServiceImpl>>,
    conversation_id: OnceLock<Uuid>,
    text: &'static str,
    steer_id: StdMutex<Option<Uuid>>,
}

impl LateSteer {
    fn new(text: &'static str) -> Arc<Self> {
        Arc::new(Self {
            service: OnceLock::new(),
            conversation_id: OnceLock::new(),
            text,
            steer_id: StdMutex::new(None),
        })
    }

    /// Point this at the fixture's service, once the fixture exists.
    fn attach(&self, fixture: &DeliveryFixture) {
        self.service
            .set(Arc::downgrade(&fixture.service))
            .unwrap_or_else(|_| panic!("the late steer must be attached exactly once"));
        self.conversation_id
            .set(fixture.conversation_id)
            .expect("the late steer must be attached exactly once");
    }

    /// Steer the running turn through the real service path, recording the
    /// id it was accepted under.
    ///
    /// Going through `ChatService::steer` rather than the queue directly is
    /// the point: it proves the conversation still passes `is_streaming_for`
    /// at this moment, which is what makes the window reachable in
    /// production.
    async fn fire(&self) {
        let service = self
            .service
            .get()
            .expect("the late steer must be attached before the turn runs")
            .upgrade()
            .expect("the fixture keeps the service alive for the whole turn");
        let conversation_id = *self
            .conversation_id
            .get()
            .expect("the late steer must be attached before the turn runs");

        let steer_id = ChatService::steer(service.as_ref(), conversation_id, self.text.to_string())
            .await
            .expect("the turn is still Running here, so the steer must be accepted");
        *self.steer_id.lock().expect("steer slot poisoned") = Some(steer_id);
    }

    fn steer_id(&self) -> Option<Uuid> {
        *self.steer_id.lock().expect("steer slot poisoned")
    }
}

/// What the interposed conversation service does to the turn writing through
/// it.
enum Interference {
    /// Refuse to persist user messages. Assistant output still lands, so the
    /// chain is intact right up to the steering message that cannot be
    /// recorded.
    RefuseUserMessages,
    /// Steer the conversation from inside `update_context_state`, which
    /// finalization calls after the delivery loop's final drain and before it
    /// releases the stream slot.
    SteerDuringTeardown(Arc<LateSteer>),
}

/// A conversation service that delegates to the mock and interferes with one
/// write on the way through.
struct InterferingConversationService {
    inner: Arc<MockConversationService>,
    interference: Interference,
}

impl InterferingConversationService {
    /// Wrap `inner`, ready to hand to a fixture as its conversation service.
    fn interposed(
        inner: Arc<MockConversationService>,
        interference: Interference,
    ) -> Arc<dyn crate::services::ConversationService> {
        Arc::new(Self {
            inner,
            interference,
        })
    }
}

#[async_trait::async_trait]
impl crate::services::ConversationService for InterferingConversationService {
    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> Result<crate::models::Conversation, ServiceError> {
        self.inner.create(title, model_profile_id).await
    }

    async fn load(&self, id: Uuid) -> Result<crate::models::Conversation, ServiceError> {
        self.inner.load(id).await
    }

    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<crate::models::ConversationMetadata>, ServiceError> {
        self.inner.list_metadata(limit, offset).await
    }

    async fn add_message(
        &self,
        conversation_id: Uuid,
        message: Message,
    ) -> Result<Message, ServiceError> {
        if matches!(self.interference, Interference::RefuseUserMessages)
            && message.role == MessageRole::User
        {
            return Err(ServiceError::Storage(
                "simulated steering persistence failure".to_string(),
            ));
        }
        self.inner.add_message(conversation_id, message).await
    }

    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<crate::models::SearchResult>, ServiceError> {
        self.inner.search(query, limit, offset).await
    }

    async fn message_count(&self, conversation_id: Uuid) -> Result<usize, ServiceError> {
        self.inner.message_count(conversation_id).await
    }

    async fn update_context_state(
        &self,
        id: Uuid,
        state: &crate::models::ContextState,
    ) -> Result<(), ServiceError> {
        if let Interference::SteerDuringTeardown(late) = &self.interference {
            late.fire().await;
        }
        self.inner.update_context_state(id, state).await
    }

    async fn get_context_state(
        &self,
        id: Uuid,
    ) -> Result<Option<crate::models::ContextState>, ServiceError> {
        self.inner.get_context_state(id).await
    }

    async fn rename(&self, id: Uuid, new_title: String) -> Result<(), ServiceError> {
        self.inner.rename(id, new_title).await
    }

    async fn delete(&self, id: Uuid) -> Result<(), ServiceError> {
        self.inner.delete(id).await
    }

    async fn set_active(&self, id: Uuid) -> Result<(), ServiceError> {
        self.inner.set_active(id).await
    }

    async fn get_active(&self) -> Result<Option<Uuid>, ServiceError> {
        self.inner.get_active().await
    }

    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>, ServiceError> {
        self.inner.get_messages(conversation_id).await
    }

    async fn update(
        &self,
        id: Uuid,
        title: Option<String>,
        model_profile_id: Option<Uuid>,
    ) -> Result<crate::models::Conversation, ServiceError> {
        self.inner.update(id, title, model_profile_id).await
    }
}

/// A steer accepted after the delivery loop's last drain, while the turn is
/// still `Running`, is announced as discarded rather than dropped.
///
/// The steer is submitted from inside `update_context_state`: finalization
/// calls that after the final drain and before it releases the stream slot,
/// which is precisely the window the entry used to vanish in. The service
/// accepts it — `SteeringQueued` proves the view was told to render it — and
/// then nothing is ever going to deliver it, so something has to say so.
///
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-003
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steer_accepted_during_teardown_is_announced_as_discarded() {
    let conversations = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let late = LateSteer::new("too late to be delivered");
    let fixture = DeliveryFixture::with_conversation_service(
        conversations.clone(),
        InterferingConversationService::interposed(
            conversations,
            Interference::SteerDuringTeardown(late.clone()),
        ),
    );
    late.attach(&fixture);
    let log = new_turn_log();

    let ((), events) = events_during(fixture.conversation_id, async {
        fixture
            .run_turns(scripted_runner(&log, &["only answer"]))
            .await;
    })
    .await;

    let steer_id = late
        .steer_id()
        .expect("the teardown write must have queued a steer");

    assert_eq!(
        recorded(&log).len(),
        1,
        "a steer that arrives during teardown must not chain a turn"
    );
    assert_eq!(
        queued_ids(&events),
        vec![steer_id],
        "the late steer must be accepted and announced as queued, so the view renders it, \
         got {events:?}"
    );
    assert_eq!(
        discarded_ids(&events),
        vec![steer_id],
        "a steer accepted after the last drain must be announced as discarded, got {events:?}"
    );
    assert!(
        delivered_ids(&events).is_empty(),
        "a steer no turn ever handed to the model must not be reported delivered, got {events:?}"
    );
    assert!(
        !fixture.is_streaming(),
        "finalization must still release the conversation's stream slot"
    );
}

/// A steering message the store refuses is not chained over.
///
/// Persisting the steer is what keeps the chained history equal to what a
/// reload rebuilds. When that write fails, seeding the model with the text
/// anyway would run a turn over a conversation the database does not have, so
/// the chain stops here. The entry is announced as discarded, not delivered:
/// the view withdraws it either way, and reporting delivery would claim the
/// instruction reached the model when it did not.
///
/// @plan PLAN-20260903-ISSUE222.P06
/// @requirement REQ-222-003
/// @requirement REQ-222-007
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steer_that_cannot_be_persisted_is_discarded_and_chains_nothing() {
    let conversations = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let fixture = DeliveryFixture::with_conversation_service(
        conversations.clone(),
        InterferingConversationService::interposed(conversations, Interference::RefuseUserMessages),
    );
    let log = new_turn_log();

    // One scripted reply: a follow-up turn would run out and panic, so
    // "chains nothing" is enforced by the runner as well as asserted.
    let ((first, second), events) = events_during(fixture.conversation_id, async {
        let first = fixture.steer("first steer").await;
        let second = fixture.steer("second steer").await;
        fixture
            .run_turns(scripted_runner(&log, &["only answer"]))
            .await;
        (first, second)
    })
    .await;

    assert_eq!(
        recorded(&log).len(),
        1,
        "a steer the store refused must not seed a follow-up turn"
    );
    assert!(
        delivered_ids(&events).is_empty(),
        "a steer that was never recorded must not be reported delivered, got {events:?}"
    );
    assert_eq!(
        discarded_ids(&events),
        vec![first, second],
        "the refused steer and every entry behind it must be announced as discarded, \
         got {events:?}"
    );
    assert_eq!(
        persisted_shape(&fixture.conversations).await,
        vec![(MessageRole::Assistant, "only answer".to_string())],
        "only the assistant output the turn actually produced may be recorded, and only once"
    );
    assert_eq!(
        completion_count(&events),
        1,
        "the turn itself finished, so the send must still finalize once, got {events:?}"
    );
    assert!(
        !fixture.is_streaming(),
        "finalization must still release the conversation's stream slot"
    );
}
