#![allow(clippy::future_not_send, clippy::unused_async)]

use chrono::Utc;
use gpui::{px, size, AppContext, EntityInputHandler, Render, TestAppContext};
use personal_agent::events::types::UserEvent;
use personal_agent::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole as ViewMessageRole,
    ProfileSummary, ViewCommand,
};
use personal_agent::ui_gpui::app_store::{
    ChatStoreSnapshot, ConversationLoadState, SettingsStoreSnapshot, StreamingStoreSnapshot,
};
use personal_agent::ui_gpui::bridge::GpuiBridge;
use personal_agent::ui_gpui::views::chat_view::{ChatState, ChatView, MessageRole, StreamingState};
use std::sync::Arc;
use uuid::Uuid;

fn conversation(id: Uuid, title: &str, message_count: usize) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now(),
        message_count,
    }
}

fn profile(
    id: Uuid,
    name: &str,
    provider_id: &str,
    model_id: &str,
    is_default: bool,
) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        is_default,
    }
}

fn payload(
    role: ViewMessageRole,
    content: &str,
    thinking_content: Option<&str>,
    timestamp: Option<u64>,
) -> ConversationMessagePayload {
    ConversationMessagePayload {
        role,
        content: content.to_string(),
        thinking_content: thinking_content.map(str::to_string),
        timestamp,
    }
}

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn drain_selection_requests() {
    while personal_agent::ui_gpui::selection_intent_channel()
        .take_pending()
        .is_some()
    {}
}

#[gpui::test]
async fn apply_store_and_settings_snapshots_seed_visible_chat_state(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let default_profile_id = Uuid::new_v4();
    let other_profile_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.apply_settings_snapshot(SettingsStoreSnapshot {
            profiles: vec![
                profile(
                    default_profile_id,
                    "Default",
                    "anthropic",
                    "claude-3-7-sonnet",
                    true,
                ),
                profile(other_profile_id, "Alt", "openai", "gpt-4o", false),
            ],
            selected_profile_id: None,
            settings_visible: false,
        });

        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Loaded Chat".to_string(),
                selection_generation: 3,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 3,
                },
                transcript: vec![
                    payload(ViewMessageRole::User, "hi", None, Some(100)),
                    payload(
                        ViewMessageRole::Assistant,
                        "hello",
                        Some("reasoned"),
                        Some(200),
                    ),
                ],
                streaming: StreamingStoreSnapshot {
                    thinking_visible: true,
                    thinking_buffer: "working".to_string(),
                    stream_buffer: "partial".to_string(),
                    last_error: None,
                    active_target: Some(conversation_id),
                },
                conversations: vec![conversation(conversation_id, "Loaded Chat", 2)],
            },
            cx,
        );
    });

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.active_conversation_id, Some(conversation_id));
        assert_eq!(view.state.conversation_title, "Loaded Chat");
        assert_eq!(view.state.messages.len(), 2);
        assert_eq!(view.state.messages[0].role, MessageRole::User);
        assert_eq!(view.state.messages[0].content, "hi");
        assert_eq!(view.state.messages[1].role, MessageRole::Assistant);
        assert_eq!(view.state.messages[1].content, "hello");
        assert_eq!(view.state.messages[1].thinking.as_deref(), Some("reasoned"));
        assert_eq!(view.state.messages[1].timestamp, Some(200));
        assert_eq!(
            view.state.messages[1].model_id.as_deref(),
            Some("claude-3-7-sonnet")
        );
        assert_eq!(
            view.state.streaming,
            StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            }
        );
        assert_eq!(view.state.thinking_content.as_deref(), Some("working"));
        assert_eq!(view.state.selected_profile_id, Some(default_profile_id));
        assert_eq!(view.state.current_model, "claude-3-7-sonnet");
        assert_eq!(view.state.profile_dropdown_index, 0);
        assert_eq!(view.state.conversation_dropdown_index, 0);
    });
}

