//! `ReadFile` tool implementation.
//!
//! This module provides a built-in `ReadFileExecutor` that reads file contents
//! with support for line ranges, truncation, and binary file detection.

use crate::agent::tool_approval_policy::ToolApprovalDecision;
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::{ToolApprovalContext, ToolCategory, ViewCommand};
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::path::Path;

/// Maximum number of lines to read before truncation
const MAX_LINES: usize = 2000;
/// Maximum file size to read (~512KB)
const MAX_SIZE_BYTES: usize = 512_000;
/// Number of bytes to check for binary detection
const BINARY_CHECK_BYTES: usize = 8_192;

/// Executor for the `ReadFile` built-in tool.
///
/// This executor provides direct filesystem access for reading file contents,
/// supporting full file reading, line range extraction, and intelligent
/// truncation with helpful continuation messages.
#[derive(Debug, Clone, Copy)]
pub struct ReadFileExecutor;

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for ReadFileExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        // Parse arguments
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::execution_failed("Missing required 'path' argument"))?;

        let start_line = args
            .get("start_line")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| usize::try_from(v).ok());
        let end_line = args
            .get("end_line")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| usize::try_from(v).ok());

        // Resolve the path
        let file_path = Path::new(path);
        let absolute_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            // Relative path - resolve against current working directory
            std::env::current_dir()
                .map_err(|e| {
                    ToolError::execution_failed(format!("Failed to get current directory: {e}"))
                })?
                .join(file_path)
        };

        let approval_path = absolute_path.display().to_string();
        check_approval(ctx.deps(), &approval_path).await?;

        // Check if file exists and is accessible
        match tokio::fs::metadata(&absolute_path).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ToolError::execution_failed(format!(
                    "File not found: {}",
                    absolute_path.display()
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(ToolError::execution_failed(format!(
                    "Permission denied: {}",
                    absolute_path.display()
                )));
            }
            Err(e) => {
                return Err(ToolError::execution_failed(format!(
                    "Failed to access file '{}': {e}",
                    absolute_path.display()
                )));
            }
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(ToolError::execution_failed(format!(
                        "Path is not a file: {}",
                        absolute_path.display()
                    )));
                }
            }
        }

        // Read the file content
        let content = tokio::fs::read(&absolute_path).await.map_err(|e| {
            ToolError::execution_failed(format!(
                "Failed to read file '{}': {e}",
                absolute_path.display()
            ))
        })?;

        // Check for binary content (null bytes in first 8KB)
        let bytes_to_check = content.len().min(BINARY_CHECK_BYTES);
        if content[..bytes_to_check].contains(&0) {
            return Err(ToolError::execution_failed(format!(
                "Cannot read binary file: {}",
                absolute_path.display()
            )));
        }

        // Convert to string
        let content_str = String::from_utf8(content).map_err(|e| {
            ToolError::execution_failed(format!(
                "Cannot read binary file (invalid UTF-8): {} - {e}",
                absolute_path.display()
            ))
        })?;

        // Split into lines
        let lines: Vec<&str> = content_str.lines().collect();
        let total_lines = lines.len();

        // Handle line range extraction or full content with truncation
        Self::process_content(&lines, total_lines, &content_str, start_line, end_line)
    }
}

