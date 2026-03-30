use chrono::Utc;
use uuid::Uuid;

use personal_agent::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary, ViewCommand,
};
use personal_agent::ui_gpui::app_store::{
    BeginSelectionMode, BeginSelectionResult, ChatStoreSnapshot, ConversationLoadState,
    StartupInputs, StartupMode, StartupSelectedConversation, StartupTranscriptResult,
    StreamingStoreSnapshot,
};
use personal_agent::ui_gpui::{GpuiAppSnapshot, GpuiAppStore};

fn make_summary(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
    }
}

fn make_profile(id: Uuid, name: &str, is_default: bool) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: "openai".to_string(),
        model_id: "gpt-4.1".to_string(),
        is_default,
    }
}

fn make_message(role: MessageRole, content: &str) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role,
        content: content.to_string(),
        thinking_content: None,
        timestamp: None,
    }
}

const fn startup_inputs() -> StartupInputs {
    StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: Vec::new(),
        selected_conversation: None,
    }
}

fn make_store_with_conversation(id: Uuid, title: &str) -> GpuiAppStore {
    GpuiAppStore::from_startup_inputs(StartupInputs {
        profiles: Vec::new(),
        selected_profile_id: None,
        conversations: vec![make_summary(id, title, 0)],
        selected_conversation: None,
    })
}

fn begin_and_ready(
    store: &GpuiAppStore,
    id: Uuid,
    messages: Vec<ConversationMessagePayload>,
) -> u64 {
    let generation = match store.begin_selection(id, BeginSelectionMode::BatchNoPublish) {
        BeginSelectionResult::BeganSelection { generation } => generation,
        BeginSelectionResult::NoOpSameSelection => panic!("expected selection to begin"),
    };
    let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
        conversation_id: id,
        selection_generation: generation,
        messages,
    }]);
    assert!(changed, "expected transcript load to change snapshot");
    generation
}

fn current_snapshot(store: &GpuiAppStore) -> GpuiAppSnapshot {
    store.current_snapshot()
}

mod from_startup_inputs_construction {
    use super::*;

    #[test]
    fn empty_inputs_produce_default_snapshot() {
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());

        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.revision, 0);
        assert_eq!(snapshot.chat.selected_conversation_id, None);
        assert_eq!(
            snapshot.chat.selected_conversation_title,
            "New Conversation"
        );
        assert_eq!(snapshot.chat.selection_generation, 0);
        assert_eq!(snapshot.chat.load_state, ConversationLoadState::Idle);
        assert!(snapshot.chat.transcript.is_empty());
        assert_eq!(snapshot.chat.streaming, StreamingStoreSnapshot::default());
        assert!(snapshot.chat.conversations.is_empty());
        assert!(snapshot.history.conversations.is_empty());
        assert_eq!(snapshot.history.selected_conversation_id, None);
        assert!(snapshot.settings.profiles.is_empty());
        assert_eq!(snapshot.settings.selected_profile_id, None);
        assert!(!snapshot.settings.settings_visible);
    }

    #[test]
    fn profiles_and_selected_profile_seed_settings_snapshot() {
        let profile_a = make_profile(Uuid::new_v4(), "Default", true);
        let profile_b = make_profile(Uuid::new_v4(), "Backup", false);
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: vec![profile_a.clone(), profile_b.clone()],
            selected_profile_id: Some(profile_b.id),
            conversations: Vec::new(),
            selected_conversation: None,
        });

        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.revision, 1);
        assert_eq!(
            snapshot.settings.profiles,
            vec![profile_a, profile_b.clone()]
        );
        assert_eq!(snapshot.settings.selected_profile_id, Some(profile_b.id));
        assert!(!snapshot.settings.settings_visible);
    }

    #[test]
    fn conversations_seed_history_and_chat_lists() {
        let a = make_summary(Uuid::new_v4(), "First", 2);
        let b = make_summary(Uuid::new_v4(), "Second", 5);
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![a.clone(), b.clone()],
            selected_conversation: None,
        });

        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.history.conversations, vec![a.clone(), b.clone()]);
        assert_eq!(snapshot.chat.conversations, vec![a, b]);
    }

    #[test]
    fn selected_conversation_with_successful_transcript_is_ready_and_populated() {
        let id = Uuid::new_v4();
        let transcript = vec![
            make_message(MessageRole::User, "hello"),
            make_message(MessageRole::Assistant, "hi"),
        ];
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![make_summary(id, "Chosen", transcript.len())],
            selected_conversation: Some(StartupSelectedConversation {
                conversation_id: id,
                mode: StartupMode::ModeA {
                    transcript_result: StartupTranscriptResult::Success(transcript.clone()),
                },
            }),
        });

        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.chat.selected_conversation_id, Some(id));
        assert_eq!(snapshot.history.selected_conversation_id, Some(id));
        assert_eq!(snapshot.chat.selection_generation, 1);
        assert_eq!(snapshot.chat.selected_conversation_title, "Chosen");
        assert_eq!(
            snapshot.chat.load_state,
            ConversationLoadState::Ready {
                conversation_id: id,
                generation: 1,
            }
        );
        assert_eq!(snapshot.chat.transcript, transcript);
    }

    #[test]
    fn selected_conversation_with_failed_transcript_is_error_and_preserves_message() {
        let id = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![make_summary(id, "Chosen", 0)],
            selected_conversation: Some(StartupSelectedConversation {
                conversation_id: id,
                mode: StartupMode::ModeA {
                    transcript_result: StartupTranscriptResult::Failure("load failed".to_string()),
                },
            }),
        });

        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.revision, 1);
        assert_eq!(
            snapshot.chat.load_state,
            ConversationLoadState::Error {
                conversation_id: id,
                generation: 1,
                message: "load failed".to_string(),
            }
        );
        assert!(snapshot.chat.transcript.is_empty());
    }

    #[test]
    fn startup_batch_sets_revision_to_one_when_it_changes_snapshot() {
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: vec![make_profile(Uuid::new_v4(), "Default", true)],
            selected_profile_id: None,
            conversations: Vec::new(),
            selected_conversation: None,
        });

        assert_eq!(current_snapshot(&store).revision, 1);
    }
}

