//! Native/built-in tools for the agent.
//!
//! This module provides tools that are built directly into the agent,
//! bypassing the MCP (Model Context Protocol) layer for efficient
//! local operations like file reading.
//!
//! # Available Tools
//!
//! - `ReadFile`: Read file contents with line range support, truncation,
//!   and binary file detection
//! - `WriteFile`: Create or overwrite files with automatic parent directory
//!   creation
//! - `Search`: Search file contents recursively by regex with ripgrep-first
//!   execution and built-in fallback
//!
//! # Adding New Native Tools
//!
//! To add a new native tool:
//!
//! 1. Create a new file in this directory (e.g., `my_tool.rs`)
//! 2. Define your tool executor implementing `ToolExecutor<McpToolContext>`
//! 3. Define a function that returns `ToolDefinition` for your tool
//! 4. Register the tool in `client_agent.rs` in the `register_native_tools` function

pub mod read_file;
pub mod search;
pub mod write_file;

pub use read_file::{get_read_file_tool_definition, ReadFileExecutor};
pub use search::{get_search_tool_definition, SearchExecutor};
pub use write_file::{get_write_file_tool_definition, WriteFileExecutor};
