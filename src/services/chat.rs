// @plan PLAN-20250125-REFACTOR.P07
//! Chat service for handling AI message streaming

use async_trait::async_trait;
use uuid::Uuid;

use crate::events::types::ToolApprovalResponseAction;

use super::{ServiceError, ServiceResult};

/// Stream event from chat service
#[derive(Debug, Clone)]
pub enum ChatStreamEvent {
    /// Token received from model
    Token(String),
    /// Message completed
    Complete {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
    },
    /// Error occurred
    Error(ServiceError),
}

/// Chat service trait for managing AI conversations
#[async_trait]
pub trait ChatService: Send + Sync {
    /// Send a message and return a stream of events
    async fn send_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>>;

    /// Cancel a conversation's active stream. @plan PLAN-20260416-ISSUE173.P03 @requirement REQ-173-002.1
    fn cancel(&self, conversation_id: Uuid);
    /// Any stream active? @plan PLAN-20260416-ISSUE173.P03 @requirement REQ-173-001.1
    fn is_streaming(&self) -> bool;
    /// Is this conversation streaming? @plan PLAN-20260416-ISSUE173.P03 @requirement REQ-173-001.1
    fn is_streaming_for(&self, conversation_id: Uuid) -> bool;

    /// Queue a steering message for a conversation's active turn.
    ///
    /// Returns the id of the queued steering message. The in-flight
    /// generation is never cancelled on the caller's behalf.
    ///
    /// @plan PLAN-20260903-ISSUE222.P01
    /// @requirement REQ-222-004
    /// @requirement REQ-222-006
    ///
    /// # Errors
    /// Returns `ServiceError::Validation` when the conversation has no turn in
    /// `StreamLifecycle::Running`, or when the conversation's steering queue is
    /// full.
    async fn steer(&self, conversation_id: Uuid, text: String) -> ServiceResult<Uuid>;

    /// Resolve a pending tool approval request from user interaction.
    async fn resolve_tool_approval(
        &self,
        request_id: String,
        decision: ToolApprovalResponseAction,
    ) -> ServiceResult<()>;
}

// Note: ChatServiceImpl is implemented in chat_impl.rs

#[cfg(test)]
mod tests;