mod begin_selection_behavior {
    use super::*;

    #[test]
    fn first_selection_begins_generation_one() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "First");

        let result = store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 1 }
        );
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.selected_conversation_id, Some(id));
        assert_eq!(snapshot.chat.selection_generation, 1);
        assert_eq!(snapshot.history.selected_conversation_id, Some(id));
        assert_eq!(snapshot.chat.selected_conversation_title, "First");
        assert_eq!(
            snapshot.chat.load_state,
            ConversationLoadState::Loading {
                conversation_id: id,
                generation: 1,
            }
        );
    }

    #[test]
    fn same_id_reselection_while_loading_is_no_op() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Loading");
        assert_eq!(
            store.begin_selection(id, BeginSelectionMode::BatchNoPublish),
            BeginSelectionResult::BeganSelection { generation: 1 }
        );

        let result = store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(result, BeginSelectionResult::NoOpSameSelection);
        assert_eq!(current_snapshot(&store).chat.selection_generation, 1);
    }

    #[test]
    fn same_id_reselection_while_ready_is_no_op() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Ready");
        begin_and_ready(&store, id, vec![make_message(MessageRole::User, "one")]);

        let result = store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(result, BeginSelectionResult::NoOpSameSelection);
        assert_eq!(current_snapshot(&store).chat.selection_generation, 1);
    }

    #[test]
    fn reselection_from_error_retries_with_new_generation() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Retry");
        let generation = match store.begin_selection(id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::BeganSelection { generation } => generation,
            BeginSelectionResult::NoOpSameSelection => panic!("expected first selection"),
        };
        let changed = store.reduce_batch(vec![ViewCommand::ConversationLoadFailed {
            conversation_id: id,
            selection_generation: generation,
            message: "boom".to_string(),
        }]);
        assert!(changed);

        let result = store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 2 }
        );
        assert_eq!(
            current_snapshot(&store).chat.load_state,
            ConversationLoadState::Loading {
                conversation_id: id,
                generation: 2,
            }
        );
    }

    #[test]
    fn selecting_different_conversation_increments_generation() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![
                make_summary(first, "First", 0),
                make_summary(second, "Second", 0),
            ],
            selected_conversation: None,
        });

        assert_eq!(
            store.begin_selection(first, BeginSelectionMode::BatchNoPublish),
            BeginSelectionResult::BeganSelection { generation: 1 }
        );
        let result = store.begin_selection(second, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 2 }
        );
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.selected_conversation_id, Some(second));
        assert_eq!(snapshot.chat.selection_generation, 2);
        assert_eq!(snapshot.chat.selected_conversation_title, "Second");
    }

    #[test]
    fn publish_immediately_notifies_subscriber_with_snapshot() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Immediate");
        let receiver = store.subscribe();

        let result = store.begin_selection(id, BeginSelectionMode::PublishImmediately);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 1 }
        );
        let published = receiver.try_recv().expect("expected snapshot publication");
        assert_eq!(published.revision, 2);
        assert_eq!(published.chat.selected_conversation_id, Some(id));
    }

    #[test]
    fn batch_no_publish_does_not_notify_subscriber() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Deferred");
        let receiver = store.subscribe();

        let result = store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 1 }
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn selection_clears_streaming_ephemera() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![
                make_summary(first, "First", 0),
                make_summary(second, "Second", 0),
            ],
            selected_conversation: None,
        });
        begin_and_ready(&store, first, Vec::new());
        assert!(store.reduce_batch(vec![
            ViewCommand::ShowThinking {
                conversation_id: first
            },
            ViewCommand::AppendThinking {
                conversation_id: first,
                content: "thinking".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: first,
                chunk: "stream".to_string(),
            },
            ViewCommand::StreamError {
                conversation_id: first,
                error: "transient".to_string(),
                recoverable: true,
            },
        ]));

        let result = store.begin_selection(second, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 2 }
        );
        assert_eq!(
            current_snapshot(&store).chat.streaming,
            StreamingStoreSnapshot::default()
        );
    }

    #[test]
    fn selection_uses_history_title_when_available() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "History Title");

        store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            current_snapshot(&store).chat.selected_conversation_title,
            "History Title"
        );
    }

    #[test]
    fn selection_falls_back_to_untitled_when_not_in_history() {
        let id = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());

        let result = store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        assert_eq!(
            result,
            BeginSelectionResult::BeganSelection { generation: 1 }
        );
        assert_eq!(
            current_snapshot(&store).chat.selected_conversation_title,
            "Untitled Conversation"
        );
    }
}

