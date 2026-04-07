use chrono::Utc;
use personal_agent::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ViewCommand,
};
use personal_agent::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, ConversationLoadState, StartupInputs, StartupMode,
    StartupSelectedConversation, StartupTranscriptResult,
};
use personal_agent::ui_gpui::{GpuiAppSnapshot, GpuiAppStore};
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

fn snapshot(store: &GpuiAppStore) -> GpuiAppSnapshot {
    store.current_snapshot()
}

fn begun_generation(result: BeginSelectionResult) -> u64 {
    match result {
        BeginSelectionResult::BeganSelection { generation } => generation,
        BeginSelectionResult::NoOpSameSelection => {
            panic!("expected selection generation to advance")
        }
    }
}

fn selected_store_with_transcript() -> (GpuiAppStore, Uuid, Vec<ConversationMessagePayload>) {
    let conversation_id = Uuid::new_v4();
    let transcript = vec![
        transcript_message(MessageRole::User, "hello"),
        transcript_message(MessageRole::Assistant, "world"),
    ];
    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: vec![conversation_summary(
            conversation_id,
            "Selected Conversation",
            transcript.len(),
        )],
        selected_conversation: Some(StartupSelectedConversation {
            conversation_id,
            mode: StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Success(transcript.clone()),
            },
        }),
    });

    (store, conversation_id, transcript)
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.1
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn bulk_replacement_via_conversation_messages_loaded() {
    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: Vec::new(),
        selected_conversation: None,
    });
    let conversation_id = Uuid::new_v4();
    let generation = begun_generation(
        store.begin_selection(conversation_id, BeginSelectionMode::BatchNoPublish),
    );
    let replacement = vec![
        transcript_message(MessageRole::User, "first"),
        transcript_message(MessageRole::Assistant, "second"),
        transcript_message(MessageRole::User, "third"),
    ];

    let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id,
        selection_generation: generation,
        messages: replacement.clone(),
    }]);

    assert!(changed);
    let snapshot = snapshot(&store);
    assert_eq!(snapshot.chat.transcript.len(), replacement.len());
    assert_eq!(snapshot.chat.transcript, replacement);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.2
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn no_clear_on_ordinary_conversation_activated() {
    let (store, conversation_id, transcript) = selected_store_with_transcript();
    let before = snapshot(&store);
    let generation = before.chat.selection_generation;

    let changed = store.reduce_batch(vec![ViewCommand::ConversationActivated {
        id: conversation_id,
        selection_generation: generation,
    }]);

    assert!(!changed);
    let after = snapshot(&store);
    assert_eq!(after.chat.transcript.len(), transcript.len());
    assert_eq!(after.chat.transcript, transcript);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.3
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn startup_first_frame_correctness() {
    let conversation_id = Uuid::new_v4();
    let transcript = vec![
        transcript_message(MessageRole::User, "startup user"),
        transcript_message(MessageRole::Assistant, "startup assistant"),
    ];

    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: vec![conversation_summary(
            conversation_id,
            "Startup Conversation",
            transcript.len(),
        )],
        selected_conversation: Some(StartupSelectedConversation {
            conversation_id,
            mode: StartupMode::ModeA {
                transcript_result: StartupTranscriptResult::Success(transcript.clone()),
            },
        }),
    });

    let snapshot = snapshot(&store);
    assert_eq!(snapshot.chat.transcript, transcript);
    assert_eq!(
        snapshot.chat.load_state,
        ConversationLoadState::Ready {
            conversation_id,
            generation: snapshot.chat.selection_generation,
        }
    );
    assert_ne!(
        snapshot.chat.load_state,
        ConversationLoadState::Loading {
            conversation_id,
            generation: snapshot.chat.selection_generation,
        }
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.5
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn stale_generation_payloads_are_ignored() {
    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: Vec::new(),
        selected_conversation: None,
    });
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let gen1 =
        begun_generation(store.begin_selection(first_id, BeginSelectionMode::BatchNoPublish));
    let gen2 =
        begun_generation(store.begin_selection(second_id, BeginSelectionMode::BatchNoPublish));
    let before = snapshot(&store);

    let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: second_id,
        selection_generation: gen1,
        messages: vec![transcript_message(MessageRole::Assistant, "stale")],
    }]);

    assert_eq!(gen1 + 1, gen2);
    assert!(!changed);
    let after = snapshot(&store);
    assert_eq!(after.chat.transcript, before.chat.transcript);
    assert_eq!(
        after.chat.load_state,
        ConversationLoadState::Loading {
            conversation_id: second_id,
            generation: gen2,
        }
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.5
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn stale_generation_failure_is_ignored() {
    let store = GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: Vec::new(),
        selected_conversation: None,
    });
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let gen1 =
        begun_generation(store.begin_selection(first_id, BeginSelectionMode::BatchNoPublish));
    let gen2 =
        begun_generation(store.begin_selection(second_id, BeginSelectionMode::BatchNoPublish));

    let changed = store.reduce_batch(vec![ViewCommand::ConversationLoadFailed {
        conversation_id: second_id,
        selection_generation: gen1,
        message: "stale failure".to_string(),
    }]);

    assert_eq!(gen1 + 1, gen2);
    assert!(!changed);
    let snapshot = snapshot(&store);
    assert_eq!(
        snapshot.chat.load_state,
        ConversationLoadState::Loading {
            conversation_id: second_id,
            generation: gen2,
        }
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.6
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn finalize_stream_durable_proof() {
    let (store, conversation_id, transcript) = selected_store_with_transcript();

    let changed = store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id,
            model_id: "test".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id,
            chunk: "hello".to_string(),
        },
        ViewCommand::FinalizeStream {
            conversation_id,
            tokens: 1,
        },
    ]);

    assert!(changed);
    let snapshot = snapshot(&store);
    assert_eq!(snapshot.chat.transcript.len(), transcript.len() + 1);
    let finalized = snapshot
        .chat
        .transcript
        .last()
        .expect("finalized assistant message should exist");
    assert_eq!(finalized.role, MessageRole::Assistant);
    assert_eq!(finalized.content, "hello");
    assert_eq!(snapshot.chat.streaming.active_target, None);
    assert!(snapshot.chat.streaming.stream_buffer.is_empty());
    assert!(!snapshot.chat.streaming.thinking_visible);
    let duplicate_changed = store.reduce_batch(vec![ViewCommand::MessageAppended {
        conversation_id,
        role: MessageRole::Assistant,
        content: "hello".to_string(),
        model_id: None,
    }]);
    assert!(!duplicate_changed);
    assert_eq!(
        store.current_snapshot().chat.transcript.len(),
        transcript.len() + 1
    );
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.6
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn finalize_stream_nil_resolves_to_active() {
    let (store, conversation_id, transcript) = selected_store_with_transcript();

    let changed = store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: Uuid::nil(),
            model_id: "test".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: Uuid::nil(),
            chunk: "hello".to_string(),
        },
        ViewCommand::FinalizeStream {
            conversation_id: Uuid::nil(),
            tokens: 1,
        },
    ]);

    assert!(changed);
    let snapshot = snapshot(&store);
    assert_eq!(
        snapshot.chat.selected_conversation_id,
        Some(conversation_id)
    );
    assert_eq!(snapshot.chat.transcript.len(), transcript.len() + 1);
    let finalized = snapshot.chat.transcript.last().unwrap();
    assert_eq!(finalized.role, MessageRole::Assistant);
    assert_eq!(finalized.content, "hello");
    assert!(snapshot.chat.streaming.stream_buffer.is_empty());
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.6
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn finalize_stream_stale_target_rejected() {
    let (store, conversation_a, transcript) = selected_store_with_transcript();
    let conversation_b = Uuid::new_v4();

    assert!(store.reduce_batch(vec![
        ViewCommand::ShowThinking {
            conversation_id: conversation_a,
            model_id: "test".to_string(),
        },
        ViewCommand::AppendStream {
            conversation_id: conversation_a,
            chunk: "hello".to_string(),
        },
    ]));
    let before = snapshot(&store);

    let changed = store.reduce_batch(vec![ViewCommand::FinalizeStream {
        conversation_id: conversation_b,
        tokens: 1,
    }]);

    assert!(!changed);
    let after = snapshot(&store);
    assert_eq!(after.chat.transcript.len(), transcript.len());
    assert_eq!(after.chat.transcript, before.chat.transcript);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.7
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn conversation_cleared_does_not_mutate_store() {
    let (store, _conversation_id, transcript) = selected_store_with_transcript();
    let before = snapshot(&store);

    let changed = store.reduce_batch(vec![ViewCommand::ConversationCleared]);

    assert!(!changed);
    let after = snapshot(&store);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.chat.transcript, transcript);
    assert_eq!(after.chat.transcript, before.chat.transcript);
}

/// @plan PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement REQ-ARCH-006.4
/// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:022-060
#[test]
fn show_tool_call_and_update_tool_call_are_transport_passthrough() {
    let (store, conversation_id, _transcript) = selected_store_with_transcript();
    let before = snapshot(&store);

    let changed = store.reduce_batch(vec![
        ViewCommand::ShowToolCall {
            conversation_id,
            tool_name: "shell".to_string(),
            status: "running".to_string(),
        },
        ViewCommand::UpdateToolCall {
            conversation_id,
            tool_name: "shell".to_string(),
            status: "done".to_string(),
            result: Some("ok".to_string()),
            duration: Some(42),
        },
    ]);

    assert!(!changed);
    let after = snapshot(&store);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.chat.transcript, before.chat.transcript);
    assert_eq!(after.chat.load_state, before.chat.load_state);
}
