//! Queued steering entries in the transcript (issue #222).
//!
//! A steer the service accepted has to be visible while it waits, gone once
//! it lands, and absent from everything the transcript treats as content.
//! These drive the real `handle_command` dispatch and the real Cmd+A path,
//! so an entry that lingers, leaks into another conversation, or shifts
//! selection identity fails here.
//!
//! @plan PLAN-20260903-ISSUE222.P04
//! @requirement REQ-222-003

#![allow(clippy::future_not_send)]
#![allow(deprecated)]

use super::state::{ChatMessage, ChatState, StreamingState};
use super::transcript::TranscriptRow;
use super::ChatView;
use crate::presentation::view_command::{ConversationSummary, ViewCommand};
use crate::ui_gpui::app_store::{ChatStoreSnapshot, ConversationLoadState, StreamingStoreSnapshot};
use chrono::Utc;
use gpui::{AppContext, Entity, KeyDownEvent, Keystroke, Modifiers, TestAppContext};
use gpui_selection_vendor::{TextSelection, TextSelectionContentKey};
use uuid::Uuid;

fn mid_turn_state(conversation_id: Uuid) -> ChatState {
    ChatState {
        active_conversation_id: Some(conversation_id),
        messages: vec![ChatMessage::user("stable")],
        streaming: StreamingState::Streaming {
            content: "partial".to_string(),
            done: false,
        },
        ..ChatState::default()
    }
}

fn queued(conversation_id: Uuid, steer_id: Uuid, text: &str) -> ViewCommand {
    ViewCommand::SteeringQueued {
        conversation_id,
        steer_id,
        text: text.to_string(),
    }
}

fn queued_texts(view: &ChatView) -> Vec<String> {
    view.state
        .queued_steering
        .iter()
        .map(|entry| (*entry.text).clone())
        .collect()
}

fn queued_rows(view: &ChatView) -> Vec<TranscriptRow> {
    view.transcript_rows()
        .into_iter()
        .filter(|row| matches!(row, TranscriptRow::QueuedSteering(_)))
        .collect()
}

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

// ── REQ-222-003: an accepted steer is visible until it lands ─────────────

/// The whole point of a queued entry is that the user can see the
/// instruction is waiting, and see it stop waiting.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn a_queued_steer_appears_in_the_transcript_until_it_is_delivered(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                queued(conversation_id, steer_id, "use the cached index"),
                cx,
            );

            assert_eq!(queued_texts(view), vec!["use the cached index".to_string()]);
            assert_eq!(
                queued_rows(view),
                vec![TranscriptRow::QueuedSteering(0)],
                "a waiting steer must have a row of its own in the transcript"
            );

            view.handle_command(
                ViewCommand::SteeringDelivered {
                    conversation_id,
                    steer_id,
                },
                cx,
            );

            assert!(
                view.state.queued_steering.is_empty(),
                "a delivered steer has been said to the model, so it stops waiting"
            );
            assert!(queued_rows(view).is_empty());
        });
    });
}

/// Two steers wait in the order they were submitted, and delivering the
/// first leaves the second alone.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn queued_steers_keep_submission_order_and_are_withdrawn_one_at_a_time(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(queued(conversation_id, first, "first"), cx);
            view.handle_command(queued(conversation_id, second, "second"), cx);
            assert_eq!(
                queued_texts(view),
                vec!["first".to_string(), "second".to_string()]
            );

            view.handle_command(
                ViewCommand::SteeringDelivered {
                    conversation_id,
                    steer_id: first,
                },
                cx,
            );

            assert_eq!(
                queued_texts(view),
                vec!["second".to_string()],
                "delivery withdraws the entry it names and only that one"
            );
        });
    });
}

/// A delivery id the view never queued belongs to something else. Removing
/// on a miss would silently drop a steer the user is still waiting on.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn delivery_of_an_unknown_id_withdraws_nothing(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(queued(conversation_id, steer_id, "still waiting"), cx);

            view.handle_command(
                ViewCommand::SteeringDelivered {
                    conversation_id,
                    steer_id: Uuid::new_v4(),
                },
                cx,
            );

            assert_eq!(
                queued_texts(view),
                vec!["still waiting".to_string()],
                "an unmatched delivery must leave the queue alone"
            );
        });
    });
}

