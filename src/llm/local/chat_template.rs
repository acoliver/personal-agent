//! Qwen chat template formatting.
//!
//! Implements the correct chat template format for Qwen3.5 models,
//! which uses `<|im_start|>` and `<|im_end|>` markers.

use crate::llm::Tool;

/// Parsed tool call from model output.
#[derive(Debug, Clone)]
pub struct ParsedToolCall {
    /// Tool name.
    pub name: String,
    /// Tool arguments as JSON.
    pub arguments: serde_json::Value,
}

/// Qwen chat template formatter.
///
/// Handles formatting of messages and parsing of responses for Qwen models.
#[derive(Clone)]
pub struct QwenChatTemplate {
    /// Start marker for messages.
    start_marker: &'static str,
    /// End marker for messages.
    end_marker: &'static str,
    /// Thinking start tag.
    think_start: &'static str,
    /// Thinking end tag.
    think_end: &'static str,
}

impl Default for QwenChatTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl QwenChatTemplate {
    /// Create a new Qwen chat template formatter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            start_marker: "<|im_start|>",
            end_marker: "<|im_end|>",
            think_start: " ",
            think_end: " ",
        }
    }

    /// Format messages into a prompt string.
    ///
    /// # Arguments
    ///
    /// * `messages` - List of (role, content) tuples
    ///
    /// # Returns
    ///
    /// Formatted prompt string ready for the model.
    #[must_use]
    pub fn format_messages(&self, messages: &[(&str, &str)]) -> String {
        let mut output = String::new();

        for (role, content) in messages {
            output.push_str(self.start_marker);
            output.push_str(role);
            output.push('\n');
            output.push_str(content);
            output.push_str(self.end_marker);
            output.push('\n');
        }

        // Start assistant response
        output.push_str(self.start_marker);
        output.push_str("assistant\n");

        output
    }

    /// Format tool definitions for inclusion in system prompt.
    ///
    /// # Arguments
    ///
    /// * `tools` - List of available tools
    ///
    /// # Returns
    ///
    /// Formatted tool descriptions.
    #[must_use]
    pub fn format_tools(tools: &[Tool]) -> String {
        let mut output = String::new();
        output.push_str("You have access to the following tools:\n\n");

        for tool in tools {
            output.push_str("### ");
            output.push_str(&tool.name);
            output.push('\n');
            output.push_str(&tool.description);
            output.push_str("\n\nParameters:\n");

            if let Some(schema) = tool.input_schema.as_object() {
                if let Some(properties) = schema.get("properties") {
                    if let Some(props) = properties.as_object() {
                        for (name, value) in props {
                            let type_str =
                                value.get("type").and_then(|t| t.as_str()).unwrap_or("any");
                            let desc = value
                                .get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            output.push_str("- ");
                            output.push_str(name);
                            output.push_str(" (");
                            output.push_str(type_str);
                            output.push_str("): ");
                            output.push_str(desc);
                            output.push('\n');
                        }
                    }
                }
            }

            output.push('\n');
        }

        output.push_str("To use a tool, respond with:\n");
        output.push_str("{\"name\": \"tool_name\", \"arguments\": {\"arg\": \"value\"}}\n");

        output
    }

    /// Parse response to extract thinking, content, and tool calls.
    ///
    /// # Arguments
    ///
    /// * `output` - Raw model output
    ///
    /// # Returns
    ///
    /// Tuple of (thinking, content, `ParsedToolCall` list).
    #[must_use]
    #[allow(dead_code)]
    pub fn parse_response(&self, output: &str) -> (Option<String>, String, Vec<ParsedToolCall>) {
        let (thinking, content) = self.parse_thinking(output);
        let tool_calls = Self::parse_tool_calls(output);
        let content = Self::strip_tool_calls(&content);

        (thinking, content, tool_calls)
    }

    /// Parse thinking content from output.
    fn parse_thinking(&self, output: &str) -> (Option<String>, String) {
        // Look for thinking tags
        let start_tag = self.think_start;
        let end_tag = self.think_end;

        if let Some(start) = output.find(start_tag) {
            let after_start = &output[start + start_tag.len()..];
            if let Some(end) = after_start.find(end_tag) {
                let thinking = after_start[..end].trim().to_string();
                let content = format!(
                    "{}{}",
                    &output[..start],
                    &after_start[end + end_tag.len()..]
                )
                .trim()
                .to_string();
                return (Some(thinking), content);
            }
        }

        (None, output.to_string())
    }

    /// Parse tool calls from output.
    fn parse_tool_calls(output: &str) -> Vec<ParsedToolCall> {
        let mut calls = Vec::new();

        // Find JSON objects that look like tool calls
        let mut remaining = output;
        while let Some(start) = remaining.find('{') {
            if let Some(end) = Self::find_matching_brace(&remaining[start..]) {
                let content = &remaining[start..=start + end];

                // Parse JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                    if let (Some(name), Some(args)) = (
                        json.get("name").and_then(|n| n.as_str()),
                        json.get("arguments"),
                    ) {
                        calls.push(ParsedToolCall {
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                    }
                }

                remaining = &remaining[start + end + 1..];
            } else {
                break;
            }
        }

        calls
    }

    /// Remove tool call JSON from content.
    fn strip_tool_calls(content: &str) -> String {
        let mut result = content.to_string();

        // Remove JSON tool call objects
        while let Some(start) = result.find("{\"name\":") {
            if let Some(end) = Self::find_matching_brace(&result[start..]) {
                result.replace_range(start..=start + end, "");
            } else {
                break;
            }
        }

        result.trim().to_string()
    }

    /// Find the index of the matching closing brace.
    fn find_matching_brace(s: &str) -> Option<usize> {
        let mut depth = 0;
        for (i, c) in s.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }
}

