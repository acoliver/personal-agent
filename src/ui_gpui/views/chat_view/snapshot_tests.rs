//! Composer-draft behaviour across conversation selection changes.
//!
//! A draft belongs to the conversation it was typed in; these pin that contract.

use super::state::ChatState;
use super::ChatView;
use crate::presentation::view_command::ConversationSummary;
use crate::ui_gpui::app_store::{ChatStoreSnapshot, ConversationLoadState, StreamingStoreSnapshot};
use chrono::Utc;
use gpui::{AppContext, TestAppContext};
use uuid::Uuid;

fn ready_snapshot_for(conversation_id: Uuid, title: &str) -> ChatStoreSnapshot {
    ChatStoreSnapshot {
        selected_conversation_id: Some(conversation_id),
        selected_conversation_title: title.to_string(),
        selection_generation: 1,
        load_state: ConversationLoadState::Ready {
            conversation_id,
            generation: 1,
        },
        transcript: Vec::new(),
        streaming: StreamingStoreSnapshot::default(),
        conversations: vec![ConversationSummary {
            id: conversation_id,
            title: title.to_string(),
            updated_at: Utc::now(),
            message_count: 0,
            preview: None,
        }],
    }
}

/// A draft typed in one conversation must not follow the user into another, or they can
/// send it to the wrong recipient without noticing.
#[gpui::test]
fn switching_to_a_conversation_without_a_draft_clears_the_composer(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.apply_store_snapshot(ready_snapshot_for(first, "First"), cx);
            view.state.input_text = "half-written thought".to_string();
            view.state.cursor_position = view.state.input_text.len();

            view.apply_store_snapshot(ready_snapshot_for(second, "Second"), cx);

            assert_eq!(
                view.state.input_text, "",
                "the previous conversation's draft must not appear in another conversation"
            );
            assert_eq!(view.state.cursor_position, 0);
        });
    });
}

/// Going back to a conversation restores exactly what was left in its composer.
#[gpui::test]
fn returning_to_a_conversation_restores_its_own_draft(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.apply_store_snapshot(ready_snapshot_for(first, "First"), cx);
            view.state.input_text = "half-written thought".to_string();
            view.state.cursor_position = view.state.input_text.len();

            view.apply_store_snapshot(ready_snapshot_for(second, "Second"), cx);
            view.apply_store_snapshot(ready_snapshot_for(first, "First"), cx);

            assert_eq!(
                view.state.input_text, "half-written thought",
                "the conversation's own draft should come back"
            );
            assert_eq!(view.state.cursor_position, "half-written thought".len());
        });
    });
}

// ── composer focus vs. overlay chrome ────────────────────────────────────

/// `MainPanel::focus_current_view` calls `focus_composer` on every frame, so anything it
/// tears down is torn down continuously. Closing the dropdowns there made both the
/// conversation and the profile dropdown impossible to open: they were reset within a
/// millisecond of being toggled on.
#[gpui::test]
fn focusing_the_composer_leaves_open_dropdowns_alone(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.state.conversation_dropdown_open = true;
            view.state.profile_dropdown_open = true;
            view.state.conversation_title_editing = true;

            view.focus_composer(cx);

            assert!(
                view.state.conversation_dropdown_open,
                "a per-frame composer refocus must not close the conversation dropdown"
            );
            assert!(
                view.state.profile_dropdown_open,
                "a per-frame composer refocus must not close the profile dropdown"
            );
            assert!(
                view.state.conversation_title_editing,
                "a per-frame composer refocus must not cancel an inline rename"
            );
            assert!(
                view.state.composer_focused,
                "the composer should be focused"
            );
        });
    });
}

/// Clicking into the composer is a deliberate gesture and should dismiss the chrome.
#[gpui::test]
fn clicking_into_the_composer_dismisses_dropdowns_and_rename(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.state.conversation_dropdown_open = true;
            view.state.profile_dropdown_open = true;
            view.state.conversation_title_editing = true;

            view.focus_composer_dismissing_overlays(cx);

            assert!(!view.state.conversation_dropdown_open);
            assert!(!view.state.profile_dropdown_open);
            assert!(!view.state.conversation_title_editing);
            assert!(view.state.composer_focused);
        });
    });
}

/// The toggle must survive the refocus that immediately follows it on the next frame.
#[gpui::test]
fn a_toggled_dropdown_survives_the_next_frames_refocus(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view, cx| {
            view.toggle_conversation_dropdown(cx);
            assert!(
                view.state.conversation_dropdown_open,
                "toggle should open it"
            );

            // What MainPanel::focus_current_view does on every render.
            view.focus_composer(cx);

            assert!(
                view.state.conversation_dropdown_open,
                "the dropdown must still be open after the next frame refocuses the composer"
            );
        });
    });
}
