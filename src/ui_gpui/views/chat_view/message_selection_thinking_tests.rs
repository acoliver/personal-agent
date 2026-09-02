//! Displayed-thinking selection identity, leaf-count, and copy tests.
//!
//! Proves the emoji filter treats thinking exactly like rendered assistant
//! content: leaf counts, content keys, and copy output stay byte-identical
//! to the rendered text, and thinking that blanks out after stripping
//! produces no leaf and no copy text.

use super::*;
use gpui::{Entity, KeyDownEvent, Keystroke, Modifiers, TestAppContext};
use gpui_selection_vendor::{TextSelection, TextSelectionContentKey};
use uuid::Uuid;

fn chat_key_event(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

fn modified_chat_key_event(key: &str, modifiers: Modifiers) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers,
            ..chat_key_event(key).keystroke
        },
        ..chat_key_event(key)
    }
}

fn cmd_a() -> KeyDownEvent {
    modified_chat_key_event(
        "a",
        Modifiers {
            platform: true,
            ..Default::default()
        },
    )
}

fn chat_view_with(
    cx: &mut TestAppContext,
    messages: Vec<ChatMessage>,
    streaming: StreamingState,
) -> Entity<ChatView> {
    let state = ChatState {
        active_conversation_id: Some(Uuid::new_v4()),
        messages,
        streaming,
        ..ChatState::default()
    };
    cx.new(|cx| ChatView::new(state, cx))
}

/// Installs a window selection standing in for a user drag, so Cmd+A has a
/// selection to replace.
fn seed_drag_selection(window: &mut gpui::Window, cx: &mut gpui::Context<ChatView>) {
    TextSelection::select_all(&[TextSelectionContentKey::new(999_999)], "drag", window, cx);
    assert!(TextSelection::has_selection(window, cx));
}

#[gpui::test]
fn emoji_filter_copies_finalized_thinking_like_the_rendered_text(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("question"),
            ChatMessage::assistant("the answer", "model").with_thinking(concat!(
                "pondering ",
                "\u{1F600}",
                " deeply"
            )),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();

    // Filter off: emoji-bearing thinking copies verbatim.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.show_thinking = true;
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_text(window, cx),
                concat!(
                    "question\n\npondering ",
                    "\u{1F600}",
                    " deeply\n\nthe answer"
                )
            );
        });
    });

    // Filter on: the copy text is byte-identical to the rendered (stripped)
    // thinking, and the content key tracks the stripped text.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.filter_emoji = true;
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_text(window, cx),
                "question\n\npondering  deeply\n\nthe answer"
            );
            let key = view.message_selection_content_key(1);

            // Raw thinking differing only by emoji strips to the same
            // displayed text, so the identity (and key) survives.
            view.state.messages[1] =
                ChatMessage::assistant("the answer", "model").with_thinking("pondering  deeply");
            view.refresh_transcript_selection_revisions();
            assert_eq!(view.message_selection_content_key(1), key);
        });
    });
}

#[gpui::test]
fn emoji_filter_copies_streaming_thinking_like_the_rendered_text(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![ChatMessage::user("go")],
        StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        },
    );
    let mut visual_cx = cx.add_empty_window().clone();

    // Filter off: emoji-bearing streaming thinking copies verbatim.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.show_thinking = true;
            view.state.thinking_content =
                Some(concat!("hmm ", "\u{1F600}", " thought").to_string());
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_text(window, cx),
                concat!("go\n\nhmm ", "\u{1F600}", " thought\n\npartial\u{258B}")
            );
        });
    });

    // Filter on: the copy text is byte-identical to the rendered (stripped)
    // thinking, and the streaming key tracks the stripped text.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.filter_emoji = true;
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_text(window, cx),
                "go\n\nhmm  thought\n\npartial\u{258B}"
            );
            let key = view.streaming_selection_content_key();

            view.state.thinking_content = Some("hmm  thought".to_string());
            view.refresh_transcript_selection_revisions();
            assert_eq!(view.streaming_selection_content_key(), key);
        });
    });
}