#[gpui::test]
async fn conversation_selection_requests_authoritative_switch_and_stops_streaming(
    cx: &mut TestAppContext,
) {
    drain_selection_requests();
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.conversations = vec![
            conversation(first_id, "First", 1),
            conversation(second_id, "Second", 2),
        ];
        view.set_conversation_id(first_id);
        view.state.streaming = StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        };

        view.toggle_conversation_dropdown(cx);
        view.move_conversation_dropdown_selection(1, cx);
        view.confirm_conversation_dropdown_selection(cx);
    });

    assert_eq!(
        user_rx.recv().expect("stop streaming event"),
        UserEvent::StopStreaming
    );
    assert_eq!(
        personal_agent::ui_gpui::selection_intent_channel().take_pending(),
        Some(second_id)
    );
    assert!(
        personal_agent::ui_gpui::selection_intent_channel()
            .take_pending()
            .is_none(),
        "selection should only be requested once"
    );

    view.read_with(cx, |view, _| {
        assert!(!view.state.conversation_dropdown_open);
        assert_eq!(view.state.conversation_dropdown_index, 1);
        assert_eq!(view.state.active_conversation_id, Some(first_id));
        assert!(view.state.chat_autoscroll_enabled);
        assert!(!view.state.conversation_title_editing);
    });
}

#[gpui::test]
async fn profile_dropdown_selection_emits_user_event_and_updates_model(cx: &mut TestAppContext) {
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.apply_settings_snapshot(SettingsStoreSnapshot {
            profiles: vec![
                profile(first_id, "Claude", "anthropic", "claude-3-7-sonnet", true),
                profile(second_id, "GPT", "openai", "gpt-4.1", false),
            ],
            selected_profile_id: Some(first_id),
            settings_visible: false,
        });

        view.toggle_profile_dropdown(cx);
        view.move_profile_dropdown_selection(1, cx);
        view.confirm_profile_dropdown_selection(cx);
    });

    assert_eq!(
        user_rx.recv().expect("profile selection event"),
        UserEvent::SelectChatProfile { id: second_id }
    );

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.selected_profile_id, Some(second_id));
        assert_eq!(view.state.profile_dropdown_index, 1);
        assert_eq!(view.state.current_model, "gpt-4.1");
        assert!(!view.state.profile_dropdown_open);
    });
}

#[gpui::test]
async fn handle_enter_emits_send_message_and_ignores_enter_during_streaming(
    cx: &mut TestAppContext,
) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.chat_autoscroll_enabled = false;
        view.state.input_text = "hello".to_string();
        view.state.cursor_position = view.state.input_text.len();
        view.handle_enter(cx);

        view.state.input_text = "ignored while streaming".to_string();
        view.state.cursor_position = view.state.input_text.len();
        view.handle_enter(cx);
    });
    cx.run_until_parked();

    assert_eq!(
        user_rx.recv().expect("send message event"),
        UserEvent::SendMessage {
            text: "hello".to_string(),
        }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "enter while streaming should not emit another message"
    );

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.input_text, "ignored while streaming");
        assert!(
            view.state.chat_autoscroll_enabled,
            "send should restore sticky autoscroll so streaming stays pinned to the bottom"
        );
        assert_eq!(
            view.state.streaming,
            StreamingState::Streaming {
                content: String::new(),
                done: false,
            }
        );
    });
}

#[gpui::test]
async fn rename_submit_and_cancel_emit_events_and_keep_title_in_sync(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.conversations = vec![conversation(conversation_id, "Original", 1)];
        view.set_conversation_id(conversation_id);

        view.start_rename_conversation(cx);
        view.handle_paste("Renamed", cx);
        view.handle_enter(cx);

        view.start_rename_conversation(cx);
        view.handle_paste("Temporary", cx);
        view.cancel_rename_conversation(cx);
    });

    assert_eq!(
        user_rx.recv().expect("rename confirmation event"),
        UserEvent::ConfirmRenameConversation {
            id: conversation_id,
            title: "Renamed".to_string(),
        }
    );
    assert_eq!(
        user_rx.recv().expect("rename cancellation event"),
        UserEvent::CancelRenameConversation
    );
    assert!(user_rx.try_recv().is_err());

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.conversation_title, "Renamed");
        assert!(!view.state.conversation_title_editing);
        assert!(view.state.conversation_title_input.is_empty());
        assert_eq!(view.state.conversations[0].title, "Renamed");
    });
}