mod reduce_batch_selection_commands {
    use super::*;

    #[test]
    fn conversation_activated_for_current_selection_and_generation_is_no_op() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Current");
        begin_and_ready(&store, id, Vec::new());
        let before = current_snapshot(&store);

        let changed = store.reduce_batch(vec![ViewCommand::ConversationActivated {
            id,
            selection_generation: before.chat.selection_generation,
        }]);

        assert!(!changed);
        let after = current_snapshot(&store);
        assert_eq!(after.revision, before.revision);
    }

    #[test]
    fn conversation_activated_for_stale_generation_is_ignored() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Current");
        begin_and_ready(&store, id, Vec::new());
        let before = current_snapshot(&store);

        let changed = store.reduce_batch(vec![ViewCommand::ConversationActivated {
            id,
            selection_generation: before.chat.selection_generation - 1,
        }]);

        assert!(!changed);
        assert_eq!(current_snapshot(&store).revision, before.revision);
    }

    #[test]
    fn conversation_messages_loaded_for_matching_selection_replaces_transcript_and_sets_ready() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        let generation = match store.begin_selection(id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::BeganSelection { generation } => generation,
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection"),
        };
        let transcript = vec![make_message(MessageRole::Assistant, "loaded")];

        let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id: id,
            selection_generation: generation,
            messages: transcript.clone(),
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.transcript, transcript);
        assert_eq!(
            snapshot.chat.load_state,
            ConversationLoadState::Ready {
                conversation_id: id,
                generation,
            }
        );
    }

    #[test]
    fn conversation_messages_loaded_with_wrong_conversation_is_ignored() {
        let id = Uuid::new_v4();
        let wrong = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        let generation = match store.begin_selection(id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::BeganSelection { generation } => generation,
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection"),
        };

        let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id: wrong,
            selection_generation: generation,
            messages: vec![make_message(MessageRole::Assistant, "ignored")],
        }]);

        assert!(!changed);
        assert!(current_snapshot(&store).chat.transcript.is_empty());
    }

    #[test]
    fn conversation_messages_loaded_with_stale_generation_is_ignored() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        let changed = store.reduce_batch(vec![ViewCommand::ConversationMessagesLoaded {
            conversation_id: id,
            selection_generation: 0,
            messages: vec![make_message(MessageRole::Assistant, "ignored")],
        }]);

        assert!(!changed);
        assert!(current_snapshot(&store).chat.transcript.is_empty());
    }

    #[test]
    fn conversation_load_failed_for_matching_selection_sets_error_message() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        let generation = match store.begin_selection(id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::BeganSelection { generation } => generation,
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection"),
        };

        let changed = store.reduce_batch(vec![ViewCommand::ConversationLoadFailed {
            conversation_id: id,
            selection_generation: generation,
            message: "broken".to_string(),
        }]);

        assert!(changed);
        assert_eq!(
            current_snapshot(&store).chat.load_state,
            ConversationLoadState::Error {
                conversation_id: id,
                generation,
                message: "broken".to_string(),
            }
        );
    }

    #[test]
    fn conversation_load_failed_with_wrong_id_is_ignored() {
        let id = Uuid::new_v4();
        let wrong = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        let generation = match store.begin_selection(id, BeginSelectionMode::BatchNoPublish) {
            BeginSelectionResult::BeganSelection { generation } => generation,
            BeginSelectionResult::NoOpSameSelection => panic!("expected selection"),
        };

        let changed = store.reduce_batch(vec![ViewCommand::ConversationLoadFailed {
            conversation_id: wrong,
            selection_generation: generation,
            message: "ignored".to_string(),
        }]);

        assert!(!changed);
        assert_eq!(
            current_snapshot(&store).chat.load_state,
            ConversationLoadState::Loading {
                conversation_id: id,
                generation,
            }
        );
    }

    #[test]
    fn conversation_load_failed_with_stale_generation_is_ignored() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        store.begin_selection(id, BeginSelectionMode::BatchNoPublish);

        let changed = store.reduce_batch(vec![ViewCommand::ConversationLoadFailed {
            conversation_id: id,
            selection_generation: 0,
            message: "ignored".to_string(),
        }]);

        assert!(!changed);
        assert_eq!(current_snapshot(&store).chat.selection_generation, 1);
    }

    #[test]
    fn empty_batch_returns_false_and_does_not_bump_revision() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Selected");
        let before = current_snapshot(&store).revision;

        let changed = store.reduce_batch(Vec::new());

        assert!(!changed);
        assert_eq!(current_snapshot(&store).revision, before);
    }
}

