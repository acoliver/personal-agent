//! `EditFile` tool implementation.
//!
//! This module provides a built-in `EditFileExecutor` that applies an exact
//! literal find-and-replace edit to an existing file. Optional line scoping
//! can be used to disambiguate duplicate matches.

use crate::agent::tool_approval_policy::ToolApprovalDecision;
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::{ToolApprovalContext, ToolCategory, ViewCommand};
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::ops::Range;
use std::path::{Path, PathBuf};

/// Executor for the `EditFile` built-in tool.
#[derive(Debug, Clone, Copy)]
pub struct EditFileExecutor;

#[derive(Debug, Clone, Copy)]
struct SearchScope {
    start_byte: usize,
    end_byte: usize,
}

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for EditFileExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let path = required_string(&args, "path")?;
        let old_text = required_string(&args, "old_text")?;
        let new_text = required_string(&args, "new_text")?;

        if old_text.is_empty() {
            return Err(ToolError::execution_failed(
                "The 'old_text' argument must not be empty",
            ));
        }

        let start_line = parse_optional_line(&args, "start_line")?;
        let end_line = parse_optional_line(&args, "end_line")?;

        let absolute_path = resolve_path(&path)?;
        let approval_path = absolute_path.display().to_string();

        check_approval(ctx.deps(), &approval_path, start_line, end_line).await?;

        let content = tokio::fs::read_to_string(&absolute_path)
            .await
            .map_err(|error| map_read_error(&absolute_path, &error))?;

        let scope = resolve_search_scope(&content, start_line, end_line)?;
        let scoped_content = &content[scope.start_byte..scope.end_byte];

        let mut matches = scoped_content.match_indices(&old_text);
        let Some((first_match, _)) = matches.next() else {
            return Err(ToolError::execution_failed(
                "Could not find 'old_text' in the specified scope. Verify the exact literal text.",
            ));
        };

        if matches.next().is_some() {
            return Err(ToolError::execution_failed(
                "Found multiple matches for 'old_text' in the specified scope. Use 'start_line'/'end_line' to narrow the scope.",
            ));
        }

        let absolute_match_start = scope.start_byte + first_match;
        let absolute_match_end = absolute_match_start + old_text.len();

        // Filter emojis from new_text (output) if enabled, but NOT from old_text
        // (input must match file content exactly for find-and-replace to work)
        let filtered_new_text = if ctx.deps().filter_emoji {
            strip_emojis(&new_text)
        } else {
            new_text
        };

        let mut updated_content =
            String::with_capacity(content.len() - old_text.len() + filtered_new_text.len());
        updated_content.push_str(&content[..absolute_match_start]);
        updated_content.push_str(&filtered_new_text);
        updated_content.push_str(&content[absolute_match_end..]);

        tokio::fs::write(&absolute_path, &updated_content)
            .await
            .map_err(|error| map_write_error(&absolute_path, &error))?;

        Ok(ToolReturn::text(format!(
            "Edited {} and wrote {} bytes",
            absolute_path.display(),
            updated_content.len()
        )))
    }
}

/// Strip emojis from a string, replacing them with empty string.
fn strip_emojis(text: &str) -> String {
    text.chars().filter(|c| !is_emoji(*c)).collect()
}

/// Check if a character is an emoji.
/// Uses Unicode ranges for emoji blocks.
const fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1F600}'..='\u{1F64F}' |  // Emoticons
        '\u{1F300}'..='\u{1F5FF}' |  // Misc Symbols and Pictographs
        '\u{1F680}'..='\u{1F6FF}' |  // Transport and Map
        '\u{1F1E0}'..='\u{1F1FF}' |  // Flags
        '\u{2600}'..='\u{26FF}'   |  // Misc symbols
        '\u{2700}'..='\u{27BF}'   |  // Dingbats
        '\u{1F900}'..='\u{1F9FF}' |  // Supplemental Symbols and Pictographs
        '\u{1FA00}'..='\u{1FA6F}' |  // Chess Symbols
        '\u{1FA70}'..='\u{1FAFF}' |  // Symbols and Pictographs Extended-A
        '\u{2B50}'                |  // Star
        '\u{2B55}'                |  // Circle
        '\u{25AA}'..='\u{25AB}'   |  // Small squares
        '\u{25B6}' | '\u{25C0}'   |  // Play buttons
        '\u{25FB}'..='\u{25FE}'   |  // Medium squares
        '\u{2934}'..='\u{2935}'   |  // Arrows
        '\u{2B05}'..='\u{2B07}'   |  // Arrows
        '\u{2B1B}'..='\u{2B1C}'   |  // Squares
        '\u{3030}'                |  // Wavy dash
        '\u{303D}'                |  // Part alternation mark
        '\u{3297}'                |  // Circled ideograph congratulation
        '\u{3299}'                |  // Circled ideograph secret
        '\u{FE0F}'                |  // Variation Selector-16
        '\u{20E3}'                |  // Combining enclosing keycap
        '\u{E0020}'..='\u{E007F}'   // Tags for emoji sequences
    )
}

