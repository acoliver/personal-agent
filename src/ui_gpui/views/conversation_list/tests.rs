//! Integration tests for `ConversationListView`.
//!
//! @plan PLAN-20260420-ISSUE180.P02
//! @requirement REQ-180-001
#![allow(clippy::future_not_send)]

use std::sync::Arc;

use chrono::{Duration, Utc};
use flume;
use gpui::{AppContext, EntityInputHandler, TestAppContext};
use uuid::Uuid;

use super::{ConversationListMode, ConversationListView};
use crate::events::types::UserEvent;
use crate::presentation::view_command::ConversationSummary;
use crate::ui_gpui::app_store::HistoryStoreSnapshot;
use crate::ui_gpui::bridge::GpuiBridge;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn summary(id: Uuid, title: &str) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at: Utc::now() - Duration::minutes(2),
        message_count: 0,
        preview: None,
    }
}

#[gpui::test]
async fn apply_store_snapshot_replaces_conversations_in_inline_mode(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));

    view.update(cx, |view, cx| {
        let snapshot = HistoryStoreSnapshot {
            conversations: vec![summary(id, "First")],
            selected_conversation_id: Some(id),
            streaming_conversation_ids: std::collections::HashSet::new(),
        };
        view.apply_store_snapshot(&snapshot, cx);
        assert_eq!(view.state.conversations.len(), 1);
        assert_eq!(view.active_conversation_id(), Some(id));
        assert_eq!(view.mode(), ConversationListMode::Inline);
    });
}

#[gpui::test]
async fn full_panel_mode_is_constructed_correctly(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::FullPanel, cx));
    view.update(cx, |view, _cx| {
        assert_eq!(view.mode(), ConversationListMode::FullPanel);
    });
}

#[gpui::test]
async fn trigger_sidebar_search_emits_event_when_query_non_empty(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));

    view.update(cx, |view, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.sidebar_search_query = "hello".to_string();
        view.trigger_sidebar_search(cx);
    });

    let event = user_rx.recv().expect("expected SearchConversations event");
    assert!(
        matches!(event, UserEvent::SearchConversations { ref query } if query == "hello"),
        "got {event:?}"
    );
}

#[gpui::test]
async fn trigger_sidebar_search_clears_results_when_query_blank(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));

    view.update(cx, |view, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.sidebar_search_query = "   ".to_string();
        view.state.sidebar_search_results = Some(Vec::new());
        view.trigger_sidebar_search(cx);
        assert!(view.state.sidebar_search_results.is_none());
    });

    assert!(
        user_rx.try_recv().is_err(),
        "blank query must not emit a search event"
    );
}

#[gpui::test]
async fn rename_flow_emits_confirm_on_submit(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let id = Uuid::new_v4();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));

    view.update(cx, |view, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.conversations = vec![summary(id, "Old name")];
        view.state.active_conversation_id = Some(id);
        view.start_rename_conversation(cx);
        assert!(view.state.conversation_title_editing);
        view.state.conversation_title_input = "New name".to_string();
        view.state.rename_replace_on_next_char = false;
        view.submit_rename_conversation(cx);
        assert!(!view.state.conversation_title_editing);
    });

    let event = user_rx.recv().expect("expected ConfirmRenameConversation");
    match event {
        UserEvent::ConfirmRenameConversation {
            id: emitted_id,
            title,
        } => {
            assert_eq!(emitted_id, id);
            assert_eq!(title, "New name");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[gpui::test]
async fn cancel_rename_emits_cancel_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let id = Uuid::new_v4();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));

    view.update(cx, |view, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.conversations = vec![summary(id, "Old name")];
        view.state.active_conversation_id = Some(id);
        view.start_rename_conversation(cx);
        view.cancel_rename_conversation(cx);
        assert!(!view.state.conversation_title_editing);
    });

    let event = user_rx.recv().expect("expected CancelRenameConversation");
    assert!(matches!(event, UserEvent::CancelRenameConversation));
}

#[gpui::test]
async fn rename_backspace_clears_buffer_when_replace_flag_set(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));
    view.update(cx, |view, cx| {
        view.state.conversations = vec![summary(id, "Old name")];
        view.state.active_conversation_id = Some(id);
        view.start_rename_conversation(cx);
        // start_rename sets replace_on_next_char = true
        view.handle_rename_backspace(cx);
        assert!(view.state.conversation_title_input.is_empty());
        assert!(!view.state.rename_replace_on_next_char);
    });
}

#[gpui::test]
async fn clear_search_resets_state(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));
    view.update(cx, |view, cx| {
        view.state.sidebar_search_query = "x".to_string();
        view.state.sidebar_search_focused = true;
        view.state.sidebar_search_results = Some(Vec::new());
        view.clear_search(cx);
        assert!(view.state.sidebar_search_query.is_empty());
        assert!(!view.state.sidebar_search_focused);
        assert!(view.state.sidebar_search_results.is_none());
    });
}

#[gpui::test]
async fn set_active_conversation_id_updates_state(cx: &mut TestAppContext) {
    let id = Uuid::new_v4();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));
    view.update(cx, |view, cx| {
        assert_eq!(view.active_conversation_id(), None);
        view.set_active_conversation_id(Some(id), cx);
        assert_eq!(view.active_conversation_id(), Some(id));
    });
}

#[gpui::test]
async fn apply_search_results_stores_them(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::Inline, cx));
    view.update(cx, |view, cx| {
        view.state.sidebar_search_query = "hi".to_string();
        view.apply_search_results(Vec::new(), cx);
        // Empty results with non-empty query should still be Some(empty vec).
        assert!(view.state.sidebar_search_results.is_some());
        assert!(view
            .state
            .sidebar_search_results
            .as_ref()
            .unwrap()
            .is_empty());
    });
}

#[gpui::test]
async fn list_input_handler_types_into_focused_search(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::FullPanel, cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.state.sidebar_search_focused = true;
            view.replace_text_in_range(None, "hist", window, cx);
            assert_eq!(view.state.sidebar_search_query, "hist");
        });
    });

    let event = user_rx.recv().expect("expected SearchConversations event");
    assert!(
        matches!(event, UserEvent::SearchConversations { ref query } if query == "hist"),
        "got {event:?}"
    );
}

#[gpui::test]
async fn list_input_handler_supports_backspace_and_escape_for_search(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| ConversationListView::new(ConversationListMode::FullPanel, cx));

    view.update(cx, |view, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.state.sidebar_search_focused = true;
        view.state.sidebar_search_query = "history".to_string();
        view.handle_key_down(&key_event("backspace"), cx);
        assert_eq!(view.state.sidebar_search_query, "histor");
        view.handle_key_down(&key_event("escape"), cx);
        assert!(!view.state.sidebar_search_focused);
    });

    let event = user_rx.recv().expect("expected SearchConversations event");
    assert!(
        matches!(event, UserEvent::SearchConversations { ref query } if query == "histor"),
        "got {event:?}"
    );
}

fn key_event(key: &str) -> gpui::KeyDownEvent {
    gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}
