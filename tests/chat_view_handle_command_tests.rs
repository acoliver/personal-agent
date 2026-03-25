#![allow(clippy::future_not_send, clippy::unused_async)]

use chrono::Utc;
use gpui::{AppContext, EntityInputHandler, TestAppContext};
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

#[gpui::test]
async fn handle_command_ignores_inactive_updates_and_replaces_active_transcript(
    cx: &mut TestAppContext,
) {
    let active_id = Uuid::new_v4();
    let inactive_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.state.current_model = "claude-3-7-sonnet".to_string();
        view.state.messages =
            vec![personal_agent::ui_gpui::views::chat_view::ChatMessage::user("stale")];
        view.state.thinking_content = Some("old".to_string());
        view.state.streaming = StreamingState::Error("old error".to_string());
        view.state.conversations = vec![conversation(active_id, "Active", 1)];
        view.set_conversation_id(active_id);

        view.handle_command(
            ViewCommand::ConversationMessagesLoaded {
                conversation_id: inactive_id,
                selection_generation: 1,
                messages: vec![payload(ViewMessageRole::User, "ignored", None, None)],
            },
            cx,
        );

        view.handle_command(
            ViewCommand::ConversationMessagesLoaded {
                conversation_id: active_id,
                selection_generation: 2,
                messages: vec![
                    payload(ViewMessageRole::User, "hi", None, Some(10)),
                    payload(
                        ViewMessageRole::Assistant,
                        "hello",
                        Some("reasoning"),
                        Some(20),
                    ),
                ],
            },
            cx,
        );

        view.handle_command(
            ViewCommand::MessageAppended {
                conversation_id: inactive_id,
                role: ViewMessageRole::Assistant,
                content: "ignored again".to_string(),
            },
            cx,
        );

        view.handle_command(
            ViewCommand::MessageAppended {
                conversation_id: active_id,
                role: ViewMessageRole::Assistant,
                content: "follow-up".to_string(),
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

#[gpui::test]
async fn handle_command_streaming_and_profile_updates_follow_visible_contract(
    cx: &mut TestAppContext,
) {
    let first_profile_id = Uuid::new_v4();
    let second_profile_id = Uuid::new_v4();
    let conversation_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.state.current_model = "claude-3-7-sonnet".to_string();
        view.state.active_conversation_id = Some(conversation_id);
        view.state.conversations = vec![conversation(conversation_id, "Current", 0)];

        view.handle_command(ViewCommand::ShowThinking { conversation_id }, cx);
        view.handle_command(
            ViewCommand::AppendThinking {
                conversation_id,
                content: "plan".to_string(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::AppendStream {
                conversation_id,
                chunk: "partial".to_string(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::FinalizeStream {
                conversation_id,
                tokens: 42,
            },
            cx,
        );
        view.handle_command(
            ViewCommand::StreamCancelled {
                conversation_id,
                partial_content: "leftover".to_string(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::StreamError {
                conversation_id,
                error: "boom".to_string(),
                recoverable: false,
            },
            cx,
        );
        view.handle_command(ViewCommand::ToggleThinkingVisibility, cx);
        view.handle_command(
            ViewCommand::ChatProfilesUpdated {
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
            },
            cx,
        );
        view.handle_command(
            ViewCommand::DefaultProfileChanged {
                profile_id: Some(second_profile_id),
            },
            cx,
        );
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

#[gpui::test]
async fn handle_command_conversation_lifecycle_maintains_selection_title_and_clear_state(
    cx: &mut TestAppContext,
) {
    let first_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    let third_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view: &mut ChatView, cx| {
        view.state.conversations = vec![
            conversation(first_id, "First", 1),
            conversation(second_id, "", 0),
        ];
        view.set_conversation_id(first_id);
        view.state.messages =
            vec![personal_agent::ui_gpui::views::chat_view::ChatMessage::user("existing")];
        view.state.streaming = StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        };
        view.state.thinking_content = Some("thinking".to_string());

        view.handle_command(
            ViewCommand::ConversationActivated {
                id: second_id,
                selection_generation: 4,
            },
            cx,
        );
        view.handle_command(
            ViewCommand::ConversationCreated {
                id: third_id,
                profile_id: Uuid::new_v4(),
            },
            cx,
        );
        view.handle_command(
            ViewCommand::ConversationTitleUpdated {
                id: third_id,
                title: "Fresh".to_string(),
            },
            cx,
        );
        view.handle_command(ViewCommand::ConversationDeleted { id: third_id }, cx);
        view.handle_command(ViewCommand::ConversationCleared, cx);
        view.handle_command(
            ViewCommand::ConversationListRefreshed {
                conversations: vec![conversation(second_id, "", 0)],
            },
            cx,
        );
        view.handle_command(
            ViewCommand::ConversationListRefreshed {
                conversations: Vec::new(),
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
