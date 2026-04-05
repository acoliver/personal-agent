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
    }
}

fn user_message(content: &str) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role: MessageRole::User,
        content: content.to_string(),
        thinking_content: None,
        timestamp: None,
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
fn begin_selection_preserves_background_active_target_and_clears_visible_streaming_state() {
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
        "precondition: conversation A is actively streaming"
    );
    assert!(
        !before_switch.chat.streaming.stream_buffer.is_empty(),
        "precondition: visible stream buffer populated"
    );
    assert!(
        !before_switch.chat.streaming.thinking_buffer.is_empty(),
        "precondition: visible thinking buffer populated"
    );
    assert!(
        before_switch.chat.streaming.thinking_visible,
        "precondition: thinking indicator visible"
    );

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
        after_switch.chat.streaming.active_target,
        Some(conversation_a),
        "background stream target should be preserved across selection switch"
    );
    assert!(
        after_switch.chat.streaming.stream_buffer.is_empty(),
        "visible stream buffer should be cleared on switch"
    );
    assert!(
        after_switch.chat.streaming.thinking_buffer.is_empty(),
        "visible thinking buffer should be cleared on switch"
    );
    assert!(
        !after_switch.chat.streaming.thinking_visible,
        "thinking indicator should be hidden after switch"
    );
    assert!(
        after_switch.chat.streaming.last_error.is_none(),
        "last error should be cleared with visible streaming state"
    );
}
