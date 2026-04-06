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

    /// Cancel the current streaming operation
    fn cancel(&self);

    /// Check if currently streaming a response
    fn is_streaming(&self) -> bool;

    /// Resolve a pending tool approval request from user interaction.
    async fn resolve_tool_approval(
        &self,
        request_id: String,
        decision: ToolApprovalResponseAction,
    ) -> ServiceResult<()>;
}

// Note: ChatServiceImpl is implemented in chat_impl.rs

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_stream_event_token() {
        let event = ChatStreamEvent::Token("test token".to_string());
        assert!(matches!(event, ChatStreamEvent::Token(ref s) if s == "test token"));
    }

    #[tokio::test]
    async fn test_stream_event_complete() {
        assert!(matches!(
            ChatStreamEvent::Complete {
                input_tokens: None,
                output_tokens: None,
            },
            ChatStreamEvent::Complete { .. }
        ));
    }

    #[tokio::test]
    async fn test_stream_event_error() {
        let event = ChatStreamEvent::Error(ServiceError::NotFound("test error".to_string()));
        assert!(
            matches!(event, ChatStreamEvent::Error(ref e) if e.to_string().contains("test error"))
        );
    }

    #[tokio::test]
    async fn test_service_error_not_found() {
        let error = ServiceError::NotFound("Conversation not found".to_string());
        assert!(error.to_string().contains("Not found"));
    }

    #[tokio::test]
    async fn test_service_error_validation() {
        let error = ServiceError::Validation("Invalid input".to_string());
        assert!(error.to_string().contains("Validation error"));
    }

    #[tokio::test]
    async fn test_service_result_type() {
        let ok: ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>> =
            Ok(Box::new(futures::stream::empty()));
        assert!(ok.is_ok());
        let err: ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>> =
            Err(ServiceError::NotFound("test".to_string()));
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_stream_type() {
        let mut stream: Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin> =
            Box::new(futures::stream::iter(vec![
                ChatStreamEvent::Token("test".to_string()),
                ChatStreamEvent::Complete {
                    input_tokens: None,
                    output_tokens: None,
                },
            ]));
        assert!(matches!(
            stream.next().await,
            Some(ChatStreamEvent::Token(_))
        ));
    }

    #[tokio::test]
    async fn test_stream_event_traits() {
        let event1 = ChatStreamEvent::Token("test".to_string());
        let event2 = event1.clone();
        assert!(matches!(event1, ChatStreamEvent::Token(_)));
        assert!(matches!(event2, ChatStreamEvent::Token(_)));
        assert!(format!("{event1:?}").contains("Token"));
    }
}
