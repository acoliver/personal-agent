//! Cmd+A / Ctrl+A whole-transcript selection and Escape clearing tests.
//!
//! Drives the real key handlers end to end: platform-shortcut routing,
//! editor/overlay precedence, logical select-all installation, Escape
//! ordering against streaming-stop, and copy refusal on stale keys.

use super::*;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{ToolApprovalContext, ToolCategory};
use crate::ui_gpui::bridge::GpuiBridge;
use gpui::{px, Entity, KeyDownEvent, Keystroke, Modifiers, TestAppContext};
use gpui_selection_vendor::{TextSelection, TextSelectionContentKey};
use std::collections::HashMap;
use std::sync::Arc;
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

fn make_chat_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(8);
    let (_view_tx, view_rx) = flume::bounded(8);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
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

fn transcript_keys(
    view: &ChatView,
    message_indexes: &[usize],
    include_streaming: bool,
) -> Vec<TextSelectionContentKey> {
    let mut keys = message_indexes
        .iter()
        .map(|&index| view.message_selection_content_key(index))
        .collect::<Vec<_>>();
    if include_streaming {
        keys.push(view.streaming_selection_content_key());
    }
    keys
}

/// Drives each `(modifiers, key)` combo through the real key handler with
/// no selection installed: a wrongly routed "a" must not run the composer
/// select-all, install a transcript selection, or touch the clipboard.
fn assert_combos_are_no_ops_without_selection(
    view: &mut ChatView,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<ChatView>,
    combos: &[(Modifiers, &'static str)],
) {
    for &(modifiers, key) in combos {
        view.handle_key_down(&modified_chat_key_event(key, modifiers), window, cx);

        assert_eq!(
            view.state.cursor_position, 0,
            "{modifiers:?}+{key} must not run composer select-all"
        );
        assert!(
            !TextSelection::has_selection(window, cx),
            "{modifiers:?}+{key} must not install a transcript selection"
        );
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("sentinel".to_string()),
            "{modifiers:?}+{key} must not touch the clipboard"
        );
    }
}

/// Drives each combo through the real key handler with the whole transcript
/// selected: a wrongly routed "c" must not copy over the sentinel, and a
/// wrongly routed "a" must not alter the selection or move the cursor.
fn assert_combos_are_no_ops_with_transcript_selection(
    view: &mut ChatView,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<ChatView>,
    transcript: &str,
    combos: &[(Modifiers, &'static str)],
) {
    for &(modifiers, key) in combos {
        view.handle_key_down(&modified_chat_key_event(key, modifiers), window, cx);

        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("sentinel".to_string()),
            "{modifiers:?}+{key} must not copy"
        );
        assert_eq!(
            TextSelection::selected_text(window, cx),
            transcript,
            "{modifiers:?}+{key} must not alter the transcript selection"
        );
        assert_eq!(
            view.state.cursor_position, 0,
            "{modifiers:?}+{key} must not move the composer cursor"
        );
    }
}

#[gpui::test]
fn cmd_a_expands_an_existing_selection_to_the_whole_transcript(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("# Heading\n\nbody", "model"),
            ChatMessage::user("third"),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                Some(transcript_keys(view, &[0, 1, 2], false))
            );
            assert_eq!(
                TextSelection::selected_text(window, cx),
                "first\n\nHeading\n\nbody\n\nthird"
            );
        });
    });
}

#[gpui::test]
fn cmd_a_excludes_approvals_and_includes_the_streaming_row(cx: &mut TestAppContext) {
    let conversation = Uuid::new_v4();
    let state = ChatState {
        active_conversation_id: Some(conversation),
        messages: vec![ChatMessage::user("stable")],
        streaming: StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        },
        approval_bubbles: HashMap::from([(
            conversation,
            vec![ToolApprovalBubble::new(
                "req-1",
                ToolApprovalContext::new("shell", ToolCategory::Shell, "git push"),
            )],
        )]),
        ..ChatState::default()
    };
    let view = cx.new(|cx| ChatView::new(state, cx));
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                Some(transcript_keys(view, &[0], true))
            );
            assert_eq!(
                TextSelection::selected_text(window, cx),
                "stable\n\npartial▋"
            );
        });
    });
}

#[gpui::test]
fn cmd_a_with_an_empty_transcript_clears_the_selection(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(TextSelection::selected_text(window, cx), "");
        });
    });
}