#[gpui::test]
async fn input_editing_and_ime_composition_follow_real_cursor_and_dropdown_rules(
    cx: &mut TestAppContext,
) {
    let conversation_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.input_text = "hié".to_string();
            view.state.cursor_position = view.state.input_text.len();
            view.move_cursor_left(cx);
            assert_eq!(view.state.cursor_position, "hi".len());

            view.handle_paste(" there", cx);
            assert_eq!(view.state.input_text, "hi thereé");
            assert_eq!(view.state.cursor_position, "hi there".len());

            view.handle_backspace(cx);
            assert_eq!(view.state.input_text, "hi theré");
            assert_eq!(view.state.cursor_position, "hi ther".len());

            view.move_cursor_home(cx);
            assert_eq!(view.state.cursor_position, 0);
            view.move_cursor_right(cx);
            assert_eq!(view.state.cursor_position, "h".len());
            view.move_cursor_end(cx);
            assert_eq!(view.state.cursor_position, view.state.input_text.len());
            view.handle_select_all(cx);
            assert_eq!(view.state.cursor_position, view.state.input_text.len());

            assert_eq!(
                view.text_for_range(0..2, &mut None, window, cx),
                Some("hi".to_string())
            );

            view.replace_text_in_range(None, "!", window, cx);
            assert_eq!(view.state.input_text, "hi theré!");
            assert_eq!(view.state.cursor_position, view.state.input_text.len());

            view.replace_and_mark_text_in_range(None, "🙂", Some(1..1), window, cx);
            assert!(view.marked_text_range(window, cx).is_some());
            assert_eq!(view.state.input_text, "hi theré!🙂");
            let utf16_len = "hi theré!🙂".encode_utf16().count();
            assert_eq!(
                view.selected_text_range(false, window, cx)
                    .expect("selection")
                    .range,
                utf16_len..utf16_len
            );

            view.replace_text_in_range(None, " done", window, cx);
            assert_eq!(view.state.input_text, "hi theré! done");
            assert_eq!(view.marked_text_range(window, cx), None);

            view.toggle_conversation_dropdown(cx);
            let blocked_before = view.state.input_text.clone();
            view.handle_paste("ignored", cx);
            view.handle_backspace(cx);
            assert_eq!(view.state.input_text, blocked_before);
            assert!(view.state.conversation_dropdown_open);

            view.toggle_conversation_dropdown(cx);
            assert!(!view.state.conversation_dropdown_open);

            view.toggle_profile_dropdown(cx);
            view.replace_text_in_range(None, "blocked", window, cx);
            assert_eq!(view.state.input_text, blocked_before);
            assert!(view.marked_text_range(window, cx).is_none());
            assert!(view.state.profile_dropdown_open);

            view.toggle_profile_dropdown(cx);
            assert!(!view.state.profile_dropdown_open);

            view.unmark_text(window, cx);
            assert_eq!(view.marked_text_range(window, cx), None);

            view.state.conversations = vec![conversation(conversation_id, "Original", 1)];
            view.set_conversation_id(conversation_id);
            view.start_rename_conversation(cx);
            assert!(view.state.conversation_title_editing);
            assert_eq!(view.state.conversation_title_input, "Original");

            view.replace_text_in_range(None, "Draft", window, cx);
            assert_eq!(view.state.conversation_title_input, "Draft");

            view.replace_and_mark_text_in_range(None, "🙂", Some(1..1), window, cx);
            assert_eq!(view.state.conversation_title_input, "Draft🙂");
            assert_eq!(view.marked_text_range(window, cx), None);

            view.handle_rename_backspace(cx);
            assert_eq!(view.state.conversation_title_input, "Draft");
        });
    });
}

/// After the store reduces `ConversationMessagesLoaded` for the active conversation
/// and `MessageAppended` for the active conversation, the snapshot contains the
/// correct transcript. The view must faithfully render it via `apply_store_snapshot`.
/// Messages targeting inactive conversations are filtered out by the store reducer,
/// so they never appear in the snapshot.
#[gpui::test]
async fn apply_store_snapshot_renders_active_transcript_ignoring_inactive_conversations(
    cx: &mut TestAppContext,
) {
    let active_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.apply_settings_snapshot(SettingsStoreSnapshot {
            profiles: vec![profile(
                Uuid::new_v4(),
                "Default",
                "anthropic",
                "claude-3-7-sonnet",
                true,
            )],
            selected_profile_id: None,
            settings_visible: false,
        });

        // The store would have reduced: ConversationActivated(active_id, gen=2),
        // ConversationMessagesLoaded(active_id, gen=2, [hi, hello]),
        // MessageAppended(active_id, Assistant, "follow-up").
        // The inactive-id messages are dropped by the store, so the snapshot
        // only contains the active conversation's messages.
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(active_id),
                selected_conversation_title: "Active".to_string(),
                selection_generation: 2,
                load_state: ConversationLoadState::Ready {
                    conversation_id: active_id,
                    generation: 2,
                },
                transcript: vec![
                    payload(ViewMessageRole::User, "hi", None, Some(10)),
                    payload(
                        ViewMessageRole::Assistant,
                        "hello",
                        Some("reasoning"),
                        Some(20),
                    ),
                    payload(ViewMessageRole::Assistant, "follow-up", None, None),
                ],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![conversation(active_id, "Active", 3)],
            },
            cx,
        );
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.messages.len(), 3);
        assert_eq!(view.state.messages[0].content, "hi");
        assert_eq!(view.state.messages[1].content, "hello");
        assert_eq!(
            view.state.messages[1].thinking.as_deref(),
            Some("reasoning")
        );
        assert_eq!(view.state.messages[1].timestamp, Some(20));
        assert_eq!(view.state.messages[2].content, "follow-up");
        assert_eq!(
            view.state.messages[2].model_id.as_deref(),
            Some("claude-3-7-sonnet")
        );
        assert_eq!(view.state.streaming, StreamingState::Idle);
        assert_eq!(view.state.thinking_content, None);
        assert!(view.state.chat_autoscroll_enabled);
    });
}

