#![allow(clippy::future_not_send)]

use super::*;
use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole,
};
use crate::ui_gpui::app_store::StreamingStoreSnapshot;
use chrono::Utc;
use gpui::{
    point, AppContext, KeyDownEvent, Keystroke, Modifiers, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TouchPhase,
};

fn chat_key_event(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
async fn page_scroll_helpers_disable_and_reenable_autoscroll(cx: &mut gpui::TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.chat_autoscroll_enabled = true;
            view.scroll_chat_page_up(cx);
            assert!(!view.state.chat_autoscroll_enabled);

            view.state.chat_autoscroll_enabled = true;
            view.scroll_chat_to_top(cx);
            assert!(!view.state.chat_autoscroll_enabled);

            view.state.chat_autoscroll_enabled = false;
            view.scroll_chat_to_end(cx);
            assert!(view.state.chat_autoscroll_enabled);
        });
    });
}

#[gpui::test]
async fn home_pageup_pagedown_and_end_keys_control_chat_scroll_autoscroll(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.chat_autoscroll_enabled = true;
            view.handle_key_down(&chat_key_event("home"), cx);
            assert!(!view.state.chat_autoscroll_enabled);

            view.state.chat_autoscroll_enabled = true;
            view.handle_key_down(&chat_key_event("pageup"), cx);
            assert!(!view.state.chat_autoscroll_enabled);

            view.state.chat_autoscroll_enabled = true;
            view.handle_key_down(&chat_key_event("pagedown"), cx);
            assert!(view.state.chat_autoscroll_enabled);

            view.state.chat_autoscroll_enabled = false;
            view.handle_key_down(&chat_key_event("end"), cx);
            assert!(view.state.chat_autoscroll_enabled);

            view.state.chat_autoscroll_enabled = false;
            view.handle_key_down(
                &KeyDownEvent {
                    keystroke: Keystroke {
                        modifiers: Modifiers {
                            platform: true,
                            ..Modifiers::default()
                        },
                        ..chat_key_event("right").keystroke
                    },
                    ..chat_key_event("right")
                },
                cx,
            );
            assert!(view.state.chat_autoscroll_enabled);
        });
    });
}

#[gpui::test]
async fn apply_store_snapshot_streaming_calls_maybe_scroll_when_autoscroll_enabled(
    cx: &mut TestAppContext,
) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let conversation_id = Uuid::new_v4();
            let snapshot = ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Conv".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: Vec::new(),
                streaming: StreamingStoreSnapshot {
                    stream_buffer: "partial".to_string(),
                    active_target: Some(conversation_id),
                    ..StreamingStoreSnapshot::default()
                },
                conversations: vec![ConversationSummary {
                    id: conversation_id,
                    title: "Conv".to_string(),
                    updated_at: Utc::now(),
                    message_count: 0,
                }],
            };

            view.conversation_id = Some(conversation_id);
            view.selection_generation = 1;
            view.state.chat_autoscroll_enabled = true;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.apply_store_snapshot(snapshot, cx);

            assert!(matches!(
                view.state.streaming,
                StreamingState::Streaming { .. }
            ));
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 1);
        });
    });
}

#[gpui::test]
async fn apply_store_snapshot_streaming_skips_maybe_scroll_when_autoscroll_disabled(
    cx: &mut TestAppContext,
) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let conversation_id = Uuid::new_v4();
            let snapshot = ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Conv".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: Vec::new(),
                streaming: StreamingStoreSnapshot {
                    stream_buffer: "partial".to_string(),
                    active_target: Some(conversation_id),
                    ..StreamingStoreSnapshot::default()
                },
                conversations: vec![ConversationSummary {
                    id: conversation_id,
                    title: "Conv".to_string(),
                    updated_at: Utc::now(),
                    message_count: 0,
                }],
            };

            view.conversation_id = Some(conversation_id);
            view.selection_generation = 1;
            view.state.chat_autoscroll_enabled = false;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.apply_store_snapshot(snapshot, cx);

            assert!(matches!(
                view.state.streaming,
                StreamingState::Streaming { .. }
            ));
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 0);
        });
    });
}

