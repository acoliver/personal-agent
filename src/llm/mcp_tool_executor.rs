//! MCP Tool Executor implementation.
//!
//! Provides the bridge between Agent tools and MCP (Model Context Protocol) tools,
//! including approval context construction for rich UI display.

use crate::agent::tool_approval_policy::ToolApprovalDecision;
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::{ToolApprovalContext, ToolCategory, ViewCommand};
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolError, ToolReturn};

/// Executor that bridges Agent tools to MCP.
pub struct McpToolExecutor {
    tool_name: String,
}

impl McpToolExecutor {
    /// Create a new MCP tool executor for the given tool name.
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for McpToolExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let provider = {
            let service_arc = crate::mcp::McpService::global();
            let provider = service_arc
                .lock()
                .await
                .find_tool_provider_metadata(&self.tool_name)
                .ok_or_else(|| {
                    ToolError::execution_failed(format!(
                        "No MCP provider found for tool {}",
                        self.tool_name
                    ))
                })?;
            provider
        };

        let (tool_identifier, decision) = {
            let policy = ctx.deps().policy.lock().await;
            let tool_identifier = policy.mcp_tool_identifier(&provider.mcp_name, &self.tool_name);
            let decision = policy.evaluate(&tool_identifier);
            drop(policy);
            (tool_identifier, decision)
        };

        match decision {
            ToolApprovalDecision::Allow => {}
            ToolApprovalDecision::Deny => {
                return Err(ToolError::execution_failed(
                    "Tool execution denied by policy",
                ));
            }
            ToolApprovalDecision::AskUser => {
                handle_mcp_approval(
                    ctx,
                    &self.tool_name,
                    &provider.mcp_name,
                    &tool_identifier,
                    &args,
                )
                .await?;
            }
        }

        // Get the global MCP service and call the tool
        let service_arc = crate::mcp::McpService::global();
        let result = service_arc
            .lock()
            .await
            .call_tool(&self.tool_name, args.clone())
            .await
            .map_err(|e| {
                ToolError::execution_failed(format!("MCP tool {} failed: {}", self.tool_name, e))
            })?;

        // Convert the JSON result to a ToolReturn
        Ok(ToolReturn::text(result.to_string()))
    }
}

/// Handle approval request for MCP tool execution.
async fn handle_mcp_approval(
    ctx: &RunContext<McpToolContext>,
    tool_name: &str,
    mcp_name: &str,
    tool_identifier: &str,
    args: &serde_json::Value,
) -> Result<(), ToolError> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let waiter = ctx
        .deps()
        .approval_gate
        .wait_for_approval(request_id.clone(), tool_identifier.to_string());

    // Build rich context for MCP tool approval
    let mut context = ToolApprovalContext::new(
        tool_name,
        ToolCategory::Mcp,
        extract_primary_target(args, tool_name),
    )
    .with_server_name(mcp_name);

    // Flatten JSON args into details
    if let Some(obj) = args.as_object() {
        for (key, value) in obj.iter().take(5) {
            // Limit to first 5 keys
            let value_str = truncate_value(value, 50);
            context = context.with_detail(key.clone(), value_str);
        }
    }

    if ctx
        .deps()
        .view_tx
        .try_send(ViewCommand::ToolApprovalRequest {
            request_id: request_id.clone(),
            context,
        })
        .is_err()
    {
        let _ = ctx.deps().approval_gate.resolve(&request_id, false);
        return Err(ToolError::execution_failed(
            "Failed to send approval request to UI (channel full or closed)",
        ));
    }

    let approved = waiter.wait().await.unwrap_or(false);
    if approved {
        Ok(())
    } else {
        Err(ToolError::execution_failed("Tool execution denied by user"))
    }
}

/// Extract a sensible primary target from MCP tool arguments.
/// Tries to find the first string argument or the first key as fallback.
fn extract_primary_target(args: &serde_json::Value, tool_name: &str) -> String {
    // Try to find the first string value in the args object
    if let Some(obj) = args.as_object() {
        for value in obj.values() {
            if let Some(s) = value.as_str() {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
        // If no string value found, use the first key
        if let Some(key) = obj.keys().next() {
            return key.clone();
        }
    }
    // Fallback to tool name
    tool_name.to_string()
}

/// Truncate a JSON value for display in approval UI.
fn truncate_value(value: &serde_json::Value, max_len: usize) -> String {
    let s = match value {
        serde_json::Value::String(s) => s.clone(),
        _ => value.to_string(),
    };
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_primary_target_finds_first_string_value() {
        let args = serde_json::json!({
            "path": "/tmp/test.txt",
            "content": "hello world"
        });
        let result = extract_primary_target(&args, "write_file");
        // Should find the first string value
        assert_eq!(result, "/tmp/test.txt");
    }

    #[test]
    fn extract_primary_target_uses_first_key_when_no_strings() {
        let args = serde_json::json!({
            "count": 42,
            "enabled": true
        });
        let result = extract_primary_target(&args, "count_tool");
        // Should use first key when no string values
        assert_eq!(result, "count");
    }

    #[test]
    fn extract_primary_target_fallback_to_tool_name() {
        let args = serde_json::json!({});
        let result = extract_primary_target(&args, "my_tool");
        // Should fallback to tool name when no args
        assert_eq!(result, "my_tool");
    }

    #[test]
    fn extract_primary_target_handles_non_object_args() {
        let args = serde_json::json!("just a string");
        let result = extract_primary_target(&args, "string_tool");
        // Should fallback to tool name for non-object args
        assert_eq!(result, "string_tool");
    }

    #[test]
    fn extract_primary_target_skips_empty_strings() {
        let args = serde_json::json!({
            "empty": "",
            "path": "/valid/path"
        });
        let result = extract_primary_target(&args, "test_tool");
        // Should skip empty strings and find the next valid one
        assert_eq!(result, "/valid/path");
    }

    #[test]
    fn truncate_value_leaves_short_strings_unchanged() {
        let value = serde_json::Value::String("short".to_string());
        let result = truncate_value(&value, 50);
        assert_eq!(result, "short");
    }

    #[test]
    fn truncate_value_truncates_long_strings() {
        let long_string = "a".repeat(100);
        let value = serde_json::Value::String(long_string.clone());
        let result = truncate_value(&value, 50);
        assert_eq!(result, format!("{}...", &long_string[..50]));
    }

    #[test]
    fn truncate_value_handles_non_string_values() {
        let value = serde_json::json!({"key": "value"});
        let result = truncate_value(&value, 50);
        // Should convert to string representation
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn truncate_value_truncates_non_string_values() {
        let value = serde_json::json!({"a": "b"});
        let result = truncate_value(&value, 5);
        // JSON representation is {"a":"b"} which is 13 chars, should be truncated
        assert!(result.ends_with("..."));
    }

    #[test]
    fn mcp_tool_executor_new_sets_tool_name() {
        let executor = McpToolExecutor::new("test_tool");
        assert_eq!(executor.tool_name, "test_tool");
    }
}
