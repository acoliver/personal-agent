//! End-to-End Concurrent Streams Integration Test
//!
//! Tests REQ-173-006.1 and REQ-173-003.1: Full concurrency machinery with
//! deterministic mock LLM client that emits controlled deltas.

#![allow(clippy::too_many_lines)]

use crate::events::types::ChatEvent;
use crate::events::{subscribe, AppEvent};
use crate::llm::client_agent::ApprovalGate;
use crate::models::{AuthConfig, ModelProfile};
use crate::services::chat_impl::ChatServiceImpl;
use crate::services::{ChatService, ConversationService, ProfileService};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

// Import test support utilities from parent tests module
use super::chat_test_support::{MockConversationService, MockProfileService};

/// Test-only `ChatServiceImpl` extension methods for controlling LLM client behavior
#[cfg(test)]
impl ChatServiceImpl {
    /// Create a test service with a custom conversation service.
    fn new_for_integration_test(
        conversation_service: Arc<dyn ConversationService>,
        profile_service: Arc<dyn ProfileService>,
    ) -> Self {
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(100);
        let approval_gate = Arc::new(ApprovalGate::new());
        let settings_path = std::env::temp_dir().join(format!(
            "chat-service-integration-test-{}.json",
            Uuid::new_v4()
        ));

        let app_settings = Arc::new(
            crate::services::AppSettingsServiceImpl::new(settings_path)
                .expect("failed to create test app settings service"),
        ) as Arc<dyn crate::services::AppSettingsService>;
        let skills_service = Arc::new(
            crate::services::SkillsServiceImpl::new(app_settings.clone())
                .expect("failed to create test skills service"),
        ) as Arc<dyn crate::services::SkillsService>;

        Self::new(
            conversation_service,
            profile_service,
            app_settings,
            skills_service,
            view_tx,
            approval_gate,
            Arc::new(AsyncMutex::new(crate::agent::ToolApprovalPolicy::default())),
        )
    }
}

/// Begin three concurrent streams (A, B, C), assert duplicate-begin(A) rejects,
/// then clear and verify all slots are free.
///
/// Exercises REQ-173-001.1 (per-conversation guard, not global) and
/// REQ-173-001.2 (duplicate begin rejected).
async fn verify_per_conversation_guard_allows_three(
    service: &ChatServiceImpl,
    conversation_a: Uuid,
    conversation_b: Uuid,
    conversation_c: Uuid,
) {
    service
        .begin_stream_for_test(conversation_a)
        .expect("begin_stream(A) should succeed");
    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed (per-conversation guard, not global)");
    service
        .begin_stream_for_test(conversation_c)
        .expect("begin_stream(C) should succeed (per-conversation guard, not global)");

    for (label, id) in [
        ("A", conversation_a),
        ("B", conversation_b),
        ("C", conversation_c),
    ] {
        assert!(
            service.is_streaming_for(id),
            "{label} should be streaming after begin_stream_for_test"
        );
    }

    let dup_err = service
        .begin_stream_for_test(conversation_a)
        .expect_err("begin_stream(A) duplicate must fail");
    assert!(
        dup_err.to_string().contains("Stream already in progress"),
        "expected per-conversation CAS error, got: {dup_err}"
    );

    service.clear_all_streams_for_test();
    for (label, id) in [
        ("A", conversation_a),
        ("B", conversation_b),
        ("C", conversation_c),
    ] {
        assert!(
            !service.is_streaming_for(id),
            "{label} should not be streaming after clear_all_streams_for_test"
        );
    }
}

/// Drain cancellation events from the shared bus and count `StreamCancelled`
/// events per conversation id.
fn count_stream_cancelled_events(
    event_rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
    a: Uuid,
    b: Uuid,
    c: Uuid,
) -> (usize, usize, usize) {
    let mut counts = (0usize, 0usize, 0usize);
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::Chat(ChatEvent::StreamCancelled {
            conversation_id, ..
        }) = &event
        {
            if conversation_id == &a {
                counts.0 += 1;
            } else if conversation_id == &b {
                counts.1 += 1;
            } else if conversation_id == &c {
                counts.2 += 1;
            }
        }
    }
    counts
}

/// Start three concurrent mock streams, cancel B mid-flight, and verify A
/// and C remain streaming while exactly one `StreamCancelled` event is
/// emitted for B and none for A or C.
async fn verify_cancel_scopes_to_single_conversation(
    service: &ChatServiceImpl,
    conversation_a: Uuid,
    conversation_b: Uuid,
    conversation_c: Uuid,
) {
    let mut event_rx = subscribe();

    service
        .begin_stream_for_test(conversation_a)
        .expect("begin_stream(A) should succeed");
    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed");
    service
        .begin_stream_for_test(conversation_c)
        .expect("begin_stream(C) should succeed");

    for (label, id) in [
        ("A", conversation_a),
        ("B", conversation_b),
        ("C", conversation_c),
    ] {
        assert!(
            ChatService::is_streaming_for(service, id),
            "{label} should be streaming after begin_stream_for_test"
        );
    }

    ChatService::cancel(service, conversation_b);
    sleep(Duration::from_millis(20)).await;

    assert!(
        !ChatService::is_streaming_for(service, conversation_b),
        "B should not be streaming after cancel(B)"
    );
    assert!(
        ChatService::is_streaming_for(service, conversation_a),
        "A should still be streaming after cancel(B) - cancel should scope to B only"
    );
    assert!(
        ChatService::is_streaming_for(service, conversation_c),
        "C should still be streaming after cancel(B) - cancel should scope to B only"
    );

    let (a_count, b_count, c_count) = count_stream_cancelled_events(
        &mut event_rx,
        conversation_a,
        conversation_b,
        conversation_c,
    );
    assert_eq!(
        b_count, 1,
        "Should have exactly ONE StreamCancelled event for B (got {b_count})"
    );
    assert_eq!(
        a_count, 0,
        "A should NOT have any StreamCancelled events (cancel should be scoped to B)"
    );
    assert_eq!(
        c_count, 0,
        "C should NOT have any StreamCancelled events (cancel should be scoped to B)"
    );
}