/// Validate that the path is absolute and return it as a `PathBuf`.
fn resolve_path(path: &str) -> Result<PathBuf, ToolError> {
    let file_path = Path::new(path);
    if !file_path.is_absolute() {
        return Err(ToolError::execution_failed(
            "The 'path' argument must be an absolute path",
        ));
    }
    Ok(file_path.to_path_buf())
}

fn required_string(args: &serde_json::Value, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ToolError::execution_failed(format!("Missing required '{key}' argument")))
}

fn parse_optional_line(args: &serde_json::Value, key: &str) -> Result<Option<usize>, ToolError> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };

    let parsed = value.as_u64().ok_or_else(|| {
        ToolError::execution_failed(format!(
            "Invalid '{key}' argument: expected a positive integer"
        ))
    })?;

    if parsed == 0 {
        return Err(ToolError::execution_failed(format!(
            "Invalid '{key}' argument: line numbers are 1-based"
        )));
    }

    usize::try_from(parsed).map(Some).map_err(|_| {
        ToolError::execution_failed(format!("Invalid '{key}' argument: value is too large"))
    })
}

fn compute_line_ranges(content: &str) -> Vec<Range<usize>> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut line_start = 0;

    for (idx, ch) in content.char_indices() {
        if ch == '\n' {
            ranges.push(line_start..idx + 1);
            line_start = idx + 1;
        }
    }

    if line_start < content.len() {
        ranges.push(line_start..content.len());
    }

    ranges
}

fn resolve_search_scope(
    content: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<SearchScope, ToolError> {
    if let (Some(start), Some(end)) = (start_line, end_line) {
        if start > end {
            return Err(ToolError::execution_failed(format!(
                "Invalid line range: start_line ({start}) cannot be greater than end_line ({end})"
            )));
        }
    }

    if start_line.is_none() && end_line.is_none() {
        return Ok(SearchScope {
            start_byte: 0,
            end_byte: content.len(),
        });
    }

    let line_ranges = compute_line_ranges(content);
    let total_lines = line_ranges.len();

    if total_lines == 0 {
        return Err(ToolError::execution_failed(
            "Invalid line range: cannot scope an empty file",
        ));
    }

    let start = start_line.unwrap_or(1);
    if start > total_lines {
        return Err(ToolError::execution_failed(format!(
            "Invalid line range: start_line ({start}) exceeds total lines ({total_lines})"
        )));
    }

    let requested_end = end_line.unwrap_or(total_lines);
    let end = requested_end.min(total_lines);

    if end < start {
        return Err(ToolError::execution_failed(format!(
            "Invalid line range: end_line ({requested_end}) is before start_line ({start})"
        )));
    }

    Ok(SearchScope {
        start_byte: line_ranges[start - 1].start,
        end_byte: line_ranges[end - 1].end,
    })
}

fn map_read_error(path: &Path, error: &std::io::Error) -> ToolError {
    let message = match error.kind() {
        std::io::ErrorKind::NotFound => format!("File not found: {}", path.display()),
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied reading file: {}", path.display())
        }
        _ => format!("Failed to read file '{}': {error}", path.display()),
    };
    ToolError::execution_failed(message)
}

fn map_write_error(path: &Path, error: &std::io::Error) -> ToolError {
    let message = match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied writing file: {}", path.display())
        }
        std::io::ErrorKind::InvalidInput => format!("Invalid path: {}", path.display()),
        _ => format!("Failed to write file '{}': {error}", path.display()),
    };
    ToolError::execution_failed(message)
}