// Legacy functions for backward compatibility

/// Strip thinking tags from output for display.
#[must_use]
pub fn parse_thinking_and_content(output: &str) -> (Option<String>, String) {
    let template = QwenChatTemplate::new();
    template.parse_thinking(output)
}

/// Format messages using Qwen chat template.
#[must_use]
pub fn format_qwen_chat(messages: &[crate::llm::Message], tools: Option<&[Tool]>) -> String {
    let template = QwenChatTemplate::new();

    // Convert messages to (role, content) tuples
    let mut tuples: Vec<(&str, &str)> = Vec::new();

    for msg in messages {
        let role = match msg.role {
            crate::llm::Role::System => "system",
            crate::llm::Role::User => "user",
            crate::llm::Role::Assistant => "assistant",
        };

        // Handle system message with tools
        if msg.role == crate::llm::Role::System {
            if let Some(tools_ref) = tools {
                let tool_system = QwenChatTemplate::format_tools(tools_ref);
                let enhanced = format!("{}\n\n{}", msg.content, tool_system);
                tuples.push(("system", Box::leak(enhanced.into_boxed_str()) as &str));
            } else {
                tuples.push((role, &msg.content));
            }
        } else {
            tuples.push((role, &msg.content));
        }
    }

    template.format_messages(&tuples)
}

/// Parse tool calls from model output.
#[must_use]
pub fn parse_tool_calls(output: &str) -> Vec<(String, serde_json::Value)> {
    QwenChatTemplate::parse_tool_calls(output)
        .into_iter()
        .map(|tc| (tc.name, tc.arguments))
        .collect()
}

/// Remove tool call JSON from content for display.
#[must_use]
pub fn strip_tool_calls(content: &str) -> String {
    QwenChatTemplate::strip_tool_calls(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{Message, Role};
    use serde_json::json;

    fn make_message(role: Role, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            thinking_content: None,
            tool_uses: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    #[test]
    fn test_format_simple_chat() {
        let messages = vec![
            make_message(Role::System, "You are helpful."),
            make_message(Role::User, "Hello!"),
            make_message(Role::Assistant, "Hi there!"),
        ];

        let formatted = format_qwen_chat(&messages, None);

        assert!(formatted.contains("<|im_start|>system"));
        assert!(formatted.contains("<|im_start|>user"));
        assert!(formatted.contains("<|im_start|>assistant"));
        assert!(formatted.contains("<|im_end|>"));
    }

    #[test]
    fn test_parse_tool_calls() {
        let output = r#"Let me check that.
{"name": "read_file", "arguments": {"path": "README.md"}}
That's what I found."#;

        let calls = parse_tool_calls(output);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "read_file");
        assert_eq!(calls[0].1["path"], "README.md");
    }

    #[test]
    fn test_parse_multiple_tool_calls() {
        let output = r#"{"name": "read_file", "arguments": {"path": "a.txt"}}
{"name": "read_file", "arguments": {"path": "b.txt"}}"#;

        let calls = parse_tool_calls(output);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn test_parse_thinking() {
        // Test with properly formatted thinking tags
        let output = " Let me think about this... The answer is 42.";

        let (thinking, content) = parse_thinking_and_content(output);

        assert!(thinking.is_some());
        assert!(content.contains("42"));
    }

    #[test]
    fn test_parse_thinking_no_tags() {
        let output = "Just_a_plain_answer_with_no_spaces.";

        let (thinking, content) = parse_thinking_and_content(output);

        assert_eq!(thinking, None);
        assert_eq!(content, "Just_a_plain_answer_with_no_spaces.");
    }

    #[test]
    fn test_strip_tool_calls() {
        let content = "Here's what I found.\n{\"name\": \"test\", \"arguments\": {}}\nMore text.";
        let stripped = strip_tool_calls(content);
        assert_eq!(stripped, "Here's what I found.\n\nMore text.");
    }

    #[test]
    fn test_format_with_tools() {
        let tools = vec![Tool {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path"}
                }
            }),
        }];

        let messages = vec![
            make_message(Role::System, "You are helpful."),
            make_message(Role::User, "Read README.md"),
        ];

        let formatted = format_qwen_chat(&messages, Some(&tools));

        assert!(formatted.contains("read_file"));
        assert!(formatted.contains("Read a file"));
    }

    #[test]
    fn test_template_format_messages() {
        let template = QwenChatTemplate::new();
        let messages = vec![("system", "You are helpful."), ("user", "Hello!")];

        let formatted = template.format_messages(&messages);

        assert!(formatted.starts_with("<|im_start|>system"));
        assert!(formatted.contains("<|im_start|>user"));
        assert!(formatted.ends_with("<|im_start|>assistant\n"));
    }
}
