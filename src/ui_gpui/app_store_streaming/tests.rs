use uuid::Uuid;

use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ViewCommand,
};
use crate::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, GpuiAppStore, StartupInputs,
};

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
        }],
    }]);
    assert!(changed);

    generation
}

fn background_streaming_state(
    store: &GpuiAppStore,
    streaming_conversation_id: Uuid,
    selected_conversation_id: Uuid,
) {
    begin_and_ready(store, streaming_conversation_id);

    let started_stream = store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: streaming_conversation_id,
        },
        ViewCommand::AppendThinking {
            conversation_id: streaming_conversation_id,
            content: "a-thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: streaming_conversation_id,
            chunk: "a-stream".to_string(),
        },
    ]);
    assert!(started_stream);

    begin_and_ready(store, selected_conversation_id);

    let snapshot = store.current_snapshot();
    assert_eq!(
        snapshot.chat.selected_conversation_id,
        Some(selected_conversation_id)
    );
    assert_eq!(
        snapshot.chat.streaming.active_target,
        Some(streaming_conversation_id),
        "loading selected conversation should preserve background stream target"
    );
    assert!(
        snapshot.chat.streaming.stream_buffer.is_empty(),
        "loading selected conversation should clear visible stream buffer"
    );
    assert!(
        snapshot.chat.streaming.thinking_buffer.is_empty(),
        "loading selected conversation should clear visible thinking buffer"
    );
}

#[test]
fn finalize_stream_for_background_target_clears_active_target_without_touching_visible_buffers() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    background_streaming_state(&store, conversation_a, conversation_b);

    let before_finalize = store.current_snapshot();
    let finalize_changed = store.reduce_batch(vec![ViewCommand::FinalizeStream {
        conversation_id: conversation_a,
        tokens: 123,
    }]);
    assert!(
        finalize_changed,
        "background finalize should clear active target and publish snapshot"
    );

    let after_finalize = store.current_snapshot();
    assert_eq!(
        after_finalize.chat.selected_conversation_id,
        Some(conversation_b),
        "selected conversation remains unchanged"
    );
    assert_eq!(
        after_finalize.chat.streaming.active_target, None,
        "background finalize should clear active target"
    );
    assert_eq!(
        after_finalize.chat.streaming.stream_buffer, before_finalize.chat.streaming.stream_buffer,
        "selected conversation stream buffer must remain untouched"
    );
    assert_eq!(
        after_finalize.chat.streaming.thinking_buffer,
        before_finalize.chat.streaming.thinking_buffer,
        "selected conversation thinking buffer must remain untouched"
    );
    assert_eq!(
        after_finalize.chat.streaming.thinking_visible,
        before_finalize.chat.streaming.thinking_visible,
        "selected conversation thinking visibility must remain untouched"
    );
    assert_eq!(
        after_finalize.chat.streaming.last_error, before_finalize.chat.streaming.last_error,
        "background finalize should not change selected conversation errors"
    );
}

#[test]
fn background_stream_error_only_clears_matching_active_target_and_preserves_selected_ui_state() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    background_streaming_state(&store, conversation_a, conversation_b);

    let before_error = store.current_snapshot();
    let error_changed = store.reduce_batch(vec![ViewCommand::StreamError {
        conversation_id: conversation_a,
        error: "background failure".to_string(),
        recoverable: false,
    }]);
    assert!(
        error_changed,
        "background stream error should clear matching active target"
    );

    let after_error = store.current_snapshot();
    assert_eq!(after_error.chat.streaming.active_target, None);
    assert_eq!(
        after_error.chat.streaming.stream_buffer, before_error.chat.streaming.stream_buffer,
        "selected stream buffer must remain unchanged"
    );
    assert_eq!(
        after_error.chat.streaming.thinking_buffer, before_error.chat.streaming.thinking_buffer,
        "selected thinking buffer must remain unchanged"
    );
    assert_eq!(
        after_error.chat.streaming.thinking_visible, before_error.chat.streaming.thinking_visible,
        "selected thinking visibility must remain unchanged"
    );
    assert_eq!(
        after_error.chat.streaming.last_error, before_error.chat.streaming.last_error,
        "background error should not overwrite selected conversation errors"
    );
}