/// The store's `reduce_messages_loaded` rejects stale generation numbers.
/// The snapshot delivered to the view only contains the fresh (matching-generation)
/// messages, so `apply_store_snapshot` renders the correct transcript.
#[gpui::test]
async fn apply_store_snapshot_uses_freshest_generation_transcript(cx: &mut TestAppContext) {
    let active_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        // The store reduced ConversationActivated(active_id, gen=2) then
        // ConversationMessagesLoaded(active_id, gen=2, fresh messages).
        // A stale gen=1 load would have been rejected by the store.
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(active_id),
                selected_conversation_title: "Active".to_string(),
                selection_generation: 2,
                load_state: ConversationLoadState::Ready {
                    conversation_id: active_id,
                    generation: 2,
                },
                transcript: vec![
                    payload(ViewMessageRole::User, "fresh user", None, Some(10)),
                    payload(
                        ViewMessageRole::Assistant,
                        "fresh assistant",
                        None,
                        Some(20),
                    ),
                ],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![conversation(active_id, "Active", 2)],
            },
            cx,
        );
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.messages.len(), 2);
        assert_eq!(view.state.messages[0].content, "fresh user");
        assert_eq!(view.state.messages[1].content, "fresh assistant");
    });
}

/// Streaming, thinking, and profile state are store-managed. The store reduces
/// `ShowThinking`, `AppendThinking`, `AppendStream`, `FinalizeStream`, `StreamCancelled`,
/// `StreamError`, `ChatProfilesUpdated`, and `DefaultProfileChanged` into snapshots.
/// The view renders them via `apply_store_snapshot` and `apply_settings_snapshot`.
/// Only `ToggleThinkingVisibility` remains a view-local command.
#[gpui::test]
#[allow(clippy::too_many_lines)]
async fn streaming_and_profile_updates_arrive_via_store_snapshots(cx: &mut TestAppContext) {
    let first_profile_id = Uuid::new_v4();
    let second_profile_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        // Snapshot after: ShowThinking + AppendThinking("plan") + AppendStream("partial")
        // The store has accumulated thinking buffer and stream buffer.
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Current".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: vec![],
                streaming: StreamingStoreSnapshot {
                    thinking_visible: true,
                    thinking_buffer: "plan".to_string(),
                    stream_buffer: "partial".to_string(),
                    last_error: None,
                    active_target: Some(conversation_id),
                },
                conversations: vec![conversation(conversation_id, "Current", 0)],
            },
            cx,
        );

        // Snapshot after: FinalizeStream consumed the stream buffer into the transcript.
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Current".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: vec![payload(
                    ViewMessageRole::Assistant,
                    "partial",
                    Some("plan"),
                    None,
                )],
                streaming: StreamingStoreSnapshot {
                    thinking_visible: true,
                    thinking_buffer: String::new(),
                    stream_buffer: String::new(),
                    last_error: None,
                    active_target: None,
                },
                conversations: vec![conversation(conversation_id, "Current", 1)],
            },
            cx,
        );

        // Snapshot after: StreamCancelled (partial content goes into transcript as cancelled)
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Current".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: vec![
                    payload(ViewMessageRole::Assistant, "partial", Some("plan"), None),
                    payload(
                        ViewMessageRole::Assistant,
                        "leftover [cancelled]",
                        None,
                        None,
                    ),
                ],
                streaming: StreamingStoreSnapshot {
                    thinking_visible: true,
                    thinking_buffer: String::new(),
                    stream_buffer: String::new(),
                    last_error: None,
                    active_target: None,
                },
                conversations: vec![conversation(conversation_id, "Current", 2)],
            },
            cx,
        );

        // Snapshot after: StreamError("boom")
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Current".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: vec![
                    payload(ViewMessageRole::Assistant, "partial", Some("plan"), None),
                    payload(
                        ViewMessageRole::Assistant,
                        "leftover [cancelled]",
                        None,
                        None,
                    ),
                ],
                streaming: StreamingStoreSnapshot {
                    thinking_visible: true,
                    thinking_buffer: String::new(),
                    stream_buffer: String::new(),
                    last_error: Some("boom".to_string()),
                    active_target: None,
                },
                conversations: vec![conversation(conversation_id, "Current", 2)],
            },
            cx,
        );

        // ToggleThinkingVisibility is view-local, NOT store-managed
        view.handle_command(ViewCommand::ToggleThinkingVisibility, cx);

        // Profile updates come through settings snapshot
        view.apply_settings_snapshot(SettingsStoreSnapshot {
            profiles: vec![
                profile(
                    first_profile_id,
                    "Default",
                    "anthropic",
                    "claude-3-7-sonnet",
                    true,
                ),
                profile(second_profile_id, "GPT", "openai", "gpt-4.1", false),
            ],
            selected_profile_id: None,
            settings_visible: false,
        });

        // DefaultProfileChanged would update the settings snapshot via the store:
        view.apply_settings_snapshot(SettingsStoreSnapshot {
            profiles: vec![
                profile(
                    first_profile_id,
                    "Default",
                    "anthropic",
                    "claude-3-7-sonnet",
                    false,
                ),
                profile(second_profile_id, "GPT", "openai", "gpt-4.1", true),
            ],
            selected_profile_id: Some(second_profile_id),
            settings_visible: false,
        });
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert_eq!(view.state.messages.len(), 2);
        assert_eq!(view.state.messages[0].content, "partial");
        assert_eq!(view.state.messages[0].thinking.as_deref(), Some("plan"));
        assert_eq!(view.state.messages[1].content, "leftover [cancelled]");
        assert_eq!(
            view.state.streaming,
            StreamingState::Error("boom".to_string())
        );
        assert_eq!(view.state.thinking_content, None);
        assert!(view.state.show_thinking);
        assert_eq!(view.state.selected_profile_id, Some(second_profile_id));
        assert_eq!(view.state.current_model, "gpt-4.1");
        assert!(!view.state.profiles[0].is_default);
        assert!(view.state.profiles[1].is_default);
    });
}

