//! Streaming projection across a stream error and the turn after it.
//!
//! Pins issue 218: an error recorded by a finished turn must not hold later
//! turns of the same conversation on `StreamingState::Error`.

use super::state::StreamingState;
use super::ChatView;
use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ViewCommand,
};
use crate::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, GpuiAppStore, StartupInputs,
};
use chrono::Utc;
use uuid::Uuid;

fn store_profile_summary(id: Uuid) -> ProfileSummary {
    ProfileSummary {
        id,
        name: "Default".to_string(),
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-7-sonnet".to_string(),
        is_default: true,
    }
}

fn streaming_store(conversation_id: Uuid) -> GpuiAppStore {
    let selected_profile_id = Uuid::new_v4();
    GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: vec![store_profile_summary(selected_profile_id)],
        selected_profile_id: Some(selected_profile_id),
        conversations: vec![ConversationSummary {
            id: conversation_id,
            title: "Conversation".to_string(),
            updated_at: Utc::now(),
            message_count: 0,
            preview: None,
        }],
        selected_conversation: None,
    })
}

fn begin_conversation_and_ready(store: &GpuiAppStore, conversation_id: Uuid) {
    let generation =
        match store.begin_selection(conversation_id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => {
                panic!("expected begin_selection to switch conversation")
            }
            BeginSelectionResult::BeganSelection { generation } => generation,
        };

    assert!(
        store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id,
            selection_generation: generation,
            messages: vec![ConversationMessagePayload {
                role: MessageRole::User,
                content: "seed".to_string(),
                thinking_content: None,
                timestamp: None,
                model_id: None,
            }],
        }])
    );
}

/// A stream error belongs to the turn it killed. Once the next turn starts in
/// the same conversation, the projection must show that turn streaming live:
/// an uncleared `last_error` would keep the view on `StreamingState::Error`,
/// which renders no thinking and no deltas for every later turn (issue 218).
#[test]
fn a_new_turn_after_a_stream_error_projects_streaming_again() {
    let conversation_id = Uuid::new_v4();
    let store = streaming_store(conversation_id);
    begin_conversation_and_ready(&store, conversation_id);

    // Turn one streams normally, then dies mid-flight.
    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id,
            model_id: "model-a".to_string(),
        },
        ViewCommand::AppendThinking {
            conversation_id,
            content: "turn-one thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id,
            chunk: "turn-one partial".to_string(),
        },
    ]));
    assert!(store.reduce_batch(vec![ViewCommand::StreamError {
        conversation_id,
        error: "429 too many requests".to_string(),
        recoverable: false,
    }]));
    let after_error = store.current_snapshot();
    assert_eq!(
        ChatView::streaming_state_from_snapshot(
            &after_error.chat.streaming,
            &after_error.chat.load_state,
        ),
        StreamingState::Error("429 too many requests".to_string()),
        "the error must still show for the turn it killed"
    );

    // Turn two starts in the same conversation; the stale error must not
    // describe it.
    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id,
            model_id: "model-a".to_string(),
        },
        ViewCommand::AppendThinking {
            conversation_id,
            content: "turn-two thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id,
            chunk: "turn-two delta".to_string(),
        },
    ]));

    let snapshot = store.current_snapshot();
    assert_eq!(
        ChatView::streaming_state_from_snapshot(
            &snapshot.chat.streaming,
            &snapshot.chat.load_state,
        ),
        StreamingState::Streaming {
            content: "turn-two delta".to_string(),
            done: false,
        },
        "the turn after an error must stream live again"
    );
    assert_eq!(
        snapshot.chat.streaming.thinking_buffer, "turn-two thinking",
        "the new turn's thinking buffer must survive"
    );
}
