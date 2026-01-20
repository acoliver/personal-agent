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
