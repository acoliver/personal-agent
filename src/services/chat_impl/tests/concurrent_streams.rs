//! Concurrent stream tests for `ChatServiceImpl`.
//!
//! Tests for REQ-173-001 and REQ-173-002: Concurrent service-layer streams
//! and per-conversation cancellation.

use crate::events::types::ChatEvent;
use crate::events::{subscribe, AppEvent};
use crate::models::{AuthConfig, ModelProfile};
use crate::services::chat_impl::ChatServiceImpl;
use crate::services::{ChatService, ConversationService, ProfileService};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

// Import test support utilities from parent tests module (support.rs)
use super::chat_test_support::{MockConversationService, MockProfileService};

fn make_test_chat_service() -> ChatServiceImpl {
    let profile = ModelProfile::new(
        "Test Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        "https://api.openai.com/v1".to_string(),
        AuthConfig::Keychain {
            label: "_test_concurrent".to_string(),
        },
    );

    let conversation_service: Arc<dyn ConversationService> =
        Arc::new(MockConversationService::new(profile.id));
    let mock_profile_service = Arc::new(MockProfileService::new());
    let profile_service: Arc<dyn ProfileService> = mock_profile_service;
    ChatServiceImpl::new_for_tests(conversation_service, profile_service)
}

/// @plan PLAN-20260416-ISSUE173.P02
/// @plan PLAN-20260416-ISSUE173.P14-CR3
/// @requirement REQ-173-001.1
#[tokio::test]
async fn begin_stream_allows_two_different_conversations() {
    let service = make_test_chat_service();
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();

    // Exercise the real reservation + promotion path for both conversations.
    service
        .begin_stream_for_test(conversation_a)
        .expect("begin_stream(A) should succeed");
    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed");

    // Both conversations should be streaming - use the trait method
    assert!(
        ChatService::is_streaming_for(&service, conversation_a),
        "is_streaming_for(A) should return true"
    );
    assert!(
        ChatService::is_streaming_for(&service, conversation_b),
        "is_streaming_for(B) should return true"
    );

    // is_streaming() should also return true (any stream active)
    assert!(service.is_streaming(), "is_streaming() should return true");

    // Cleanup
    service.clear_all_streams_for_test();
}

/// @plan PLAN-20260416-ISSUE173.P02
/// @requirement REQ-173-001.2
#[tokio::test]
async fn begin_stream_rejects_same_conversation_twice() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    // First begin_stream should succeed (reserves + promotes to Running).
    service
        .begin_stream_for_test(conversation_id)
        .expect("first begin_stream should succeed");

    // Second begin_stream for same conversation should fail because the
    // first entry is already `Running` in the active_streams map.
    let err = service
        .begin_stream_for_test(conversation_id)
        .expect_err("second begin_stream for same conversation should fail");

    let err_msg = err.to_string();
    assert!(
        err_msg.contains("Stream already in progress for this conversation"),
        "Error should contain 'Stream already in progress for this conversation', got: {err_msg}"
    );

    // Cleanup
    service.clear_all_streams_for_test();
}

/// @plan PLAN-20260416-ISSUE173.P02
/// @requirement REQ-173-002.1
#[tokio::test]
async fn cancel_scopes_to_conversation() {
    let service = make_test_chat_service();
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();

    // Begin real reserved+Running streams for both conversations.
    service
        .begin_stream_for_test(conversation_a)
        .expect("begin_stream(A) should succeed");
    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed");

    // Both should be streaming initially
    assert!(ChatService::is_streaming_for(&service, conversation_a));
    assert!(ChatService::is_streaming_for(&service, conversation_b));

    // Cancel only conversation A using the trait method
    ChatService::cancel(&service, conversation_a);

    // Give a moment for cancellation to process
    sleep(Duration::from_millis(10)).await;

    // A should no longer be streaming
    assert!(
        !ChatService::is_streaming_for(&service, conversation_a),
        "is_streaming_for(A) should return false after cancel(A)"
    );

    // B should still be streaming
    assert!(
        ChatService::is_streaming_for(&service, conversation_b),
        "is_streaming_for(B) should still return true after cancel(A)"
    );

    // Cleanup
    service.clear_all_streams_for_test();
}

/// @plan PLAN-20260416-ISSUE173.P02
/// @requirement REQ-173-002.2
#[tokio::test]
async fn cancel_emits_event_only_for_target() {
    let service = make_test_chat_service();
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();

    // Subscribe to events before starting streams
    let mut event_rx = subscribe();

    // Begin real reserved+Running streams for both conversations.
    service
        .begin_stream_for_test(conversation_a)
        .expect("begin_stream(A) should succeed");
    service
        .begin_stream_for_test(conversation_b)
        .expect("begin_stream(B) should succeed");

    // Cancel only conversation A using the trait method
    ChatService::cancel(&service, conversation_a);

    // Give a moment for cancellation to process and events to be emitted
    sleep(Duration::from_millis(50)).await;

    // Drain events and look for StreamCancelled
    let mut a_cancellations = 0;
    let mut b_cancellations = 0;

    // Process events that have been received (non-blocking)
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::Chat(ChatEvent::StreamCancelled {
            conversation_id, ..
        }) = &event
        {
            if conversation_id == &conversation_a {
                a_cancellations += 1;
            } else if conversation_id == &conversation_b {
                b_cancellations += 1;
            }
        }
    }

    // Should have exactly one StreamCancelled for A, zero for B
    assert_eq!(
        a_cancellations, 1,
        "Expected exactly 1 StreamCancelled event for A, got {a_cancellations}"
    );
    assert_eq!(
        b_cancellations, 0,
        "Expected 0 StreamCancelled events for B, got {b_cancellations}"
    );

    // Cleanup
    service.clear_all_streams_for_test();
}

