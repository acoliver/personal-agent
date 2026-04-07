//! `Search` tool implementation.
//!
//! This module provides a built-in `SearchExecutor` that searches file contents
//! using regex across a directory tree. It prefers ripgrep when available and
//! falls back to a built-in recursive search implementation.

use crate::agent::tool_approval_policy::ToolApprovalDecision;
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::{ToolApprovalContext, ToolCategory, ViewCommand};
use glob::Pattern;
use ignore::WalkBuilder;
use regex::Regex;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Maximum number of matches returned by the tool.
const MAX_MATCHES: usize = 200;
/// Number of bytes sampled for binary-file detection.
const BINARY_CHECK_BYTES: usize = 8_192;
/// Maximum amount of time to wait for ripgrep execution.
const RIPGREP_TIMEOUT: Duration = Duration::from_secs(30);

/// Executor for the `Search` built-in tool.
#[derive(Debug, Clone, Copy)]
pub struct SearchExecutor;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchMatch {
    file_path: String,
    line_number: usize,
    line_content: String,
}

#[derive(Debug)]
struct SearchArgs {
    pattern: String,
    path: String,
    include: Option<String>,
}

#[derive(Debug)]
enum RipgrepError {
    NotFound,
    Timeout,
    Failed(String),
}

#[derive(Debug)]
struct IncludeFilter {
    pattern: Pattern,
    has_separator: bool,
}

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for SearchExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let search_args = parse_search_args(&args).map_err(ToolError::execution_failed)?;

        check_approval(
            ctx.deps(),
            &search_args.pattern,
            &search_args.path,
            search_args.include.as_deref(),
        )
        .await?;
        execute_search(search_args)
            .await
            .map(ToolReturn::text)
            .map_err(ToolError::execution_failed)
    }
}

/// Check tool approval policy and await user decision if required.
async fn check_approval(
    tool_context: &McpToolContext,
    pattern: &str,
    path: &str,
    include: Option<&str>,
) -> Result<(), ToolError> {
    let decision = {
        let policy = tool_context.policy.lock().await;
        policy.evaluate("Search")
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
                .wait_for_approval(request_id.clone(), "Search".to_string());

            // Build rich context for approval UI
            let mut context = ToolApprovalContext::new("Search", ToolCategory::Search, path);
            context = context.with_detail("pattern", pattern);
            if let Some(inc) = include {
                context = context.with_detail("include", inc);
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

async fn execute_search(search_args: SearchArgs) -> Result<String, String> {
    let result = match search_with_ripgrep(
        &search_args.pattern,
        &search_args.path,
        search_args.include.as_deref(),
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(RipgrepError::NotFound | RipgrepError::Timeout) => search_with_builtin(
            &search_args.pattern,
            &search_args.path,
            search_args.include.as_deref(),
        ),
        Err(RipgrepError::Failed(message)) => Err(message),
    }?;

    Ok(format_results(&search_args.pattern, &result.0, result.1))
}

fn parse_search_args(args: &serde_json::Value) -> Result<SearchArgs, String> {
    let pattern = parse_required_string(args, "pattern")?.to_string();
    let path = parse_optional_string(args, "path")?
        .filter(|value| !value.is_empty())
        .map_or_else(|| ".".to_string(), std::string::ToString::to_string);
    let include = parse_optional_string(args, "include")?.map(std::string::ToString::to_string);

    Ok(SearchArgs {
        pattern,
        path,
        include,
    })
}

fn parse_required_string<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    parse_optional_string(args, key)?.ok_or_else(|| format!("Missing required '{key}' argument"))
}

fn parse_optional_string<'a>(
    args: &'a serde_json::Value,
    key: &str,
) -> Result<Option<&'a str>, String> {
    let Some(value) = args.get(key) else {
        return Ok(None);
    };

    if value.is_null() {
        return Ok(None);
    }

    value
        .as_str()
        .map(Some)
        .ok_or_else(|| format!("'{key}' must be a string"))
}

