use chrono::{Duration, Utc};
use personal_agent::presentation::view_command::ConversationSummary;
use personal_agent::ui_gpui::views::{ConversationItem, HistoryState};
use uuid::Uuid;

fn summary(
    id: Uuid,
    title: &str,
    updated_at: chrono::DateTime<chrono::Utc>,
    message_count: usize,
) -> ConversationSummary {
    ConversationSummary {
        id,
        title: title.to_string(),
        updated_at,
        message_count,
        preview: None,
    }
}

fn format_date(dt: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> String {
    let diff = now.signed_duration_since(dt);

    if diff.num_minutes() < 1 {
        "Just now".to_string()
    } else if diff.num_hours() < 1 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_days() < 1 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}d ago", diff.num_days())
    }
}

fn items_from_snapshot(
    conversations: &[ConversationSummary],
    selected_conversation_id: Option<Uuid>,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<ConversationItem> {
    conversations
        .iter()
        .map(|conversation| {
            let title = if conversation.title.trim().is_empty() {
                "Untitled Conversation".to_string()
            } else {
                conversation.title.clone()
            };

            ConversationItem::new(conversation.id, title)
                .with_date(format_date(conversation.updated_at, now))
                .with_message_count(conversation.message_count)
                .with_selected(Some(conversation.id) == selected_conversation_id)
        })
        .collect()
}

fn apply_conversation_list_refreshed(
    state: &mut HistoryState,
    conversations: &[ConversationSummary],
    now: chrono::DateTime<chrono::Utc>,
) {
    let selected_conversation_id = state.selected_conversation_id;
    state.conversations = items_from_snapshot(conversations, selected_conversation_id, now);
}

fn apply_conversation_activated(state: &mut HistoryState, id: Uuid) {
    state.selected_conversation_id = Some(id);
    refresh_selection_flags(state);
}

fn apply_conversation_created(state: &mut HistoryState, id: Uuid) {
    if !state
        .conversations
        .iter()
        .any(|conversation| conversation.id == id)
    {
        state.conversations.insert(
            0,
            ConversationItem::new(id, "New Conversation")
                .with_date("Just now")
                .with_message_count(0)
                .with_selected(Some(id) == state.selected_conversation_id),
        );
    }
}

fn apply_conversation_deleted(state: &mut HistoryState, id: Uuid) {
    state
        .conversations
        .retain(|conversation| conversation.id != id);
    if state.selected_conversation_id == Some(id) {
        state.selected_conversation_id = state
            .conversations
            .first()
            .map(|conversation| conversation.id);
        refresh_selection_flags(state);
    }
}

fn apply_conversation_renamed(state: &mut HistoryState, id: Uuid, new_title: &str) {
    if let Some(conversation) = state
        .conversations
        .iter_mut()
        .find(|conversation| conversation.id == id)
    {
        conversation.title = new_title.to_string();
    }
}

fn apply_store_snapshot(
    state: &mut HistoryState,
    conversations: &[ConversationSummary],
    selected_conversation_id: Option<Uuid>,
    now: chrono::DateTime<chrono::Utc>,
) {
    *state = HistoryState::new()
        .with_selected_conversation_id(selected_conversation_id)
        .with_conversations(items_from_snapshot(
            conversations,
            selected_conversation_id,
            now,
        ));
}

fn refresh_selection_flags(state: &mut HistoryState) {
    let selected_conversation_id = state.selected_conversation_id;
    for conversation in &mut state.conversations {
        conversation.is_selected = Some(conversation.id) == selected_conversation_id;
    }
}

#[test]
fn conversation_item_builders_set_expected_fields() {
    let id = Uuid::new_v4();

    let item = ConversationItem::new(id, "Project chat")
        .with_date("2h ago")
        .with_message_count(7)
        .with_selected(true);

    assert_eq!(item.id, id);
    assert_eq!(item.title, "Project chat");
    assert_eq!(item.date_display, "2h ago");
    assert_eq!(item.message_count, 7);
    assert!(item.is_selected);
}

#[test]
fn history_state_builders_set_expected_fields() {
    let selected_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let conversations = vec![
        ConversationItem::new(selected_id, "Selected"),
        ConversationItem::new(other_id, "Other"),
    ];

    let state = HistoryState::new()
        .with_conversations(conversations.clone())
        .with_selected_conversation_id(Some(selected_id));

    assert_eq!(state.conversations, conversations);
    assert_eq!(state.selected_conversation_id, Some(selected_id));
}

#[test]
fn format_date_covers_just_now_minutes_hours_and_days_ranges() {
    let now = Utc::now();

    assert_eq!(format_date(now - Duration::seconds(30), now), "Just now");
    assert_eq!(format_date(now - Duration::minutes(5), now), "5m ago");
    assert_eq!(format_date(now - Duration::hours(3), now), "3h ago");
    assert_eq!(format_date(now - Duration::days(2), now), "2d ago");
}

#[test]
fn items_from_snapshot_projects_titles_selection_counts_and_dates() {
    let now = Utc::now();
    let selected_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();

    let items = items_from_snapshot(
        &[
            summary(other_id, "", now - Duration::days(2), 9),
            summary(
                selected_id,
                "Selected from snapshot",
                now - Duration::minutes(5),
                1,
            ),
        ],
        Some(selected_id),
        now,
    );

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Untitled Conversation");
    assert_eq!(items[0].date_display, "2d ago");
    assert_eq!(items[0].message_count, 9);
    assert!(!items[0].is_selected);
    assert_eq!(items[1].title, "Selected from snapshot");
    assert_eq!(items[1].date_display, "5m ago");
    assert_eq!(items[1].message_count, 1);
    assert!(items[1].is_selected);
}

#[test]
fn history_state_transitions_cover_refresh_activate_create_delete_rename_and_ignore() {
    let selected_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let created_id = Uuid::new_v4();
    let renamed_id = Uuid::new_v4();
    let now = Utc::now();
    let mut state = HistoryState::new();

    apply_conversation_list_refreshed(
        &mut state,
        &[
            summary(selected_id, "", now, 0),
            summary(other_id, "Minutes", now - Duration::minutes(5), 2),
            summary(renamed_id, "Hours", now - Duration::hours(3), 4),
        ],
        now,
    );

    assert_eq!(state.conversations.len(), 3);
    assert_eq!(state.conversations[0].title, "Untitled Conversation");
    assert_eq!(state.conversations[0].date_display, "Just now");
    assert_eq!(state.conversations[1].date_display, "5m ago");
    assert_eq!(state.conversations[2].date_display, "3h ago");

    apply_conversation_activated(&mut state, selected_id);
    assert_eq!(state.selected_conversation_id, Some(selected_id));
    assert!(state.conversations[0].is_selected);
    assert!(!state.conversations[1].is_selected);

    apply_conversation_created(&mut state, created_id);
    assert_eq!(state.conversations[0].id, created_id);
    assert_eq!(state.conversations[0].title, "New Conversation");
    assert_eq!(state.conversations[0].date_display, "Just now");
    assert_eq!(state.conversations[0].message_count, 0);
    assert!(!state.conversations[0].is_selected);

    apply_conversation_created(&mut state, created_id);
    assert_eq!(
        state
            .conversations
            .iter()
            .filter(|item| item.id == created_id)
            .count(),
        1
    );

    apply_conversation_renamed(&mut state, renamed_id, "Renamed conversation");
    assert_eq!(
        state
            .conversations
            .iter()
            .find(|item| item.id == renamed_id)
            .expect("renamed conversation exists")
            .title,
        "Renamed conversation"
    );

    let unchanged = state.clone();
    assert_eq!(state.conversations, unchanged.conversations);
    assert_eq!(
        state.selected_conversation_id,
        unchanged.selected_conversation_id
    );

    apply_conversation_deleted(&mut state, selected_id);
    assert!(state
        .conversations
        .iter()
        .all(|item| item.id != selected_id));
    assert_eq!(state.selected_conversation_id, Some(created_id));
    assert!(
        state
            .conversations
            .iter()
            .find(|item| item.id == created_id)
            .expect("created conversation still exists")
            .is_selected
    );
}

#[test]
fn apply_store_snapshot_replaces_state_and_selection() {
    let now = Utc::now();
    let selected_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let mut state = HistoryState::new()
        .with_selected_conversation_id(Some(Uuid::new_v4()))
        .with_conversations(vec![ConversationItem::new(Uuid::new_v4(), "Old")]);

    apply_store_snapshot(
        &mut state,
        &[
            summary(other_id, "", now - Duration::days(2), 9),
            summary(selected_id, "Selected from snapshot", now, 1),
        ],
        Some(selected_id),
        now,
    );

    assert_eq!(state.selected_conversation_id, Some(selected_id));
    assert_eq!(state.conversations.len(), 2);
    assert_eq!(state.conversations[0].title, "Untitled Conversation");
    assert_eq!(state.conversations[0].message_count, 9);
    assert!(!state.conversations[0].is_selected);
    assert_eq!(state.conversations[1].title, "Selected from snapshot");
    assert!(state.conversations[1].is_selected);
}
