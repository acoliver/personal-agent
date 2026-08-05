//! Auto-naming a conversation from its first prompt.
//!
//! Runs alongside the chat stream: the user's first message decides the conversation
//! title, and a failure here must never affect the answer the user is waiting for.

use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::events::types::ConversationEvent;
use crate::events::{emit, AppEvent};
use crate::models::{Conversation, MessageRole, ModelProfile};
use crate::services::conversation_title::{
    is_placeholder_title, sanitize_generated_title, ConversationTitleGenerator,
};
use crate::services::ConversationService;

/// Upper bound on how long the title request may take.
///
/// Auto-naming is a convenience; a provider that stalls should simply leave the
/// placeholder in place instead of holding a task open behind the user's back.
const TITLE_GENERATION_TIMEOUT: Duration = Duration::from_secs(30);

/// Everything the background titling task needs, captured while the conversation is
/// already loaded on the send path.
pub(super) struct TitleGenerationRequest {
    profile: ModelProfile,
    first_prompt: String,
}

impl TitleGenerationRequest {
    /// Build a request when `conversation` is an untitled conversation whose only user
    /// message is the prompt that was just sent.
    ///
    /// Returns `None` for conversations that already have a real title or that already
    /// contain earlier user turns, so a conversation is auto-named at most once and a
    /// user-chosen title is never replaced.
    pub(super) fn for_first_prompt(
        conversation: &Conversation,
        profile: &ModelProfile,
    ) -> Option<Self> {
        if !is_placeholder_title(conversation.title.as_deref()) {
            return None;
        }

        let mut user_messages = conversation
            .messages
            .iter()
            .filter(|message| message.role == MessageRole::User);

        let first_prompt = user_messages.next()?.content.clone();
        if user_messages.next().is_some() {
            return None;
        }
        if first_prompt.trim().is_empty() {
            return None;
        }

        Some(Self {
            profile: profile.clone(),
            first_prompt,
        })
    }
}

/// Generate a title for `conversation_id` and persist it.
///
/// Every failure mode — generation error, timeout, storage error — leaves the existing
/// placeholder title untouched and is logged rather than surfaced.
pub(super) async fn generate_and_apply_title(
    generator: Arc<dyn ConversationTitleGenerator>,
    conversation_service: Arc<dyn ConversationService>,
    conversation_id: Uuid,
    request: TitleGenerationRequest,
) {
    let proposal = generator.propose_title(&request.profile, &request.first_prompt);

    let proposal = match tokio::time::timeout(TITLE_GENERATION_TIMEOUT, proposal).await {
        Ok(Ok(proposal)) => proposal,
        Ok(Err(error)) => {
            tracing::info!(
                conversation_id = %conversation_id,
                error = %error,
                "Conversation title generation failed; keeping placeholder title"
            );
            return;
        }
        Err(_) => {
            tracing::info!(
                conversation_id = %conversation_id,
                timeout_secs = TITLE_GENERATION_TIMEOUT.as_secs(),
                "Conversation title generation timed out; keeping placeholder title"
            );
            return;
        }
    };

    let Some(title) = sanitize_generated_title(&proposal) else {
        tracing::info!(
            conversation_id = %conversation_id,
            "Model returned no usable conversation title; keeping placeholder title"
        );
        return;
    };

    // The user can rename the conversation while the request is in flight. Re-read the
    // stored title so an auto-generated name never overwrites a deliberate one.
    match conversation_service.load(conversation_id).await {
        Ok(conversation) if !is_placeholder_title(conversation.title.as_deref()) => {
            tracing::debug!(
                conversation_id = %conversation_id,
                "Conversation was titled while generating; discarding generated title"
            );
            return;
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(
                conversation_id = %conversation_id,
                error = %error,
                "Failed to re-read conversation before applying generated title"
            );
            return;
        }
    }

    if let Err(error) = conversation_service
        .rename(conversation_id, title.clone())
        .await
    {
        tracing::warn!(
            conversation_id = %conversation_id,
            error = %error,
            "Failed to persist generated conversation title"
        );
        return;
    }

    let _ = emit(AppEvent::Conversation(ConversationEvent::TitleUpdated {
        id: conversation_id,
        title,
    }));
}
