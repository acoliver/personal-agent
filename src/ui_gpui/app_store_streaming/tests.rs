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

#[test]
fn background_streaming_state_is_projected_per_selected_conversation() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    let generation_a = begin_and_ready(&store, conversation_a);

    let started_stream = store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: conversation_a,
            model_id: "model-a".to_string(),
        },
        ViewCommand::AppendThinking {
            conversation_id: conversation_a,
            content: "a-thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_a,
            chunk: "a-stream".to_string(),
        },
    ]);
    assert!(started_stream);

    let snapshot_on_a = store.current_snapshot();
    assert_eq!(
        snapshot_on_a.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(
        snapshot_on_a.chat.streaming.active_target,
        Some(conversation_a)
    );
    assert_eq!(snapshot_on_a.chat.streaming.stream_buffer, "a-stream");
    assert_eq!(snapshot_on_a.chat.streaming.thinking_buffer, "a-thinking");
    assert!(snapshot_on_a.chat.streaming.thinking_visible);
    assert_eq!(
        snapshot_on_a.chat.streaming.model_id.as_deref(),
        Some("model-a")
    );

    let generation_b = begin_and_ready(&store, conversation_b);
    assert!(generation_b > generation_a);
    let snapshot_on_b = store.current_snapshot();
    assert_eq!(
        snapshot_on_b.chat.selected_conversation_id,
        Some(conversation_b)
    );
    assert!(snapshot_on_b.chat.streaming.stream_buffer.is_empty());
    assert!(snapshot_on_b.chat.streaming.thinking_buffer.is_empty());
    assert!(!snapshot_on_b.chat.streaming.thinking_visible);
    assert_eq!(
        snapshot_on_b.chat.streaming.active_target, None,
        "selected conversation B should not project A as active target"
    );

    let generation_back_to_a = begin_and_ready(&store, conversation_a);
    assert!(generation_back_to_a > generation_b);
    let snapshot_back_on_a = store.current_snapshot();
    assert_eq!(
        snapshot_back_on_a.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(
        snapshot_back_on_a.chat.streaming.active_target,
        Some(conversation_a)
    );
    assert_eq!(snapshot_back_on_a.chat.streaming.stream_buffer, "a-stream");
    assert_eq!(
        snapshot_back_on_a.chat.streaming.thinking_buffer,
        "a-thinking"
    );
    assert!(snapshot_back_on_a.chat.streaming.thinking_visible);
}

#[test]
fn finalize_stream_for_background_target_preserves_selected_projection() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: conversation_a,
            model_id: "model-a".to_string(),
        },
        ViewCommand::AppendThinking {
            conversation_id: conversation_a,
            content: "a-thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_a,
            chunk: "a-stream".to_string(),
        },
    ]));

    begin_and_ready(&store, conversation_b);
    let before_finalize = store.current_snapshot();
    assert_eq!(
        before_finalize.chat.selected_conversation_id,
        Some(conversation_b)
    );
    assert!(before_finalize.chat.streaming.stream_buffer.is_empty());

    let finalize_changed = store.reduce_batch(vec![ViewCommand::FinalizeStream {
        conversation_id: conversation_a,
        tokens: 123,
    }]);
    assert!(finalize_changed);

    let after_finalize = store.current_snapshot();
    assert_eq!(
        after_finalize.chat.selected_conversation_id,
        Some(conversation_b)
    );
    assert_eq!(
        after_finalize.chat.streaming, before_finalize.chat.streaming,
        "finalizing a background stream should not mutate selected conversation projection"
    );

    begin_and_ready(&store, conversation_a);
    let snapshot_a = store.current_snapshot();
    assert_eq!(
        snapshot_a.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(snapshot_a.chat.streaming.active_target, None);
    assert!(snapshot_a.chat.streaming.stream_buffer.is_empty());
    assert!(snapshot_a.chat.streaming.thinking_buffer.is_empty());
    assert!(!snapshot_a.chat.streaming.thinking_visible);
}

#[test]
fn finalize_stream_for_explicit_target_when_another_stream_is_active() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: conversation_a,
            model_id: "model-a".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_a,
            chunk: "a-stream".to_string(),
        },
    ]));

    begin_and_ready(&store, conversation_b);
    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: conversation_b,
            model_id: "model-b".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_b,
            chunk: "b-stream".to_string(),
        },
    ]));

    let finalize_changed = store.reduce_batch(vec![ViewCommand::FinalizeStream {
        conversation_id: conversation_a,
        tokens: 7,
    }]);
    assert!(finalize_changed);

    let snapshot_on_b = store.current_snapshot();
    assert_eq!(
        snapshot_on_b.chat.selected_conversation_id,
        Some(conversation_b)
    );
    assert_eq!(
        snapshot_on_b.chat.streaming.active_target,
        Some(conversation_b),
        "finalizing A should not clear B's active stream projection"
    );
    assert_eq!(snapshot_on_b.chat.streaming.stream_buffer, "b-stream");

    begin_and_ready(&store, conversation_a);
    let snapshot_on_a = store.current_snapshot();
    assert_eq!(
        snapshot_on_a.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(snapshot_on_a.chat.streaming.active_target, None);
    assert!(snapshot_on_a.chat.streaming.stream_buffer.is_empty());
}

#[test]
fn background_stream_error_preserves_selected_projection_and_is_scoped() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    begin_and_ready(&store, conversation_a);
    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: conversation_a,
            model_id: "model-a".to_string(),
        },
        ViewCommand::AppendThinking {
            conversation_id: conversation_a,
            content: "a-thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_a,
            chunk: "a-stream".to_string(),
        },
    ]));

    begin_and_ready(&store, conversation_b);
    let before_error = store.current_snapshot();

    let error_changed = store.reduce_batch(vec![ViewCommand::StreamError {
        conversation_id: conversation_a,
        error: "background failure".to_string(),
        recoverable: false,
    }]);
    assert!(error_changed);

    let after_error = store.current_snapshot();
    assert_eq!(
        after_error.chat.streaming, before_error.chat.streaming,
        "background stream error should not mutate selected conversation projection"
    );

    begin_and_ready(&store, conversation_a);
    let snapshot_a = store.current_snapshot();
    assert_eq!(
        snapshot_a.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(
        snapshot_a.chat.streaming.last_error.as_deref(),
        Some("background failure")
    );
    assert_eq!(snapshot_a.chat.streaming.active_target, None);
}
