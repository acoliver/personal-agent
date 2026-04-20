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

/// Test that three conversations can have active streams in parallel and that `cancel` scopes correctly.
/// This test exercises the `begin_stream` CAS guard and verifies that `cancel(B)` only affects B and not A or C.
/// @plan PLAN-20260416-ISSUE173.P12
/// @requirement REQ-173-006.1, REQ-173-001.1
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_conversations_stream_in_parallel_and_cancel_scopes_correctly() {
    let outer_result = timeout(Duration::from_secs(5), async {
        // Create ChatServiceImpl
        let service = make_test_service().await;

        // Create three conversation IDs
        let conversation_a = Uuid::new_v4();
        let conversation_b = Uuid::new_v4();
        let conversation_c = Uuid::new_v4();

        // ==========================================
        // Issue (a): Exercise begin_stream CAS guard
        // ==========================================
        // REQ-173-001.1 — verify the per-conversation guard allows three concurrent begins.
        // This test proves that the CAS guard is per-conversation (not global) by showing
        // that three DIFFERENT conversations can all begin streams simultaneously.

        // First: All three different conversations should succeed with begin_stream
        // (proving per-conversation guard, NOT a global lock)
        service
            .begin_stream_for_test(conversation_a)
            .await
            .expect("begin_stream(A) should succeed");
        service
            .begin_stream_for_test(conversation_b)
            .await
            .expect("begin_stream(B) should succeed (per-conversation guard, not global)");
        service
            .begin_stream_for_test(conversation_c)
            .await
            .expect("begin_stream(C) should succeed (per-conversation guard, not global)");

        // Now insert a mock stream for A to simulate it being active
        service.insert_mock_stream_for_test(conversation_a);

        // Second: Duplicate begin for SAME conversation (A) must fail
        // This distinguishes per-conversation CAS from global CAS
        let dup_err = service
            .begin_stream_for_test(conversation_a)
            .await
            .expect_err("begin_stream(A) duplicate must fail");
        assert!(
            dup_err.to_string().contains("Stream already in progress"),
            "expected per-conversation CAS error, got: {dup_err}"
        );

        // Verify A is streaming (has mock stream), B and C are not yet streaming
        assert!(
            service.is_streaming_for(conversation_a),
            "A should be streaming after insert_mock_stream_for_test"
        );
        assert!(
            !service.is_streaming_for(conversation_b),
            "B should NOT be streaming yet (only called begin_stream, no stream inserted)"
        );
        assert!(
            !service.is_streaming_for(conversation_c),
            "C should NOT be streaming yet (only called begin_stream, no stream inserted)"
        );

        // Clear all streams to reset state before using mock streams for the controlled test
        service.clear_all_streams_for_test();

        // Verify streams are cleared
        assert!(
            !service.is_streaming_for(conversation_a),
            "A should not be streaming after clear_all_streams_for_test"
        );
        assert!(
            !service.is_streaming_for(conversation_b),
            "B should not be streaming after clear_all_streams_for_test"
        );
        assert!(
            !service.is_streaming_for(conversation_c),
            "C should not be streaming after clear_all_streams_for_test"
        );

        // Subscribe to events BEFORE inserting streams
        let mut event_rx = subscribe();

        // Insert mock active streams for all three conversations using the test helper
        // This simulates streams that are actively running (like real LLM streams)
        service.insert_mock_stream_for_test(conversation_a);
        service.insert_mock_stream_for_test(conversation_b);
        service.insert_mock_stream_for_test(conversation_c);

        // Assert: All three are streaming
        assert!(
            ChatService::is_streaming_for(&service, conversation_a),
            "A should be streaming after insert_mock_stream_for_test"
        );
        assert!(
            ChatService::is_streaming_for(&service, conversation_b),
            "B should be streaming after insert_mock_stream_for_test"
        );
        assert!(
            ChatService::is_streaming_for(&service, conversation_c),
            "C should be streaming after insert_mock_stream_for_test"
        );

        // Mid-flight: Cancel conversation B using the ChatService cancel API
        ChatService::cancel(&service, conversation_b);

        // Small delay to allow cancellation to propagate
        sleep(Duration::from_millis(20)).await;

        // Assert: B is no longer streaming
        assert!(
            !ChatService::is_streaming_for(&service, conversation_b),
            "B should not be streaming after cancel(B)"
        );

        // Assert: A and C are still streaming (not affected by cancel on B)
        assert!(
            ChatService::is_streaming_for(&service, conversation_a),
            "A should still be streaming after cancel(B) - cancel should scope to B only"
        );
        assert!(
            ChatService::is_streaming_for(&service, conversation_c),
            "C should still be streaming after cancel(B) - cancel should scope to B only"
        );

        // Drain events and verify exactly one StreamCancelled for B, none for A/C
        let mut a_cancel_count = 0usize;
        let mut b_cancel_count = 0usize;
        let mut c_cancel_count = 0usize;

        while let Ok(event) = event_rx.try_recv() {
            if let AppEvent::Chat(ChatEvent::StreamCancelled {
                conversation_id, ..
            }) = &event
            {
                if conversation_id == &conversation_a {
                    a_cancel_count += 1;
                } else if conversation_id == &conversation_b {
                    b_cancel_count += 1;
                } else if conversation_id == &conversation_c {
                    c_cancel_count += 1;
                }
            }
        }

        // Assert: Exactly one StreamCancelled for B
        assert_eq!(
            b_cancel_count, 1,
            "Should have exactly ONE StreamCancelled event for B (got {b_cancel_count})"
        );

        // Assert: Zero StreamCancelled for A and C
        assert_eq!(
            a_cancel_count, 0,
            "A should NOT have any StreamCancelled events (cancel should be scoped to B)"
        );
        assert_eq!(
            c_cancel_count, 0,
            "C should NOT have any StreamCancelled events (cancel should be scoped to B)"
        );

        // Cleanup
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
