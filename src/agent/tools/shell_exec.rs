//! `ShellExec` tool implementation.
//!
//! This module provides a built-in `ShellExecExecutor` that runs shell commands
//! with timeout handling and structured output formatting.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;

use crate::agent::{ToolApprovalDecision, ToolApprovalPolicy};
use crate::llm::client_agent::McpToolContext;
use crate::presentation::view_command::ViewCommand;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};

const DEFAULT_TIMEOUT_SECS: u32 = 300;
const MAX_TIMEOUT_SECS: u32 = 900;

type OutputReaderTask = tokio::task::JoinHandle<Vec<u8>>;
type OutputReaderTasks = (OutputReaderTask, OutputReaderTask);

#[derive(Debug, Clone)]
struct ShellExecParams {
    command: String,
    working_dir: Option<String>,
    timeout_secs: u32,
}

#[derive(Debug, Clone)]
struct ShellExecResult {
    command: String,
    directory: String,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timed_out: bool,
}

impl ShellExecResult {
    fn format_for_agent(&self) -> String {
        let stdout = if self.stdout.trim().is_empty() {
            "(empty)".to_string()
        } else {
            self.stdout.clone()
        };

        let mut stderr_value = if self.stderr.trim().is_empty() {
            "(empty)".to_string()
        } else {
            self.stderr.clone()
        };

        if self.timed_out {
            if stderr_value == "(empty)" {
                stderr_value = "Command timed out before completion".to_string();
            } else {
                stderr_value.push('\n');
                stderr_value.push_str("Command timed out before completion");
            }
        }

        let exit_code = self
            .exit_code
            .map_or_else(|| "(none)".to_string(), |code| code.to_string());

        format!(
            "Command: {}\nDirectory: {}\nStdout: {}\nStderr: {}\nExit Code: {}",
            self.command, self.directory, stdout, stderr_value, exit_code
        )
    }
}

/// Executor for the `ShellExec` built-in tool.
#[derive(Debug, Clone, Copy)]
pub struct ShellExecExecutor;

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for ShellExecExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let params = parse_params(&args)?;

        check_approval(ctx.deps(), &params.command).await?;

        let result = execute_shell_command(&params).await?;
        Ok(ToolReturn::text(result.format_for_agent()))
    }
}

fn parse_params(args: &serde_json::Value) -> Result<ShellExecParams, ToolError> {
    let command = args
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ToolError::execution_failed("Missing required 'command' argument"))?
        .to_string();

    let working_dir = args
        .get("working_dir")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let timeout_secs_u32 = match args.get("timeout_secs") {
        None => DEFAULT_TIMEOUT_SECS,
        Some(value) => {
            let timeout_secs_raw = value.as_u64().ok_or_else(|| {
                ToolError::execution_failed(format!(
                    "Invalid timeout_secs: {value} (must be between 1 and {MAX_TIMEOUT_SECS})"
                ))
            })?;

            let timeout_secs_u32 = u32::try_from(timeout_secs_raw).map_err(|_| {
                ToolError::execution_failed(format!(
                    "Invalid timeout_secs: {timeout_secs_raw} (must be between 1 and {MAX_TIMEOUT_SECS})"
                ))
            })?;

            if !(1..=MAX_TIMEOUT_SECS).contains(&timeout_secs_u32) {
                return Err(ToolError::execution_failed(format!(
                    "Invalid timeout_secs: {timeout_secs_u32} (must be between 1 and {MAX_TIMEOUT_SECS})"
                )));
            }

            timeout_secs_u32
        }
    };

    Ok(ShellExecParams {
        command,
        working_dir,
        timeout_secs: timeout_secs_u32,
    })
}

