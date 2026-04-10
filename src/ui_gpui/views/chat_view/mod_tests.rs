#![allow(clippy::future_not_send)]

use super::*;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ViewCommand,
};
use crate::ui_gpui::app_store::StreamingStoreSnapshot;
use crate::ui_gpui::bridge::GpuiBridge;
use chrono::Utc;
use gpui::{
    point, AppContext, KeyDownEvent, Keystroke, Modifiers, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TouchPhase,
};
use std::sync::Arc;

// ── messages_from_payload tests ──────────────────────────────────────────

#[test]
fn messages_from_payload_uses_model_id_when_present() {
    // Verify that assistant messages with a stored model_id display the correct model name
    let messages = vec![ConversationMessagePayload {
        role: MessageRole::Assistant,
        content: "Hello".to_string(),
        thinking_content: None,
        timestamp: None,
        model_id: Some("gpt-4o".to_string()),
    }];

    let result = ChatView::messages_from_payload(messages);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].model_label.as_deref(), Some("gpt-4o"));
}

#[test]
fn messages_from_payload_shows_unknown_when_model_id_missing() {
    let messages = vec![ConversationMessagePayload {
        role: MessageRole::Assistant,
        content: "Hello".to_string(),
        thinking_content: None,
        timestamp: None,
        model_id: None,
    }];

    let result = ChatView::messages_from_payload(messages);

    assert_eq!(result.len(), 1);
    // Should show "unknown" instead of the current profile
    assert_eq!(result[0].model_label.as_deref(), Some("unknown"));
}

#[test]
fn messages_from_payload_user_messages_have_no_model_label() {
    let messages = vec![ConversationMessagePayload {
        role: MessageRole::User,
        content: "Hello".to_string(),
        thinking_content: None,
        timestamp: None,
        model_id: Some("gpt-4o".to_string()), // Even with model_id, user messages don't show model
    }];

    let result = ChatView::messages_from_payload(messages);

    assert_eq!(result.len(), 1);
    assert!(result[0].model_label.is_none());
}

#[test]
fn messages_from_payload_preserves_thinking_and_timestamp() {
    let messages = vec![ConversationMessagePayload {
        role: MessageRole::Assistant,
        content: "Hello".to_string(),
        thinking_content: Some("Let me think...".to_string()),
        timestamp: Some(1_234_567_890),
        model_id: Some("claude-3".to_string()),
    }];

    let result = ChatView::messages_from_payload(messages);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].thinking.as_deref(), Some("Let me think..."));
    assert_eq!(result[0].timestamp, Some(1_234_567_890));
}

fn chat_key_event(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

fn make_chat_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(8);
    let (_view_tx, view_rx) = flume::bounded(8);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

#[gpui::test]
async fn sidebar_helpers_toggle_visibility_and_emit_search_events(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();
    let (bridge, user_rx) = make_chat_bridge();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_bridge(bridge.clone());
            assert!(view.state.sidebar_visible);
            view.toggle_sidebar(cx);
            assert!(!view.state.sidebar_visible);

            view.state.sidebar_search_query = "  skills  ".to_string();
            view.trigger_sidebar_search(cx);
            assert_eq!(
                user_rx.try_recv().ok(),
                Some(UserEvent::SearchConversations {
                    query: "  skills  ".to_string(),
                })
            );

            view.state.sidebar_search_query = "   ".to_string();
            view.state.sidebar_search_results = Some(Vec::new());
            view.trigger_sidebar_search(cx);
            assert!(view.state.sidebar_search_results.is_none());
            assert!(user_rx.try_recv().is_err());
        });
    });
}

