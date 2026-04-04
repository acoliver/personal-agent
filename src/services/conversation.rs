// @plan PLAN-20250125-REFACTOR.P09
//! Conversation service for managing chat conversations
//!
//! Provides CRUD operations and management of conversation history.

use async_trait::async_trait;
use uuid::Uuid;

use crate::models::{ContextState, Conversation, ConversationMetadata, Message, SearchResult};
use crate::services::ServiceResult;

/// Conversation service trait
#[async_trait]
pub trait ConversationService: Send + Sync {
    /// Create a new conversation
    ///
    /// # Arguments
    /// * `title` - Optional title for the conversation
    /// * `model_profile_id` - The model profile to use for this conversation
    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> ServiceResult<Conversation>;

    /// Load a conversation by ID
    async fn load(&self, id: Uuid) -> ServiceResult<Conversation>;

    /// List conversation metadata, ordered by `updated_at` DESC.
    ///
    /// Returns lightweight metadata without loading message content.
    /// `limit` defaults to 100 (max 1000). `offset` defaults to 0.
    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<ConversationMetadata>>;

    /// Add a message to a conversation.
    ///
    /// Replaces the former `add_user_message` and `add_assistant_message` methods.
    /// The caller constructs the `Message` (using `Message::user()`, `Message::assistant()`,
    /// etc.) and passes it here. The returned `Message` is the persisted form.
    async fn add_message(&self, conversation_id: Uuid, message: Message) -> ServiceResult<Message>;

    /// Full-text search across conversation titles and message content.
    ///
    /// Returns results ranked by relevance (title matches rank higher than content).
    /// `limit` defaults to 100 (max 1000). `offset` defaults to 0.
    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<SearchResult>>;

    /// Return the number of messages in a conversation.
    async fn message_count(&self, conversation_id: Uuid) -> ServiceResult<usize>;

    /// Persist context state for a conversation (e.g., summarization window).
    async fn update_context_state(&self, id: Uuid, state: &ContextState) -> ServiceResult<()>;

    /// Retrieve persisted context state, or `None` if none has been stored.
    async fn get_context_state(&self, id: Uuid) -> ServiceResult<Option<ContextState>>;

    /// Rename a conversation
    ///
    /// # Arguments
    /// * `id` - The conversation to rename
    /// * `new_title` - The new title
    async fn rename(&self, id: Uuid, new_title: String) -> ServiceResult<()>;

    /// Delete a conversation
    async fn delete(&self, id: Uuid) -> ServiceResult<()>;

    /// Set the active conversation
    async fn set_active(&self, id: Uuid) -> ServiceResult<()>;

    /// Get the currently active conversation ID
    async fn get_active(&self) -> ServiceResult<Option<Uuid>>;

    /// Get message history for a conversation
    async fn get_messages(&self, conversation_id: Uuid) -> ServiceResult<Vec<Message>>;

    /// Update conversation metadata
    async fn update(
        &self,
        id: Uuid,
        title: Option<String>,
        model_profile_id: Option<Uuid>,
    ) -> ServiceResult<Conversation>;
}
