use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use url::Url;

/// Stream lifecycle state captured when an error was logged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorLogStreamLifecycle {
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for ErrorLogStreamLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Run terminal state captured for stream/tool diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorLogRunStatus {
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

impl std::fmt::Display for ErrorLogRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Sanitized context for a tool call related to an error.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorLogToolContext {
    pub tool_name: String,
    pub tool_call_id: Option<String>,
    pub success: Option<bool>,
    pub summary: Option<String>,
}

/// Structured diagnostic fields kept separate from the compact display message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorLogDiagnosticContext {
    pub underlying_error: Option<String>,
    pub subsystem: Option<String>,
    pub code_path: Option<String>,
    pub conversation_id: Option<uuid::Uuid>,
    pub profile_id: Option<uuid::Uuid>,
    pub profile_name: Option<String>,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub base_url_host: Option<String>,
    pub run_status: Option<ErrorLogRunStatus>,
    pub stream_lifecycle: Option<ErrorLogStreamLifecycle>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub partial_assistant_response_len: Option<usize>,
    pub thinking_len: Option<usize>,
    pub tool_calls: Vec<ErrorLogToolContext>,
    pub recent_events: Vec<String>,
    pub persisted_message_ids: Vec<uuid::Uuid>,
    pub sequence_numbers: Vec<i64>,
}

