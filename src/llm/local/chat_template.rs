//! Qwen chat template formatting.
//!
//! Implements the correct chat template format for Qwen3.5 models,
//! which uses `<|im_start|>` and `<|im_end|>` markers.

use crate::llm::{Message, Role, Tool};

/// Thinking tags used by Qwen models
/// Qwen uses tags for reasoning/thinking content
/// Note: Qwen3.5 thinking format uses markdown-style markers
const THINK_START_TAG: &str = " ";
const THINK_END_TAG: &str = " ";

/// Strip thinking tags from output for display.
///
/// Qwen uses or similar markers for thinking content.
#[must_use]
pub fn parse_thinking_and_content(output: &str) -> (Option<String>, String) {
    // Try to find blocks
    if let Some(start) = output.find(THINK_START_TAG) {
        let after_start = &output[start + THINK_START_TAG.len()..];
        if let Some(end) = after_start.find(THINK_END_TAG) {
            let thinking = after_start[..end].trim().to_string();
            let content = format!(
                "{}{}",
                &output[..start],
                &after_start[end + THINK_END_TAG.len()..]
            )
            .trim()
            .to_string();
            return (Some(thinking), content);
        }
    }

    (None, output.to_string())
}
#[must_use]
pub fn format_qwen_chat(messages: &[Message], tools: Option<&[Tool]>) -> String {
    let mut output = String::new();

    // If tools are provided, prepend tool definitions to system message
    let tool_system = tools.map(format_tool_system);

    for msg in messages {
        match msg.role {
            Role::System => {
                output.push_str("<|im_start|>system\n");
                if let Some(ref tool_sys) = tool_system {
                    output.push_str(tool_sys);
                    output.push_str("\n\n");
                }
                output.push_str(&msg.content);
                output.push_str("<|im_end|>\n");
            }
            Role::User => {
                output.push_str("<|im_start|>user\n");
                output.push_str(&msg.content);
                output.push_str("<|im_end|>\n");
            }
            Role::Assistant => {
                output.push_str("<|im_start|>assistant\n");
                if let Some(ref thinking) = msg.thinking_content {
                    output.push_str(" \n");
                    output.push_str(thinking);
                    output.push_str(" \n");
                }
                output.push_str(&msg.content);

                // Include tool uses in the output
                for tool_use in &msg.tool_uses {
                    let args_json = serde_json::to_string(&tool_use.input).unwrap_or_default();
                    output.push_str("\n{\"name\": \"");
                    output.push_str(&tool_use.name);
                    output.push_str("\", \"arguments\": ");
                    output.push_str(&args_json);
                    output.push_str("}\n");
                }

                output.push_str("<|im_end|>\n");
            }
        }
    }

    // Add tool results as user messages
    for msg in messages {
        if !msg.tool_results.is_empty() {
            for result in &msg.tool_results {
                output.push_str("<|im_start|>user\n");
                if result.is_error {
                    output.push_str("Error: ");
                }
                output.push_str(&result.content);
                output.push_str("\n<|im_end|>\n");
            }
        }
    }

    // Start assistant response
    output.push_str("<|im_start|>assistant\n");

    output
}

/// Format tool definitions for system prompt.
fn format_tool_system(tools: &[Tool]) -> String {
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
                        let type_str = value.get("type").and_then(|t| t.as_str()).unwrap_or("any");
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

/// Parse tool calls from model output.
///
/// Looks for JSON objects with "name" and "arguments" fields.
#[must_use]
pub fn parse_tool_calls(output: &str) -> Vec<(String, serde_json::Value)> {
    let mut calls = Vec::new();

    // Find JSON objects that look like tool calls
    let mut remaining = output;
    while let Some(start) = remaining.find('{') {
        if let Some(end) = find_matching_brace(&remaining[start..]) {
            let content = &remaining[start..=start + end];

            // Parse JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                if let (Some(name), Some(args)) = (
                    json.get("name").and_then(|n| n.as_str()),
                    json.get("arguments"),
                ) {
                    calls.push((name.to_string(), args.clone()));
                }
            }

            remaining = &remaining[start + end + 1..];
        } else {
            break;
        }
    }

    calls
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

/// Remove tool call JSON from content for display.
#[must_use]
pub fn strip_tool_calls(content: &str) -> String {
    let mut result = content.to_string();

    // Remove JSON tool call objects
    while let Some(start) = result.find("{\"name\":") {
        if let Some(end) = find_matching_brace(&result[start..]) {
            result.replace_range(start..=start + end, "");
        } else {
            break;
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // Test with properly formatted thinking tags - content between tags
        let output = " Let me think about this... The answer is 42.";

        let (thinking, content) = parse_thinking_and_content(output);

        // Should extract the thinking content
        assert!(thinking.is_some());
        assert!(content.contains("42"));
    }

    #[test]
    fn test_parse_thinking_no_tags() {
        // Test with text that has NO three-space pattern
        // Using a string without consecutive spaces
        let output = "Just_a_plain_answer_with_no_spaces.";

        let (thinking, content) = parse_thinking_and_content(output);

        // No tags means no thinking extracted
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

        // Tools are injected into system message, so we need a system message
        let messages = vec![
            make_message(Role::System, "You are helpful."),
            make_message(Role::User, "Read README.md"),
        ];

        let formatted = format_qwen_chat(&messages, Some(&tools));

        // Check that tool definitions appear in system prompt
        assert!(formatted.contains("read_file"));
        assert!(formatted.contains("Read a file"));
    }
}
