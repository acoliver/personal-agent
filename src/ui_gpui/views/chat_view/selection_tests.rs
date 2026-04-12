#![allow(clippy::future_not_send)]
//! Transcript text selection tests (issue #151).

use super::state::TextSelection;
use super::*;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{ConversationMessagePayload, MessageRole};
use crate::ui_gpui::bridge::GpuiBridge;
use gpui::{point, AppContext, TestAppContext};
use std::sync::Arc;

fn make_chat_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(8);
    let (_view_tx, view_rx) = flume::bounded(8);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

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

// ── Stage 4: Block model, cross-block, multi-byte, armed/clear tests ────

/// Build transcript with thinking blocks and verify block structure.
#[gpui::test]
async fn build_transcript_buffer_alternates_message_and_thinking(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |_view: &mut ChatView, _cx| {
            let messages = vec![
                ChatMessage {
                    role: state::MessageRole::User,
                    content: "Hello".to_string(),
                    thinking: None,
                    model_label: None,
                    timestamp: None,
                },
                ChatMessage {
                    role: state::MessageRole::Assistant,
                    content: "World".to_string(),
                    thinking: Some("deep thought".to_string()),
                    model_label: Some("model".to_string()),
                    timestamp: None,
                },
            ];
            let (text, ranges) = ChatView::build_transcript_buffer(&messages, None, true, false);

            // User message: body only (no thinking), so 1 range.
            // Assistant message: body + thinking, so 2 ranges.
            // Total: 3 block ranges.
            assert_eq!(ranges.len(), 3, "expected 3 block ranges");
            assert_eq!(&text[ranges[0].clone()], "Hello");
            assert_eq!(&text[ranges[1].clone()], "World");
            assert_eq!(&text[ranges[2].clone()], "deep thought");

            // Ranges should be non-overlapping and ordered.
            for pair in ranges.windows(2) {
                assert!(
                    pair[0].end <= pair[1].start,
                    "ranges should be non-overlapping: {:?} vs {:?}",
                    pair[0],
                    pair[1]
                );
            }
        });
    });
}

/// When `filter_emoji` is true, `build_transcript_buffer` returns empty.
#[gpui::test]
async fn build_transcript_buffer_empty_when_filter_emoji(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |_view: &mut ChatView, _cx| {
            let messages = vec![ChatMessage {
                role: state::MessageRole::User,
                content: "Hello".to_string(),
                thinking: None,
                model_label: None,
                timestamp: None,
            }];
            let (text, ranges) = ChatView::build_transcript_buffer(&messages, None, true, true);
            assert!(text.is_empty());
            assert!(ranges.is_empty());
        });
    });
}

/// Copy selection spanning message body and thinking block.
#[gpui::test]
async fn handle_copy_across_message_and_thinking_blocks(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.transcript_text = "Hello\ndeep thought\n".to_string();
            view.transcript_block_ranges = vec![0..5, 6..18];

            // Select from "llo" in body through "deep" in thinking: bytes 2..10
            view.set_text_selection(2, 10, false);

            view.handle_copy(cx);
        });
    });

    let clipboard = visual_cx.read_from_clipboard();
    let value = clipboard
        .as_ref()
        .and_then(gpui::ClipboardItem::text)
        .expect("clipboard text written");
    assert_eq!(value, "llo\ndeep");
}

/// Multi-byte content across blocks preserves valid UTF-8 on copy.
#[gpui::test]
async fn multibyte_content_across_blocks_preserves_valid_utf8(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let block1 = "café";
            let block2 = "résumé";
            let mut text = String::new();
            let start1 = text.len();
            text.push_str(block1);
            let end1 = text.len();
            text.push('\n');
            let start2 = text.len();
            text.push_str(block2);
            let end2 = text.len();
            text.push('\n');

            view.transcript_text = text;
            view.transcript_block_ranges = vec![start1..end1, start2..end2];

            view.set_text_selection(0, end2, false);
            view.handle_copy(cx);
        });
    });

    let clipboard = visual_cx.read_from_clipboard();
    let value = clipboard
        .as_ref()
        .and_then(gpui::ClipboardItem::text)
        .expect("clipboard text written");
    assert!(value.contains("café"));
    assert!(value.contains("résumé"));
}

