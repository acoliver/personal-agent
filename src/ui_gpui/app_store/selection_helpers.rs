use uuid::Uuid;

use crate::presentation::view_command::{
    ConversationMessagePayload, ConversationSummary, MessageRole, ProfileSummary,
};
use crate::ui_gpui::app_store_types::ConversationLoadState;

use super::{clear_streaming_ephemera_only, AppStoreInner, SelectedTitleProvenance};

pub(super) fn maybe_sync_selected_title(inner: &mut AppStoreInner) -> bool {
    let Some(conversation_id) = inner.snapshot.chat.selected_conversation_id else {
        return false;
    };
    maybe_upgrade_selected_title_from_history(inner, conversation_id)
}

pub(super) fn maybe_upgrade_selected_title_from_history(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    if inner.snapshot.chat.selected_conversation_id != Some(conversation_id) {
        return false;
    }

    if let Some(history_title) =
        authoritative_history_title(&inner.snapshot.history.conversations, conversation_id)
    {
        if matches!(
            inner.title_provenance,
            SelectedTitleProvenance::LiteralFallback
        ) && inner.snapshot.chat.selected_conversation_title != history_title
        {
            inner.snapshot.chat.selected_conversation_title = history_title;
            inner.title_provenance = SelectedTitleProvenance::HistoryBacked;
            return true;
        }
    }

    false
}

pub(super) fn apply_selected_title_from_history(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    if let Some(history_title) =
        authoritative_history_title(&inner.snapshot.history.conversations, conversation_id)
    {
        inner.snapshot.chat.selected_conversation_title = history_title;
        inner.title_provenance = SelectedTitleProvenance::HistoryBacked;
        return true;
    }
    false
}

fn authoritative_history_title(
    conversations: &[ConversationSummary],
    conversation_id: Uuid,
) -> Option<String> {
    conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)
        .map(|conversation| normalize_title(&conversation.title))
}

pub(super) fn load_state_targets_different_conversation(
    load_state: &ConversationLoadState,
    conversation_id: Uuid,
) -> bool {
    match load_state {
        ConversationLoadState::Loading {
            conversation_id: active_id,
            ..
        }
        | ConversationLoadState::Ready {
            conversation_id: active_id,
            ..
        }
        | ConversationLoadState::Error {
            conversation_id: active_id,
            ..
        } => *active_id != conversation_id,
        ConversationLoadState::Idle => false,
    }
}

pub(super) fn append_persisted_message_if_target_matches_selected(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    role: MessageRole,
    content: String,
    model_id: Option<String>,
) -> bool {
    if matches!(role, MessageRole::User | MessageRole::Assistant)
        && inner.snapshot.chat.selected_conversation_id != Some(conversation_id)
    {
        return false;
    }

    inner
        .snapshot
        .chat
        .transcript
        .push(ConversationMessagePayload {
            role,
            content,
            thinking_content: None,
            timestamp: None,
            model_id,
        });
    true
}

pub(super) fn mutate_profiles_snapshot(
    inner: &mut AppStoreInner,
    profiles: Vec<ProfileSummary>,
    selected_profile_id: Option<Uuid>,
) -> bool {
    if inner.snapshot.settings.profiles == profiles
        && inner.snapshot.settings.selected_profile_id == selected_profile_id
    {
        return false;
    }
    inner.snapshot.settings.profiles = profiles;
    inner.snapshot.settings.selected_profile_id = selected_profile_id;
    true
}

pub(super) fn mutate_history_and_selected_title_if_targeted(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    title: &str,
) -> bool {
    update_conversation_title(inner, conversation_id, title)
}

pub(super) fn mutate_history_and_selected_selection_if_targeted(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let previous_history_len = inner.snapshot.history.conversations.len();
    let previous_chat_len = inner.snapshot.chat.conversations.len();
    inner
        .snapshot
        .history
        .conversations
        .retain(|conversation| conversation.id != conversation_id);
    inner
        .snapshot
        .chat
        .conversations
        .retain(|conversation| conversation.id != conversation_id);

    if inner.snapshot.chat.selected_conversation_id == Some(conversation_id) {
        inner.snapshot.chat.selected_conversation_id = inner
            .snapshot
            .history
            .conversations
            .first()
            .map(|conversation| conversation.id);
        inner.snapshot.history.selected_conversation_id =
            inner.snapshot.chat.selected_conversation_id;
        if let Some(next_selected) = inner.snapshot.chat.selected_conversation_id {
            apply_selected_title_from_history(inner, next_selected);
        } else {
            inner.snapshot.chat.selected_conversation_title = "New Conversation".to_string();
            inner.snapshot.chat.load_state = ConversationLoadState::Idle;
            inner.snapshot.chat.transcript.clear();
            clear_streaming_ephemera_only(inner);
        }
        true
    } else {
        inner.snapshot.history.conversations.len() != previous_history_len
            || inner.snapshot.chat.conversations.len() != previous_chat_len
    }
}

fn update_conversation_title(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    title: &str,
) -> bool {
    let normalized = normalize_title(title);
    let mut changed = false;

    if let Some(conversation) = inner
        .snapshot
        .history
        .conversations
        .iter_mut()
        .find(|conversation| conversation.id == conversation_id)
    {
        if conversation.title != normalized {
            conversation.title.clone_from(&normalized);
            changed = true;
        }
    }

    if let Some(conversation) = inner
        .snapshot
        .chat
        .conversations
        .iter_mut()
        .find(|conversation| conversation.id == conversation_id)
    {
        if conversation.title != normalized {
            conversation.title.clone_from(&normalized);
            changed = true;
        }
    }

    if inner.snapshot.chat.selected_conversation_id == Some(conversation_id)
        && inner.snapshot.chat.selected_conversation_title != normalized
    {
        inner.snapshot.chat.selected_conversation_title = normalized;
        inner.title_provenance = SelectedTitleProvenance::HistoryBacked;
        changed = true;
    }

    changed
}

fn normalize_title(title: &str) -> String {
    if title.trim().is_empty() {
        "Untitled Conversation".to_string()
    } else {
        title.to_string()
    }
}
