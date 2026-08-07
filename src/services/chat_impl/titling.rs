//! Auto-naming a conversation from its first prompt.
//!
//! Runs alongside the chat stream: the user's first message decides the conversation
//! title, and a failure here must never affect the answer the user is waiting for.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::models::{Conversation, MessageRole, ModelProfile};
use crate::presentation::view_command::ViewCommand;
use crate::services::conversation_title::{
    is_placeholder_title, sanitize_generated_title, ConversationTitleGenerator,
};
use crate::services::{ConversationService, ServiceError};

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
    /// Build a request when `conversation` still carries a placeholder title.
    ///
    /// Returns `None` once the conversation has a real title, so a title the user chose
    /// — or an earlier generated one — is not replaced. A conversation whose first
    /// attempt failed (offline, bad key, unusable answer) is still a placeholder, so the
    /// next send tries again rather than leaving it named "New Conversation" forever.
    ///
    /// The prompt is always the conversation's first user message, so a retry names the
    /// conversation after what it is actually about rather than after a follow-up.
    pub(super) fn for_untitled_conversation(
        conversation: &Conversation,
        profile: &ModelProfile,
    ) -> Option<Self> {
        if !is_placeholder_title(conversation.title.as_deref()) {
            return None;
        }

        let first_prompt = conversation
            .messages
            .iter()
            .find(|message| message.role == MessageRole::User)?
            .content
            .clone();
        if first_prompt.trim().is_empty() {
            return None;
        }

        Some(Self {
            profile: profile.clone(),
            first_prompt,
        })
    }
}

/// Generate a title for `conversation_id`, persist it, and tell the UI.
///
/// Every failure mode — generation error, timeout, unusable answer, storage error —
/// leaves the existing placeholder title untouched and is logged rather than surfaced.
pub(super) async fn generate_and_apply_title(
    generator: Arc<dyn ConversationTitleGenerator>,
    conversation_service: Arc<dyn ConversationService>,
    view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    conversation_id: Uuid,
    request: TitleGenerationRequest,
    cancel: CancellationToken,
) {
    let Some(title) = propose_title(&generator, conversation_id, &request, &cancel).await else {
        return;
    };

    // The user can rename the conversation during the seconds the request is in flight.
    // Re-read the stored title so a deliberate rename made in that window wins. This is
    // a read-then-write, not a compare-and-swap: `ConversationService::rename` is an
    // unconditional update, so a rename landing between this read and the write below
    // would still be overwritten. Closing that remaining sub-millisecond gap would mean
    // a conditional-rename operation across the service trait and every implementation,
    // which is not worth it for a title the user can simply set again.
    match conversation_service.load(conversation_id).await {
        Ok(conversation) if !is_placeholder_title(conversation.title.as_deref()) => {
            tracing::debug!(
                conversation_id = %conversation_id,
                "Conversation was titled while generating; discarding generated title"
            );
            return;
        }
        Ok(_) => {}
        Err(ServiceError::NotFound(_)) => {
            tracing::debug!(
                conversation_id = %conversation_id,
                "Conversation was deleted while generating its title"
            );
            return;
        }
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

    // Delivered on the view channel rather than the global event bus: the bus is a
    // 16-slot broadcast that also carries one message per streamed token, and both
    // presenters treat lag as normal and drop what they missed. A one-shot title update
    // cannot survive that, so it goes down the same reliable queue the service already
    // uses for its other UI updates.
    let _ = view_tx
        .send(ViewCommand::ConversationTitleUpdated {
            id: conversation_id,
            title,
        })
        .await;
}

/// Ask the model for a title, bounded by the timeout and by `cancel`.
///
/// Returns the sanitized title, or `None` when there is nothing usable to apply.
async fn propose_title(
    generator: &Arc<dyn ConversationTitleGenerator>,
    conversation_id: Uuid,
    request: &TitleGenerationRequest,
    cancel: &CancellationToken,
) -> Option<String> {
    let proposal = generator.propose_title(&request.profile, &request.first_prompt);

    let proposal = tokio::select! {
        biased;
        () = cancel.cancelled() => {
            tracing::debug!(
                conversation_id = %conversation_id,
                "Chat turn cancelled; abandoning conversation title generation"
            );
            return None;
        }
        result = tokio::time::timeout(TITLE_GENERATION_TIMEOUT, proposal) => result,
    };

    let proposal = match proposal {
        Ok(Ok(proposal)) => proposal,
        Ok(Err(error)) => {
            tracing::info!(
                conversation_id = %conversation_id,
                error = %error,
                "Conversation title generation failed; keeping placeholder title"
            );
            return None;
        }
        Err(_) => {
            tracing::info!(
                conversation_id = %conversation_id,
                timeout_secs = TITLE_GENERATION_TIMEOUT.as_secs(),
                "Conversation title generation timed out; keeping placeholder title"
            );
            return None;
        }
    };

    let title = sanitize_generated_title(&proposal);
    if title.is_none() {
        tracing::info!(
            conversation_id = %conversation_id,
            "Model returned no usable conversation title; keeping placeholder title"
        );
    }
    title
}
