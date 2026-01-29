// @plan PLAN-20250125-REFACTOR.P09
//! Conversation service for managing chat conversations
//!
//! Provides CRUD operations and management of conversation history.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::{Conversation, Message, MessageRole};
use crate::services::{ServiceError, ServiceResult};

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

    /// List all conversations, optionally filtered
    async fn list(&self, limit: Option<usize>, offset: Option<usize>) -> ServiceResult<Vec<Conversation>>;

    /// Add a user message to a conversation
    ///
    /// # Arguments
    /// * `conversation_id` - The conversation to add to
    /// * `content` - The message content
    async fn add_user_message(&self, conversation_id: Uuid, content: String) -> ServiceResult<Message>;

    /// Add an assistant message to a conversation
    ///
    /// # Arguments
    /// * `conversation_id` - The conversation to add to
    /// * `content` - The message content
    async fn add_assistant_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Message>;

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

/// @plan PLAN-20250125-REFACTOR.P09
/// Conversation service implementation stub (replaced by conversation_impl)
#[deprecated(note = "Use conversation_impl::ConversationServiceImpl instead")]
pub struct ConversationServiceImplStub;
