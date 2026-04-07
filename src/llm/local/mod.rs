//! Local LLM inference module.
//!
//! Provides in-process LLM inference using the llama-gguf library.
//! This module implements hardware detection, model download,
//! and inference for running Qwen3.5-4B locally.
//!
//! # Architecture
//!
//! - `hardware` - Detects system RAM, CPU, and GPU capabilities
//! - `model_manager` - Downloads models from `HuggingFace`, manages cache
//! - `engine` - Thread-based inference wrapper
//! - `provider` - Integrates with `LlmClient` for seamless switching
//! - `chat_template` - Qwen-specific prompt formatting
//! - `capabilities` - Model feature detection
//! - `error` - Error types for local model operations
//!
//! # Usage
//!
//! ```ignore
//! use crate::llm::local::{LocalProvider, HardwareCapabilities};
//!
//! // Check if local inference is possible
//! let hw = HardwareCapabilities::detect();
//! if hw.can_run_local_model() {
//! let provider = LocalProvider::with_default_model()?;
//! let response = provider.request(&messages, &tools).await?;
//! }
//! ```

mod capabilities;
mod chat_template;
mod engine;
pub mod error;
mod hardware;
mod model_manager;
mod provider;

pub use capabilities::{ModelCapabilities, ToolReliability};
pub use chat_template::{
    format_qwen_chat, parse_thinking_and_content, parse_tool_calls, strip_tool_calls,
};
pub use engine::{InferenceEvent, InferenceRequest, LocalEngine};
pub use error::{LocalModelError, LocalModelResult};
pub use hardware::{HardwareCapabilities, HardwareStatus};
pub use model_manager::{
    format_bytes, LocalModelManager, DEFAULT_MODEL_DISPLAY_NAME, DEFAULT_MODEL_FILE,
    DEFAULT_MODEL_REPO, DEFAULT_MODEL_SIZE_BYTES,
};
pub use provider::{is_local_available, is_model_downloaded, LocalProvider};