/// Check tool approval policy and await user decision if required.
async fn check_approval(tool_context: &McpToolContext, path: &str) -> Result<(), ToolError> {
    let decision = {
        let policy = tool_context.policy.lock().await;
        policy.evaluate("ReadFile")
    };

    match decision {
        ToolApprovalDecision::Allow => Ok(()),
        ToolApprovalDecision::Deny => Err(ToolError::execution_failed(
            "Tool execution denied by policy",
        )),
        ToolApprovalDecision::AskUser => {
            let request_id = uuid::Uuid::new_v4().to_string();
            let waiter = tool_context
                .approval_gate
                .wait_for_approval(request_id.clone(), "ReadFile".to_string());

            // Build rich context for approval UI
            let context = ToolApprovalContext::new("ReadFile", ToolCategory::FileRead, path);

            if tool_context
                .view_tx
                .try_send(ViewCommand::ToolApprovalRequest {
                    request_id: request_id.clone(),
                    context,
                })
                .is_err()
            {
                let _ = tool_context.approval_gate.resolve(&request_id, false);
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
    }
}

impl ReadFileExecutor {
    /// Process the content based on line range or truncation rules.
    fn process_content(
        lines: &[&str],
        total_lines: usize,
        content_str: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<ToolReturn, ToolError> {
        // Validate line numbers are 1-based (check even when only one is provided)
        if let Some(start) = start_line {
            if start == 0 {
                return Err(ToolError::execution_failed(
                    "Line numbers are 1-based and must be greater than 0",
                ));
            }
        }
        if let Some(end) = end_line {
            if end == 0 {
                return Err(ToolError::execution_failed(
                    "Line numbers are 1-based and must be greater than 0",
                ));
            }
        }

        // Validate line range relationship
        if let (Some(start), Some(end)) = (start_line, end_line) {
            if start > end {
                return Err(ToolError::execution_failed(format!(
                    "Invalid line range: start_line ({start}) cannot be greater than end_line ({end})"
                )));
            }
        }

        let result = match (start_line, end_line) {
            // Both start and end specified - extract range
            (Some(start), Some(end)) => {
                if start > total_lines {
                    return Err(ToolError::execution_failed(format!(
                        "Invalid line range: start_line ({start}) exceeds total lines ({total_lines})"
                    )));
                }

                // Convert to 0-based indices
                let start_idx = start - 1;
                let end_idx = total_lines.min(end);

                let selected_lines: Vec<&str> = lines[start_idx..end_idx].to_vec();
                selected_lines.join("\n")
            }
            // Only start specified - extract from start to end of file or cap
            (Some(start), None) => {
                if start > total_lines {
                    return Err(ToolError::execution_failed(format!(
                        "Invalid line range: start_line ({start}) exceeds total lines ({total_lines})"
                    )));
                }

                let start_idx = start - 1;
                let remaining_lines = total_lines - start_idx;

                if remaining_lines > MAX_LINES {
                    let selected_lines: Vec<&str> =
                        lines[start_idx..start_idx + MAX_LINES].to_vec();
                    let truncated_content = selected_lines.join("\n");
                    format!(
                        "{}\n\n[... {} lines remaining, use start_line={} to continue ...]",
                        truncated_content,
                        remaining_lines - MAX_LINES,
                        start_idx + MAX_LINES + 1
                    )
                } else {
                    lines[start_idx..].join("\n")
                }
            }
            // Only end specified - not supported
            (None, Some(_end)) => {
                return Err(ToolError::execution_failed(
                    "Cannot specify end_line without start_line",
                ));
            }
            // No range specified - full file with truncation
            (None, None) => Self::truncate_full_content(content_str, lines, total_lines),
        };

        Ok(ToolReturn::text(result))
    }

    /// Truncate full content when no line range is specified.
    fn truncate_full_content(content_str: &str, lines: &[&str], total_lines: usize) -> String {
        let content_bytes = content_str.as_bytes();

        if content_bytes.len() > MAX_SIZE_BYTES {
            Self::truncate_by_bytes(content_str, content_bytes, total_lines)
        } else if total_lines > MAX_LINES {
            Self::truncate_by_lines(lines, total_lines)
        } else {
            content_str.to_string()
        }
    }

    /// Truncate content by finding a line boundary near `MAX_SIZE_BYTES`.
    fn truncate_by_bytes(content_str: &str, content_bytes: &[u8], total_lines: usize) -> String {
        // Find the line boundary before MAX_SIZE_BYTES
        let max_boundary = content_bytes.len().min(MAX_SIZE_BYTES);
        let mut end_byte = max_boundary;

        // Search backwards for a newline
        while end_byte > 0 && content_bytes[end_byte - 1] != b'\n' {
            end_byte -= 1;
        }

        // If no newline was found, fall back to max_boundary.
        // Always move to a valid UTF-8 boundary before slicing.
        if end_byte == 0 {
            end_byte = max_boundary;
        }
        while end_byte > 0 && !content_str.is_char_boundary(end_byte) {
            end_byte -= 1;
        }

        let truncated_content = &content_str[..end_byte];
        let shown_lines = truncated_content.lines().count();
        let remaining_lines = total_lines.saturating_sub(shown_lines);

        format!(
            "{}\n\n[... {} lines and {} bytes remaining, use start_line={} to continue ...]",
            truncated_content,
            remaining_lines,
            content_bytes.len() - end_byte,
            shown_lines + 1
        )
    }

    /// Truncate content by limiting to `MAX_LINES`.
    fn truncate_by_lines(lines: &[&str], total_lines: usize) -> String {
        let selected_lines: Vec<&str> = lines[..MAX_LINES].to_vec();
        let truncated_content = selected_lines.join("\n");
        format!(
            "{}\n\n[... {} lines remaining, use start_line={} to continue ...]",
            truncated_content,
            total_lines - MAX_LINES,
            MAX_LINES + 1
        )
    }
}

/// Get the `ReadFile` tool definition.
#[must_use]
pub fn get_read_file_tool_definition() -> ToolDefinition {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The path to the file to read (absolute or relative to current directory)"
            },
            "start_line": {
                "type": "integer",
                "description": "Optional 1-based line number to start reading from",
                "minimum": 1
            },
            "end_line": {
                "type": "integer",
                "description": "Optional 1-based line number to stop reading at (inclusive). Requires start_line to be specified.",
                "minimum": 1
            }
        },
        "required": ["path"]
    });

    ToolDefinition::new("ReadFile", "Read the contents of a file with optional line range support. Supports reading text files with automatic truncation for large files (max 2000 lines or ~500KB). Use start_line and end_line parameters to read specific portions of a file.")
        .with_parameters(input_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_approval_policy::ToolApprovalPolicy;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn yolo_context() -> RunContext<McpToolContext> {
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy {
            yolo_mode: true,
            ..Default::default()
        }));

        RunContext::new(
            McpToolContext {
                view_tx,
                approval_gate,
                policy,
            },
            "test-model",
        )
    }

    #[tokio::test]
    async fn read_simple_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\nline3\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({"path": path});
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_ok());
        let tool_return = result.unwrap();
        match &tool_return.content {
            serdes_ai::core::messages::ToolReturnContent::Text { content } => {
                assert_eq!(content, "line1\nline2\nline3\n");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn read_file_not_found() {
        let executor = ReadFileExecutor;
        let args = serde_json::json!({"path": "/nonexistent/path/file.txt"});
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn read_file_missing_path() {
        let executor = ReadFileExecutor;
        let args = serde_json::json!({});
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Missing required 'path' argument"));
    }

    #[tokio::test]
    async fn read_file_with_line_range() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\nline3\nline4\nline5\n")
            .unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "start_line": 2,
            "end_line": 4
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_ok());
        let tool_return = result.unwrap();
        match &tool_return.content {
            serdes_ai::core::messages::ToolReturnContent::Text { content } => {
                assert_eq!(content, "line2\nline3\nline4");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn read_file_with_start_line_only() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\nline3\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "start_line": 2
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_ok());
        let tool_return = result.unwrap();
        match &tool_return.content {
            serdes_ai::core::messages::ToolReturnContent::Text { content } => {
                assert_eq!(content, "line2\nline3");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn read_file_invalid_start_greater_than_end() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\nline3\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "start_line": 3,
            "end_line": 2
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("cannot be greater than end_line"));
    }

    #[tokio::test]
    async fn read_file_start_line_exceeds_total() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "start_line": 10
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("exceeds total lines"));
    }

    #[tokio::test]
    async fn read_binary_file_rejected() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"text\x00binary\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({"path": path});
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Cannot read binary file"));
    }

    #[tokio::test]
    async fn read_file_with_only_end_line_fails() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\nline3\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "end_line": 2
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err
            .to_string()
            .contains("Cannot specify end_line without start_line"));
    }

    #[tokio::test]
    async fn read_file_start_line_zero_rejected() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(
            b"line1
line2
",
        )
        .unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        // start_line: 0 should be rejected (1-based indexing)
        let args = serde_json::json!({
            "path": path,
            "start_line": 0
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Line numbers are 1-based"));
    }

    #[tokio::test]
    async fn read_file_end_line_zero_rejected() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"line1\nline2\n").unwrap();
        let path = file.path().to_str().unwrap();

        let executor = ReadFileExecutor;
        // end_line: 0 should be rejected (1-based indexing)
        let args = serde_json::json!({
            "path": path,
            "start_line": 1,
            "end_line": 0
        });
        let run_ctx = yolo_context();
        let result = executor.execute(args, &run_ctx).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Line numbers are 1-based"));
    }

    #[test]
    fn get_read_file_tool_definition_returns_valid_schema() {
        let def = get_read_file_tool_definition();
        assert_eq!(def.name, "ReadFile");
        assert!(!def.description.is_empty());
        assert!(def.parameters().is_object());
    }

    #[tokio::test]
    async fn read_file_requires_approval_when_not_yolo() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(
            b"line1
line2
",
        )
        .unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let executor = ReadFileExecutor;
        let args = serde_json::json!({"path": path.clone()});

        let (view_tx, mut view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy::default()));
        let run_ctx = RunContext::new(
            McpToolContext {
                view_tx,
                approval_gate: approval_gate.clone(),
                policy,
            },
            "test-model",
        );

        let handle = tokio::spawn(async move { executor.execute(args, &run_ctx).await });

        let request = view_rx
            .recv()
            .await
            .expect("approval request should be emitted");
        let request_id = match request {
            ViewCommand::ToolApprovalRequest {
                request_id,
                context,
            } => {
                assert_eq!(context.tool_name, "ReadFile");
                assert_eq!(context.category, ToolCategory::FileRead);
                assert_eq!(context.primary_target, path);
                request_id
            }
            other => panic!("expected ToolApprovalRequest, got {other:?}"),
        };

        let _ = approval_gate.resolve(&request_id, true);

        let result = handle.await.expect("task should complete");
        assert!(result.is_ok(), "approval should allow read execution");
    }
}
