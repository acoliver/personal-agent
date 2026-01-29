// @plan PLAN-20250125-REFACTOR.P07
//! Chat service for handling AI message streaming
//!
//! Provides the core interface for sending messages to AI models and managing
//! streaming responses.

use async_trait::async_trait;
use uuid::Uuid;

use super::{ServiceError, ServiceResult};

/// Stream event from chat service
#[derive(Debug, Clone)]
pub enum ChatStreamEvent {
    /// Token received from model
    Token(String),
    /// Message completed
    Complete,
    /// Error occurred
    Error(ServiceError),
}

/// Chat service trait for managing AI conversations
#[async_trait]
pub trait ChatService: Send + Sync {
    /// Send a message to the AI model
    ///
    /// # Arguments
    /// * `conversation_id` - The conversation to send the message to
    /// * `content` - The message content to send
    ///
    /// # Returns
    /// A channel/receiver that yields stream events
    async fn send_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>>;

    /// Cancel the current streaming operation
    fn cancel(&self);

    /// Check if currently streaming a response
    fn is_streaming(&self) -> bool;
}

// Note: ChatServiceImpl is implemented in chat_impl.rs
// This file only defines the trait interface

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    /// Test ChatStreamEvent variants
    #[tokio::test]
    async fn test_stream_event_token() {
        let event = ChatStreamEvent::Token("test token".to_string());
        match event {
            ChatStreamEvent::Token(content) => {
                assert_eq!(content, "test token");
            }
            _ => panic!("Wrong event type"),
        }
    }

    /// Test ChatStreamEvent Complete
    #[tokio::test]
    async fn test_stream_event_complete() {
        let event = ChatStreamEvent::Complete;
        match event {
            ChatStreamEvent::Complete => {
                // Success
            }
            _ => panic!("Wrong event type"),
        }
    }

    /// Test ChatStreamEvent Error
    #[tokio::test]
    async fn test_stream_event_error() {
        let error = ServiceError::NotFound("test error".to_string());
        let event = ChatStreamEvent::Error(error.clone());
        match event {
            ChatStreamEvent::Error(err) => {
                assert!(err.to_string().contains("test error"));
            }
            _ => panic!("Wrong event type"),
        }
    }

    /// Test ServiceError variants work correctly
    #[tokio::test]
    async fn test_service_error_not_found() {
        let error = ServiceError::NotFound("Conversation not found".to_string());
        assert!(error.to_string().contains("Not found"));
    }

    /// Test ServiceError validation
    #[tokio::test]
    async fn test_service_error_validation() {
        let error = ServiceError::Validation("Invalid input".to_string());
        assert!(error.to_string().contains("Validation error"));
    }

    /// Test ServiceResult type alias
    #[tokio::test]
    async fn test_service_result_type() {
        // Verify ServiceResult<T> works correctly
        let ok_result: ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>> =
            Ok(Box::new(futures::stream::empty()));
        assert!(ok_result.is_ok());

        let err_result: ServiceResult<Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin>> =
            Err(ServiceError::NotFound("test".to_string()));
        assert!(err_result.is_err());
    }

    /// Test that stream is properly typed
    #[tokio::test]
    async fn test_stream_type() {
        // Verify we can create a stream of the correct type
        let stream: Box<dyn futures::Stream<Item = ChatStreamEvent> + Send + Unpin> =
            Box::new(futures::stream::iter(vec![
                ChatStreamEvent::Token("test".to_string()),
                ChatStreamEvent::Complete,
            ]));

        let mut stream = stream;
        if let Some(event) = stream.next().await {
            match event {
                ChatStreamEvent::Token(_) => {
                    // Success
                }
                _ => panic!("Expected Token event"),
            }
        }
    }

    /// Test that ChatStreamEvent is Clone and Debug
    #[tokio::test]
    async fn test_stream_event_traits() {
        let event1 = ChatStreamEvent::Token("test".to_string());
        let event2 = event1.clone();

        // Verify Clone works
        assert_eq!(matches!(event1, ChatStreamEvent::Token(_)), true);
        assert_eq!(matches!(event2, ChatStreamEvent::Token(_)), true);

        // Verify Debug works (called implicitly in assertions)
        let debug_str = format!("{:?}", event1);
        assert!(debug_str.contains("Token") || debug_str.contains("test"));
    }
}