#[gpui::test]
async fn apply_store_snapshot_stream_finalize_calls_maybe_scroll_when_autoscroll_enabled(
    cx: &mut TestAppContext,
) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let conversation_id = Uuid::new_v4();
            let finalized_transcript = vec![ConversationMessagePayload {
                role: MessageRole::Assistant,
                content: "final response".to_string(),
                thinking_content: None,
                timestamp: None,
            }];
            let snapshot = ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Conv".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: finalized_transcript,
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![ConversationSummary {
                    id: conversation_id,
                    title: "Conv".to_string(),
                    updated_at: Utc::now(),
                    message_count: 1,
                }],
            };

            view.conversation_id = Some(conversation_id);
            view.selection_generation = 1;
            view.state.chat_autoscroll_enabled = true;
            view.state.streaming = StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            };
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.apply_store_snapshot(snapshot, cx);

            assert_eq!(view.state.streaming, StreamingState::Idle);
            assert_eq!(view.state.messages.len(), 1);
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 1);
        });
    });
}

#[gpui::test]
async fn apply_store_snapshot_stream_finalize_skips_maybe_scroll_when_autoscroll_disabled(
    cx: &mut TestAppContext,
) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let conversation_id = Uuid::new_v4();
            let finalized_transcript = vec![ConversationMessagePayload {
                role: MessageRole::Assistant,
                content: "final response".to_string(),
                thinking_content: None,
                timestamp: None,
            }];
            let snapshot = ChatStoreSnapshot {
                selected_conversation_id: Some(conversation_id),
                selected_conversation_title: "Conv".to_string(),
                selection_generation: 1,
                load_state: ConversationLoadState::Ready {
                    conversation_id,
                    generation: 1,
                },
                transcript: finalized_transcript,
                streaming: StreamingStoreSnapshot::default(),
                conversations: vec![ConversationSummary {
                    id: conversation_id,
                    title: "Conv".to_string(),
                    updated_at: Utc::now(),
                    message_count: 1,
                }],
            };

            view.conversation_id = Some(conversation_id);
            view.selection_generation = 1;
            view.state.chat_autoscroll_enabled = false;
            view.state.streaming = StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            };
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.apply_store_snapshot(snapshot, cx);

            assert_eq!(view.state.streaming, StreamingState::Idle);
            assert_eq!(view.state.messages.len(), 1);
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 0);
        });
    });
}

#[gpui::test]
async fn send_message_and_start_streaming_reenables_autoscroll_and_starts_stream(
    cx: &mut TestAppContext,
) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.chat_autoscroll_enabled = false;
            view.state.input_text = "queued".to_string();
            view.state.cursor_position = view.state.input_text.len();
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.send_message_and_start_streaming("hello".to_string(), cx);

            assert!(view.state.chat_autoscroll_enabled);
            assert!(view.state.input_text.is_empty());
            assert_eq!(view.state.cursor_position, 0);
            assert_eq!(
                view.state.streaming,
                StreamingState::Streaming {
                    content: String::new(),
                    done: false,
                }
            );
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 1);
        });
    });
}

#[gpui::test]
async fn handle_enter_reenables_autoscroll_before_starting_stream(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.chat_autoscroll_enabled = false;
            view.state.input_text = "hello".to_string();
            view.state.cursor_position = view.state.input_text.len();
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.handle_enter(cx);

            assert!(view.state.chat_autoscroll_enabled);
            assert_eq!(
                view.state.streaming,
                StreamingState::Streaming {
                    content: String::new(),
                    done: false,
                }
            );
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 1);
        });
    });
}

#[gpui::test]
async fn wheel_scroll_down_reenables_autoscroll_only_when_near_bottom(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            let downward_event = ScrollWheelEvent {
                position: point(px(0.0), px(0.0)),
                delta: ScrollDelta::Pixels(point(px(0.0), px(-16.0))),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Moved,
            };

            view.state.chat_autoscroll_enabled = false;
            view.chat_scroll_handle
                .set_offset(point(px(0.0), px(-120.0)));
            view.refresh_autoscroll_state_after_wheel(&downward_event);
            assert!(!view.state.chat_autoscroll_enabled);

            view.chat_scroll_handle.set_offset(point(px(0.0), px(0.0)));
            view.refresh_autoscroll_state_after_wheel(&downward_event);
            assert!(view.state.chat_autoscroll_enabled);
        });
    });
}