impl ErrorLogDiagnosticContext {
    #[must_use]
    pub fn sanitized(&self) -> Self {
        Self {
            underlying_error: sanitize_optional(self.underlying_error.as_deref()),
            subsystem: sanitize_optional(self.subsystem.as_deref()),
            code_path: sanitize_optional(self.code_path.as_deref()),
            conversation_id: self.conversation_id,
            profile_id: self.profile_id,
            profile_name: sanitize_optional(self.profile_name.as_deref()),
            provider_id: sanitize_optional(self.provider_id.as_deref()),
            model_id: sanitize_optional(self.model_id.as_deref()),
            base_url_host: sanitize_optional(self.base_url_host.as_deref()),
            run_status: self.run_status.clone(),
            stream_lifecycle: self.stream_lifecycle.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            total_tokens: self.total_tokens,
            partial_assistant_response_len: self.partial_assistant_response_len,
            thinking_len: self.thinking_len,
            tool_calls: self
                .tool_calls
                .iter()
                .map(|tool| ErrorLogToolContext {
                    tool_name: sanitize_text(&tool.tool_name),
                    tool_call_id: sanitize_optional(tool.tool_call_id.as_deref()),
                    success: tool.success,
                    summary: sanitize_optional(tool.summary.as_deref()),
                })
                .collect(),
            recent_events: self
                .recent_events
                .iter()
                .map(|event| sanitize_text(event))
                .collect(),
            persisted_message_ids: self.persisted_message_ids.clone(),
            sequence_numbers: self.sequence_numbers.clone(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

pub(crate) fn sanitize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(sanitize_text)
        .filter(|value| !value.trim().is_empty())
}

/// Redact common secrets from diagnostic text before display/export.
#[must_use]
pub fn sanitize_text(value: &str) -> String {
    let mut sanitized = value.to_string();
    let patterns = [
        r#"(?i)(api[_-]?key\s*[=:]\s*)[^\s,;&\"]+"#,
        r#"(?i)(access[_-]?token\s*[=:]\s*)[^\s,;&\"]+"#,
        r#"(?i)(refresh[_-]?token\s*[=:]\s*)[^\s,;&\"]+"#,
        r#"(?i)(\btoken\s*[=:]\s*)[^\s,;&\"]+"#,
        r#"(?i)(secret\s*[=:]\s*)[^\s,;&\"]+"#,
        r#"(?i)(password\s*[=:]\s*)[^\s,;&\"]+"#,
        r"(?i)(authorization\s*:\s*)[^\r\n]+",
    ];

    for pattern in patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            sanitized = regex.replace_all(&sanitized, "${1}[REDACTED]").into_owned();
        }
    }

    sanitized
}

/// Return only the host component from a base URL.
#[must_use]
pub fn base_url_host(base_url: &str) -> Option<String> {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
}

pub(crate) fn write_diagnostic_context_text(
    output: &mut String,
    diagnostics: &ErrorLogDiagnosticContext,
) {
    let _ = writeln!(output, "Diagnostics:");
    write_diagnostic_identity_text(output, diagnostics);
    write_diagnostic_status_text(output, diagnostics);
    write_diagnostic_tool_context_text(output, diagnostics);
    write_diagnostic_events_text(output, diagnostics);
    write_diagnostic_persistence_text(output, diagnostics);
}

fn write_diagnostic_identity_text(output: &mut String, diagnostics: &ErrorLogDiagnosticContext) {
    write_optional_line(
        output,
        "Underlying error",
        diagnostics.underlying_error.as_deref(),
    );
    write_optional_line(output, "Subsystem", diagnostics.subsystem.as_deref());
    write_optional_line(output, "Code path", diagnostics.code_path.as_deref());
    write_optional_line(
        output,
        "Conversation id",
        diagnostics
            .conversation_id
            .map(|id| id.to_string())
            .as_deref(),
    );
    write_optional_line(
        output,
        "Profile id",
        diagnostics.profile_id.map(|id| id.to_string()).as_deref(),
    );
    write_optional_line(output, "Profile name", diagnostics.profile_name.as_deref());
    write_optional_line(output, "Provider id", diagnostics.provider_id.as_deref());
    write_optional_line(output, "Model id", diagnostics.model_id.as_deref());
    write_optional_line(
        output,
        "Base URL host",
        diagnostics.base_url_host.as_deref(),
    );
}

fn write_diagnostic_status_text(output: &mut String, diagnostics: &ErrorLogDiagnosticContext) {
    write_optional_line(
        output,
        "Run status",
        diagnostics
            .run_status
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
    );
    write_optional_line(
        output,
        "Stream lifecycle",
        diagnostics
            .stream_lifecycle
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
    );
    write_optional_line(
        output,
        "Input tokens",
        diagnostics.input_tokens.map(|v| v.to_string()).as_deref(),
    );
    write_optional_line(
        output,
        "Output tokens",
        diagnostics.output_tokens.map(|v| v.to_string()).as_deref(),
    );
    write_optional_line(
        output,
        "Total tokens",
        diagnostics.total_tokens.map(|v| v.to_string()).as_deref(),
    );
    write_optional_line(
        output,
        "Partial assistant response length",
        diagnostics
            .partial_assistant_response_len
            .map(|v| v.to_string())
            .as_deref(),
    );
    write_optional_line(
        output,
        "Thinking length",
        diagnostics.thinking_len.map(|v| v.to_string()).as_deref(),
    );
}

fn write_diagnostic_tool_context_text(
    output: &mut String,
    diagnostics: &ErrorLogDiagnosticContext,
) {
    if diagnostics.tool_calls.is_empty() {
        return;
    }

    let _ = writeln!(output, "Tool calls:");
    for tool in &diagnostics.tool_calls {
        let _ = writeln!(
            output,
            "- {} ({}) success={} summary={}",
            tool.tool_name,
            tool.tool_call_id.as_deref().unwrap_or("unknown"),
            tool.success
                .map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            tool.summary.as_deref().unwrap_or("")
        );
    }
}

fn write_diagnostic_events_text(output: &mut String, diagnostics: &ErrorLogDiagnosticContext) {
    if diagnostics.recent_events.is_empty() {
        return;
    }

    let _ = writeln!(output, "Recent events:");
    for event in &diagnostics.recent_events {
        let _ = writeln!(output, "- {event}");
    }
}

fn write_diagnostic_persistence_text(output: &mut String, diagnostics: &ErrorLogDiagnosticContext) {
    if !diagnostics.persisted_message_ids.is_empty() {
        let ids = diagnostics
            .persisted_message_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(output, "Persisted message ids: {ids}");
    }

    if !diagnostics.sequence_numbers.is_empty() {
        let ids = diagnostics
            .sequence_numbers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(output, "Sequence numbers: {ids}");
    }
}

fn write_optional_line(output: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            let _ = writeln!(output, "{label}: {value}");
        }
    }
}
