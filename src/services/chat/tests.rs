//! Trait-level tests for the chat service contract.
//!
//! Split out of `chat.rs` so the trait declaration is the whole of that
//! file, which keeps the structural length check reading the trait rather
//! than the trait plus its tests.

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
    assert!(matches!(event, ChatStreamEvent::Error(ref e) if e.to_string().contains("test error")));
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