/// Check tool approval policy and await user decision if required.
async fn check_approval(tool_context: &McpToolContext, command: &str) -> Result<(), ToolError> {
    let (decision, identifier) = {
        let policy = tool_context.policy.lock().await;
        (
            policy.evaluate_compound_command(command),
            ToolApprovalPolicy::extract_shell_identifier(command),
        )
    };

    let tool_identifier = if identifier.is_empty() {
        "ShellExec".to_string()
    } else {
        identifier
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
                .wait_for_approval(request_id.clone(), tool_identifier.clone());

            if tool_context
                .view_tx
                .send(ViewCommand::ToolApprovalRequest {
                    request_id: request_id.clone(),
                    tool_name: "ShellExec".to_string(),
                    tool_argument: command.to_string(),
                })
                .await
                .is_err()
            {
                let _ = tool_context.approval_gate.resolve(&request_id, false);
                return Err(ToolError::execution_failed(
                    "Failed to send approval request to UI (channel closed)",
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

async fn execute_shell_command(params: &ShellExecParams) -> Result<ShellExecResult, ToolError> {
    let resolved_directory = resolve_working_directory(params.working_dir.as_deref())?;
    let mut child = spawn_shell_child(params, &resolved_directory)?;
    let (stdout_task, stderr_task) = spawn_output_readers(&mut child, &params.command)?;
    let (status, timed_out) =
        wait_for_child_with_timeout(&mut child, params.timeout_secs, &params.command).await?;
    let (stdout, stderr) = collect_output(stdout_task, stderr_task, &params.command).await?;

    Ok(ShellExecResult {
        command: params.command.clone(),
        directory: resolved_directory.display().to_string(),
        stdout,
        stderr,
        exit_code: status.code(),
        timed_out,
    })
}

fn spawn_shell_child(
    params: &ShellExecParams,
    resolved_directory: &Path,
) -> Result<tokio::process::Child, ToolError> {
    let (shell, shell_flag) = shell_program_and_flag();
    let mut command = tokio::process::Command::new(shell);
    command
        .arg(shell_flag)
        .arg(&params.command)
        .current_dir(resolved_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command.spawn().map_err(|error| {
        ToolError::execution_failed(format!(
            "Failed to execute command '{}' in '{}': {}",
            params.command,
            resolved_directory.display(),
            error
        ))
    })
}

fn spawn_output_readers(
    child: &mut tokio::process::Child,
    command: &str,
) -> Result<OutputReaderTasks, ToolError> {
    let mut stdout_pipe = child.stdout.take().ok_or_else(|| {
        ToolError::execution_failed(format!("Failed to capture stdout for command '{command}'"))
    })?;
    let mut stderr_pipe = child.stderr.take().ok_or_else(|| {
        ToolError::execution_failed(format!("Failed to capture stderr for command '{command}'"))
    })?;

    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buffer).await;
        buffer
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buffer).await;
        buffer
    });

    Ok((stdout_task, stderr_task))
}

async fn wait_for_child_with_timeout(
    child: &mut tokio::process::Child,
    timeout_secs: u32,
    command: &str,
) -> Result<(std::process::ExitStatus, bool), ToolError> {
    let wait_result =
        tokio::time::timeout(Duration::from_secs(u64::from(timeout_secs)), child.wait()).await;

    match wait_result {
        Ok(status_result) => status_result
            .map(|status| (status, false))
            .map_err(|error| {
                ToolError::execution_failed(format!(
                    "Failed to wait for command '{command}': {error}"
                ))
            }),
        Err(_elapsed) => {
            if let Err(error) = child.kill().await {
                tracing::warn!("failed to kill timed-out shell process: {}", error);
            }

            child
                .wait()
                .await
                .map(|status| (status, true))
                .map_err(|error| {
                    ToolError::execution_failed(format!(
                        "Failed to wait for timed-out command '{command}': {error}"
                    ))
                })
        }
    }
}

async fn collect_output(
    stdout_task: OutputReaderTask,
    stderr_task: OutputReaderTask,
    command: &str,
) -> Result<(String, String), ToolError> {
    let stdout = stdout_task.await.map_err(|error| {
        ToolError::execution_failed(format!("Failed to collect stdout for '{command}': {error}"))
    })?;
    let stderr = stderr_task.await.map_err(|error| {
        ToolError::execution_failed(format!("Failed to collect stderr for '{command}': {error}"))
    })?;

    Ok((
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    ))
}

fn resolve_working_directory(working_dir: Option<&str>) -> Result<PathBuf, ToolError> {
    match working_dir {
        Some(path) => {
            let candidate = Path::new(path);
            let directory = if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|error| {
                        ToolError::execution_failed(format!(
                            "Failed to determine current directory for working_dir '{path}': {error}",
                        ))
                    })?
                    .join(candidate)
            };

            let metadata = std::fs::metadata(&directory).map_err(|error| {
                ToolError::execution_failed(format!(
                    "Invalid working_dir '{}': {}",
                    directory.display(),
                    error
                ))
            })?;

            if !metadata.is_dir() {
                return Err(ToolError::execution_failed(format!(
                    "Invalid working_dir '{}': not a directory",
                    directory.display()
                )));
            }

            Ok(directory)
        }
        None => std::env::current_dir().map_err(|error| {
            ToolError::execution_failed(format!("Failed to determine current directory: {error}"))
        }),
    }
}

const fn shell_program_and_flag() -> (&'static str, &'static str) {
    if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("bash", "-c")
    }
}

