//! State for the shared `ConversationListView`.
//!
//! Single source of truth for everything visible in the popout sidebar
//! and the popin History panel: conversations list, current selection,
//! per-row streaming indicators, search query/results, inline rename
//! editor and inline delete confirmation.
//!
//! @plan PLAN-20260420-ISSUE180.P01
//! @requirement REQ-180-001

use std::collections::HashSet;

use uuid::Uuid;

use crate::presentation::view_command::{ConversationSearchResult, ConversationSummary};
use crate::ui_gpui::app_store::HistoryStoreSnapshot;

/// State shared by the popout sidebar and the popin History panel.
///
/// All fields are pure UI state — there is no rendering, no bridge,
/// and no event emission here.
#[derive(Clone, Default)]
pub struct ConversationListState {
    /// Conversations sourced from the history store snapshot.
    pub conversations: Vec<ConversationSummary>,
    /// Currently selected conversation, mirrored from the store snapshot.
    pub active_conversation_id: Option<Uuid>,
    /// Conversation ids currently streaming in the background.
    pub streaming_conversation_ids: HashSet<Uuid>,
    /// Current search query typed in the search box.
    pub sidebar_search_query: String,
    /// Whether the search box currently has input focus.
    pub sidebar_search_focused: bool,
    /// Search results from the backend, if a search is active.
    pub sidebar_search_results: Option<Vec<ConversationSearchResult>>,
    /// Conversation pending delete confirmation (inline UI).
    pub delete_confirming_id: Option<Uuid>,
    /// Whether the inline title-rename editor is open for the active row.
    pub conversation_title_editing: bool,
    /// Working buffer for the inline title-rename editor.
    pub conversation_title_input: String,
    /// When true, the next character in the rename editor replaces the buffer.
    pub rename_replace_on_next_char: bool,
}

