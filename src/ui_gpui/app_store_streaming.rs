//! Streaming and ephemeral state helpers for the authoritative app store.
//!
//! Extracted from `app_store.rs` to reduce file length while keeping the
//! reducer dispatch in the main module.

use uuid::Uuid;

use crate::presentation::view_command::{ConversationMessagePayload, MessageRole};

use super::app_store::{clear_streaming_ephemera_only, AppStoreInner, FinalizedStreamGuard};

fn non_empty_or_none(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn streaming_state_mut(
    inner: &mut AppStoreInner,
    target: Uuid,
) -> &mut super::app_store_types::ConversationStreamingState {
    inner.streaming_states.entry(target).or_default()
}

fn remove_empty_state_for_target(inner: &mut AppStoreInner, target: Uuid) {
    let should_remove = inner.streaming_states.get(&target).is_some_and(|state| {
        !state.thinking_visible
            && state.stream_buffer.is_empty()
            && state.thinking_buffer.is_empty()
            && state.last_error.is_none()
            && state.model_id.is_none()
    });
    if should_remove {
        inner.streaming_states.remove(&target);
    }
}

/// @plan PLAN-20260416-ISSUE173.P09
/// @requirement REQ-173-004.3
pub(super) fn resolve_nil_or_explicit_target(
    inner: &AppStoreInner,
    conversation_id: Uuid,
) -> Option<Uuid> {
    if conversation_id == Uuid::nil() {
        tracing::warn!(
            "Received nil conversation_id for streaming event; falling back to active/selected target"
        );
        // Prefer the selected conversation if it is currently active; otherwise any
        // one from the set (arbitrary but stable is unimportant — this is a legacy
        // fallback for nil ids).
        let selected = inner.snapshot.chat.selected_conversation_id;
        selected
            .filter(|id| inner.active_streaming_targets.contains(id))
            .or_else(|| inner.active_streaming_targets.iter().copied().next())
            .or(selected)
    } else {
        Some(conversation_id)
    }
}

/// @plan PLAN-20260416-ISSUE173.P09
/// @requirement REQ-173-004.1
pub(super) fn show_thinking_for_target(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    model_id: String,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };

    let state = streaming_state_mut(inner, target);
    let changed = !state.thinking_visible || state.model_id.as_deref() != Some(model_id.as_str());
    state.thinking_visible = true;
    state.model_id = Some(model_id);

    if changed {
        inner.active_streaming_targets.insert(target);
    }

    changed
}

pub(super) fn hide_thinking_for_target(inner: &mut AppStoreInner, conversation_id: Uuid) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };

    let Some(state) = inner.streaming_states.get_mut(&target) else {
        return false;
    };

    if !state.thinking_visible {
        return false;
    }

    state.thinking_visible = false;
    remove_empty_state_for_target(inner, target);
    true
}

/// @plan PLAN-20260416-ISSUE173.P09
/// @requirement REQ-173-004.1
pub(super) fn append_thinking_buffer_for_target(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    content: &str,
) -> bool {
    if content.is_empty() {
        return false;
    }

    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };

    let state = streaming_state_mut(inner, target);
    state.thinking_visible = true;
    state.thinking_buffer.push_str(content);
    inner.active_streaming_targets.insert(target);
    true
}

/// @plan PLAN-20260416-ISSUE173.P09
/// @requirement REQ-173-004.1
pub(super) fn append_stream_buffer_for_target(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    chunk: &str,
) -> bool {
    if chunk.is_empty() {
        return false;
    }

    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };

    let state = streaming_state_mut(inner, target);
    state.stream_buffer.push_str(chunk);
    inner.active_streaming_targets.insert(target);
    true
}

/// @plan PLAN-20260416-ISSUE173.P09
/// @requirement REQ-173-004.1
pub(super) fn finalize_stream_for_target(inner: &mut AppStoreInner, conversation_id: Uuid) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };

    let Some(state) = inner.streaming_states.get(&target).cloned() else {
        return false;
    };

    if inner.snapshot.chat.selected_conversation_id == Some(target)
        && !state.stream_buffer.is_empty()
    {
        let assistant_payload = ConversationMessagePayload {
            role: MessageRole::Assistant,
            content: state.stream_buffer.clone(),
            thinking_content: non_empty_or_none(&state.thinking_buffer),
            timestamp: None,
            model_id: state.model_id.clone(),
        };
        inner.snapshot.chat.transcript.push(assistant_payload);
        inner.finalized_stream_guards.insert(
            target,
            FinalizedStreamGuard {
                conversation_id: target,
                transcript_len_after_finalize: inner.snapshot.chat.transcript.len(),
            },
        );
    } else {
        inner.finalized_stream_guards.remove(&target);
    }

    inner.active_streaming_targets.remove(&target);
    inner.streaming_states.remove(&target);
    clear_streaming_ephemera_only(inner);
    true
}

/// @plan PLAN-20260416-ISSUE173.P09
/// @requirement REQ-173-004.1
pub(super) fn clear_streaming_ephemera_for_target(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    error: Option<String>,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };

    let has_state = inner.streaming_states.contains_key(&target);
    let was_active = inner.active_streaming_targets.contains(&target);
    if !has_state && !was_active {
        return false;
    }

    if let Some(error_message) = error {
        let state = streaming_state_mut(inner, target);
        state.thinking_visible = false;
        state.stream_buffer.clear();
        state.thinking_buffer.clear();
        state.last_error = Some(error_message);
    } else {
        inner.streaming_states.remove(&target);
    }
    inner.active_streaming_targets.remove(&target);

    true
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_concurrent;