mod reduce_batch_streaming_and_thinking_commands {
    use super::*;

    #[test]
    fn show_thinking_sets_visible_and_active_target() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());

        let changed = store.reduce_batch(vec![ViewCommand::ShowThinking {
            conversation_id: id,
        }]);

        assert!(changed);
        let streaming = current_snapshot(&store).chat.streaming;
        assert!(streaming.thinking_visible);
        assert_eq!(streaming.active_target, Some(id));
    }

    #[test]
    fn hide_thinking_clears_visibility() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![ViewCommand::ShowThinking {
            conversation_id: id
        }]));

        let changed = store.reduce_batch(vec![ViewCommand::HideThinking {
            conversation_id: id,
        }]);

        assert!(changed);
        assert!(!current_snapshot(&store).chat.streaming.thinking_visible);
    }

    #[test]
    fn append_thinking_appends_to_buffer() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());

        let changed = store.reduce_batch(vec![
            ViewCommand::AppendThinking {
                conversation_id: id,
                content: "foo".to_string(),
            },
            ViewCommand::AppendThinking {
                conversation_id: id,
                content: "bar".to_string(),
            },
        ]);

        assert!(changed);
        let streaming = current_snapshot(&store).chat.streaming;
        assert_eq!(streaming.thinking_buffer, "foobar");
        assert!(streaming.thinking_visible);
    }

    #[test]
    fn append_stream_appends_to_buffer() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());

        let changed = store.reduce_batch(vec![
            ViewCommand::AppendStream {
                conversation_id: id,
                chunk: "foo".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: id,
                chunk: "bar".to_string(),
            },
        ]);

        assert!(changed);
        let streaming = current_snapshot(&store).chat.streaming;
        assert_eq!(streaming.stream_buffer, "foobar");
        assert_eq!(streaming.active_target, Some(id));
    }

    #[test]
    fn finalize_stream_materializes_assistant_message_and_sets_finalize_guard() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![
            ViewCommand::AppendThinking {
                conversation_id: id,
                content: "plan".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: id,
                chunk: "answer".to_string(),
            },
        ]));

        let changed = store.reduce_batch(vec![ViewCommand::FinalizeStream {
            conversation_id: id,
            tokens: 42,
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.transcript.len(), 1);
        assert_eq!(snapshot.chat.transcript[0].role, MessageRole::Assistant);
        assert_eq!(snapshot.chat.transcript[0].content, "answer");
        assert_eq!(
            snapshot.chat.transcript[0].thinking_content.as_deref(),
            Some("plan")
        );
        assert_eq!(snapshot.chat.streaming, StreamingStoreSnapshot::default());

        let deduped = store.reduce_batch(vec![ViewCommand::MessageAppended {
            conversation_id: id,
            role: MessageRole::Assistant,
            content: "answer".to_string(),
        }]);
        assert!(!deduped);
        assert_eq!(current_snapshot(&store).chat.transcript.len(), 1);
    }

    #[test]
    fn finalize_stream_with_empty_buffer_is_no_op() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![ViewCommand::ShowThinking {
            conversation_id: id
        }]));

        let changed = store.reduce_batch(vec![ViewCommand::FinalizeStream {
            conversation_id: id,
            tokens: 0,
        }]);

        assert!(!changed);
        assert!(current_snapshot(&store).chat.transcript.is_empty());
    }

    #[test]
    fn stream_cancelled_clears_ephemera() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![
            ViewCommand::AppendThinking {
                conversation_id: id,
                content: "abc".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: id,
                chunk: "def".to_string(),
            },
        ]));

        let changed = store.reduce_batch(vec![ViewCommand::StreamCancelled {
            conversation_id: id,
            partial_content: "def".to_string(),
        }]);

        assert!(changed);
        assert_eq!(
            current_snapshot(&store).chat.streaming,
            StreamingStoreSnapshot::default()
        );
    }

    #[test]
    fn stream_error_clears_ephemera_and_sets_last_error() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![
            ViewCommand::AppendThinking {
                conversation_id: id,
                content: "abc".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: id,
                chunk: "def".to_string(),
            },
        ]));

        let changed = store.reduce_batch(vec![ViewCommand::StreamError {
            conversation_id: id,
            error: "network".to_string(),
            recoverable: false,
        }]);

        assert!(changed);
        let streaming = current_snapshot(&store).chat.streaming;
        assert_eq!(streaming.last_error.as_deref(), Some("network"));
        assert!(!streaming.thinking_visible);
        assert!(streaming.stream_buffer.is_empty());
        assert!(streaming.thinking_buffer.is_empty());
        assert_eq!(streaming.active_target, None);
    }

    #[test]
    fn streaming_commands_for_wrong_conversation_are_ignored() {
        let selected = Uuid::new_v4();
        let wrong = Uuid::new_v4();
        let store = make_store_with_conversation(selected, "Chat");
        begin_and_ready(&store, selected, Vec::new());

        let changed = store.reduce_batch(vec![
            ViewCommand::ShowThinking {
                conversation_id: wrong,
            },
            ViewCommand::AppendThinking {
                conversation_id: wrong,
                content: "abc".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: wrong,
                chunk: "def".to_string(),
            },
            ViewCommand::FinalizeStream {
                conversation_id: wrong,
                tokens: 1,
            },
            ViewCommand::StreamCancelled {
                conversation_id: wrong,
                partial_content: String::new(),
            },
            ViewCommand::StreamError {
                conversation_id: wrong,
                error: "err".to_string(),
                recoverable: true,
            },
        ]);

        assert!(!changed);
        assert_eq!(
            current_snapshot(&store).chat.streaming,
            StreamingStoreSnapshot::default()
        );
    }

    #[test]
    fn nil_conversation_id_resolves_to_active_target() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![
            ViewCommand::ShowThinking {
                conversation_id: id
            },
            ViewCommand::AppendThinking {
                conversation_id: Uuid::nil(),
                content: "idea".to_string(),
            },
            ViewCommand::AppendStream {
                conversation_id: Uuid::nil(),
                chunk: "done".to_string(),
            },
            ViewCommand::FinalizeStream {
                conversation_id: Uuid::nil(),
                tokens: 3,
            },
        ]));

        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.transcript.len(), 1);
        assert_eq!(snapshot.chat.transcript[0].content, "done");
        assert_eq!(
            snapshot.chat.transcript[0].thinking_content.as_deref(),
            Some("idea")
        );
    }
}

