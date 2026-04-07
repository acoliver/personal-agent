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