/// Conversation lifecycle (create, delete, rename, list refresh) is store-managed.
/// The store reduces these commands and publishes snapshots that the view renders.
/// `ConversationCleared` is the one remaining view-local command.
#[gpui::test]
#[allow(clippy::too_many_lines)]
async fn conversation_lifecycle_via_store_snapshots_and_cleared_resets_ephemeral_state(
    cx: &mut TestAppContext,
) {
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let third_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        // Initial state: first_id selected
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(first_id),
                selected_conversation_title: "First".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id: first_id,
                    generation: 1,
                },
                transcript: vec![payload(ViewMessageRole::User, "existing", None, None)],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![
                    conversation(first_id, "First", 1),
                    conversation(second_id, "", 0),
                ],
            },
            cx,
        );

        // Store reduces ConversationActivated(second_id, gen=4)
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(second_id),
                selected_conversation_title: "Untitled Conversation".to_string(),
                selection_generation: 4,
                load_state: ConversationLoadState::Ready {
                    conversation_id: second_id,
                    generation: 4,
                },
                transcript: vec![],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![
                    conversation(first_id, "First", 1),
                    conversation(second_id, "", 0),
                ],
            },
            cx,
        );

        // Store reduces ConversationCreated(third_id) + ConversationTitleUpdated(third_id, "Fresh")
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(third_id),
                selected_conversation_title: "Fresh".to_string(),
                selection_generation: 5,
                load_state: ConversationLoadState::Ready {
                    conversation_id: third_id,
                    generation: 5,
                },
                transcript: vec![],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![
                    conversation(third_id, "Fresh", 0),
                    conversation(first_id, "First", 1),
                    conversation(second_id, "", 0),
                ],
            },
            cx,
        );

        // Store reduces ConversationDeleted(third_id) → falls back to first conversation
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(first_id),
                selected_conversation_title: "First".to_string(),
                selection_generation: 5,
                load_state: ConversationLoadState::Ready {
                    conversation_id: first_id,
                    generation: 5,
                },
                transcript: vec![payload(ViewMessageRole::User, "existing", None, None)],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![
                    conversation(first_id, "First", 1),
                    conversation(second_id, "", 0),
                ],
            },
            cx,
        );

        // ConversationCleared is the one command still handled directly
        view.handle_command(ViewCommand::ConversationCleared, cx);

        // Store reduces ConversationListRefreshed([second_id])
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: Some(first_id),
                selected_conversation_title: "First".to_string(),
                selection_generation: 5,
                load_state: ConversationLoadState::Ready {
                    conversation_id: first_id,
                    generation: 5,
                },
                transcript: vec![payload(ViewMessageRole::User, "existing", None, None)],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![conversation(second_id, "", 0)],
            },
            cx,
        );

        // Store reduces ConversationListRefreshed([]) — no conversations left
        view.apply_store_snapshot(
            ChatStoreSnapshot {
                selected_conversation_id: None,
                selected_conversation_title: "New Conversation".to_string(),
                selection_generation: 5,
                load_state: ConversationLoadState::Idle,
                transcript: vec![],
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![],
            },
            cx,
        );
    });
    cx.run_until_parked();

    view.read_with(cx, |view, _| {
        assert!(view.state.conversations.is_empty());
        assert_eq!(view.state.active_conversation_id, None);
        assert_eq!(view.state.conversation_title, "New Conversation");
        assert!(view.state.messages.is_empty());
        assert_eq!(view.state.streaming, StreamingState::Idle);
        assert_eq!(view.state.thinking_content, None);
        assert_eq!(view.state.conversation_dropdown_index, 0);
        assert!(!view.state.conversation_dropdown_open);
        assert!(!view.state.conversation_title_editing);
    });
}