#[gpui::test]
async fn conversation_search_results_command_respects_empty_query_behavior(
    cx: &mut TestAppContext,
) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.state.sidebar_search_query.clear();
            view.handle_command(
                ViewCommand::ConversationSearchResults {
                    results: Vec::new(),
                },
                cx,
            );
            assert!(view.state.sidebar_search_results.is_none());

            view.state.sidebar_search_query = "skills".to_string();
            view.handle_command(
                ViewCommand::ConversationSearchResults {
                    results: Vec::new(),
                },
                cx,
            );
            assert_eq!(view.state.sidebar_search_results, Some(Vec::new()));
        });
    });
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
                    preview: None,
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
                    preview: None,
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
                model_id: None,
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
                    preview: None,
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
                model_id: None,
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
                    preview: None,
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
async fn apply_store_snapshot_thinking_only_calls_maybe_scroll_when_autoscroll_enabled(
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
                    thinking_buffer: "thinking...".to_string(),
                    active_target: Some(conversation_id),
                    ..StreamingStoreSnapshot::default()
                },
                conversations: vec![ConversationSummary {
                    id: conversation_id,
                    title: "Conv".to_string(),
                    updated_at: Utc::now(),
                    message_count: 0,
                    preview: None,
                }],
            };

            view.conversation_id = Some(conversation_id);
            view.selection_generation = 1;
            view.state.chat_autoscroll_enabled = true;
            view.state.streaming = StreamingState::Idle;
            view.state.thinking_content = None;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.apply_store_snapshot(snapshot, cx);

            assert!(matches!(
                view.state.streaming,
                StreamingState::Streaming { .. }
            ));
            assert_eq!(view.state.thinking_content.as_deref(), Some("thinking..."));
            assert_eq!(view.maybe_scroll_chat_to_bottom_invocations.get(), 1);
        });
    });
}

#[gpui::test]
async fn apply_store_snapshot_thinking_only_skips_maybe_scroll_when_autoscroll_disabled(
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
                    thinking_buffer: "thinking...".to_string(),
                    active_target: Some(conversation_id),
                    ..StreamingStoreSnapshot::default()
                },
                conversations: vec![ConversationSummary {
                    id: conversation_id,
                    title: "Conv".to_string(),
                    updated_at: Utc::now(),
                    message_count: 0,
                    preview: None,
                }],
            };

            view.conversation_id = Some(conversation_id);
            view.selection_generation = 1;
            view.state.chat_autoscroll_enabled = false;
            view.state.streaming = StreamingState::Idle;
            view.state.thinking_content = None;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.apply_store_snapshot(snapshot, cx);

            assert!(matches!(
                view.state.streaming,
                StreamingState::Streaming { .. }
            ));
            assert_eq!(view.state.thinking_content.as_deref(), Some("thinking..."));
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
            view.state.conversation_dropdown_open = true;
            view.state.profile_dropdown_open = true;
            view.state.conversation_title_editing = true;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.send_message_and_start_streaming("hello".to_string(), cx);

            assert!(view.state.chat_autoscroll_enabled);
            assert!(view.state.input_text.is_empty());
            assert_eq!(view.state.cursor_position, 0);
            assert!(!view.state.conversation_dropdown_open);
            assert!(!view.state.profile_dropdown_open);
            assert!(!view.state.conversation_title_editing);
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

// ── transcript text selection tests (issue #151) ─────────────────────────

use super::state::TextSelection;

/// Helper that mirrors the transcript layout `render_chat_area` builds for a
/// list of messages: each user/assistant content body becomes a block, joined
/// by '\n', with a trailing '\n' after every message.
fn install_transcript(view: &mut ChatView, blocks: &[&str]) {
    view.transcript_text.clear();
    view.transcript_block_ranges.clear();
    for block in blocks {
        let start = view.transcript_text.len();
        view.transcript_text.push_str(block);
        let end = view.transcript_text.len();
        view.transcript_block_ranges.push(start..end);
        view.transcript_text.push('\n');
    }
}

#[gpui::test]
async fn set_text_selection_clamps_and_orders_endpoints(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            install_transcript(view, &["hello world"]);

            // Reverse-order endpoints get swapped.
            view.set_text_selection(7, 2, true);
            let sel = view.state.text_selection.as_ref().expect("selection set");
            assert_eq!(sel.range, 2..7);
            assert!(sel.is_dragging);

            // Endpoints past EOF are clamped to transcript_text.len().
            view.set_text_selection(0, 9_999, false);
            let sel = view.state.text_selection.as_ref().expect("selection set");
            assert_eq!(sel.range.end, view.transcript_text.len());
            assert!(!sel.is_dragging);
        });
    });
}

