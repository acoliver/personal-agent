//! Error types for LLM integration

use std::fmt::Debug;
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

    /// Failed to read API key file
    #[error("Failed to read keyfile {path}: {source}")]
    KeyfileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// No API key configured
    #[error("No API key configured for profile")]
    NoApiKey,

    /// Local model error
    #[error("Local model error: {0}")]
    LocalModel(String),

    /// Local model not downloaded
    #[error("Local model not downloaded")]
    LocalModelNotDownloaded,

    /// Insufficient memory for local model
    #[error("Insufficient memory: need {needed_gb:.1}GB, have {available_gb:.1}GB")]
    InsufficientMemory { needed_gb: f64, available_gb: f64 },
}

/// Result type for LLM operations
pub type LlmResult<T> = Result<T, LlmError>;

/// Format streaming and transport errors with their full debug context.
#[must_use]
pub(crate) fn debug_error_message<E: Debug>(error: &E) -> String {
    format!("{error:?}")
}

impl LlmError {
    /// Check if the error is recoverable
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, Self::Stream(_) | Self::Io(_) | Self::SerdesAi(_))
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
    use super::debug_error_message;

    #[test]
    fn debug_error_message_preserves_debug_context() {
        let message = debug_error_message(&std::io::Error::other("network down"));

        assert!(message.contains("network down"));
        assert!(message.contains("Custom"));
    }
}