async fn search_with_ripgrep(
    pattern: &str,
    path: &str,
    include: Option<&str>,
) -> Result<(Vec<SearchMatch>, bool), RipgrepError> {
    let mut command = Command::new("rg");
    command
        .arg("--json")
        .arg("--line-number")
        .arg("--no-heading")
        .arg("--color=never");

    if let Some(glob) = include {
        command.arg("--glob").arg(glob);
    }

    command.arg("--").arg(pattern).arg(path);

    let output = timeout(RIPGREP_TIMEOUT, command.output())
        .await
        .map_err(|_| RipgrepError::Timeout)?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                RipgrepError::NotFound
            } else {
                RipgrepError::Failed(format!("Failed to execute ripgrep: {error}"))
            }
        })?;

    let exit_code = output.status.code();
    if !output.status.success() && exit_code != Some(1) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if stderr.trim().is_empty() {
            "ripgrep search failed".to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(RipgrepError::Failed(message));
    }

    let mut matches = Vec::new();
    let mut truncated = false;

    for line in output.stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }

        if let Some(search_match) = parse_ripgrep_match(line) {
            if matches.len() < MAX_MATCHES {
                matches.push(search_match);
            } else {
                truncated = true;
                break;
            }
        }
    }

    Ok((matches, truncated))
}

fn parse_ripgrep_match(line: &[u8]) -> Option<SearchMatch> {
    let value: serde_json::Value = serde_json::from_slice(line).ok()?;
    let event_type = value.get("type")?.as_str()?;
    if event_type != "match" {
        return None;
    }

    let data = value.get("data")?;
    let file_path = data.get("path")?.get("text")?.as_str()?.to_string();
    let line_number = data
        .get("line_number")?
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())?;

    let line_content = data
        .get("lines")
        .and_then(|lines| lines.get("text"))
        .and_then(serde_json::Value::as_str)
        .map_or_else(String::new, |content| trim_line_ending(content).to_string());

    Some(SearchMatch {
        file_path,
        line_number,
        line_content,
    })
}

fn trim_line_ending(content: &str) -> &str {
    content.trim_end_matches(['\n', '\r'])
}

fn search_with_builtin(
    pattern: &str,
    path: &str,
    include: Option<&str>,
) -> Result<(Vec<SearchMatch>, bool), String> {
    let regex = Regex::new(pattern).map_err(|error| format!("Invalid regex pattern: {error}"))?;
    let include_filter = compile_include_filter(include)?;
    let root_path = PathBuf::from(path);

    let metadata = std::fs::metadata(&root_path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => format!("Search path not found: {}", root_path.display()),
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied: {}", root_path.display())
        }
        _ => format!(
            "Failed to read search path metadata '{}': {error}",
            root_path.display()
        ),
    })?;

    if metadata.is_file() {
        Ok(search_single_file(
            &root_path,
            &regex,
            include_filter.as_ref(),
        ))
    } else if metadata.is_dir() {
        Ok(search_directory(
            &root_path,
            &regex,
            include_filter.as_ref(),
        ))
    } else {
        Err(format!(
            "Search path is neither a file nor directory: {}",
            root_path.display()
        ))
    }
}

fn compile_include_filter(include: Option<&str>) -> Result<Option<IncludeFilter>, String> {
    include
        .map(|pattern| {
            Pattern::new(pattern)
                .map(|compiled| IncludeFilter {
                    pattern: compiled,
                    has_separator: pattern.contains('/') || pattern.contains('\\'),
                })
                .map_err(|error| format!("Invalid include glob pattern '{pattern}': {error}"))
        })
        .transpose()
}

fn search_single_file(
    file_path: &Path,
    regex: &Regex,
    include_filter: Option<&IncludeFilter>,
) -> (Vec<SearchMatch>, bool) {
    let mut matches = Vec::new();
    let mut truncated = false;

    let root = file_path.parent().unwrap_or_else(|| Path::new("."));
    if !matches_include(file_path, root, include_filter) {
        return (matches, truncated);
    }

    collect_matches_from_file(file_path, regex, &mut matches, &mut truncated);
    (matches, truncated)
}