#[gpui::test]
async fn chat_composer_layout_keeps_send_button_right_aligned_while_input_grows(
    cx: &mut TestAppContext,
) {
    let (view, visual_cx) =
        cx.add_window_view(|_window, cx| ChatView::new(ChatState::default(), cx));

    visual_cx.simulate_resize(size(px(780.0), px(600.0)));
    visual_cx.run_until_parked();

    let long_input = "layout stability ".repeat(30);

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.input_text = "short".to_string();
            view.state.cursor_position = view.state.input_text.len();
            cx.notify();

            let _ = view.render(window, cx);
        });
    });

    let input_bar_bounds_before = visual_cx
        .debug_bounds("chat-input-bar")
        .expect("chat input bar bounds after initial render");
    let input_bounds_before = visual_cx
        .debug_bounds("chat-input-field")
        .expect("chat input field bounds after initial render");
    let send_bounds_before = visual_cx
        .debug_bounds("chat-send-button")
        .expect("send button bounds after initial render");

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.input_text = long_input.clone();
            view.state.cursor_position = view.state.input_text.len();
            cx.notify();

            let _ = view.render(window, cx);
        });
    });

    let input_bar_bounds_after = visual_cx
        .debug_bounds("chat-input-bar")
        .expect("chat input bar bounds after long input render");
    let input_bounds_after = visual_cx
        .debug_bounds("chat-input-field")
        .expect("chat input field bounds after long input render");
    let send_bounds_after = visual_cx
        .debug_bounds("chat-send-button")
        .expect("send button bounds after long input render");

    assert_eq!(
        send_bounds_before.right(),
        send_bounds_after.right(),
        "send button must not drift horizontally while composing"
    );
    assert_eq!(
        send_bounds_after.right(),
        input_bar_bounds_after.right() - px(12.0),
        "send button should stay pinned to the input bar right padding"
    );

    assert_eq!(
        input_bar_bounds_before.left(),
        input_bar_bounds_after.left(),
        "input bar origin should remain stable"
    );
    assert_eq!(
        input_bounds_before.left(),
        input_bounds_after.left(),
        "composer text field should stay anchored to the left edge"
    );
    assert!(
        input_bounds_after.right() < send_bounds_after.left(),
        "input field must keep horizontal space for the send button"
    );
}
