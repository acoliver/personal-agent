//! Error types for LLM integration

use thiserror::Error;

/// Errors that can occur during LLM operations
#[derive(Debug, Error)]
pub enum LlmError {
    /// Error from `SerdesAI` library
    #[error("SerdesAI error: {0}")]
    SerdesAi(String),

    /// Invalid model configuration
    #[error("Invalid model configuration: {0}")]
    InvalidConfig(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Model not supported
    #[error("Model not supported: {0}")]
    UnsupportedModel(String),

    /// Streaming error
    #[error("Streaming error: {0}")]
    Stream(String),

    /// Message conversion error
    #[error("Message conversion error: {0}")]
    MessageConversion(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for LLM operations
pub type LlmResult<T> = Result<T, LlmError>;

impl LlmError {
    /// Check if the error is recoverable
    #[must_use] 
    pub const fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Stream(_) | Self::Io(_) | Self::SerdesAi(_)
        )
    }

    /// Check if the error is due to configuration
    #[must_use] 
    pub const fn is_config_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidConfig(_) | Self::Auth(_) | Self::UnsupportedModel(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LlmError::InvalidConfig("test".to_string());
        assert!(err.to_string().contains("Invalid model configuration"));
    }

    #[test]
    fn test_is_recoverable() {
        assert!(LlmError::Stream("test".to_string()).is_recoverable());
        assert!(!LlmError::InvalidConfig("test".to_string()).is_recoverable());
    }

    #[test]
    fn test_is_config_error() {
        assert!(LlmError::InvalidConfig("test".to_string()).is_config_error());
        assert!(LlmError::Auth("test".to_string()).is_config_error());
        assert!(!LlmError::Stream("test".to_string()).is_config_error());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let llm_err: LlmError = io_err.into();
        assert!(matches!(llm_err, LlmError::Io(_)));
    }

    #[test]
    fn test_from_json_error() {
        let json_err = serde_json::from_str::<String>("invalid json").unwrap_err();
        let llm_err: LlmError = json_err.into();
        assert!(matches!(llm_err, LlmError::Json(_)));
    }
}
