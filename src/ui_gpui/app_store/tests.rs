use chrono::Utc;
use uuid::Uuid;

use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ViewCommand,
};
use crate::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, ConversationLoadState, GpuiAppStore, StartupInputs,
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

fn conversation_summary(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
        preview: None,
    }
}

fn user_message(content: &str) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role: MessageRole::User,
        content: content.to_string(),
        thinking_content: None,
        timestamp: None,
        model_id: None,
    }
}

fn startup_inputs(first_id: Uuid, second_id: Uuid, selected_profile_id: Uuid) -> StartupInputs {
    StartupInputs {
        profiles: vec![profile_summary(selected_profile_id)],
        selected_profile_id: Some(selected_profile_id),
        conversations: vec![
            conversation_summary(first_id, "Conversation A", 1),
            conversation_summary(second_id, "Conversation B", 0),
        ],
        selected_conversation: None,
    }
}

#[test]
fn begin_selection_reprojects_streaming_state_without_cross_conversation_leakage() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    let initial_selection_generation =
        match store.begin_selection(conversation_a, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => 1,
            BeginSelectionResult::BeganSelection { generation } => generation,
        };

    let changed = store.reduce_batch(vec![
        ViewCommand::ConversationMessagesLoaded {
            conversation_id: conversation_a,
            selection_generation: initial_selection_generation,
            messages: vec![user_message("hello")],
        },
        ViewCommand::ShowThinking {
            conversation_id: conversation_a,
            model_id: "test".to_string(),
        },
        ViewCommand::AppendThinking {
            conversation_id: conversation_a,
            content: "thinking".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_a,
            chunk: "partial".to_string(),
        },
    ]);
    assert!(changed);

    let before_switch = store.current_snapshot();
    assert_eq!(
        before_switch.chat.streaming.active_target,
        Some(conversation_a),
        "precondition: conversation A projects itself as active while selected"
    );
    assert_eq!(before_switch.chat.streaming.stream_buffer, "partial");
    assert_eq!(before_switch.chat.streaming.thinking_buffer, "thinking");
    assert!(before_switch.chat.streaming.thinking_visible);

    let switch_generation =
        match store.begin_selection(conversation_b, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => {
                panic!("expected begin_selection to switch to conversation B")
            }
            BeginSelectionResult::BeganSelection { generation } => generation,
        };
    assert!(switch_generation > initial_selection_generation);

    let after_switch = store.current_snapshot();
    assert_eq!(
        after_switch.chat.selected_conversation_id,
        Some(conversation_b)
    );
    assert_eq!(
        after_switch.chat.load_state,
        ConversationLoadState::Loading {
            conversation_id: conversation_b,
            generation: switch_generation,
        }
    );
    assert_eq!(
        after_switch.chat.streaming.active_target, None,
        "conversation B should not project conversation A as active"
    );
    assert!(after_switch.chat.streaming.stream_buffer.is_empty());
    assert!(after_switch.chat.streaming.thinking_buffer.is_empty());
    assert!(!after_switch.chat.streaming.thinking_visible);
    assert!(after_switch.chat.streaming.last_error.is_none());

    let switch_back_generation =
        match store.begin_selection(conversation_a, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => {
                panic!("expected begin_selection to switch back to conversation A")
            }
            BeginSelectionResult::BeganSelection { generation } => generation,
        };
    assert!(switch_back_generation > switch_generation);

    let after_switch_back = store.current_snapshot();
    assert_eq!(
        after_switch_back.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(
        after_switch_back.chat.streaming.active_target,
        Some(conversation_a)
    );
    assert_eq!(after_switch_back.chat.streaming.stream_buffer, "partial");
    assert_eq!(after_switch_back.chat.streaming.thinking_buffer, "thinking");
    assert!(after_switch_back.chat.streaming.thinking_visible);
}

