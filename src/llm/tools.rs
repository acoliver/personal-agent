//! Tool support for LLM interactions
//!
//! This module defines types for tool use interactions between the LLM and external tools,
//! particularly MCP tools.

use serde::{Deserialize, Serialize};

/// A tool use request from the LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_use_creation() {
        let tool_use = ToolUse::new(
            "toolu_123",
            "get_weather",
            serde_json::json!({"location": "NYC"}),
        );

        assert_eq!(tool_use.id, "toolu_123");
        assert_eq!(tool_use.name, "get_weather");
        assert_eq!(tool_use.input["location"], "NYC");
    }

    #[test]
    fn test_tool_result_success() {
        let result = ToolResult::success("toolu_123", "Temperature: 72°F");

        assert_eq!(result.tool_use_id, "toolu_123");
        assert_eq!(result.content, "Temperature: 72°F");
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("toolu_123", "Location not found");

        assert_eq!(result.tool_use_id, "toolu_123");
        assert_eq!(result.content, "Location not found");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_definition() {
        let tool = Tool::new(
            "search",
            "Search the web",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
        );

        assert_eq!(tool.name, "search");
        assert_eq!(tool.description, "Search the web");
        assert!(tool.input_schema.is_object());
    }

    #[test]
    fn test_tool_use_serde_roundtrip() {
        let tool_use = ToolUse::new(
            "toolu_456",
            "calculate",
            serde_json::json!({"x": 10, "y": 20}),
        );

        let json = serde_json::to_string(&tool_use).unwrap();
        let deserialized: ToolUse = serde_json::from_str(&json).unwrap();

        assert_eq!(tool_use, deserialized);
    }

    #[test]
    fn test_tool_result_serde_roundtrip() {
        let result = ToolResult::success("toolu_789", "Result: 42");

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_tool_serde_roundtrip() {
        let tool = Tool::new(
            "add",
            "Add two numbers",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "a": {"type": "number"},
                    "b": {"type": "number"}
                }
            }),
        );

        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&json).unwrap();

        assert_eq!(tool.name, deserialized.name);
        assert_eq!(tool.description, deserialized.description);
    }
}