/// Check tool approval policy and await user decision if required.
async fn check_approval(
    tool_context: &McpToolContext,
    path: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<(), ToolError> {
    let decision = {
        let policy = tool_context.policy.lock().await;
        policy.evaluate("EditFile")
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
                .wait_for_approval(request_id.clone(), "EditFile".to_string());

            // Build rich context for approval UI
            let mut context = ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, path);
            if let Some(start) = start_line {
                context = context.with_detail(
                    "line_range",
                    end_line.map_or_else(|| format!("{start}"), |end| format!("{start}-{end}")),
                );
            }

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

/// Get the `EditFile` tool definition.
#[must_use]
pub fn get_edit_file_tool_definition() -> ToolDefinition {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "The absolute path to the file to edit"
            },
            "old_text": {
                "type": "string",
                "description": "The exact literal text to find"
            },
            "new_text": {
                "type": "string",
                "description": "The replacement text"
            },
            "start_line": {
                "type": "integer",
                "description": "Optional 1-based line number where search scope starts",
                "minimum": 1
            },
            "end_line": {
                "type": "integer",
                "description": "Optional 1-based line number where search scope ends (inclusive)",
                "minimum": 1
            }
        },
        "required": ["path", "old_text", "new_text"]
    });

    ToolDefinition::new(
        "EditFile",
        "Edit an existing file by replacing one exact literal old_text match with new_text. Use optional start_line/end_line to scope the search and disambiguate duplicates.",
    )
    .with_parameters(input_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_approval_policy::ToolApprovalPolicy;
    use std::fs;
    use tempfile::tempdir;

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
                ..Default::default()
            },
            "test-model",
        )
    }

    #[tokio::test]
    async fn edit_file_single_match_replaces_text() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "alpha\nbeta\ngamma\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "beta",
            "new_text": "delta"
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_ok(), "edit should succeed: {result:?}");
        let updated = fs::read_to_string(&path).expect("file should be readable");
        assert_eq!(updated, "alpha\ndelta\ngamma\n");
    }

    #[tokio::test]
    async fn edit_file_returns_error_when_text_not_found() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "alpha\nbeta\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "missing",
            "new_text": "delta"
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_err());
        let error = result.expect_err("missing text should fail");
        assert!(error.to_string().contains("Could not find 'old_text'"));
    }

    #[tokio::test]
    async fn edit_file_returns_error_when_multiple_matches_exist() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "duplicate\nline\nduplicate\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "duplicate",
            "new_text": "updated"
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_err());
        let error = result.expect_err("multiple matches should fail");
        assert!(error.to_string().contains("Found multiple matches"));
        assert!(error.to_string().contains("start_line"));
    }

    #[tokio::test]
    async fn edit_file_line_scope_disambiguates_match() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "duplicate\nline\nduplicate\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "duplicate",
            "new_text": "updated",
            "start_line": 3,
            "end_line": 3
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_ok(), "scoped edit should succeed: {result:?}");
        let updated = fs::read_to_string(&path).expect("file should be readable");
        assert_eq!(updated, "duplicate\nline\nupdated\n");
    }

    #[tokio::test]
    async fn edit_file_returns_io_error_for_missing_file() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("missing.txt");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "x",
            "new_text": "y"
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_err());
        let error = result.expect_err("missing file should fail");
        assert!(error.to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn edit_file_rejects_empty_old_text() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "alpha\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "",
            "new_text": "beta"
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_err());
        let error = result.expect_err("empty old_text should fail");
        assert!(error.to_string().contains("must not be empty"));
    }

    #[tokio::test]
    async fn edit_file_allows_same_replacement_text() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "alpha\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "alpha",
            "new_text": "alpha"
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(
            result.is_ok(),
            "no-op replacement should succeed: {result:?}"
        );
        let updated = fs::read_to_string(&path).expect("file should be readable");
        assert_eq!(updated, "alpha\n");
    }

    #[tokio::test]
    async fn edit_file_rejects_out_of_bounds_line_range() {
        let dir = tempdir().expect("temp dir should be created");
        let path = dir.path().join("test.txt");
        fs::write(&path, "alpha\nbeta\n").expect("seed file should be written");

        let executor = EditFileExecutor;
        let args = serde_json::json!({
            "path": path,
            "old_text": "beta",
            "new_text": "delta",
            "start_line": 4,
            "end_line": 4
        });

        let result = executor.execute(args, &yolo_context()).await;

        assert!(result.is_err());
        let error = result.expect_err("out-of-bounds range should fail");
        assert!(error.to_string().contains("exceeds total lines"));
    }

    #[test]
    fn get_edit_file_tool_definition_returns_valid_schema() {
        let def = get_edit_file_tool_definition();
        assert_eq!(def.name, "EditFile");
        assert!(!def.description.is_empty());
        assert!(def.parameters().is_object());
    }
}
