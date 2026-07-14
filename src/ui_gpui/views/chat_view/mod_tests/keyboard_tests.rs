use super::*;

fn copy_modifiers() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers {
            platform: true,
            ..Modifiers::default()
        }
    } else {
        Modifiers {
            control: true,
            ..Modifiers::default()
        }
    }
}

#[gpui::test]
async fn platform_copy_shortcut_copies_active_message_selection(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let selected_text = "ISSUE151_ALPHA".to_string();
            view.state
                .messages
                .push(ChatMessage::user(selected_text.clone()));
            view.active_message_selection = Some(ActiveMessageSelection {
                message_index: 0,
                revision: MessageRevision::new("new:0:User", &selected_text, 0, false),
                selection: Selection::new(0, selected_text.len()),
                dragging: false,
            });

            view.handle_key_down(&modified_chat_key_event("c", copy_modifiers()), cx);

            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some(selected_text)
            );
        });
    });
}

#[gpui::test]
async fn stale_message_selection_is_cleared_instead_of_copied(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            let stale_text = "stale selection";
            view.state
                .messages
                .push(ChatMessage::user("current content"));
            view.active_message_selection = Some(ActiveMessageSelection {
                message_index: 0,
                revision: MessageRevision::new("new:0:User", stale_text, 0, false),
                selection: Selection::new(0, stale_text.len()),
                dragging: false,
            });
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));

            view.handle_key_down(&modified_chat_key_event("c", copy_modifiers()), cx);

            assert!(view.active_message_selection.is_none());
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_string())
            );
        });
    });
}

#[gpui::test]
async fn empty_selection_from_another_message_clears_active_selection(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    view.update(cx, |view, _cx| {
        let content = "selected";
        view.active_message_selection = Some(ActiveMessageSelection {
            message_index: 0,
            revision: MessageRevision::new("new:0:User", content, 0, false),
            selection: Selection::new(0, content.len()),
            dragging: false,
        });

        view.apply_selectable_message_event(
            1,
            MessageRevision::new("new:1:Assistant", "other", 0, false),
            crate::ui_gpui::components::markdown_content::selectable_markdown::SelectableMarkdownEvent::SelectionChanged {
                selection: None,
                selected_text: String::new(),
                dragging: false,
            },
        );

        assert!(view.active_message_selection.is_none());
    });
}

#[gpui::test]
async fn composer_selection_supports_copy_cut_delete_and_replacement(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));

    view.update(cx, |view, cx| {
        view.state.input_text = "hello world".to_string();
        view.state.cursor_position = 5;
        view.set_composer_selection_from_pointer(
            super::super::composer_selection::ComposerSelection::new(0, 5),
            cx,
        );
        assert_eq!(view.composer_selected_text(), "hello");
        assert!(view.copy_composer_selection(cx));
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("hello".to_string())
        );

        assert!(view.cut_composer_selection(cx));
        assert_eq!(view.state.input_text, " world");
        assert_eq!(view.state.cursor_position, 0);

        view.state.input_text = "a😀b".to_string();
        view.state.cursor_position = 1;
        view.composer_selection = super::super::composer_selection::ComposerSelection::caret(1);
        view.handle_delete(cx);
        assert_eq!(view.state.input_text, "ab");

        view.state.input_text = "hello world".to_string();
        view.composer_selection = super::super::composer_selection::ComposerSelection::new(6, 11);
        view.replace_composer_selection("GPUI", cx);
        assert_eq!(view.state.input_text, "hello GPUI");
        assert_eq!(view.state.cursor_position, "hello GPUI".len());
    });
}

#[gpui::test]
async fn modified_enter_inserts_newline_without_submitting(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();
    let (bridge, user_rx) = make_chat_bridge();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_bridge(bridge.clone());
            view.state.input_text = "firstsecond".to_string();
            view.state.cursor_position = "first".len();

            view.handle_key_down(
                &modified_chat_key_event(
                    "enter",
                    Modifiers {
                        shift: true,
                        ..Modifiers::default()
                    },
                ),
                cx,
            );

            assert_eq!(view.state.input_text, "first\nsecond");
            assert_eq!(view.state.cursor_position, "first\n".len());
            assert!(user_rx.try_recv().is_err());
            assert_eq!(view.state.streaming, StreamingState::Idle);

            view.handle_key_down(
                &modified_chat_key_event(
                    "enter",
                    Modifiers {
                        control: true,
                        ..Modifiers::default()
                    },
                ),
                cx,
            );

            assert_eq!(view.state.input_text, "first\n\nsecond");
            assert_eq!(view.state.cursor_position, "first\n\n".len());
            assert!(user_rx.try_recv().is_err());
            assert_eq!(view.state.streaming, StreamingState::Idle);

            view.handle_key_down(
                &modified_chat_key_event(
                    "enter",
                    Modifiers {
                        alt: true,
                        ..Modifiers::default()
                    },
                ),
                cx,
            );

            assert_eq!(view.state.input_text, "first\n\n\nsecond");
            assert_eq!(view.state.cursor_position, "first\n\n\n".len());
            assert!(user_rx.try_recv().is_err());
            assert_eq!(view.state.streaming, StreamingState::Idle);
        });
    });
}

#[gpui::test]
async fn plain_enter_still_submits_message(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();
    let (bridge, user_rx) = make_chat_bridge();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_bridge(bridge.clone());
            view.state.input_text = "send me".to_string();
            view.state.cursor_position = view.state.input_text.len();

            view.handle_key_down(&chat_key_event("enter"), cx);

            assert_eq!(
                user_rx.try_recv().ok(),
                Some(UserEvent::SendMessage {
                    text: "send me".to_string(),
                    conversation_id: None,
                })
            );
            assert!(view.state.input_text.is_empty());
            assert_eq!(view.state.cursor_position, 0);
            assert_eq!(
                view.state.streaming,
                StreamingState::Streaming {
                    content: String::new(),
                    done: false,
                }
            );
        });
    });
}

#[gpui::test]
async fn send_message_targets_selected_conversation(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();
    let (bridge, user_rx) = make_chat_bridge();
    let conversation_id = Uuid::new_v4();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_bridge(bridge.clone());
            view.conversation_id = Some(conversation_id);
            view.state.input_text = "continue here".to_string();
            view.state.cursor_position = view.state.input_text.len();

            view.handle_key_down(&chat_key_event("enter"), cx);

            assert_eq!(
                user_rx.try_recv().ok(),
                Some(UserEvent::SendMessage {
                    text: "continue here".to_string(),
                    conversation_id: Some(conversation_id),
                })
            );
        });
    });
}