fn search_directory(
    root_path: &Path,
    regex: &Regex,
    include_filter: Option<&IncludeFilter>,
) -> (Vec<SearchMatch>, bool) {
    let mut matches = Vec::new();
    let mut truncated = false;

    let walker = WalkBuilder::new(root_path).standard_filters(true).build();

    for entry_result in walker {
        let Ok(entry) = entry_result else {
            continue;
        };

        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }

        let file_path = entry.path();
        if !matches_include(file_path, root_path, include_filter) {
            continue;
        }

        collect_matches_from_file(file_path, regex, &mut matches, &mut truncated);

        if truncated {
            break;
        }
    }

    (matches, truncated)
}

fn matches_include(
    file_path: &Path,
    root_path: &Path,
    include_filter: Option<&IncludeFilter>,
) -> bool {
    let Some(include_filter) = include_filter else {
        return true;
    };

    let relative_path = file_path.strip_prefix(root_path).unwrap_or(file_path);
    let normalized_relative = normalize_path(relative_path);

    if include_filter.has_separator {
        include_filter.pattern.matches(&normalized_relative)
    } else {
        let file_name = file_path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or_default();

        include_filter.pattern.matches(file_name)
            || include_filter.pattern.matches(&normalized_relative)
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn collect_matches_from_file(
    file_path: &Path,
    regex: &Regex,
    matches: &mut Vec<SearchMatch>,
    truncated: &mut bool,
) {
    let bytes = match std::fs::read(file_path) {
        Ok(bytes) => bytes,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            return;
        }
        Err(error) => {
            tracing::debug!(
                file = %file_path.display(),
                error = %error,
                "Skipping file that could not be read during Search"
            );
            return;
        }
    };

    if is_binary_content(&bytes) {
        return;
    }

    let Ok(content) = std::str::from_utf8(&bytes) else {
        return;
    };

    for (line_index, line) in content.lines().enumerate() {
        if regex.is_match(line) {
            if matches.len() < MAX_MATCHES {
                matches.push(SearchMatch {
                    file_path: file_path.display().to_string(),
                    line_number: line_index + 1,
                    line_content: line.to_string(),
                });
            } else {
                *truncated = true;
                break;
            }
        }
    }
}

fn is_binary_content(content: &[u8]) -> bool {
    let bytes_to_check = content.len().min(BINARY_CHECK_BYTES);
    content[..bytes_to_check].contains(&0)
}

fn format_results(pattern: &str, matches: &[SearchMatch], truncated: bool) -> String {
    use std::collections::BTreeMap;

    if matches.is_empty() {
        return format!("No matches found for pattern \"{pattern}\"");
    }

    let mut grouped_matches: BTreeMap<&str, Vec<&SearchMatch>> = BTreeMap::new();
    for search_match in matches {
        grouped_matches
            .entry(search_match.file_path.as_str())
            .or_default()
            .push(search_match);
    }

    let mut output = String::new();
    for (index, (file_path, file_matches)) in grouped_matches.into_iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }

        output.push_str("File: ");
        output.push_str(file_path);

        for file_match in file_matches {
            output.push('\n');
            output.push('L');
            output.push_str(&file_match.line_number.to_string());
            output.push_str(": ");
            output.push_str(&file_match.line_content);
        }

        output.push('\n');
        output.push_str("---");
    }

    if truncated {
        output.push_str("\n[Results truncated at 200 matches]");
    }

    output
}

