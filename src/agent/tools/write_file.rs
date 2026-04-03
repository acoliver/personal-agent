//! `WriteFile` tool implementation.
//!
//! This module provides a built-in `WriteFileExecutor` that creates or fully
//! overwrites files with support for creating parent directories.

use crate::agent::tool_approval_policy::ToolApprovalDecision;
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::ViewCommand;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::path::Path;

/// Executor for the `WriteFile` built-in tool.
///
/// This executor provides direct filesystem access for writing file contents.
/// It creates parent directories as needed and fully overwrites existing files.
#[derive(Debug, Clone, Copy)]
pub struct WriteFileExecutor;

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for WriteFileExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::execution_failed("Missing required 'path' argument"))?
            .to_string();

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::execution_failed("Missing required 'content' argument"))?
            .to_string();

        check_approval(ctx.deps(), &path).await?;

        let absolute_path = resolve_path(&path)?;
        ensure_parent_dirs(&absolute_path).await?;
        write_content(&absolute_path, &content).await?;

        let line_count = content.lines().count();
        Ok(ToolReturn::text(format!(
            "Wrote {} lines to {}",
            line_count,
            absolute_path.display()
        )))
    }
}

/// Check tool approval policy and await user decision if required.
async fn check_approval(tool_context: &McpToolContext, path: &str) -> Result<(), ToolError> {
    let decision = {
        let policy = tool_context.policy.lock().await;
        policy.evaluate("WriteFile")
    };

    match decision {
        ToolApprovalDecision::Allow => Ok(()),
        ToolApprovalDecision::Deny => Err(ToolError::execution_failed(
            "Tool execution denied by policy",
        )),
        ToolApprovalDecision::AskUser => {
            let request_id = uuid::Uuid::new_v4().to_string();
            let rx = tool_context
                .approval_gate
                .wait_for_approval(request_id.clone(), "WriteFile".to_string());

            if tool_context
                .view_tx
                .try_send(ViewCommand::ToolApprovalRequest {
                    request_id: request_id.clone(),
                    tool_name: "WriteFile".to_string(),
                    tool_argument: path.to_string(),
                })
                .is_err()
            {
                let _ = tool_context.approval_gate.resolve(&request_id, false);
                return Err(ToolError::execution_failed(
                    "Failed to send approval request to UI (channel full or closed)",
                ));
            }

            let approved = rx.await.unwrap_or(false);

            if approved {
                Ok(())
            } else {
                Err(ToolError::execution_failed("Tool execution denied by user"))
            }
        }
    }
}

/// Validate that the path is absolute and return it as a `PathBuf`.
fn resolve_path(path: &str) -> Result<std::path::PathBuf, ToolError> {
    let file_path = Path::new(path);
    if !file_path.is_absolute() {
        return Err(ToolError::execution_failed(
            "The 'path' argument must be an absolute path",
        ));
    }
    Ok(file_path.to_path_buf())
}

/// Create parent directories if they don't exist.
async fn ensure_parent_dirs(absolute_path: &Path) -> Result<(), ToolError> {
    if let Some(parent) = absolute_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::execution_failed(format!(
                    "Failed to create parent directories for '{}': {e}",
                    absolute_path.display()
                ))
            })?;
        }
    }
    Ok(())
}

/// Write content to a file with descriptive error mapping.
async fn write_content(absolute_path: &Path, content: &str) -> Result<(), ToolError> {
    tokio::fs::write(absolute_path, content).await.map_err(|e| {
        let msg = match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                format!(
                    "Permission denied writing to file: {}",
                    absolute_path.display()
                )
            }
            std::io::ErrorKind::InvalidInput => {
                format!("Invalid path: {}", absolute_path.display())
            }
            _ if e.raw_os_error() == Some(28) => {
                format!("Disk full: cannot write to {}", absolute_path.display())
            }
            _ => format!("Failed to write file '{}': {e}", absolute_path.display()),
        };
        ToolError::execution_failed(msg)
    })
}