mod reduce_batch_message_append {
    use super::*;

    #[test]
    fn message_appended_for_selected_conversation_updates_transcript() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());

        let changed = store.reduce_batch(vec![ViewCommand::MessageAppended {
            conversation_id: id,
            role: MessageRole::User,
            content: "hello".to_string(),
        }]);

        assert!(changed);
        assert_eq!(
            current_snapshot(&store).chat.transcript,
            vec![make_message(MessageRole::User, "hello")]
        );
    }

    #[test]
    fn message_appended_for_wrong_conversation_is_ignored() {
        let selected = Uuid::new_v4();
        let wrong = Uuid::new_v4();
        let store = make_store_with_conversation(selected, "Chat");
        begin_and_ready(&store, selected, Vec::new());

        let changed = store.reduce_batch(vec![ViewCommand::MessageAppended {
            conversation_id: wrong,
            role: MessageRole::Assistant,
            content: "ignored".to_string(),
        }]);

        assert!(!changed);
        assert!(current_snapshot(&store).chat.transcript.is_empty());
    }

    #[test]
    fn assistant_message_matching_finalize_guard_is_deduplicated() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        assert!(store.reduce_batch(vec![
            ViewCommand::AppendStream {
                conversation_id: id,
                chunk: "same".to_string(),
            },
            ViewCommand::FinalizeStream {
                conversation_id: id,
                tokens: 1,
            },
        ]));

        let changed = store.reduce_batch(vec![ViewCommand::MessageAppended {
            conversation_id: id,
            role: MessageRole::Assistant,
            content: "same".to_string(),
        }]);

        assert!(!changed);
        assert_eq!(current_snapshot(&store).chat.transcript.len(), 1);
    }
}

mod reduce_batch_conversation_lifecycle {
    use super::*;