/// Get the `Search` tool definition.
#[must_use]
pub fn get_search_tool_definition() -> ToolDefinition {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Regex pattern to search for"
            },
            "path": {
                "type": "string",
                "description": "Optional file or directory path to search in (defaults to current working directory)"
            },
            "include": {
                "type": "string",
                "description": "Optional glob filter for file paths, for example '*.rs' or 'src/**/*.md'"
            }
        },
        "required": ["pattern"]
    });

    ToolDefinition::new(
        "Search",
        "Search file contents by regex pattern. Searches recursively under a directory (or a specific file), returns matching lines with file paths and line numbers, respects .gitignore, and truncates at 200 matches.",
    )
    .with_parameters(input_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_approval_policy::ToolApprovalPolicy;
    use std::io::Write;
    use tempfile::tempdir;

    fn yolo_context() -> RunContext<McpToolContext> {
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(8);
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

    #[test]
    fn parse_search_args_defaults_path_to_dot() {
        let parsed = parse_search_args(&serde_json::json!({
            "pattern": "needle",
            "path": ""
        }))
        .expect("args should parse");

        assert_eq!(parsed.pattern, "needle");
        assert_eq!(parsed.path, ".");
        assert!(parsed.include.is_none());
    }

    #[test]
    fn parse_search_args_rejects_non_string_fields() {
        let pattern_error = parse_search_args(&serde_json::json!({
            "pattern": 42
        }))
        .expect_err("non-string pattern should fail");
        assert!(pattern_error.contains("'pattern' must be a string"));

        let include_error = parse_search_args(&serde_json::json!({
            "pattern": "needle",
            "include": 42
        }))
        .expect_err("non-string include should fail");
        assert!(include_error.contains("'include' must be a string"));
    }

    #[test]
    fn parse_ripgrep_match_handles_match_and_non_match_events() {
        let match_line = serde_json::json!({
            "type": "match",
            "data": {
                "path": { "text": "src/main.rs" },
                "line_number": 7,
                "lines": { "text": "fn main() {\n" }
            }
        })
        .to_string();

        let parsed = parse_ripgrep_match(match_line.as_bytes()).expect("match line should parse");
        assert_eq!(parsed.file_path, "src/main.rs");
        assert_eq!(parsed.line_number, 7);
        assert_eq!(parsed.line_content, "fn main() {");

        let begin_line = serde_json::json!({
            "type": "begin",
            "data": {}
        })
        .to_string();
        assert!(parse_ripgrep_match(begin_line.as_bytes()).is_none());
    }

    #[test]
    fn compile_include_filter_rejects_invalid_pattern() {
        let result = compile_include_filter(Some("["));
        assert!(result.is_err());
        let error = result.expect_err("invalid glob should fail");
        assert!(error.contains("Invalid include glob pattern"));
    }

    #[test]
    fn search_with_builtin_returns_not_found_error_for_missing_path() {
        let dir = tempdir().expect("temp dir should be created");
        let missing_path = dir.path().join("does-not-exist");

        let result =
            search_with_builtin("needle", missing_path.to_str().expect("utf-8 path"), None);
        assert!(result.is_err());
        let error = result.expect_err("missing path should fail");
        assert!(error.contains("Search path not found"));
    }

    #[test]
    fn search_with_builtin_file_path_respects_include_filter() {
        let dir = tempdir().expect("temp dir should be created");
        let file_path = dir.path().join("match.txt");
        std::fs::write(&file_path, "needle\n").expect("file write should succeed");

        let (matches, truncated) = search_with_builtin(
            "needle",
            file_path.to_str().expect("utf-8 path"),
            Some("*.rs"),
        )
        .expect("search should succeed");

        assert!(!truncated);
        assert!(matches.is_empty());
    }

    #[test]
    fn format_results_appends_truncation_notice() {
        let matches = vec![SearchMatch {
            file_path: "src/main.rs".to_string(),
            line_number: 42,
            line_content: "needle here".to_string(),
        }];

        let output = format_results("needle", &matches, true);
        assert!(output.contains("File: src/main.rs"));
        assert!(output.contains("L42: needle here"));
        assert!(output.contains("[Results truncated at 200 matches]"));
    }

    #[test]
    fn normalize_path_replaces_backslashes() {
        let normalized = normalize_path(Path::new(r"dir\file.txt"));
        assert_eq!(normalized, "dir/file.txt");
    }

    #[test]
    fn get_search_tool_definition_returns_valid_schema() {
        let definition = get_search_tool_definition();
        assert_eq!(definition.name, "Search");
        assert!(definition.parameters().is_object());
    }

    #[tokio::test]
    async fn execute_search_requires_pattern() {
        let executor = SearchExecutor;
        let run_ctx = yolo_context();

        let result = executor.execute(serde_json::json!({}), &run_ctx).await;
        assert!(result.is_err());
        let error = result.expect_err("missing pattern should fail");
        assert!(error
            .to_string()
            .contains("Missing required 'pattern' argument"));
    }

    #[test]
    fn search_with_builtin_returns_invalid_regex_error() {
        let result = search_with_builtin("(", ".", None);
        assert!(result.is_err());
        let error = result.expect_err("invalid regex should fail");

        assert!(error.contains("Invalid regex pattern"));
    }

    #[test]
    fn search_with_builtin_finds_matches() {
        let dir = tempdir().expect("temp dir should be created");
        let file_path = dir.path().join("sample.rs");

        let mut file = std::fs::File::create(&file_path).expect("file should be created");
        writeln!(file, "fn helper() {{}}\nfn main() {{}}\n").expect("file write should succeed");

        let (matches, truncated) = search_with_builtin(
            "fn\\s+main",
            dir.path().to_str().expect("utf-8 path"),
            Some("*.rs"),
        )
        .expect("search should succeed");

        assert!(!truncated);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line_number, 2);
        assert!(matches[0].line_content.contains("fn main()"));
    }

    #[test]
    fn search_with_builtin_respects_include_filter() {
        let dir = tempdir().expect("temp dir should be created");
        let rs_file = dir.path().join("match.rs");
        let txt_file = dir.path().join("match.txt");

        std::fs::write(&rs_file, "needle\n").expect("rs file write should succeed");
        std::fs::write(&txt_file, "needle\n").expect("txt file write should succeed");

        let output = search_with_builtin(
            "needle",
            dir.path().to_str().expect("utf-8 path"),
            Some("*.rs"),
        )
        .expect("search should succeed");

        assert_eq!(output.0.len(), 1);
        assert!(output.0[0].file_path.ends_with("match.rs"));
    }

    #[test]
    fn search_with_builtin_truncates_results() {
        let dir = tempdir().expect("temp dir should be created");
        let file_path = dir.path().join("many.txt");

        let mut content = String::new();
        for _ in 0..250 {
            content.push_str("needle\n");
        }
        std::fs::write(&file_path, content).expect("file write should succeed");

        let (matches, truncated) =
            search_with_builtin("needle", dir.path().to_str().expect("utf-8 path"), None)
                .expect("search should succeed");

        assert_eq!(matches.len(), MAX_MATCHES);
        assert!(truncated);
    }

    #[test]
    fn format_results_groups_matches_by_file() {
        let matches = vec![
            SearchMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 10,
                line_content: "fn main() {".to_string(),
            },
            SearchMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 15,
                line_content: "fn helper() {".to_string(),
            },
            SearchMatch {
                file_path: "src/lib.rs".to_string(),
                line_number: 5,
                line_content: "pub fn api() {".to_string(),
            },
        ];

        let output = format_results("fn", &matches, false);

        let expected = [
            "File: src/lib.rs",
            "L5: pub fn api() {",
            "---",
            "File: src/main.rs",
            "L10: fn main() {",
            "L15: fn helper() {",
            "---",
        ]
        .join("\n");

        assert_eq!(output, expected);
    }

    #[test]
    fn format_results_handles_no_matches() {
        let output = format_results("missing", &[], false);
        assert_eq!(output, "No matches found for pattern \"missing\"");
    }

    #[tokio::test]
    async fn search_requires_approval_when_not_yolo() {
        let dir = tempdir().expect("temp dir should be created");
        let file_path = dir.path().join("sample.txt");
        std::fs::write(&file_path, "needle\n").expect("file write should succeed");

        let path = dir
            .path()
            .to_str()
            .expect("temp dir path should be utf-8")
            .to_string();

        let executor = SearchExecutor;
        let args = serde_json::json!({ "pattern": "needle", "path": path.clone() });

        let (view_tx, mut view_rx) = tokio::sync::mpsc::channel(10);
        let approval_gate = std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new());
        let policy = std::sync::Arc::new(tokio::sync::Mutex::new(ToolApprovalPolicy::default()));
        let run_ctx = RunContext::new(
            McpToolContext {
                view_tx,
                approval_gate: approval_gate.clone(),
                policy,
                ..Default::default()
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
                assert_eq!(context.tool_name, "Search");
                assert_eq!(context.category, ToolCategory::Search);
                assert_eq!(context.primary_target, path);
                request_id
            }
            other => panic!("expected ToolApprovalRequest, got {other:?}"),
        };

        let _ = approval_gate.resolve(&request_id, true);

        let result = handle.await.expect("task should complete");
        assert!(result.is_ok(), "approval should allow search execution");
    }
}