#[gpui::test]
fn thinking_blank_after_emoji_stripping_copies_nothing(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("question"),
            ChatMessage::assistant("the answer", "model").with_thinking("\u{1F600}"),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.show_thinking = true;
            view.state.filter_emoji = true;
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            // Finalized thinking that blanks out after stripping adds no
            // copy text and no separator.
            assert_eq!(
                TextSelection::selected_text(window, cx),
                "question\n\nthe answer"
            );

            // Streaming thinking that blanks out behaves the same.
            view.state.streaming = StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            };
            view.state.thinking_content = Some("\u{1F600}".to_string());
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_text(window, cx),
                concat!("question\n\nthe answer\n\npartial", "\u{258B}")
            );
        });
    });
}

#[gpui::test]
fn user_message_thinking_never_copies_or_shifts_identity(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("question").with_thinking("secret reasoning"),
            ChatMessage::assistant("the answer", "model"),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();

    // Copy output contains no thinking text for the user row, with and
    // without the emoji filter.
    for filter_emoji in [false, true] {
        visual_cx.update(|window, app| {
            view.update(app, |view, cx| {
                view.state.show_thinking = true;
                view.state.filter_emoji = filter_emoji;
                view.refresh_transcript_selection_revisions();
                seed_drag_selection(window, cx);
                view.handle_key_down(&cmd_a(), window, cx);

                assert_eq!(
                    TextSelection::selected_text(window, cx),
                    "question

the answer"
                );
            });
        });
    }

    // Mutating only the user message's thinking keeps its content key.
    visual_cx.update(|_, app| {
        view.update(app, |view, _| {
            let key = view.message_selection_content_key(0);
            view.state.messages[0] = ChatMessage::user("question").with_thinking("other");
            view.refresh_transcript_selection_revisions();
            assert_eq!(view.message_selection_content_key(0), key);
        });
    });
}

#[test]
fn emoji_bearing_thinking_counts_one_leaf_with_and_without_the_filter() {
    use super::transcript::{transcript_row_leaf_count, TranscriptRow};

    let messages =
        vec![ChatMessage::assistant("answer", "model").with_thinking("thought\u{1F600}deeply")];
    let streaming = StreamingState::Idle;
    let hidden = transcript_row_leaf_count(
        TranscriptRow::Message(0),
        &messages,
        &streaming,
        true,
        false,
        None,
    );
    for filter_emoji in [false, true] {
        let shown = transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &messages,
            &streaming,
            filter_emoji,
            true,
            None,
        );
        assert_eq!(
            shown,
            hidden + 1,
            "emoji-bearing thinking counts one leaf with filter_emoji={filter_emoji}"
        );
    }

    let streaming_state = StreamingState::Streaming {
        content: "partial".to_string(),
        done: false,
    };
    let streaming_hidden = transcript_row_leaf_count(
        TranscriptRow::Streaming,
        &messages,
        &streaming_state,
        true,
        false,
        Some("thought\u{1F600}deeply"),
    );
    for filter_emoji in [false, true] {
        let streaming_shown = transcript_row_leaf_count(
            TranscriptRow::Streaming,
            &messages,
            &streaming_state,
            filter_emoji,
            true,
            Some("thought\u{1F600}deeply"),
        );
        assert_eq!(
            streaming_shown,
            streaming_hidden + 1,
            "emoji-bearing streaming thinking counts one leaf with filter_emoji={filter_emoji}"
        );
    }

    // Thinking that blanks out after stripping produces no leaf.
    let blank = vec![ChatMessage::assistant("answer", "model").with_thinking("\u{1F600}")];
    assert_eq!(
        transcript_row_leaf_count(
            TranscriptRow::Message(0),
            &blank,
            &streaming,
            true,
            true,
            None
        ),
        hidden
    );
    assert_eq!(
        transcript_row_leaf_count(
            TranscriptRow::Streaming,
            &messages,
            &streaming_state,
            true,
            true,
            Some("\u{1F600}")
        ),
        streaming_hidden
    );
}
