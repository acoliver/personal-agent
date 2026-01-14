//! LLM integration module
//!
//! This module provides integration with the `SerdesAI` library for LLM interactions.

mod client;
mod error;
mod events;
mod stream;

pub use client::LLMClient;
pub use error::{LlmError, LlmResult};
pub use events::ChatStreamEvent;
pub use stream::send_message_stream;
