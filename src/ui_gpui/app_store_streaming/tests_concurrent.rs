//! Concurrent streaming target tracking tests (TDD for HashSet-based `active_streaming_targets`).
//!
//! @plan PLAN-20260416-ISSUE173.P08
//!
//! These tests verify that multiple conversations can be streaming simultaneously,
//! with each tracked in an `active_streaming_targets` `HashSet`.

use std::collections::HashSet;
use uuid::Uuid;

use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ViewCommand,
};
use crate::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, GpuiAppStore, StartupInputs,
};
use crate::ui_gpui::app_store_types::project_streaming_snapshot;

fn profile_summary(id: Uuid) -> ProfileSummary {
    ProfileSummary {
        id,
        name: "Default".to_string(),
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-7-sonnet".to_string(),
        is_default: true,
    }
}

fn conversation_summary(id: Uuid, title: &str) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: chrono::Utc::now(),
        message_count: 0,
        preview: None,
    }
}

fn startup_inputs(first_id: Uuid, second_id: Uuid, selected_profile_id: Uuid) -> StartupInputs {
    StartupInputs {
        profiles: vec![profile_summary(selected_profile_id)],
        selected_profile_id: Some(selected_profile_id),
        conversations: vec![
            conversation_summary(first_id, "Conversation A"),
            conversation_summary(second_id, "Conversation B"),
        ],
        selected_conversation: None,
    }
}

fn begin_and_ready(store: &GpuiAppStore, conversation_id: Uuid) -> u64 {
    let generation =
        match store.begin_selection(conversation_id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => {
                panic!("expected begin_selection to switch conversation")
            }
            BeginSelectionResult::BeganSelection { generation } => generation,
        };

    let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id,
        selection_generation: generation,
        messages: vec![ConversationMessagePayload {
            role: MessageRole::User,
            content: "seed".to_string(),
            thinking_content: None,
            timestamp: None,
            model_id: None,
        }],
    }]);
    assert!(changed);

    generation
}

/// @plan PLAN-20260416-ISSUE173.P08
/// @requirement REQ-173-004.1
#[test]
fn multiple_targets_can_be_tracked_concurrently() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    // Start streaming on conversation A
    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![ViewCommand::ShowThinking {
        conversation_id: conversation_a,
        model_id: "model-a".to_string(),
    }]));

    // Start streaming on conversation B (concurrent with A)
    begin_and_ready(&store, conversation_b);
    assert!(store.reduce_batch(vec![ViewCommand::ShowThinking {
        conversation_id: conversation_b,
        model_id: "model-b".to_string(),
    }]));

    // Verify both conversations are tracked in the active streaming targets set
    let targets = store.active_streaming_targets_for_test();
    assert!(
        targets.contains(&conversation_a),
        "conversation A should be in the active streaming targets set"
    );
    assert!(
        targets.contains(&conversation_b),
        "conversation B should be in the active streaming targets set"
    );
}

/// @plan PLAN-20260416-ISSUE173.P08
/// @requirement REQ-173-004.2
#[test]
fn finalize_removes_only_targeted_conversation_from_set() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    // Start streaming on both conversations
    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![ViewCommand::AppendStream {
        conversation_id: conversation_a,
        chunk: "a-chunk".to_string(),
    }]));

    begin_and_ready(&store, conversation_b);
    assert!(store.reduce_batch(vec![ViewCommand::AppendStream {
        conversation_id: conversation_b,
        chunk: "b-chunk".to_string(),
    }]));

    // Verify both are in the set
    let targets = store.active_streaming_targets_for_test();
    assert!(targets.contains(&conversation_a));
    assert!(targets.contains(&conversation_b));

    // Finalize only conversation A (via store API)
    assert!(store.reduce_batch(vec![ViewCommand::FinalizeStream {
        conversation_id: conversation_a,
        tokens: 42,
    }]));

    // Verify A is removed but B remains
    let targets = store.active_streaming_targets_for_test();
    assert!(
        !targets.contains(&conversation_a),
        "conversation A should be removed from active streaming targets after finalize"
    );
    assert!(
        targets.contains(&conversation_b),
        "conversation B should still be in active streaming targets set"
    );
}

/// @plan PLAN-20260416-ISSUE173.P08
/// @requirement REQ-173-004.2
#[test]
fn streaming_state_snapshot_active_flag_reflects_set_membership() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let conversation_c = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    // Start streaming on A and B
    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![ViewCommand::AppendStream {
        conversation_id: conversation_a,
        chunk: "a-chunk".to_string(),
    }]));

    begin_and_ready(&store, conversation_b);
    assert!(store.reduce_batch(vec![ViewCommand::AppendStream {
        conversation_id: conversation_b,
        chunk: "b-chunk".to_string(),
    }]));

    // Construct a HashSet representing the active streaming targets (A and B)
    let mut active_set: HashSet<Uuid> = HashSet::new();
    active_set.insert(conversation_a);
    active_set.insert(conversation_b);

    // When selected conversation is A, projection should show A as active
    // Note: This uses the new project_streaming_snapshot signature that accepts &HashSet<Uuid>
    // instead of Option<Uuid>. This will fail to compile until P09 updates the signature.
    let streaming_states = crate::ui_gpui::app_store_types::ConversationStreamingState::default();
    let mut state_map: std::collections::HashMap<
        Uuid,
        crate::ui_gpui::app_store_types::ConversationStreamingState,
    > = std::collections::HashMap::new();
    state_map.insert(conversation_a, streaming_states.clone());
    state_map.insert(conversation_b, streaming_states);

    let snapshot_for_a = project_streaming_snapshot(&state_map, Some(conversation_a), &active_set);
    assert_eq!(
        snapshot_for_a.active_target,
        Some(conversation_a),
        "when A is selected and in the active set, snapshot should show A as active"
    );

    // When selected conversation is C (not streaming), projection should show None
    let snapshot_for_c = project_streaming_snapshot(&state_map, Some(conversation_c), &active_set);
    assert_eq!(
        snapshot_for_c.active_target, None,
        "when C is selected and not in the active set, snapshot should show None"
    );
}

/// @plan PLAN-20260416-ISSUE173.P10
/// @requirement REQ-173-004.3
#[test]
fn history_snapshot_exposes_streaming_conversation_ids() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    // Start streaming on conversation A using the begin_and_ready + ShowThinking pattern
    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![ViewCommand::ShowThinking {
        conversation_id: conversation_a,
        model_id: "model-a".to_string(),
    }]));

    // Start streaming on conversation B (concurrent with A)
    begin_and_ready(&store, conversation_b);
    assert!(store.reduce_batch(vec![ViewCommand::ShowThinking {
        conversation_id: conversation_b,
        model_id: "model-b".to_string(),
    }]));

    // Read the history snapshot from the store via the test accessor
    // This accesses the not-yet-existent field: snapshot.history.streaming_conversation_ids
    let streaming_ids = store.streaming_conversation_ids_for_test();

    // Assert snapshot.history.streaming_conversation_ids contains A and B
    assert!(
        streaming_ids.contains(&conversation_a),
        "history snapshot should expose conversation A as streaming"
    );
    assert!(
        streaming_ids.contains(&conversation_b),
        "history snapshot should expose conversation B as streaming"
    );
}
