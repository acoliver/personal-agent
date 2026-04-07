//! LLM integration using `SerdesAI`
//!
//! This module provides the bridge between `PersonalAgent`'s config/profile system
//! and the `SerdesAI` library for making LLM requests.

mod client;
pub mod client_agent;
pub mod error;
pub mod events;
pub mod mcp_tool_executor;
mod normalizing_model;
mod provider_quirks;
pub(crate) mod sse_normalize;
mod stream;
pub mod tools;

pub use client::{LlmClient, Message, Role, StreamEvent};
pub use client_agent::{AgentClientExt, McpToolContext};
pub use error::{LlmError, LlmResult};
pub use stream::send_message_stream;
pub use tools::{Tool, ToolResult, ToolUse};