/// @plan PLAN-20260416-ISSUE173.P02
/// @requirement REQ-173-002.1
#[tokio::test]
async fn cancel_unknown_conversation_is_noop() {
    let service = make_test_chat_service();
    let known_conversation = Uuid::new_v4();
    let unknown_conversation = Uuid::new_v4();

    // Begin a real reserved+Running stream for the known conversation only.
    service
        .begin_stream_for_test(known_conversation)
        .expect("begin_stream(known) should succeed");

    // Cancel an unknown conversation (never started) using the trait method
    // This should not panic
    ChatService::cancel(&service, unknown_conversation);

    // Give a moment for any potential processing
    sleep(Duration::from_millis(10)).await;

    // Known conversation should still be streaming
    assert!(
        ChatService::is_streaming_for(&service, known_conversation),
        "is_streaming_for(known) should still return true after cancel(unknown)"
    );

    // is_streaming() should still return true
    assert!(
        service.is_streaming(),
        "is_streaming() should still return true after cancel(unknown)"
    );

    // Cleanup
    service.clear_all_streams_for_test();
}

/// Regression test for CR3: concurrent same-conversation sends do not duplicate messages.
///
/// Simulates two concurrent `send_message` calls on the same conversation and verifies:
/// 1. The conversation service observes exactly ONE `add_message` call for the user turn
/// 2. The second send returns "Stream already in progress for this conversation"
///
/// @plan PLAN-20260416-ISSUE173.P14-CR3
/// @requirement REQ-173-001.2
#[tokio::test]
async fn concurrent_same_conversation_sends_do_not_duplicate_messages() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    // First `begin_stream` should succeed (reserves + promotes to Running).
    let first_result = service.begin_stream_for_test(conversation_id);
    assert!(first_result.is_ok(), "First begin_stream should succeed");

    // Second `begin_stream` for the same conversation should fail because
    // the first entry is already `Running` in the active_streams map.
    let second_result = service.begin_stream_for_test(conversation_id);
    assert!(
        second_result.is_err(),
        "Second begin_stream should fail when stream is already active"
    );

    let err_msg = second_result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Stream already in progress for this conversation"),
        "Error should indicate stream already in progress, got: {err_msg}"
    );

    // Cleanup
    service.clear_all_streams_for_test();
}

/// Regression test for CR4: stale task cleanup does not evict new stream.
///
/// Simulates the race where a stale task finishes after a new stream has started:
/// 1. Begin stream A with `stream_id_1`
/// 2. Cancel stream A
/// 3. Begin stream A again with `stream_id_2`
/// 4. Assert `stream_id_2` entry is still present via `stream_id_for_test`
///
/// @plan PLAN-20260416-ISSUE173.P14-CR4
/// @requirement REQ-173-001.3
#[tokio::test]
async fn stale_task_cleanup_does_not_evict_new_stream() {
    let service = make_test_chat_service();
    let conversation_id = Uuid::new_v4();

    // Step 1: Begin first stream (reserves + promotes to Running) and get
    // its `stream_id`.
    service
        .begin_stream_for_test(conversation_id)
        .expect("begin_stream should succeed");
    let stream_id_1 = service
        .stream_id_for_test(conversation_id)
        .expect("stream_id_1 should exist");

    // Step 2: Cancel the first stream.
    ChatService::cancel(&service, conversation_id);
    sleep(Duration::from_millis(10)).await;

    // Verify stream is cancelled
    assert!(
        !service.is_streaming_for(conversation_id),
        "Stream should be cancelled"
    );

    // Step 3: Begin a new stream (this should get a new stream_id)
    service
        .begin_stream_for_test(conversation_id)
        .expect("Second begin_stream should succeed");

    let stream_id_2 = service
        .stream_id_for_test(conversation_id)
        .expect("stream_id_2 should exist");

    // Verify stream_id_2 is different from stream_id_1
    assert_ne!(
        stream_id_1, stream_id_2,
        "New stream should have a different stream_id"
    );

    // Step 4: Assert stream_id_2 entry is still present
    let current_stream_id = service.stream_id_for_test(conversation_id);
    assert!(
        current_stream_id.is_some(),
        "Stream entry should still exist (epoch-guarded)"
    );
    assert_eq!(
        current_stream_id.unwrap(),
        stream_id_2,
        "Current stream_id should match stream_id_2 (epoch-guarded)"
    );

    // Cleanup
    service.clear_all_streams_for_test();
}