#[gpui::test]
fn cmd_a_prefers_editors_and_overlays_over_the_transcript(cx: &mut TestAppContext) {
    let view = chat_view_with(cx, vec![ChatMessage::user("only")], StreamingState::Idle);
    let mut visual_cx = cx.add_empty_window().clone();

    // Sidebar search: select-all stays a no-op and the selection survives.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.set_sidebar_search_focused(true, cx);
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            assert_eq!(TextSelection::selected_text(window, cx), "drag");
            view.set_sidebar_search_focused(false, cx);
        });
    });

    // Title edit, conversation dropdown, and profile dropdown keep the
    // composer-style cursor move and leave the selection untouched.
    for overlay in ["title", "conversation", "profile"] {
        visual_cx.update(|window, app| {
            view.update(app, |view, cx| {
                view.state.input_text = "hello".to_string();
                view.state.cursor_position = 0;
                match overlay {
                    "title" => {
                        view.state.conversation_title_editing = true;
                        view.state.conversation_title_input = "Rename".to_string();
                    }
                    "conversation" => view.state.conversation_dropdown_open = true,
                    _ => view.state.profile_dropdown_open = true,
                }
                seed_drag_selection(window, cx);
                view.handle_key_down(&cmd_a(), window, cx);

                assert_eq!(
                    TextSelection::selected_text(window, cx),
                    "drag",
                    "{overlay} overlay keeps the transcript selection"
                );
                assert_eq!(
                    view.state.cursor_position,
                    view.state.input_text.len(),
                    "{overlay} overlay preserves the composer cursor move"
                );
                match overlay {
                    "title" => {
                        assert!(view.state.conversation_title_editing);
                        view.state.conversation_title_editing = false;
                    }
                    "conversation" => {
                        assert!(view.state.conversation_dropdown_open);
                        view.state.conversation_dropdown_open = false;
                    }
                    _ => {
                        assert!(view.state.profile_dropdown_open);
                        view.state.profile_dropdown_open = false;
                    }
                }
            });
        });
    }
}

#[gpui::test]
fn cmd_a_without_a_selection_keeps_the_composer_behavior(cx: &mut TestAppContext) {
    let view = chat_view_with(cx, vec![ChatMessage::user("hi")], StreamingState::Idle);
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.input_text = "hello".to_string();
            view.state.cursor_position = 0;
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(view.state.cursor_position, view.state.input_text.len());
            assert!(!TextSelection::has_selection(window, cx));
        });
    });
}

/// Ctrl+Shift and Ctrl+Alt combinations must not masquerade as plain
/// Ctrl+A/C: they leave the composer cursor, transcript selection, and
/// clipboard untouched on every platform. On non-macOS these are the
/// combinations the exact-plain-Ctrl guard in `routes_platform_shortcut`
/// exists to reject; on macOS the control branch never fires. The
/// Ctrl+platform companion cases are covered by the non-macOS test below,
/// since macOS keeps Command routing for platform-modified events.
#[gpui::test]
fn ctrl_shift_and_ctrl_alt_combos_neither_select_all_nor_copy(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("second", "model"),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();
    let ctrl_shift = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let ctrl_alt = Modifiers {
        control: true,
        alt: true,
        ..Default::default()
    };

    // Without a selection, a wrongly routed "a" would run the composer
    // select-all and move the cursor to the end of the input.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.input_text = "hello".to_string();
            view.state.cursor_position = 0;
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));
            assert_combos_are_no_ops_without_selection(
                view,
                window,
                cx,
                &[
                    (ctrl_shift, "a"),
                    (ctrl_shift, "A"),
                    (ctrl_alt, "a"),
                    (ctrl_alt, "A"),
                ],
            );
        });
    });

    // With a whole-transcript selection installed, a wrongly routed "c"
    // would copy the transcript over the sentinel, and a wrongly routed
    // "a" would reinstall the selection.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            let transcript = "first

second";
            assert_eq!(
                TextSelection::selected_text(window, cx),
                transcript,
                "Cmd+A installs the whole transcript for the copy guard below"
            );
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));
            assert_combos_are_no_ops_with_transcript_selection(
                view,
                window,
                cx,
                transcript,
                &[
                    (ctrl_shift, "c"),
                    (ctrl_shift, "C"),
                    (ctrl_alt, "c"),
                    (ctrl_alt, "a"),
                    (ctrl_alt, "c"),
                ],
            );
        });
    });
}