/// Get the `ShellExec` tool definition.
#[must_use]
pub fn get_shell_exec_tool_definition() -> ToolDefinition {
    let input_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "The shell command to execute"
            },
            "working_dir": {
                "type": "string",
                "description": "Optional working directory for command execution"
            },
            "timeout_secs": {
                "type": "integer",
                "description": "Optional timeout in seconds (default 300, max 900)",
                "minimum": 1,
                "maximum": MAX_TIMEOUT_SECS
            }
        },
        "required": ["command"]
    });

    ToolDefinition::new(
        "ShellExec",
        "Execute a shell command and return structured stdout/stderr/exit-code output.",
    )
    .with_parameters(input_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_approval_policy::ToolApprovalPolicy;

    fn make_context_with_policy(policy: ToolApprovalPolicy) -> McpToolContext {
        let (view_tx, _view_rx) = tokio::sync::mpsc::channel(8);
        McpToolContext {
            view_tx,
            approval_gate: std::sync::Arc::new(crate::llm::client_agent::ApprovalGate::new()),
            policy: std::sync::Arc::new(tokio::sync::Mutex::new(policy)),
        }
    }

    #[test]
    fn parse_params_defaults_timeout() {
        let params =
            parse_params(&serde_json::json!({"command": "echo hi"})).expect("params should parse");
        assert_eq!(params.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn parse_params_rejects_out_of_range_timeout() {
        let error = parse_params(&serde_json::json!({
            "command": "echo hi",
            "timeout_secs": 901
        }))
        .expect_err("timeout greater than max should fail");

        assert!(error.to_string().contains("Invalid timeout_secs"));
    }

    #[test]
    fn parse_params_rejects_malformed_timeout_values() {
        for value in [
            serde_json::json!("5"),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!(true),
            serde_json::json!({"secs": 5}),
        ] {
            let error = parse_params(&serde_json::json!({
                "command": "echo hi",
                "timeout_secs": value,
            }))
            .expect_err("malformed timeout should fail");

            assert!(error.to_string().contains("Invalid timeout_secs"));
        }
    }

    #[test]
    fn parse_params_rejects_null_timeout() {
        let error = parse_params(&serde_json::json!({
            "command": "echo hi",
            "timeout_secs": null,
        }))
        .expect_err("null timeout should fail");

        assert!(error.to_string().contains("Invalid timeout_secs"));
    }

    #[test]
    fn result_formatter_matches_issue_shape() {
        let result = ShellExecResult {
            command: "echo hi".to_string(),
            directory: "/tmp".to_string(),
            stdout: "hi\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            timed_out: false,
        };

        let formatted = result.format_for_agent();
        assert!(formatted.contains("Command: echo hi"));
        assert!(formatted.contains("Directory: /tmp"));
        assert!(formatted.contains("Stdout: hi"));
        assert!(formatted.contains("Stderr: (empty)"));
        assert!(formatted.contains("Exit Code: 0"));
    }

    #[tokio::test]
    async fn shell_exec_allows_command_when_policy_allows_identifier() {
        let policy = ToolApprovalPolicy {
            persistent_allowlist: vec!["echo".to_string()],
            ..ToolApprovalPolicy::default()
        };
        let context = make_context_with_policy(policy);

        let executor = ShellExecExecutor;
        let args = serde_json::json!({"command": "echo shell-exec-test"});
        let run_ctx = RunContext::new(context, "test-model");

        let output = executor
            .execute(args, &run_ctx)
            .await
            .expect("execution should succeed");

        let serdes_ai::core::messages::ToolReturnContent::Text { content: text } = output.content
        else {
            panic!("expected text content");
        };
        assert!(text.contains("Command: echo shell-exec-test"));
        assert!(text.contains("Exit Code: 0"));
    }

    #[tokio::test]
    async fn shell_exec_denies_command_when_policy_denies_compound_segment() {
        let policy = ToolApprovalPolicy {
            persistent_allowlist: vec!["ls".to_string()],
            persistent_denylist: vec!["rm".to_string()],
            ..ToolApprovalPolicy::default()
        };
        let context = make_context_with_policy(policy);

        let executor = ShellExecExecutor;
        let args = serde_json::json!({"command": "ls && rm -rf /tmp/never"});
        let run_ctx = RunContext::new(context, "test-model");

        let error = executor
            .execute(args, &run_ctx)
            .await
            .expect_err("execution should be denied");

        assert!(error.to_string().contains("denied by policy"));
    }

    #[tokio::test]
    async fn shell_exec_timeout_reports_timed_out_notice() {
        let policy = ToolApprovalPolicy {
            yolo_mode: true,
            ..ToolApprovalPolicy::default()
        };
        let context = make_context_with_policy(policy);

        let executor = ShellExecExecutor;
        let args = serde_json::json!({"command": "sleep 2", "timeout_secs": 1});
        let run_ctx = RunContext::new(context, "test-model");

        let output = executor
            .execute(args, &run_ctx)
            .await
            .expect("execution should return timeout result");

        let serdes_ai::core::messages::ToolReturnContent::Text { content: text } = output.content
        else {
            panic!("expected text content");
        };

        assert!(text.contains("Command timed out before completion"));
    }

    #[test]
    fn shell_exec_tool_definition_is_valid() {
        let def = get_shell_exec_tool_definition();
        assert_eq!(def.name, "ShellExec");
        assert!(!def.description.is_empty());
        assert!(def.parameters().is_object());
    }
}
