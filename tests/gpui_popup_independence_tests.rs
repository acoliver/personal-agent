use std::sync::Arc;

use chrono::Utc;
use personal_agent::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ViewCommand,
};
use personal_agent::ui_gpui::app_store::{
    BeginSelectionMode, ConversationLoadState, StartupInputs, StartupMode,
    StartupSelectedConversation, StartupTranscriptResult,
};
use personal_agent::ui_gpui::GpuiAppStore;
use uuid::Uuid;

fn conversation_summary(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
        preview: None,
    }
}

fn transcript_message(role: MessageRole, content: &str) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role,
        content: content.to_string(),
        thinking_content: None,
        timestamp: None,
        model_id: None,
    }
}

fn startup_store() -> (
    GpuiAppStore,
    Uuid,
    Uuid,
    Vec<ConversationMessagePayload>,
    Vec<ConversationMessagePayload>,
) {
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let first_transcript = vec![transcript_message(
        MessageRole::User,
        "first startup message",
    )];
    let second_transcript = vec![
        transcript_message(MessageRole::User, "second conversation message"),
        transcript_message(MessageRole::Assistant, "second conversation reply"),
    ];

    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: vec![
            conversation_summary(first_id, "Conversation One", first_transcript.len()),
            conversation_summary(second_id, "Conversation Two", second_transcript.len()),
        ],
        selected_conversation: Some(StartupSelectedConversation {
            conversation_id: first_id,
            mode: StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Success(first_transcript.clone()),
            },
        }),
    });

    (
        store,
        first_id,
        second_id,
        first_transcript,
        second_transcript,
    )
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P07
/// @requirement REQ-ARCH-004.1
/// @requirement REQ-INT-001.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:037-144
#[test]
fn popup_absent_store_mutation_reaches_store() {
    let (store, _first_id, second_id, _first_transcript, second_transcript) = startup_store();

    assert_eq!(store.subscriber_count(), 0);

    let selection = store.begin_selection(second_id, BeginSelectionMode::PublishImmediately);
    let generation = match selection {
        personal_agent::ui_gpui::app_store::BeginSelectionResult::BeganSelection { generation } => {
            generation
        }
        personal_agent::ui_gpui::app_store::BeginSelectionResult::NoOpSameSelection => {
            panic!("expected selection change for popup-absent mutation proof")
        }
    };

    let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: second_id,
        selection_generation: generation,
        messages: second_transcript.clone(),
    }]);

    assert!(changed);

    let snapshot = store.current_snapshot();
    assert_eq!(snapshot.chat.selected_conversation_id, Some(second_id));
    assert_eq!(snapshot.history.selected_conversation_id, Some(second_id));
    assert_eq!(snapshot.chat.transcript, second_transcript);
    assert_eq!(
        snapshot.chat.load_state,
        ConversationLoadState::Ready {
            conversation_id: second_id,
            generation,
        }
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P07
/// @requirement REQ-ARCH-004.2
/// @requirement REQ-INT-001.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:037-144
#[test]
fn popup_reopen_reads_latest_snapshot_without_replay() {
    let (store, _first_id, second_id, _first_transcript, second_transcript) = startup_store();

    let initial_popup_rx = store.subscribe();
    assert_eq!(store.subscriber_count(), 1);

    drop(initial_popup_rx);
    store.prune_disconnected_subscribers();
    assert_eq!(store.subscriber_count(), 0);

    let selection = store.begin_selection(second_id, BeginSelectionMode::PublishImmediately);
    let generation = match selection {
        personal_agent::ui_gpui::app_store::BeginSelectionResult::BeganSelection { generation } => {
            generation
        }
        personal_agent::ui_gpui::app_store::BeginSelectionResult::NoOpSameSelection => {
            panic!("expected selection change while popup closed")
        }
    };
    assert!(
        store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id: second_id,
            selection_generation: generation,
            messages: second_transcript.clone(),
        }])
    );

    let reopened_popup_rx = store.subscribe();
    assert_eq!(store.subscriber_count(), 1);
    assert!(reopened_popup_rx.try_recv().is_err());

    let snapshot = store.current_snapshot();
    assert_eq!(snapshot.chat.selected_conversation_id, Some(second_id));
    assert_eq!(snapshot.chat.transcript, second_transcript);
    assert_eq!(
        snapshot.chat.load_state,
        ConversationLoadState::Ready {
            conversation_id: second_id,
            generation,
        }
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P07
/// @requirement REQ-ARCH-004.3
/// @requirement REQ-INT-001.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:037-144
#[test]
fn store_handle_identity_survives_popup_lifecycle() {
    let (store, _first_id, second_id, _first_transcript, second_transcript) = startup_store();
    let original = Arc::new(store);
    let clone = Arc::clone(&original);

    let popup_rx = clone.subscribe();
    assert_eq!(clone.subscriber_count(), 1);
    drop(popup_rx);
    clone.prune_disconnected_subscribers();
    assert_eq!(clone.subscriber_count(), 0);

    assert!(Arc::ptr_eq(&original, &clone));

    let selection = original.begin_selection(second_id, BeginSelectionMode::PublishImmediately);
    let generation = match selection {
        personal_agent::ui_gpui::app_store::BeginSelectionResult::BeganSelection { generation } => {
            generation
        }
        personal_agent::ui_gpui::app_store::BeginSelectionResult::NoOpSameSelection => {
            panic!("expected selection change through original store handle")
        }
    };
    assert!(
        original.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id: second_id,
            selection_generation: generation,
            messages: second_transcript.clone(),
        }])
    );

    let clone_snapshot = clone.current_snapshot();
    assert_eq!(
        clone_snapshot.chat.selected_conversation_id,
        Some(second_id)
    );
    assert_eq!(clone_snapshot.chat.transcript, second_transcript);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P07
/// @requirement REQ-ARCH-004.1
/// @requirement REQ-INT-001.3
/// @pseudocode analysis/pseudocode/03-main-panel-integration.md:037-144
#[test]
fn disconnected_subscriber_does_not_block_store_mutation() {
    let (store, _first_id, second_id, _first_transcript, second_transcript) = startup_store();

    let dropped_rx = store.subscribe();
    let live_rx = store.subscribe();
    assert_eq!(store.subscriber_count(), 2);

    drop(dropped_rx);

    let selection = store.begin_selection(second_id, BeginSelectionMode::PublishImmediately);
    let generation = match selection {
        personal_agent::ui_gpui::app_store::BeginSelectionResult::BeganSelection { generation } => {
            generation
        }
        personal_agent::ui_gpui::app_store::BeginSelectionResult::NoOpSameSelection => {
            panic!("expected selection change with one disconnected subscriber")
        }
    };

    assert!(
        store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id: second_id,
            selection_generation: generation,
            messages: second_transcript.clone(),
        }])
    );

    let snapshot = store.current_snapshot();
    assert_eq!(snapshot.chat.selected_conversation_id, Some(second_id));
    assert_eq!(snapshot.chat.transcript, second_transcript);

    let selection_snapshot = live_rx
        .try_recv()
        .expect("remaining subscriber should receive publish after begin_selection");
    assert_eq!(
        selection_snapshot.chat.selected_conversation_id,
        Some(second_id)
    );
    let loaded_snapshot = live_rx
        .try_recv()
        .expect("remaining subscriber should receive loaded transcript snapshot");
    assert_eq!(loaded_snapshot.chat.transcript, second_transcript);
}