    #[test]
    fn conversation_created_adds_to_lists_selects_and_sets_ready_empty_transcript() {
        let existing = make_summary(Uuid::new_v4(), "Existing", 1);
        let new_id = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![existing.clone()],
            selected_conversation: None,
        });

        let changed = store.reduce_batch(vec![ViewCommand::ConversationCreated {
            id: new_id,
            profile_id: Uuid::new_v4(),
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.history.conversations[0].id, new_id);
        assert_eq!(snapshot.chat.conversations[0].id, new_id);
        assert_eq!(snapshot.chat.selected_conversation_id, Some(new_id));
        assert_eq!(snapshot.history.selected_conversation_id, Some(new_id));
        assert_eq!(
            snapshot.chat.selected_conversation_title,
            "New Conversation"
        );
        assert!(snapshot.chat.transcript.is_empty());
        assert_eq!(
            snapshot.chat.load_state,
            ConversationLoadState::Ready {
                conversation_id: new_id,
                generation: 1,
            }
        );
        assert_eq!(snapshot.history.conversations[1], existing);
    }

    #[test]
    fn deleting_selected_conversation_selects_next_and_clears_transcript_if_none_left() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![
                make_summary(first, "First", 0),
                make_summary(second, "Second", 0),
            ],
            selected_conversation: None,
        });
        begin_and_ready(&store, first, vec![make_message(MessageRole::User, "keep")]);

        assert!(store.reduce_batch(vec![ViewCommand::ConversationDeleted { id: first }]));
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.selected_conversation_id, Some(second));
        assert_eq!(snapshot.history.selected_conversation_id, Some(second));
        assert_eq!(snapshot.chat.selected_conversation_title, "Second");

        assert!(store.reduce_batch(vec![ViewCommand::ConversationDeleted { id: second }]));
        let empty_snapshot = current_snapshot(&store);
        assert_eq!(empty_snapshot.chat.selected_conversation_id, None);
        assert_eq!(empty_snapshot.history.selected_conversation_id, None);
        assert_eq!(
            empty_snapshot.chat.selected_conversation_title,
            "New Conversation"
        );
        assert_eq!(empty_snapshot.chat.load_state, ConversationLoadState::Idle);
        assert!(empty_snapshot.chat.transcript.is_empty());
        assert_eq!(
            empty_snapshot.chat.streaming,
            StreamingStoreSnapshot::default()
        );
    }

    #[test]
    fn deleting_non_selected_conversation_only_removes_it_from_lists() {
        let selected = Uuid::new_v4();
        let other = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(StartupInputs {
            profiles: Vec::new(),
            selected_profile_id: None,
            conversations: vec![
                make_summary(selected, "Selected", 0),
                make_summary(other, "Other", 0),
            ],
            selected_conversation: None,
        });
        begin_and_ready(
            &store,
            selected,
            vec![make_message(MessageRole::User, "keep")],
        );

        let changed = store.reduce_batch(vec![ViewCommand::ConversationDeleted { id: other }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.chat.selected_conversation_id, Some(selected));
        assert_eq!(
            snapshot.chat.transcript,
            vec![make_message(MessageRole::User, "keep")]
        );
        assert_eq!(snapshot.history.conversations.len(), 1);
        assert_eq!(snapshot.chat.conversations.len(), 1);
        assert_eq!(snapshot.history.conversations[0].id, selected);
    }

    #[test]
    fn conversation_renamed_updates_history_chat_and_selected_title() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Old");
        begin_and_ready(&store, id, Vec::new());

        let changed = store.reduce_batch(vec![ViewCommand::ConversationRenamed {
            id,
            new_title: "Renamed".to_string(),
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.history.conversations[0].title, "Renamed");
        assert_eq!(snapshot.chat.conversations[0].title, "Renamed");
        assert_eq!(snapshot.chat.selected_conversation_title, "Renamed");
    }

    #[test]
    fn conversation_title_updated_behaves_like_renamed() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Old");
        begin_and_ready(&store, id, Vec::new());

        let changed = store.reduce_batch(vec![ViewCommand::ConversationTitleUpdated {
            id,
            title: "Updated".to_string(),
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.history.conversations[0].title, "Updated");
        assert_eq!(snapshot.chat.conversations[0].title, "Updated");
        assert_eq!(snapshot.chat.selected_conversation_title, "Updated");
    }

    #[test]
    fn conversation_list_refreshed_replaces_history_and_chat_lists() {
        let selected_id = Uuid::new_v4();
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());
        let fallback_selection =
            store.begin_selection(selected_id, BeginSelectionMode::BatchNoPublish);
        assert_eq!(
            fallback_selection,
            BeginSelectionResult::BeganSelection { generation: 1 }
        );
        assert_eq!(
            current_snapshot(&store).chat.selected_conversation_title,
            "Untitled Conversation"
        );
        let refreshed = make_summary(selected_id, "Recovered Title", 2);
        let other = make_summary(Uuid::new_v4(), "Other", 4);

        let changed = store.reduce_batch(vec![ViewCommand::ConversationListRefreshed {
            conversations: vec![refreshed.clone(), other.clone()],
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(
            snapshot.history.conversations,
            vec![refreshed.clone(), other.clone()]
        );
        assert_eq!(snapshot.chat.conversations, vec![refreshed, other]);
        assert_eq!(snapshot.chat.selected_conversation_title, "Recovered Title");
    }
}

mod reduce_batch_profiles_and_settings {
    use super::*;

    #[test]
    fn chat_profiles_updated_sets_profiles_and_selected_profile_id() {
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());
        let profile = make_profile(Uuid::new_v4(), "Default", true);

        let changed = store.reduce_batch(vec![ViewCommand::ChatProfilesUpdated {
            profiles: vec![profile.clone()],
            selected_profile_id: Some(profile.id),
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert_eq!(snapshot.settings.profiles, vec![profile.clone()]);
        assert_eq!(snapshot.settings.selected_profile_id, Some(profile.id));
    }

    #[test]
    fn show_settings_when_not_visible_sets_visible_and_profiles() {
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());
        let profile = make_profile(Uuid::new_v4(), "Visible", true);

        let changed = store.reduce_batch(vec![ViewCommand::ShowSettings {
            profiles: vec![profile.clone()],
            selected_profile_id: None,
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert!(snapshot.settings.settings_visible);
        assert_eq!(snapshot.settings.profiles, vec![profile]);
        assert_eq!(snapshot.settings.selected_profile_id, None);
    }

    #[test]
    fn show_settings_when_already_visible_updates_profiles() {
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());
        assert!(store.reduce_batch(vec![ViewCommand::ShowSettings {
            profiles: Vec::new(),
            selected_profile_id: None,
        }]));
        let profile = make_profile(Uuid::new_v4(), "Updated", false);

        let changed = store.reduce_batch(vec![ViewCommand::ShowSettings {
            profiles: vec![profile.clone()],
            selected_profile_id: Some(profile.id),
        }]);

        assert!(changed);
        let snapshot = current_snapshot(&store);
        assert!(snapshot.settings.settings_visible);
        assert_eq!(snapshot.settings.profiles, vec![profile.clone()]);
        assert_eq!(snapshot.settings.selected_profile_id, Some(profile.id));
    }
}

mod subscription_and_revision_behavior {
    use super::*;

    #[test]
    fn subscribe_receiver_gets_snapshot_on_next_publish() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        let receiver = store.subscribe();

        let changed = store.reduce_batch(vec![ViewCommand::ConversationCreated {
            id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
        }]);

        assert!(changed);
        let snapshot = receiver.try_recv().expect("expected a published snapshot");
        assert_eq!(snapshot.revision, current_snapshot(&store).revision);
    }

    #[test]
    fn multiple_subscribers_all_receive_publications() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        let first = store.subscribe();
        let second = store.subscribe();

        assert!(store.reduce_batch(vec![ViewCommand::ConversationCreated {
            id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
        }]));

        let first_snapshot = first
            .try_recv()
            .expect("first subscriber should receive snapshot");
        let second_snapshot = second
            .try_recv()
            .expect("second subscriber should receive snapshot");
        assert_eq!(first_snapshot.revision, second_snapshot.revision);
    }

    #[test]
    fn reduce_batch_with_changes_bumps_revision_and_notifies_subscribers() {
        let id = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        let receiver = store.subscribe();
        let before_revision = current_snapshot(&store).revision;

        let changed = store.reduce_batch(vec![ViewCommand::MessageAppended {
            conversation_id: id,
            role: MessageRole::User,
            content: "hello".to_string(),
        }]);

        assert!(changed);
        let published = receiver.try_recv().expect("expected publication");
        assert_eq!(published.revision, before_revision + 1);
        assert_eq!(current_snapshot(&store).revision, before_revision + 1);
    }

    #[test]
    fn reduce_batch_no_op_does_not_bump_revision_or_notify() {
        let id = Uuid::new_v4();
        let wrong = Uuid::new_v4();
        let store = make_store_with_conversation(id, "Chat");
        begin_and_ready(&store, id, Vec::new());
        let receiver = store.subscribe();
        let before_revision = current_snapshot(&store).revision;

        let changed = store.reduce_batch(vec![ViewCommand::MessageAppended {
            conversation_id: wrong,
            role: MessageRole::User,
            content: "ignored".to_string(),
        }]);

        assert!(!changed);
        assert_eq!(current_snapshot(&store).revision, before_revision);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn subscriber_count_reflects_connected_subscribers() {
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());
        let first = store.subscribe();
        let second = store.subscribe();

        assert_eq!(store.subscriber_count(), 2);
        drop(first);
        assert_eq!(store.subscriber_count(), 1);
        drop(second);
        assert_eq!(store.subscriber_count(), 0);
    }

    #[test]
    fn prune_disconnected_subscribers_removes_dropped_receivers() {
        let store = GpuiAppStore::from_startup_inputs(startup_inputs());
        let kept = store.subscribe();
        let dropped = store.subscribe();
        assert_eq!(store.subscriber_count(), 2);
        drop(dropped);

        store.prune_disconnected_subscribers();

        assert_eq!(store.subscriber_count(), 1);
        assert!(store.reduce_batch(vec![ViewCommand::ShowSettings {
            profiles: Vec::new(),
            selected_profile_id: None,
        }]));
        assert!(kept.try_recv().is_ok());
    }
}

#[test]
fn restore_after_clear_returns_current_snapshot() {
    let id = Uuid::new_v4();
    let store = make_store_with_conversation(id, "Restore");
    begin_and_ready(
        &store,
        id,
        vec![make_message(MessageRole::Assistant, "hello")],
    );

    let restored = store.restore_after_clear();

    assert_eq!(restored.revision, current_snapshot(&store).revision);
    assert_eq!(
        restored.chat.transcript,
        current_snapshot(&store).chat.transcript
    );
}

#[test]
fn public_snapshot_type_is_observable_from_store() {
    let store = GpuiAppStore::from_startup_inputs(startup_inputs());

    let snapshot: GpuiAppSnapshot = store.current_snapshot();
    let _chat: ChatStoreSnapshot = snapshot.chat.clone();

    assert_eq!(snapshot.revision, 0);
}

/// Every command variant that `reduce_view_command_without_publish` handles
/// must be declared store-managed so the bridge pump filters it from the
/// direct-forward path. This test prevents regressions (e.g. the
/// `ConversationLoadFailed` gap that was caught during review).
mod is_store_managed_covers_all_reduced_variants {
    use personal_agent::presentation::view_command::ViewCommand;
    use personal_agent::ui_gpui::app_store::is_store_managed;
    use uuid::Uuid;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn conversation_list_refreshed_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationListRefreshed {
            conversations: Vec::new()
        }));
    }

    #[test]
    fn conversation_activated_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationActivated {
            id: id(),
            selection_generation: 1,
        }));
    }

    #[test]
    fn conversation_created_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationCreated {
            id: id(),
            profile_id: id(),
        }));
    }

    #[test]
    fn conversation_deleted_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationDeleted {
            id: id()
        }));
    }

    #[test]
    fn conversation_renamed_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationRenamed {
            id: id(),
            new_title: "x".to_string(),
        }));
    }

    #[test]
    fn conversation_title_updated_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationTitleUpdated {
            id: id(),
            title: "x".to_string(),
        }));
    }

    #[test]
    fn conversation_messages_loaded_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationMessagesLoaded {
            conversation_id: id(),
            selection_generation: 1,
            messages: Vec::new(),
        }));
    }

    #[test]
    fn conversation_load_failed_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ConversationLoadFailed {
            conversation_id: id(),
            selection_generation: 1,
            message: "err".to_string(),
        }));
    }

    #[test]
    fn message_appended_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::MessageAppended {
            conversation_id: id(),
            role: personal_agent::presentation::view_command::MessageRole::User,
            content: "hi".to_string(),
        }));
    }

    #[test]
    fn show_thinking_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ShowThinking {
            conversation_id: id(),
        }));
    }

    #[test]
    fn hide_thinking_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::HideThinking {
            conversation_id: id(),
        }));
    }

    #[test]
    fn append_thinking_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::AppendThinking {
            conversation_id: id(),
            content: "x".to_string(),
        }));
    }

    #[test]
    fn append_stream_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::AppendStream {
            conversation_id: id(),
            chunk: "x".to_string(),
        }));
    }

    #[test]
    fn finalize_stream_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::FinalizeStream {
            conversation_id: id(),
            tokens: 1,
        }));
    }

    #[test]
    fn stream_cancelled_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::StreamCancelled {
            conversation_id: id(),
            partial_content: String::new(),
        }));
    }

    #[test]
    fn stream_error_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::StreamError {
            conversation_id: id(),
            error: "x".to_string(),
            recoverable: false,
        }));
    }

    #[test]
    fn chat_profiles_updated_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ChatProfilesUpdated {
            profiles: Vec::new(),
            selected_profile_id: None,
        }));
    }

    #[test]
    fn show_settings_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::ShowSettings {
            profiles: Vec::new(),
            selected_profile_id: None,
        }));
    }

    #[test]
    fn default_profile_changed_is_store_managed() {
        assert!(is_store_managed(&ViewCommand::DefaultProfileChanged {
            profile_id: Some(id()),
        }));
    }

    #[test]
    fn non_store_commands_are_not_managed() {
        assert!(!is_store_managed(&ViewCommand::ConversationCleared));
        assert!(!is_store_managed(&ViewCommand::ToggleThinkingVisibility));
        assert!(!is_store_managed(&ViewCommand::NavigateTo {
            view: personal_agent::presentation::view_command::ViewId::Chat
        }));
        assert!(!is_store_managed(&ViewCommand::NavigateBack));
        assert!(!is_store_managed(&ViewCommand::ShowNotification {
            message: "hi".to_string()
        }));
        assert!(!is_store_managed(&ViewCommand::ShowError {
            title: "t".to_string(),
            message: "m".to_string(),
            severity: personal_agent::presentation::view_command::ErrorSeverity::Error,
        }));
    }
}