/// Word selection does not cross block boundary.
#[gpui::test]
async fn select_word_does_not_cross_block_boundary(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            install_transcript(view, &["abcdef", "ghijkl"]);

            let offset = view.transcript_block_ranges[0].end - 1;
            view.select_word_at_offset(offset, cx);

            let sel = view.state.text_selection.as_ref().expect("selection set");
            let selected = &view.transcript_text[sel.range.clone()];
            assert!(
                !selected.contains("ghijkl"),
                "selection should not cross block boundary: got {selected:?}"
            );
        });
    });
}

/// Verifies `clear_transcript_selection` resets all selection-related state.
#[gpui::test]
async fn clear_transcript_selection_resets_all_state(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            install_transcript(view, &["test content"]);
            view.set_text_selection(0, 4, true);
            view.transcript_drag_anchor = Some(0);
            view.transcript_selection_armed = true;
            view.transcript_pending_click = Some(PendingClick {
                position: point(gpui::px(10.0), gpui::px(10.0)),
                click_count: 2,
            });

            view.clear_transcript_selection();

            assert!(view.state.text_selection.is_none());
            assert!(view.transcript_drag_anchor.is_none());
            assert!(!view.transcript_selection_armed);
            assert!(view.transcript_pending_click.is_none());
        });
    });
}

/// Conversation switch (`apply_store_snapshot` Ready) clears selection.
#[gpui::test]
async fn conversation_switch_clears_selection_and_armed_state(cx: &mut TestAppContext) {
    use crate::ui_gpui::app_store_types::ChatStoreSnapshot;

    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();
    let (bridge, _user_rx) = make_chat_bridge();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_bridge(bridge.clone());

            install_transcript(view, &["old content"]);
            view.set_text_selection(0, 3, false);
            view.transcript_selection_armed = true;

            let snapshot = ChatStoreSnapshot {
                transcript: vec![ConversationMessagePayload {
                    role: MessageRole::User,
                    content: "new content".to_string(),
                    thinking_content: None,
                    timestamp: None,
                    model_id: None,
                }],
                ..ChatStoreSnapshot::default()
            };
            view.apply_store_snapshot(snapshot, cx);

            assert!(
                view.state.text_selection.is_none(),
                "selection should be cleared"
            );
            assert!(
                !view.transcript_selection_armed,
                "armed flag should be cleared"
            );
        });
    });
}

/// `build_selectable_styled_text` snaps mid-byte selection to char boundaries.
#[test]
fn build_selectable_styled_text_snaps_to_char_boundaries() {
    use crate::ui_gpui::components::build_selectable_styled_text;

    let text = "café"; // 'é' is bytes 3..5
    let styled = build_selectable_styled_text(text, Some(&(4..5)), gpui::black());
    // Should not panic — that's the primary assertion.
    let _ = styled.layout();
}

/// `build_transcript_buffer` includes streaming thinking content.
#[gpui::test]
async fn build_transcript_buffer_includes_streaming_thinking(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |_view: &mut ChatView, _cx| {
            let messages = vec![ChatMessage {
                role: state::MessageRole::User,
                content: "Hello".to_string(),
                thinking: None,
                model_label: None,
                timestamp: None,
            }];
            let (text, ranges) = ChatView::build_transcript_buffer(
                &messages,
                Some("streaming thought"),
                true,
                false,
            );

            assert_eq!(ranges.len(), 2);
            assert_eq!(&text[ranges[0].clone()], "Hello");
            assert_eq!(&text[ranges[1].clone()], "streaming thought");
        });
    });
}