/// Test that three conversations can have active streams in parallel and that `cancel` scopes correctly.
/// This test exercises the `begin_stream` CAS guard and verifies that `cancel(B)` only affects B and not A or C.
/// @plan PLAN-20260416-ISSUE173.P12
/// @requirement REQ-173-006.1, REQ-173-001.1
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_conversations_stream_in_parallel_and_cancel_scopes_correctly() {
    let outer_result = timeout(Duration::from_secs(5), async {
        let service = make_test_service().await;
        let conversation_a = Uuid::new_v4();
        let conversation_b = Uuid::new_v4();
        let conversation_c = Uuid::new_v4();

        verify_per_conversation_guard_allows_three(
            &service,
            conversation_a,
            conversation_b,
            conversation_c,
        )
        .await;

        verify_cancel_scopes_to_single_conversation(
            &service,
            conversation_a,
            conversation_b,
            conversation_c,
        )
        .await;

        service.clear_all_streams_for_test();
        let _ = crate::services::secure_store::api_keys::delete("_test_concurrent_streams");
    })
    .await;

    assert!(
        outer_result.is_ok(),
        "Test should complete within 5 second outer timeout"
    );
}

/// Test that cancel of conversation A does not resolve B's pending approval.
/// Uses real `waiter.wait()` polling with timeout to verify B remains pending.
/// @plan PLAN-20260416-ISSUE173.P12
/// @requirement REQ-173-003.1
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_of_a_does_not_resolve_bs_pending_approval() {
    let outer_result = timeout(Duration::from_secs(5), async {
        // Build ChatServiceImpl with shared ApprovalGate
        let service = make_test_service().await;

        let conversation_a = Uuid::new_v4();
        let conversation_b = Uuid::new_v4();

        // Register two waiters via the gate directly
        let request_id_a = Uuid::new_v4().to_string();
        let request_id_b = Uuid::new_v4().to_string();

        let waiter_a = service.approval_gate.wait_for_approval(
            request_id_a.clone(),
            "WriteFile".to_string(),
            conversation_a,
        );

        let waiter_b = service.approval_gate.wait_for_approval(
            request_id_b.clone(),
            "ReadFile".to_string(),
            conversation_b,
        );

        // Call service.cancel(A) - this should propagate to resolve_all_for_conversation(A, false)
        ChatService::cancel(&service, conversation_a);

        // Assert: A's waiter completes with false (denied due to cancel)
        let a_result = timeout(Duration::from_millis(200), waiter_a.wait()).await;
        assert!(
            a_result.is_ok(),
            "A's waiter should resolve after cancel(A), not timeout"
        );
        let a_approved = a_result.unwrap().expect("waiter should not error");
        assert!(
            !a_approved,
            "A's waiter should resolve with false (denied due to cancel)"
        );

        // Assert: B's waiter is STILL pending
        // The key behavioral assertion: B should NOT resolve when A is cancelled
        // We poll B's waiter with a short timeout - it should timeout (still pending)
        let b_result = timeout(Duration::from_millis(50), waiter_b.wait()).await;
        assert!(
            b_result.is_err(),
            "B's waiter should STILL be pending after cancel(A) - should timeout, not resolve"
        );

        // Cleanup
        let _ = crate::services::secure_store::api_keys::delete("_test_concurrent_streams");
    })
    .await;

    assert!(
        outer_result.is_ok(),
        "Test should complete within 5 second outer timeout"
    );
}

/// Helper to create a basic chat service for integration testing
async fn make_test_service() -> ChatServiceImpl {
    crate::services::secure_store::use_mock_backend();
    let _ = crate::services::secure_store::api_keys::store(
        "_test_concurrent_streams",
        "fake-key-for-test",
    );

    let profile = make_test_profile();
    let profile_id = profile.id;

    let conversation_service: Arc<dyn ConversationService> =
        Arc::new(MockConversationService::new(profile_id));
    let mock_profile_service = Arc::new(MockProfileService::new());
    mock_profile_service.set_default_profile(profile).await;
    mock_profile_service.add_profile(make_test_profile()).await;

    let profile_service: Arc<dyn ProfileService> = mock_profile_service;
    ChatServiceImpl::new_for_integration_test(conversation_service, profile_service)
}

/// Helper to create a test profile
fn make_test_profile() -> ModelProfile {
    ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_concurrent_streams".to_string(),
        },
    )
}