impl ConversationListState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_conversations(mut self, conversations: Vec<ConversationSummary>) -> Self {
        self.conversations = conversations;
        self
    }

    #[must_use]
    pub const fn with_active_conversation_id(mut self, id: Option<Uuid>) -> Self {
        self.active_conversation_id = id;
        self
    }

    #[must_use]
    pub fn with_streaming_conversation_ids(mut self, ids: HashSet<Uuid>) -> Self {
        self.streaming_conversation_ids = ids;
        self
    }

    /// Apply a history store snapshot to this state, mutating in place.
    /// Conversations, selected id, and streaming ids are all replaced.
    /// Ephemeral UI state (search, rename, delete-confirm) is preserved.
    pub fn apply_history_snapshot(&mut self, snapshot: &HistoryStoreSnapshot) {
        self.conversations.clone_from(&snapshot.conversations);
        self.active_conversation_id = snapshot.selected_conversation_id;
        self.streaming_conversation_ids
            .clone_from(&snapshot.streaming_conversation_ids);
        // If the active conversation was deleted, clear the delete-confirm guard.
        if let Some(pending) = self.delete_confirming_id {
            if !self.conversations.iter().any(|c| c.id == pending) {
                self.delete_confirming_id = None;
            }
        }
        // Defensive cleanup: if no rename is in flight, drop any leftover
        // input bytes / replace-on-next-char flag so a previous aborted rename
        // can't poison the next start_rename_conversation call. We deliberately
        // do NOT clear the buffer when the user IS renaming, so that snapshot
        // refreshes (which arrive while typing) leave the in-progress edit
        // intact.
        if !self.conversation_title_editing {
            self.conversation_title_input.clear();
            self.rename_replace_on_next_char = false;
        }
    }

    /// Returns true if the given conversation id is currently streaming.
    #[must_use]
    pub fn is_streaming(&self, conversation_id: Uuid) -> bool {
        self.streaming_conversation_ids.contains(&conversation_id)
    }

    /// Return the title for a conversation, falling back to the standard
    /// "Untitled Conversation" label when blank.
    #[must_use]
    pub fn display_title(title: &str) -> String {
        if title.trim().is_empty() {
            "Untitled Conversation".to_string()
        } else {
            title.to_string()
        }
    }

    /// Reset the inline search state (results + query + focus).
    pub fn clear_search(&mut self) {
        self.sidebar_search_query.clear();
        self.sidebar_search_results = None;
        self.sidebar_search_focused = false;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::ConversationListState;
    use crate::presentation::view_command::ConversationSummary;
    use crate::ui_gpui::app_store::HistoryStoreSnapshot;

    fn conversation_summary(
        id: Uuid,
        title: &str,
        updated_at: chrono::DateTime<Utc>,
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

    /// @plan PLAN-20260420-ISSUE180.P01
    /// @requirement REQ-180-001
    ///
    /// Migrated from `tests/history_view_tests.rs` — verifies the state
    /// projects titles/dates/counts/selection from the history snapshot.
    #[test]
    fn applies_snapshot_projects_titles_selection_counts_and_dates() {
        let selected_id = Uuid::new_v4();
        let older_id = Uuid::new_v4();
        let snapshot = HistoryStoreSnapshot {
            conversations: vec![
                conversation_summary(older_id, "", Utc::now() - Duration::days(2), 9),
                conversation_summary(
                    selected_id,
                    "Selected conversation",
                    Utc::now() - Duration::minutes(5),
                    1,
                ),
            ],
            selected_conversation_id: Some(selected_id),
            streaming_conversation_ids: HashSet::new(),
        };

        let mut state = ConversationListState::new();
        state.apply_history_snapshot(&snapshot);

        assert_eq!(state.conversations.len(), 2);
        assert_eq!(state.conversations[0].id, older_id);
        // Empty titles are surfaced verbatim from the snapshot — display
        // fallback to "Untitled Conversation" happens at render time.
        assert!(state.conversations[0].title.is_empty());
        assert_eq!(state.conversations[0].message_count, 9);
        assert_eq!(state.conversations[1].id, selected_id);
        assert_eq!(state.conversations[1].title, "Selected conversation");
        assert_eq!(state.active_conversation_id, Some(selected_id));
    }

    /// @plan PLAN-20260420-ISSUE180.P01
    /// @requirement REQ-180-001
    ///
    /// Migrated from `tests/history_view_tests.rs` — verifies that
    /// successive snapshots cover create/rename/delete and that the
    /// active id tracks the snapshot selection.
    #[test]
    fn snapshot_transitions_cover_refresh_activate_create_delete_rename() {
        let selected_id = Uuid::new_v4();
        let created_id = Uuid::new_v4();
        let mut state = ConversationListState::new();

        let initial = HistoryStoreSnapshot {
            conversations: vec![conversation_summary(selected_id, "Selected", Utc::now(), 0)],
            selected_conversation_id: Some(selected_id),
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&initial);
        assert_eq!(state.conversations.len(), 1);
        assert_eq!(state.active_conversation_id, Some(selected_id));

        let created = HistoryStoreSnapshot {
            conversations: vec![
                conversation_summary(created_id, "New Conversation", Utc::now(), 0),
                conversation_summary(selected_id, "Selected", Utc::now(), 0),
            ],
            selected_conversation_id: Some(created_id),
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&created);
        assert_eq!(state.conversations[0].id, created_id);
        assert_eq!(state.active_conversation_id, Some(created_id));

        let renamed = HistoryStoreSnapshot {
            conversations: vec![
                conversation_summary(created_id, "Renamed conversation", Utc::now(), 0),
                conversation_summary(selected_id, "Selected", Utc::now(), 0),
            ],
            selected_conversation_id: Some(created_id),
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&renamed);
        assert_eq!(state.conversations[0].title, "Renamed conversation");

        let deleted = HistoryStoreSnapshot {
            conversations: vec![conversation_summary(
                created_id,
                "Renamed conversation",
                Utc::now(),
                0,
            )],
            selected_conversation_id: Some(created_id),
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&deleted);
        assert_eq!(state.active_conversation_id, Some(created_id));
        assert!(state.conversations.iter().all(|c| c.id != selected_id));
    }

    /// @plan PLAN-20260420-ISSUE180.P01
    /// @requirement REQ-180-001
    ///
    /// Migrated from `tests/history_view_tests.rs` — verifies that
    /// applying a snapshot replaces conversations and selection wholesale.
    #[test]
    fn apply_store_snapshot_replaces_state_and_selection() {
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();

        let mut state = ConversationListState::new()
            .with_conversations(vec![conversation_summary(first_id, "First", Utc::now(), 0)])
            .with_active_conversation_id(Some(first_id));

        let next = HistoryStoreSnapshot {
            conversations: vec![conversation_summary(second_id, "Second", Utc::now(), 0)],
            selected_conversation_id: Some(second_id),
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&next);

        assert_eq!(state.conversations.len(), 1);
        assert_eq!(state.conversations[0].id, second_id);
        assert_eq!(state.active_conversation_id, Some(second_id));
    }

    /// Streaming ids drive the per-row indicator helper in the snapshot.
    #[test]
    fn snapshot_replaces_streaming_conversation_ids() {
        let conv_id = Uuid::new_v4();
        let mut streaming_ids = HashSet::new();
        streaming_ids.insert(conv_id);

        let snapshot = HistoryStoreSnapshot {
            conversations: vec![conversation_summary(conv_id, "Streaming", Utc::now(), 0)],
            selected_conversation_id: None,
            streaming_conversation_ids: streaming_ids,
        };

        let mut state = ConversationListState::new();
        state.apply_history_snapshot(&snapshot);
        assert!(state.is_streaming(conv_id));

        let cleared = HistoryStoreSnapshot {
            conversations: snapshot.conversations,
            selected_conversation_id: None,
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&cleared);
        assert!(!state.is_streaming(conv_id));
    }

    #[test]
    fn delete_confirming_id_is_cleared_when_target_is_removed() {
        let pending_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();

        let mut state = ConversationListState::new();
        state.delete_confirming_id = Some(pending_id);

        let snapshot = HistoryStoreSnapshot {
            conversations: vec![conversation_summary(other_id, "Other", Utc::now(), 0)],
            selected_conversation_id: Some(other_id),
            streaming_conversation_ids: HashSet::new(),
        };
        state.apply_history_snapshot(&snapshot);

        assert!(state.delete_confirming_id.is_none());
    }

    #[test]
    fn clear_search_resets_query_results_and_focus() {
        let mut state = ConversationListState::new();
        state.sidebar_search_query = "skills".to_string();
        state.sidebar_search_focused = true;
        state.sidebar_search_results = Some(Vec::new());

        state.clear_search();

        assert!(state.sidebar_search_query.is_empty());
        assert!(!state.sidebar_search_focused);
        assert!(state.sidebar_search_results.is_none());
    }

    #[test]
    fn display_title_falls_back_for_blank_titles() {
        assert_eq!(
            ConversationListState::display_title(""),
            "Untitled Conversation"
        );
        assert_eq!(
            ConversationListState::display_title("   "),
            "Untitled Conversation"
        );
        assert_eq!(ConversationListState::display_title("Hello"), "Hello");
    }
}
