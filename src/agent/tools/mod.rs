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
//! - `EditFile`: Apply an exact literal find-and-replace edit in an existing
//!   file, with optional line range scoping for disambiguation
//! - `ShellExec`: Execute shell commands with timeout and approval-policy
//!   checks
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

pub mod activate_skill;
pub mod edit_file;
pub mod read_file;
pub mod search;
pub mod shell_exec;
pub mod write_file;

pub use activate_skill::{get_activate_skill_tool_definition, ActivateSkillExecutor};
pub use edit_file::{get_edit_file_tool_definition, EditFileExecutor};
pub use read_file::{get_read_file_tool_definition, ReadFileExecutor};
pub use search::{get_search_tool_definition, SearchExecutor};
pub use shell_exec::{get_shell_exec_tool_definition, ShellExecExecutor};
pub use write_file::{get_write_file_tool_definition, WriteFileExecutor};
