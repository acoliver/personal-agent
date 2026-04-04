//! Streaming and ephemeral state helpers for the authoritative app store.
//!
//! Extracted from `app_store.rs` to reduce file length while keeping the
//! reducer dispatch in the main module.

use uuid::Uuid;

use crate::presentation::view_command::{ConversationMessagePayload, MessageRole};

use super::app_store::{
    clear_streaming_ephemera_only, non_empty_or_none, AppStoreInner, FinalizedStreamGuard,
};

pub(super) fn resolve_nil_or_explicit_target(
    inner: &AppStoreInner,
    conversation_id: Uuid,
) -> Option<Uuid> {
    if conversation_id == Uuid::nil() {
        inner
            .snapshot
            .chat
            .streaming
            .active_target
            .or(inner.snapshot.chat.selected_conversation_id)
    } else {
        Some(conversation_id)
    }
}

pub(super) fn show_thinking_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }
    let mut changed = if inner.snapshot.chat.streaming.thinking_visible {
        false
    } else {
        inner.snapshot.chat.streaming.thinking_visible = true;
        true
    };
    changed |= if inner.snapshot.chat.streaming.active_target == Some(target) {
        false
    } else {
        inner.snapshot.chat.streaming.active_target = Some(target);
        true
    };
    if inner.snapshot.chat.streaming.stream_buffer.is_empty()
        && inner.snapshot.chat.streaming.thinking_buffer.is_empty()
        && inner.snapshot.chat.streaming.last_error.is_none()
    {
        changed = true;
    }
    changed
}

pub(super) fn hide_thinking_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }
    if !inner.snapshot.chat.streaming.thinking_visible {
        return false;
    }
    inner.snapshot.chat.streaming.thinking_visible = false;
    true
}

pub(super) fn append_thinking_buffer_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    content: &str,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) || content.is_empty() {
        return false;
    }
    inner.snapshot.chat.streaming.active_target = Some(target);
    inner.snapshot.chat.streaming.thinking_visible = true;
    inner
        .snapshot
        .chat
        .streaming
        .thinking_buffer
        .push_str(content);
    true
}

pub(super) fn append_stream_buffer_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    chunk: &str,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) || chunk.is_empty() {
        return false;
    }
    inner.snapshot.chat.streaming.active_target = Some(target);
    inner.snapshot.chat.streaming.stream_buffer.push_str(chunk);
    true
}

pub(super) fn finalize_stream_if_target_matches_selected_or_nil(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }
    if inner.snapshot.chat.streaming.active_target != Some(target) {
        return false;
    }

    if inner.snapshot.chat.streaming.stream_buffer.is_empty() {
        inner.last_finalized_stream_guard = None;
    } else {
        let assistant_payload = ConversationMessagePayload {
            role: MessageRole::Assistant,
            content: inner.snapshot.chat.streaming.stream_buffer.clone(),
            thinking_content: non_empty_or_none(&inner.snapshot.chat.streaming.thinking_buffer),
            timestamp: None,
        };
        inner.snapshot.chat.transcript.push(assistant_payload);
        inner.last_finalized_stream_guard = Some(FinalizedStreamGuard {
            conversation_id: target,
            transcript_len_after_finalize: inner.snapshot.chat.transcript.len(),
        });
    }

    clear_streaming_ephemera_only(inner);
    true
}

pub(super) fn clear_streaming_ephemera_for_target(
    inner: &mut AppStoreInner,
    conversation_id: Uuid,
    error: Option<String>,
) -> bool {
    let Some(target) = resolve_nil_or_explicit_target(inner, conversation_id) else {
        return false;
    };
    clear_streaming_ephemera_if_selected_target_matches(inner, target, error)
}

fn clear_streaming_ephemera_if_selected_target_matches(
    inner: &mut AppStoreInner,
    target: Uuid,
    error: Option<String>,
) -> bool {
    if inner.snapshot.chat.selected_conversation_id != Some(target) {
        return false;
    }

    let previous = inner.snapshot.chat.streaming.clone();
    let mut next = previous.clone();
    next.active_target = None;
    next.stream_buffer.clear();
    next.thinking_buffer.clear();
    next.thinking_visible = false;
    next.last_error = error;
    if previous == next {
        return false;
    }
    inner.snapshot.chat.streaming = next;
    true
}