#[gpui::test]
async fn set_text_selection_returns_none_when_transcript_empty(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            view.transcript_text.clear();
            view.transcript_block_ranges.clear();
            view.state.text_selection = Some(TextSelection {
                range: 0..3,
                is_dragging: true,
            });
            view.set_text_selection(0, 5, true);
            assert!(view.state.text_selection.is_none());
        });
    });
}

#[gpui::test]
async fn select_word_at_offset_selects_a_single_word(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["hello brave new world"]);

            // Click inside "brave" (offset 8 lands on the 'a').
            view.select_word_at_offset(8, cx);
            let sel = view.state.text_selection.as_ref().expect("selection set");
            assert_eq!(&view.transcript_text[sel.range.clone()], "brave");
            assert!(!sel.is_dragging);
        });
    });
}

#[gpui::test]
async fn select_paragraph_at_offset_selects_full_line(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["first line", "second line", "third line"]);

            // Pick an offset inside the second message.
            let target = view.transcript_block_ranges[1].start + 3;
            view.select_paragraph_at_offset(target, cx);
            let sel = view.state.text_selection.as_ref().expect("selection set");
            assert_eq!(&view.transcript_text[sel.range.clone()], "second line");
            assert!(!sel.is_dragging);
        });
    });
}

#[gpui::test]
async fn clear_transcript_selection_drops_active_range(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            install_transcript(view, &["abc"]);
            view.set_text_selection(0, 3, true);
            assert!(view.state.text_selection.is_some());
            view.clear_transcript_selection();
            assert!(view.state.text_selection.is_none());
        });
    });
}

#[gpui::test]
async fn handle_copy_uses_transcript_selection_when_present(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["copy this please"]);
            // Pollute the input text so we can detect the wrong fallback.
            view.state.input_text = "INPUT TEXT".to_string();
            view.set_text_selection(5, 9, false);

            view.handle_copy(cx);
        });
    });

    let clipboard = visual_cx.read_from_clipboard();
    let value = clipboard
        .as_ref()
        .and_then(gpui::ClipboardItem::text)
        .expect("clipboard text written");
    assert_eq!(value, "this");
}

#[gpui::test]
async fn handle_copy_falls_back_to_input_text_when_no_selection(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["transcript body"]);
            view.state.text_selection = None;
            view.state.input_text = "draft message".to_string();

            view.handle_copy(cx);
        });
    });

    let clipboard = visual_cx.read_from_clipboard();
    let value = clipboard
        .as_ref()
        .and_then(gpui::ClipboardItem::text)
        .expect("clipboard text written");
    assert_eq!(value, "draft message");
}

#[gpui::test]
async fn handle_copy_falls_back_to_sidebar_search_query_when_focused(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["body"]);
            view.state.text_selection = None;
            view.state.sidebar_search_focused = true;
            view.state.sidebar_search_query = "skills query".to_string();
            view.state.input_text = "should not be used".to_string();

            view.handle_copy(cx);
        });
    });

    let clipboard = visual_cx.read_from_clipboard();
    let value = clipboard
        .as_ref()
        .and_then(gpui::ClipboardItem::text)
        .expect("clipboard text written");
    assert_eq!(value, "skills query");
}

#[gpui::test]
async fn handle_copy_falls_back_to_title_input_when_renaming(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["body"]);
            view.state.text_selection = None;
            view.state.conversation_title_editing = true;
            view.state.conversation_title_input = "Renamed Conversation".to_string();
            view.state.input_text = "should not be used".to_string();

            view.handle_copy(cx);
        });
    });

    let clipboard = visual_cx.read_from_clipboard();
    let value = clipboard
        .as_ref()
        .and_then(gpui::ClipboardItem::text)
        .expect("clipboard text written");
    assert_eq!(value, "Renamed Conversation");
}