/// Ctrl+platform combinations must not masquerade as plain Ctrl+A/C either:
/// the Ctrl branch outranks the platform branch on non-macOS, so
/// Ctrl+Super+A (Linux) and Ctrl+Win+A (Windows) leave the composer cursor,
/// transcript selection, and clipboard untouched. Gated to non-macOS because
/// the platform modifier is Command there and platform-modified events keep
/// their existing Command routing.
#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn ctrl_platform_combos_neither_select_all_nor_copy(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("second", "model"),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();

    // Without a selection, a wrongly routed "a" would run the composer
    // select-all and move the cursor to the end of the input.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.input_text = "hello".to_string();
            view.state.cursor_position = 0;
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));

            let ctrl_platform = Modifiers {
                control: true,
                platform: true,
                ..Default::default()
            };
            for key in ["a", "A"] {
                view.handle_key_down(&modified_chat_key_event(key, ctrl_platform), window, cx);

                assert_eq!(
                    view.state.cursor_position, 0,
                    "Ctrl+platform+{key} must not run composer select-all"
                );
                assert!(
                    !TextSelection::has_selection(window, cx),
                    "Ctrl+platform+{key} must not install a transcript selection"
                );
                assert_eq!(
                    cx.read_from_clipboard().and_then(|item| item.text()),
                    Some("sentinel".to_string()),
                    "Ctrl+platform+{key} must not touch the clipboard"
                );
            }
        });
    });

    // With a whole-transcript selection installed, a wrongly routed "c"
    // would copy the transcript over the sentinel.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            assert_eq!(
                TextSelection::selected_text(window, cx),
                "first

second",
                "Cmd+A installs the whole transcript for the copy guard below"
            );
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));

            let ctrl_platform = Modifiers {
                control: true,
                platform: true,
                ..Default::default()
            };
            for key in ["c", "C"] {
                view.handle_key_down(&modified_chat_key_event(key, ctrl_platform), window, cx);

                assert_eq!(
                    cx.read_from_clipboard().and_then(|item| item.text()),
                    Some("sentinel".to_string()),
                    "Ctrl+platform+{key} must not copy"
                );
                assert_eq!(
                    TextSelection::selected_text(window, cx),
                    "first

second",
                    "Ctrl+platform+{key} must not alter the transcript selection"
                );
                assert_eq!(
                    view.state.cursor_position, 0,
                    "Ctrl+platform+{key} must not move the composer cursor"
                );
            }
        });
    });
}

#[test]
fn non_macos_routes_only_ctrl_a_and_ctrl_c() {
    let ctrl = Modifiers {
        control: true,
        ..Default::default()
    };
    let command = Modifiers {
        platform: true,
        ..Default::default()
    };
    let plain = Modifiers::default();

    assert!(ChatView::routes_platform_shortcut(ctrl, "a", true));
    assert!(ChatView::routes_platform_shortcut(ctrl, "c", true));
    assert!(ChatView::routes_platform_shortcut(ctrl, "A", true));
    assert!(!ChatView::routes_platform_shortcut(ctrl, "x", true));
    assert!(!ChatView::routes_platform_shortcut(ctrl, "v", true));
    assert!(!ChatView::routes_platform_shortcut(plain, "a", true));
    assert!(!ChatView::routes_platform_shortcut(ctrl, "a", false));
    assert!(ChatView::routes_platform_shortcut(command, "a", false));
}

#[test]
fn non_macos_ctrl_a_and_ctrl_c_reject_any_companion_modifier() {
    let exact_ctrl = Modifiers {
        control: true,
        ..Default::default()
    };
    let ctrl_shift = Modifiers {
        control: true,
        shift: true,
        ..Default::default()
    };
    let ctrl_alt = Modifiers {
        control: true,
        alt: true,
        ..Default::default()
    };
    let ctrl_function = Modifiers {
        control: true,
        function: true,
        ..Default::default()
    };
    let ctrl_platform = Modifiers {
        control: true,
        platform: true,
        ..Default::default()
    };

    for key in ["a", "c", "A", "C"] {
        assert!(
            ChatView::routes_platform_shortcut(exact_ctrl, key, true),
            "exact plain Ctrl+{key} still routes"
        );
        assert!(
            !ChatView::routes_platform_shortcut(ctrl_shift, key, true),
            "Ctrl+Shift+{key} must not route"
        );
        assert!(
            !ChatView::routes_platform_shortcut(ctrl_alt, key, true),
            "Ctrl+Alt+{key} must not route"
        );
        assert!(
            !ChatView::routes_platform_shortcut(ctrl_function, key, true),
            "Ctrl+Fn+{key} must not route"
        );
        assert!(
            !ChatView::routes_platform_shortcut(ctrl_platform, key, true),
            "Ctrl+platform+{key} must not route"
        );
    }

    // On non-macOS the Ctrl branch outranks the platform branch, so a
    // Control event that also holds the platform modifier is rejected
    // instead of routing as a platform shortcut, while a platform-only
    // event keeps the pre-existing platform routing. On macOS the control
    // branch never fires and Command routing is unchanged.
    let platform_only = Modifiers {
        platform: true,
        ..Default::default()
    };
    assert!(ChatView::routes_platform_shortcut(platform_only, "a", true));
    assert!(ChatView::routes_platform_shortcut(
        ctrl_platform,
        "a",
        false
    ));
}