/// Get the `WriteFile` tool definition.
#[must_use]
pub fn get_write_file_tool_definition() -> ToolDefinition {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The absolute file path to write to"
            },
            "content": {
                "type": "string",
                "description": "The full file content to write"
            }
        },
        "required": ["path", "content"]
    });

    ToolDefinition::new("WriteFile", "Create a new file or fully overwrite an existing file. Creates any missing parent directories. Returns a confirmation message with the number of lines written.")
        .with_parameters(input_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_approval_policy::ToolApprovalPolicy;
    use std::io::Read;
    use tempfile::tempdir;

    #[tokio::test]
    async fn write_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_file.txt");
        let path_str = file_path.to_str().unwrap();

        // Create a mock context with a policy that allows the tool
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());

        // Use YOLO mode to bypass approval
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy {
            yolo_mode: true,
            ..Default::default()
        }));

        let context = McpToolContext {
            view_tx,
            approval_gate,
            policy,
        };

        let executor = WriteFileExecutor;
        let args = serde_json::json!({
            "path": path_str,
            "content": "line1\nline2\nline3\n"
        });

        let run_ctx = RunContext::new(context, "test-model");
        let result = executor.execute(args, &run_ctx).await;

        assert!(
            result.is_ok(),
            "Expected success but got: {:?}",
            result.err()
        );
        let tool_return = result.unwrap();
        match &tool_return.content {
            serdes_ai::core::messages::ToolReturnContent::Text { content } => {
                assert!(content.contains("Wrote 3 lines"));
                assert!(content.contains(path_str));
            }
            _ => panic!("Expected text content"),
        }

        // Verify the file was written correctly
        let mut file = std::fs::File::open(&file_path).unwrap();
        let mut read_content = String::new();
        file.read_to_string(&mut read_content).unwrap();
        assert_eq!(read_content, "line1\nline2\nline3\n");
    }

    #[tokio::test]
    async fn write_file_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let nested_path = dir.path().join("a").join("b").join("c").join("file.txt");
        let path_str = nested_path.to_str().unwrap();

        // Create a mock context with YOLO mode
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());

        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy {
            yolo_mode: true,
            ..Default::default()
        }));

        let context = McpToolContext {
            view_tx,
            approval_gate,
            policy,
        };

        let executor = WriteFileExecutor;
        let args = serde_json::json!({
            "path": path_str,
            "content": "nested content"
        });

        let run_ctx = RunContext::new(context, "test-model");
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_ok());
        assert!(nested_path.exists());
    }

    #[tokio::test]
    async fn write_file_overwrites_existing() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");

        // Create initial file
        std::fs::write(&file_path, "original content").unwrap();

        let path_str = file_path.to_str().unwrap();

        // Create a mock context with YOLO mode
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());

        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy {
            yolo_mode: true,
            ..Default::default()
        }));

        let context = McpToolContext {
            view_tx,
            approval_gate,
            policy,
        };

        let executor = WriteFileExecutor;
        let args = serde_json::json!({
            "path": path_str,
            "content": "new overwritten content"
        });

        let run_ctx = RunContext::new(context, "test-model");
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_ok());

        // Verify file was overwritten
        let read_content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, "new overwritten content");
    }

    #[tokio::test]
    async fn write_file_missing_path() {
        let executor = WriteFileExecutor;
        let args = serde_json::json!({"content": "some content"});

        // Create minimal context
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy::default()));
        let context = McpToolContext {
            view_tx,
            approval_gate,
            policy,
        };
        let run_ctx = RunContext::new(context, "test-model");

        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Missing required 'path' argument"));
    }

    #[tokio::test]
    async fn write_file_missing_content() {
        let executor = WriteFileExecutor;
        let args = serde_json::json!({"path": "/tmp/test.txt"});

        // Create minimal context
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy::default()));
        let context = McpToolContext {
            view_tx,
            approval_gate,
            policy,
        };
        let run_ctx = RunContext::new(context, "test-model");

        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err
            .to_string()
            .contains("Missing required 'content' argument"));
    }

    #[tokio::test]
    async fn write_file_rejects_relative_path() {
        let executor = WriteFileExecutor;
        let args = serde_json::json!({"path": "relative/path.txt", "content": "data"});

        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy {
            yolo_mode: true,
            ..Default::default()
        }));
        let context = McpToolContext {
            view_tx,
            approval_gate,
            policy,
        };
        let run_ctx = RunContext::new(context, "test-model");

        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("absolute path"));
    }

    #[test]
    fn get_write_file_tool_definition_returns_valid_schema() {
        let def = get_write_file_tool_definition();
        assert_eq!(def.name, "WriteFile");
        assert!(!def.description.is_empty());
        assert!(def.parameters().is_object());
    }
}
