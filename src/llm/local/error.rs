//! Error types for local LLM operations.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during local model operations.
#[derive(Debug, Error)]
pub enum LocalModelError {
    /// Model file not found at expected path.
    #[error("Local model not found at {0}")]
    ModelNotFound(PathBuf),

    /// Model file is corrupted or invalid.
    #[error("Local model file is corrupted: {0}")]
    ModelCorrupted(String),

    /// Insufficient system memory to load model.
    #[error("Insufficient memory: need {needed_gb:.1}GB, have {available_gb:.1}GB available")]
    InsufficientMemory { needed_gb: f64, available_gb: f64 },

    /// Failed to load model into memory.
    #[error("Failed to load model: {0}")]
    ModelLoadFailed(String),

    /// Failed to load model (engine error).
    #[error("Failed to load model engine: {0}")]
    LoadFailed(String),

    /// Inference failed during generation.
    #[error("Inference error: {0}")]
    InferenceError(String),

    /// Download failed.
    #[error("Download failed: {0}")]
    DownloadFailed(String),

    /// Download was cancelled by user.
    #[error("Download cancelled")]
    DownloadCancelled,

    /// Failed to verify model checksum.
    #[error("Model checksum verification failed: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    /// Cache directory could not be created or accessed.
    #[error("Cache directory error: {0}")]
    CacheDirError(String),

    /// `HuggingFace` API error.
    #[error("HuggingFace API error: {0}")]
    HuggingFaceError(String),

    /// Thread spawning failed.
    #[error("Failed to spawn inference thread: {0}")]
    ThreadSpawnFailed(String),

    /// Model is not loaded.
    #[error("Model is not loaded")]
    ModelNotLoaded,

    /// Context window exceeded.
    #[error("Context window exceeded: {tokens} tokens > {max_tokens} max")]
    ContextWindowExceeded { tokens: usize, max_tokens: usize },

    /// Unsupported model format.
    #[error("Unsupported model format: {0}")]
    UnsupportedFormat(String),

    /// IO error wrapper.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for local model operations.
pub type LocalModelResult<T> = std::result::Result<T, LocalModelError>;