#[gpui::test]
fn escape_clears_selection_and_stops_the_owner_loop_without_stopping_the_stream(
    cx: &mut TestAppContext,
) {
    let view = chat_view_with(cx, vec![ChatMessage::user("message")], StreamingState::Idle);
    let (bridge, user_rx) = make_chat_bridge();
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.set_bridge_with_cx(bridge.clone(), cx);
            view.state.streaming = StreamingState::Streaming {
                content: "partial".to_string(),
                done: false,
            };
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.apply_selection_auto_scroll_command(Some(px(4.)), window.window_handle(), cx);
            assert!(view.selection_auto_scroll.is_active());

            view.handle_key_down(&chat_key_event("escape"), window, cx);

            assert!(!TextSelection::has_selection(window, cx));
            assert!(!view.selection_auto_scroll.is_active());
            assert!(matches!(
                view.state.streaming,
                StreamingState::Streaming { .. }
            ));
        });
    });
    assert!(user_rx.is_empty(), "Escape must not stop the stream");
}

#[gpui::test]
fn escape_without_a_selection_still_stops_the_stream(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![ChatMessage::user("message")],
        StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        },
    );
    let conversation = view.read_with(cx, |view, _| view.state.active_conversation_id);
    let (bridge, user_rx) = make_chat_bridge();
    let mut visual_cx = cx.add_empty_window().clone();
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.set_bridge_with_cx(bridge.clone(), cx);
            view.handle_key_down(&chat_key_event("escape"), window, cx);

            assert!(matches!(view.state.streaming, StreamingState::Idle));
        });
    });
    match user_rx.try_recv() {
        Ok(UserEvent::StopStreaming { conversation_id }) => {
            assert_eq!(Some(conversation_id), conversation);
        }
        other => panic!("expected StopStreaming, got {other:?}"),
    }
}

#[gpui::test]
fn copy_refuses_and_clears_when_any_selected_key_went_stale(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![
            ChatMessage::user("first"),
            ChatMessage::assistant("second", "model"),
            ChatMessage::user("third"),
        ],
        StreamingState::Idle,
    );
    let mut visual_cx = cx.add_empty_window().clone();

    // Conversation switch regenerates every key.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            view.state.active_conversation_id = Some(Uuid::new_v4());
            view.refresh_transcript_selection_revisions();

            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));
            view.handle_key_down(&chat_key_event("cmd-c"), window, cx);

            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_string())
            );
        });
    });

    // Deleting an interior message invalidates the keys after it.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.messages.remove(1);
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            // Restore the deleted message but keep its old key stale by
            // re-selecting against a mutated transcript.
            view.state
                .messages
                .insert(1, ChatMessage::assistant("second edited", "model"));
            view.refresh_transcript_selection_revisions();

            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));
            view.handle_key_down(&chat_key_event("cmd-c"), window, cx);

            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_string())
            );
        });
    });
}

#[gpui::test]
fn copy_refuses_when_the_streaming_row_or_emoji_filter_mutates(cx: &mut TestAppContext) {
    let view = chat_view_with(
        cx,
        vec![ChatMessage::assistant("hello 😀", "model")],
        StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        },
    );
    let mut visual_cx = cx.add_empty_window().clone();

    // Stream churn replaces the streaming row key.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            view.state.streaming = StreamingState::Streaming {
                content: "partial response".to_string(),
                done: false,
            };
            view.refresh_transcript_selection_revisions();

            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));
            view.handle_key_down(&chat_key_event("cmd-c"), window, cx);

            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_string())
            );
        });
    });

    // Emoji-filter toggle regenerates assistant message keys.
    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.state.streaming = StreamingState::Idle;
            view.refresh_transcript_selection_revisions();
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            view.state.filter_emoji = true;
            view.refresh_transcript_selection_revisions();

            cx.write_to_clipboard(gpui::ClipboardItem::new_string("sentinel".to_string()));
            view.handle_key_down(&chat_key_event("cmd-c"), window, cx);

            assert!(!TextSelection::has_selection(window, cx));
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("sentinel".to_string())
            );
        });
    });
}
