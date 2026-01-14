//! Stream events for LLM responses

use serde::{Deserialize, Serialize};

/// Events emitted during streaming chat responses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    /// Text delta from the assistant
    TextDelta {
        /// The text content
        content: String,
    },

    /// Thinking/reasoning delta (if enabled and supported)
    ThinkingDelta {
        /// The thinking content
        content: String,
    },

    /// Stream completed successfully
    Complete {
        /// Total input tokens used (if available)
        input_tokens: Option<u32>,
        /// Total output tokens used (if available)
        output_tokens: Option<u32>,
    },

    /// Error occurred during streaming
    Error {
        /// Error message
        message: String,
        /// Whether the error is recoverable
        recoverable: bool,
    },
}

impl ChatStreamEvent {
    /// Create a text delta event
    #[must_use]
    pub const fn text(content: String) -> Self {
        Self::TextDelta { content }
    }

    /// Create a thinking delta event
    #[must_use]
    pub const fn thinking(content: String) -> Self {
        Self::ThinkingDelta { content }
    }

    /// Create a completion event
    #[must_use]
    pub const fn complete(input_tokens: Option<u32>, output_tokens: Option<u32>) -> Self {
        Self::Complete {
            input_tokens,
            output_tokens,
        }
    }

    /// Create an error event
    #[must_use]
    pub const fn error(message: String, recoverable: bool) -> Self {
        Self::Error {
            message,
            recoverable,
        }
    }

    /// Check if this is a text delta
    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self, Self::TextDelta { .. })
    }

    /// Check if this is a thinking delta
    #[must_use]
    pub const fn is_thinking(&self) -> bool {
        matches!(self, Self::ThinkingDelta { .. })
    }

    /// Check if this is a completion event
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    /// Check if this is an error event
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Extract text content if this is a text delta
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::TextDelta { content } => Some(content),
            _ => None,
        }
    }

    /// Extract thinking content if this is a thinking delta
    #[must_use]
    pub fn as_thinking(&self) -> Option<&str> {
        match self {
            Self::ThinkingDelta { content } => Some(content),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_delta() {
        let event = ChatStreamEvent::text("Hello".to_string());
        assert!(event.is_text());
        assert_eq!(event.as_text(), Some("Hello"));
    }

    #[test]
    fn test_thinking_delta() {
        let event = ChatStreamEvent::thinking("Thinking...".to_string());
        assert!(event.is_thinking());
        assert_eq!(event.as_thinking(), Some("Thinking..."));
    }

    #[test]
    fn test_complete() {
        let event = ChatStreamEvent::complete(Some(100), Some(50));
        assert!(event.is_complete());
    }

    #[test]
    fn test_error() {
        let event = ChatStreamEvent::error("Test error".to_string(), true);
        assert!(event.is_error());
    }

    #[test]
    fn test_serialization() {
        let event = ChatStreamEvent::text("Test".to_string());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ChatStreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_complete_serialization() {
        let event = ChatStreamEvent::complete(Some(100), Some(50));
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"complete\""));
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"output_tokens\":50"));
    }

    #[test]
    fn test_error_serialization() {
        let event = ChatStreamEvent::error("Test error".to_string(), true);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"Test error\""));
        assert!(json.contains("\"recoverable\":true"));
    }
}
