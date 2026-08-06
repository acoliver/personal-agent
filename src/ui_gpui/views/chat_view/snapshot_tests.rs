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