#[test]
fn begin_selection_clears_previous_transcript_in_snapshot() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let selected_profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(startup_inputs(
        conversation_a,
        conversation_b,
        selected_profile_id,
    ));

    let generation_a =
        match store.begin_selection(conversation_a, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => 1,
            BeginSelectionResult::BeganSelection { generation } => generation,
        };

    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: conversation_a,
        selection_generation: generation_a,
        messages: vec![user_message("hello from A")],
    }]);

    let before_switch = store.current_snapshot();
    assert_eq!(
        before_switch.chat.transcript.len(),
        1,
        "precondition: conversation A transcript is populated"
    );

    let generation_b =
        match store.begin_selection(conversation_b, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => {
                panic!("expected begin_selection to switch to conversation B")
            }
            BeginSelectionResult::BeganSelection { generation } => generation,
        };

    let after_switch = store.current_snapshot();
    assert_eq!(
        after_switch.chat.selected_conversation_id,
        Some(conversation_b)
    );
    assert_eq!(
        after_switch.chat.load_state,
        ConversationLoadState::Loading {
            conversation_id: conversation_b,
            generation: generation_b,
        }
    );
    assert!(
        after_switch.chat.transcript.is_empty(),
        "transcript must be cleared in snapshot while new conversation is Loading"
    );

    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: conversation_b,
        selection_generation: generation_b,
        messages: vec![user_message("hello from B")],
    }]);

    let after_load_b = store.current_snapshot();
    assert_eq!(after_load_b.chat.transcript.len(), 1);
    assert_eq!(after_load_b.chat.transcript[0].content, "hello from B");

    let generation_a_again =
        match store.begin_selection(conversation_a, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => {
                panic!("expected begin_selection to switch back to A")
            }
            BeginSelectionResult::BeganSelection { generation } => generation,
        };

    let after_switch_back = store.current_snapshot();
    assert!(
        after_switch_back.chat.transcript.is_empty(),
        "switching back to A must also clear B's transcript from snapshot during Loading"
    );

    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: conversation_a,
        selection_generation: generation_a_again,
        messages: vec![user_message("hello from A")],
    }]);

    let after_reload_a = store.current_snapshot();
    assert_eq!(after_reload_a.chat.transcript.len(), 1);
    assert_eq!(after_reload_a.chat.transcript[0].content, "hello from A");
}

#[test]
fn deleting_selected_conversation_resets_transcript_and_load_state() {
    let conversation_a = Uuid::new_v4();
    let conversation_b = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: vec![profile_summary(profile_id)],
        selected_profile_id: Some(profile_id),
        conversations: vec![
            conversation_summary(conversation_a, "Conversation A", 1),
            conversation_summary(conversation_b, "Conversation B", 0),
        ],
        selected_conversation: None,
    });

    let generation_a =
        match store.begin_selection(conversation_a, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection switch"),
            BeginSelectionResult::BeganSelection { generation } => generation,
        };
    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: conversation_a,
        selection_generation: generation_a,
        messages: vec![user_message("message in A")],
    }]);

    let before_delete = store.current_snapshot();
    assert_eq!(
        before_delete.chat.selected_conversation_id,
        Some(conversation_a)
    );
    assert_eq!(before_delete.chat.transcript.len(), 1);

    assert!(store.reduce_batch(vec![ViewCommand::ConversationDeleted {
        id: conversation_a,
    }]));

    let after_delete = store.current_snapshot();
    assert_eq!(
        after_delete.chat.selected_conversation_id,
        Some(conversation_b),
        "deleting selected A must retarget selection to the remaining conversation B"
    );
    assert!(
        after_delete.chat.transcript.is_empty(),
        "deleting the selected conversation must reset the transcript so A's messages do not render under B"
    );
    match after_delete.chat.load_state {
        ConversationLoadState::Loading {
            conversation_id,
            generation,
        } => {
            assert_eq!(
                conversation_id, conversation_b,
                "retargeted selection must load the new conversation"
            );
            assert!(
                generation > generation_a,
                "retargeted selection must bump the selection generation"
            );
            assert_eq!(
                generation, after_delete.chat.selection_generation,
                "snapshot selection_generation must match the Loading generation"
            );
        }
        other => panic!(
            "retargeted selection must enter Loading state so subscribers initiate a transcript load, got {other:?}"
        ),
    }
    assert_eq!(
        after_delete.chat.selected_conversation_title, "Conversation B",
        "selected title must follow the retargeted conversation"
    );
}

#[test]
fn deleting_selected_last_conversation_leaves_empty_idle_state() {
    let conversation_a = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: vec![profile_summary(profile_id)],
        selected_profile_id: Some(profile_id),
        conversations: vec![conversation_summary(conversation_a, "Only", 1)],
        selected_conversation: None,
    });

    let generation_a =
        match store.begin_selection(conversation_a, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection switch"),
            BeginSelectionResult::BeganSelection { generation } => generation,
        };
    store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: conversation_a,
        selection_generation: generation_a,
        messages: vec![user_message("only msg")],
    }]);

    assert!(store.reduce_batch(vec![ViewCommand::ConversationDeleted {
        id: conversation_a,
    }]));

    let after = store.current_snapshot();
    assert_eq!(after.chat.selected_conversation_id, None);
    assert!(after.chat.transcript.is_empty());
    assert_eq!(after.chat.load_state, ConversationLoadState::Idle);
    assert_eq!(after.chat.selected_conversation_title, "New Conversation");
}