/// A refused steer was never queued, so there is no entry under it to
/// withdraw. Removing anything here would take away a different steer that
/// the service is still holding.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-004
#[gpui::test]
fn a_rejection_withdraws_no_queued_entry(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let steer_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(queued(conversation_id, steer_id, "accepted earlier"), cx);

            view.handle_command(
                ViewCommand::SteeringRejected {
                    conversation_id,
                    error: "Steering queue is full".to_string(),
                },
                cx,
            );

            assert_eq!(
                queued_texts(view),
                vec!["accepted earlier".to_string()],
                "a refusal concerns a steer that was never queued"
            );
        });
    });
}

/// Queued entries describe the transcript on screen. A steer accepted for
/// another conversation is not part of it.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn a_steer_for_another_conversation_is_not_shown_here(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                queued(Uuid::new_v4(), Uuid::new_v4(), "meant for elsewhere"),
                cx,
            );

            assert!(
                view.state.queued_steering.is_empty(),
                "another conversation's steer must not appear in this transcript"
            );
        });
    });
}

/// Switching away drops the entries with the transcript they annotated, the
/// way the cleared conversation's approval bubbles go.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn switching_conversations_clears_queued_entries(cx: &mut TestAppContext) {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.apply_store_snapshot(ready_snapshot_for(first, "First"), cx);
            view.handle_command(queued(first, Uuid::new_v4(), "for the first"), cx);
            assert_eq!(queued_texts(view), vec!["for the first".to_string()]);

            view.apply_store_snapshot(ready_snapshot_for(second, "Second"), cx);

            assert!(
                view.state.queued_steering.is_empty(),
                "a queued steer must not follow the user into another conversation"
            );
        });
    });
}

/// Clearing a conversation takes its queued steers with it.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn clearing_the_conversation_clears_queued_entries(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let view = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(queued(conversation_id, Uuid::new_v4(), "waiting"), cx);
            assert_eq!(view.state.queued_steering.len(), 1);

            view.handle_command(ViewCommand::ConversationCleared, cx);

            assert!(
                view.state.queued_steering.is_empty(),
                "the turn these were waiting for has been cleared away"
            );
        });
    });
}

// ── Selection identity must not skew ─────────────────────────────────────

fn cmd_a() -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Modifiers {
                platform: true,
                ..Modifiers::default()
            },
            ..Keystroke::parse("a").expect("a keystroke")
        },
        is_held: false,
        prefer_character_input: false,
    }
}

fn transcript_keys(view: &ChatView) -> Vec<TextSelectionContentKey> {
    vec![
        view.message_selection_content_key(0),
        view.streaming_selection_content_key(),
    ]
}

/// Installs a window selection standing in for a user drag, because
/// `select_all_for_focused_surface` reaches the transcript only when one
/// already exists.
fn seed_drag_selection(window: &mut gpui::Window, cx: &mut gpui::Context<ChatView>) {
    TextSelection::select_all(&[TextSelectionContentKey::new(999_999)], "drag", window, cx);
}

/// Select-all is where row identity, leaf counting and the copy document all
/// have to agree. A queued entry adds a row, so if it were counted as
/// content the keys and the copied text would both skew.
///
/// @plan PLAN-20260903-ISSUE222.P04
/// @requirement REQ-222-003
#[gpui::test]
fn a_queued_entry_leaves_select_all_identical(cx: &mut TestAppContext) {
    let conversation_id = Uuid::new_v4();
    let view: Entity<ChatView> = cx.new(|cx| ChatView::new(mid_turn_state(conversation_id), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);
            let keys_before = TextSelection::selected_content_keys(window, cx);
            let text_before = TextSelection::selected_text(window, cx);
            assert_eq!(keys_before, Some(transcript_keys(view)));
            assert_eq!(text_before, "stable\n\npartial▋");

            view.handle_command(
                queued(conversation_id, Uuid::new_v4(), "do not copy me"),
                cx,
            );
            seed_drag_selection(window, cx);
            view.handle_key_down(&cmd_a(), window, cx);

            assert_eq!(
                TextSelection::selected_content_keys(window, cx),
                keys_before,
                "a queued entry must not add, remove or renumber a selection key"
            );
            assert_eq!(
                TextSelection::selected_text(window, cx),
                text_before,
                "a queued steer has not been said yet, so it is not part of the transcript"
            );
        });
    });
}
