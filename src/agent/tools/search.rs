//! `Search` tool implementation.
//!
//! This module provides a built-in `SearchExecutor` that searches file contents
//! using regex across a directory tree. It prefers ripgrep when available and
//! falls back to a built-in recursive search implementation.

use crate::llm::client_agent::McpToolContext;
use glob::Pattern;
use ignore::WalkBuilder;
use regex::Regex;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Maximum number of matches returned by the tool.
const MAX_MATCHES: usize = 200;
/// Number of bytes sampled for binary-file detection.
const BINARY_CHECK_BYTES: usize = 8_192;

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
        _ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        execute_search(args)
            .await
            .map(ToolReturn::text)
            .map_err(ToolError::execution_failed)
    }
}

async fn execute_search(args: serde_json::Value) -> Result<String, String> {
    let search_args = parse_search_args(&args)?;

    let result = match search_with_ripgrep(
        &search_args.pattern,
        &search_args.path,
        search_args.include.as_deref(),
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(RipgrepError::NotFound) => search_with_builtin(
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

    let output = command.output().await.map_err(|error| {
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
        search_single_file(&root_path, &regex, include_filter.as_ref())
    } else if metadata.is_dir() {
        search_directory(&root_path, &regex, include_filter.as_ref())
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
) -> Result<(Vec<SearchMatch>, bool), String> {
    let mut matches = Vec::new();
    let mut truncated = false;

    let root = file_path.parent().unwrap_or_else(|| Path::new("."));
    if !matches_include(file_path, root, include_filter) {
        return Ok((matches, truncated));
    }

    collect_matches_from_file(file_path, regex, &mut matches, &mut truncated)?;
    Ok((matches, truncated))
}

fn search_directory(
    root_path: &Path,
    regex: &Regex,
    include_filter: Option<&IncludeFilter>,
) -> Result<(Vec<SearchMatch>, bool), String> {
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

        collect_matches_from_file(file_path, regex, &mut matches, &mut truncated)?;

        if truncated {
            break;
        }
    }

    Ok((matches, truncated))
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
) -> Result<(), String> {
    let bytes = match std::fs::read(file_path) {
        Ok(bytes) => bytes,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            return Ok(());
        }
        Err(error) => {
            return Err(format!(
                "Failed to read file '{}': {error}",
                file_path.display()
            ));
        }
    };

    if is_binary_content(&bytes) {
        return Ok(());
    }

    let Ok(content) = std::str::from_utf8(&bytes) else {
        return Ok(());
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

    Ok(())
}

fn is_binary_content(content: &[u8]) -> bool {
    let bytes_to_check = content.len().min(BINARY_CHECK_BYTES);
    content[..bytes_to_check].contains(&0)
}

fn format_results(pattern: &str, matches: &[SearchMatch], truncated: bool) -> String {
    if matches.is_empty() {
        return format!("No matches found for pattern \"{pattern}\"");
    }

    let mut output = String::new();
    for (index, search_match) in matches.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }

        output.push_str("File: ");
        output.push_str(&search_match.file_path);
        output.push('\n');
        output.push('L');
        output.push_str(&search_match.line_number.to_string());
        output.push_str(": ");
        output.push_str(&search_match.line_content);
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
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn get_search_tool_definition_returns_valid_schema() {
        let definition = get_search_tool_definition();
        assert_eq!(definition.name, "Search");
        assert!(definition.parameters().is_object());
    }

    #[tokio::test]
    async fn execute_search_requires_pattern() {
        let result = execute_search(serde_json::json!({})).await;
        assert!(result.is_err());
        let error = result.expect_err("missing pattern should fail");
        assert!(error.contains("Missing required 'pattern' argument"));
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
    fn format_results_handles_no_matches() {
        let output = format_results("missing", &[], false);
        assert_eq!(output, "No matches found for pattern \"missing\"");
    }
}
