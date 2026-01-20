//! Tool support for LLM interactions
//!
//! This module defines types for tool use interactions between the LLM and external tools,
//! particularly MCP tools.

use serde::{Deserialize, Serialize};

/// A tool use request from the LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolUse {
    /// Unique identifier for this tool use
    pub id: String,
    /// Name of the tool being called
    pub name: String,
    /// Input arguments for the tool (JSON)
    pub input: serde_json::Value,
}

impl ToolUse {
    /// Create a new tool use request
    pub fn new(id: impl Into<String>, name: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            input,
        }
    }
}

/// Result from executing a tool
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    /// ID of the tool use this is responding to
    pub tool_use_id: String,
    /// Content returned from the tool (typically JSON or text)
    pub content: String,
    /// Whether this result represents an error
    #[serde(default)]
    pub is_error: bool,
}

impl ToolResult {
    /// Create a new successful tool result
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    /// Create a new error tool result
    pub fn error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: error.into(),
            is_error: true,
        }
    }
}

/// A tool definition for the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Name of the tool
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// JSON Schema for the tool's input parameters
    pub input_schema: serde_json::Value,
}

impl Tool {
    /// Create a new tool definition
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}
