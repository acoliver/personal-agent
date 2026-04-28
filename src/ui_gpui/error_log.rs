//! Global error log ring buffer for capturing and displaying runtime errors.
//!
//! Provides a thread-safe, in-memory store of the last [`ErrorLogStore::MAX_ENTRIES`]
//! errors, with unviewed-count tracking for the title-bar badge.
//!
//! # Usage
//!
//! Push errors via `ErrorLogStore::global().push(|id| ErrorLogEntry { id, .. })`.
//! Read entries via `ErrorLogStore::global().entries()` (newest-first).
//! Clear unviewed badge via `ErrorLogStore::global().mark_all_viewed()`.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use url::Url;

/// Severity classification for a logged error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverityTag {
    /// Error occurred during LLM response streaming.
    Stream,
    /// Authentication or authorisation failure (HTTP 401/403).
    Auth,
    /// Network connection or timeout error.
    Connection,
    /// Error from an MCP tool call or server.
    Mcp,
    /// Internal application error.
    Internal,
}

impl std::fmt::Display for ErrorSeverityTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stream => write!(f, "STREAM"),
            Self::Auth => write!(f, "AUTH"),
            Self::Connection => write!(f, "CONN"),
            Self::Mcp => write!(f, "MCP"),
            Self::Internal => write!(f, "INTERNAL"),
        }
    }
}
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

/// A single entry in the error log ring buffer.
#[derive(Clone, Debug)]
pub struct ErrorLogEntry {
    /// Monotonically increasing identifier assigned at push time.
    pub id: u64,
    /// UTC timestamp of when the error was captured.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Broad category of the error.
    pub severity: ErrorSeverityTag,
    /// Human-readable source label, e.g. `"anthropic / claude-sonnet-4-20250514"`.
    pub source: String,
    /// Human-readable error message.
    pub message: String,
    /// Optional raw HTTP body or underlying error detail.
    pub raw_detail: Option<String>,
    /// Title of the conversation in which the error occurred, if known.
    pub conversation_title: Option<String>,
    /// Structured diagnostic context for drilldown/export.
    pub diagnostics: Option<ErrorLogDiagnosticContext>,

    /// UUID of the conversation in which the error occurred, if known.
    pub conversation_id: Option<uuid::Uuid>,
}

/// Thread-safe, ring-buffer error log store.
///
/// Holds at most [`ErrorLogStore::MAX_ENTRIES`] entries (newest first).
/// Cheap atomic reads let the title-bar badge poll `unviewed_count()` every
/// render frame without taking a lock.
pub struct ErrorLogStore {
    entries: Mutex<VecDeque<ErrorLogEntry>>,
    unviewed: AtomicUsize,
    next_id: AtomicU64,
}

impl ErrorLogStore {
    /// Maximum number of entries retained in the ring buffer.
    pub const MAX_ENTRIES: usize = 100;

    /// Create a new, empty store.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            unviewed: AtomicUsize::new(0),
            next_id: AtomicU64::new(0),
        }
    }

    /// Push a new entry, assigning it the next monotonic `id`.
    ///
    /// The `entry_builder` closure receives the assigned `id` so the caller
    /// can embed it in the entry without a separate allocation step.
    ///
    /// If the buffer is already at capacity the oldest entry is dropped.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn push(&self, entry_builder: impl FnOnce(u64) -> ErrorLogEntry) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let entry = entry_builder(id);

        {
            let mut guard = self.entries.lock().expect("error log mutex poisoned");

            // Deduplicate: skip if a recent entry (within 2 s) shares the same
            // source AND conversation_id. This handles the triple-fire from
            // client_agent.rs where a single LLM error fires the callback up to
            // 3 times with differently-wrapped message strings.
            if let Some(last) = guard.front() {
                let elapsed = entry.timestamp - last.timestamp;
                if last.source == entry.source
                    && last.conversation_id == entry.conversation_id
                    && elapsed < chrono::Duration::seconds(2)
                {
                    return;
                }
            }

            guard.push_front(entry);
            if guard.len() > Self::MAX_ENTRIES {
                guard.pop_back();
            }
        }
        self.unviewed.fetch_add(1, Ordering::Relaxed);
    }

    /// Return a snapshot of all entries, newest first.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn entries(&self) -> Vec<ErrorLogEntry> {
        self.entries
            .lock()
            .expect("error log mutex poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// Return the current count of unviewed errors.
    #[must_use]
    pub fn unviewed_count(&self) -> usize {
        self.unviewed.load(Ordering::Relaxed)
    }

    /// Reset the unviewed count to zero (call when the error log view is opened).
    pub fn mark_all_viewed(&self) {
        self.unviewed.store(0, Ordering::Relaxed);
    }

    /// Empty the ring buffer and reset both the unviewed count and the ID counter.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn clear(&self) {
        self.entries
            .lock()
            .expect("error log mutex poisoned")
            .clear();
        self.unviewed.store(0, Ordering::Relaxed);
        self.next_id.store(0, Ordering::Relaxed);
    }

    /// Return the process-wide singleton `ErrorLogStore`.
    #[must_use]
    pub fn global() -> &'static Self {
        &GLOBAL_ERROR_LOG
    }
}

impl Default for ErrorLogStore {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_ERROR_LOG: once_cell::sync::Lazy<ErrorLogStore> =
    once_cell::sync::Lazy::new(ErrorLogStore::new);

// ---------------------------------------------------------------------------
// Classifier
// ---------------------------------------------------------------------------

/// Classify an error message into a severity tag using keyword heuristics.
///
/// Checks for common patterns in error text:
/// - Auth: "401", "403", "unauthorized", "forbidden", "invalid api key", "`invalid_api_key`", "authentication"
/// - Connection: "timeout", "connection refused", "ECONNREFUSED", "dns", "network", "ETIMEDOUT", "connection reset"
/// - MCP: "mcp", "tool call"
/// - Internal: default for anything not matching above (call sites remap to domain-appropriate tag)
#[must_use]
pub fn classify_error_severity(error_msg: &str) -> ErrorSeverityTag {
    let lower = error_msg.to_ascii_lowercase();

    if lower.contains("401")
        || lower.contains("403")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("invalid api key")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
    {
        ErrorSeverityTag::Auth
    } else if lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection refused")
        || lower.contains("econnrefused")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("etimedout")
        || lower.contains("connection reset")
    {
        ErrorSeverityTag::Connection
    } else if lower.contains("mcp") || lower.contains("tool call") {
        ErrorSeverityTag::Mcp
    } else {
        ErrorSeverityTag::Internal
    }
}

fn sanitize_optional(value: Option<&str>) -> Option<String> {
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

/// Render a single error log entry as plain text for clipboard/export usage.
#[must_use]
pub fn render_error_entry_text(entry: &ErrorLogEntry) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "[{}] {}",
        entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
        entry.severity
    );
    let _ = writeln!(output, "Source: {}", entry.source);

    if let Some(conversation) = entry.conversation_title.as_deref() {
        let _ = writeln!(output, "Conversation: {conversation}");
    } else if let Some(conversation_id) = entry.conversation_id.as_ref() {
        let _ = writeln!(output, "Conversation: {conversation_id}");
    }

    let _ = writeln!(output, "Message:");
    let _ = writeln!(output, "{}", sanitize_text(entry.message.trim_end()));

    if let Some(raw_detail) = entry.raw_detail.as_deref().map(str::trim) {
        if !raw_detail.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "Raw Detail:");
            let _ = writeln!(output, "{}", sanitize_text(raw_detail));
        }
    }

    if let Some(diagnostics) = entry.diagnostics.as_ref() {
        let diagnostics = diagnostics.sanitized();
        if !diagnostics.is_empty() {
            let _ = writeln!(output);
            write_diagnostic_context_text(&mut output, &diagnostics);
        }
    }

    output.trim_end().to_string()
}

fn write_diagnostic_context_text(output: &mut String, diagnostics: &ErrorLogDiagnosticContext) {
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

#[derive(Serialize)]
struct ErrorLogExportEntry {
    id: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
    severity: ErrorSeverityTag,
    source: String,
    message: String,
    raw_detail: Option<String>,
    conversation_title: Option<String>,
    conversation_id: Option<uuid::Uuid>,
    diagnostics: Option<ErrorLogDiagnosticContext>,
}

impl From<&ErrorLogEntry> for ErrorLogExportEntry {
    fn from(entry: &ErrorLogEntry) -> Self {
        Self {
            id: entry.id,
            timestamp: entry.timestamp,
            severity: entry.severity.clone(),
            source: sanitize_text(&entry.source),
            message: sanitize_text(&entry.message),
            raw_detail: sanitize_optional(entry.raw_detail.as_deref()),
            conversation_title: sanitize_optional(entry.conversation_title.as_deref()),
            conversation_id: entry.conversation_id,
            diagnostics: entry
                .diagnostics
                .as_ref()
                .map(ErrorLogDiagnosticContext::sanitized),
        }
    }
}

/// Render a complete error log snapshot as structured JSON.
///
/// # Errors
///
/// Returns a serialization error if an entry cannot be encoded as JSON.
pub fn render_error_log_json(entries: &[ErrorLogEntry]) -> Result<String, serde_json::Error> {
    let export_entries = entries
        .iter()
        .map(ErrorLogExportEntry::from)
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&export_entries)
}

/// Render a complete error log snapshot as plain text.
#[must_use]
pub fn render_error_log_text(entries: &[ErrorLogEntry]) -> String {
    if entries.is_empty() {
        return "No errors recorded".to_string();
    }

    entries
        .iter()
        .map(render_error_entry_text)
        .collect::<Vec<_>>()
        .join(
            "

----------------------------------------

",
        )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Build a minimal `ErrorLogEntry` for testing, using the given `id`.
    ///
    /// Each entry gets a unique `source` so that the dedup guard (which
    /// compares `source` + `conversation_id` within 2 s) does not suppress
    /// entries that are meant to be distinct test rows.
    fn make_entry(id: u64) -> ErrorLogEntry {
        ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: format!("test/{id}"),
            message: format!("error {id}"),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        }
    }

    fn fresh_store() -> ErrorLogStore {
        ErrorLogStore::new()
    }

    // --- Ring buffer caps at MAX_ENTRIES ---

    #[test]
    fn test_ring_buffer_caps_at_max_entries() {
        let store = fresh_store();
        for _ in 0..150 {
            store.push(make_entry);
        }
        assert_eq!(store.entries().len(), ErrorLogStore::MAX_ENTRIES);
    }

    // --- Push drops the oldest entry when full ---

    #[test]
    fn test_push_full_drops_oldest() {
        let store = fresh_store();
        // Fill to capacity
        for _ in 0..ErrorLogStore::MAX_ENTRIES {
            store.push(make_entry);
        }
        // One more push — oldest (id=0) should be gone
        store.push(make_entry);
        let entries = store.entries();
        assert_eq!(entries.len(), ErrorLogStore::MAX_ENTRIES);
        // Newest is at front; id 0 (the first-pushed, now oldest) must not appear
        assert!(
            entries.iter().all(|e| e.id != 0),
            "oldest entry should have been evicted"
        );
    }

    // --- Push increments unviewed count ---

    #[test]
    fn test_push_increments_unviewed_count() {
        let store = fresh_store();
        assert_eq!(store.unviewed_count(), 0);
        store.push(make_entry);
        assert_eq!(store.unviewed_count(), 1);
        store.push(make_entry);
        assert_eq!(store.unviewed_count(), 2);
    }

    // --- mark_all_viewed resets count to 0 ---

    #[test]
    fn test_mark_all_viewed_resets_count() {
        let store = fresh_store();
        store.push(make_entry);
        store.push(make_entry);
        assert_eq!(store.unviewed_count(), 2);
        store.mark_all_viewed();
        assert_eq!(store.unviewed_count(), 0);
    }

    // --- clear empties buffer and resets unviewed and id counter ---

    #[test]
    fn test_clear_empties_and_resets() {
        let store = fresh_store();
        store.push(make_entry);
        store.push(make_entry);
        store.clear();
        assert_eq!(store.entries().len(), 0);
        assert_eq!(store.unviewed_count(), 0);
        // After clear, the id counter restarts from 0
        store.push(make_entry);
        let entries = store.entries();
        assert_eq!(entries[0].id, 0, "id counter should reset after clear");
    }

    // --- entries() returns newest-first order ---

    #[test]
    fn test_entries_newest_first() {
        let store = fresh_store();
        store.push(make_entry); // id=0
        store.push(make_entry); // id=1
        store.push(make_entry); // id=2
        let entries = store.entries();
        // Monotonically decreasing ids mean newest is first
        assert_eq!(entries[0].id, 2);
        assert_eq!(entries[1].id, 1);
        assert_eq!(entries[2].id, 0);
    }

    // --- Thread safety: push from 2 threads concurrently ---

    #[test]
    fn test_concurrent_push_thread_safety() {
        use std::sync::Arc;
        let store = Arc::new(fresh_store());

        let s1 = Arc::clone(&store);
        let h1 = thread::spawn(move || {
            for _ in 0..10 {
                s1.push(make_entry);
            }
        });

        let s2 = Arc::clone(&store);
        let h2 = thread::spawn(move || {
            for _ in 0..10 {
                s2.push(make_entry);
            }
        });

        h1.join().expect("thread 1 panicked");
        h2.join().expect("thread 2 panicked");

        assert_eq!(store.entries().len(), 20);
        assert_eq!(store.unviewed_count(), 20);
    }

    // --- Concurrent push + mark_all_viewed doesn't panic ---

    #[test]
    fn test_concurrent_push_and_mark_all_viewed_no_panic() {
        use std::sync::Arc;
        let store = Arc::new(fresh_store());

        let s_push = Arc::clone(&store);
        let pusher = thread::spawn(move || {
            for _ in 0..50 {
                s_push.push(make_entry);
            }
        });

        let s_mark = Arc::clone(&store);
        let marker = thread::spawn(move || {
            for _ in 0..10 {
                s_mark.mark_all_viewed();
            }
        });

        pusher.join().expect("pusher thread panicked");
        marker.join().expect("marker thread panicked");
        // No assertion on exact count — just confirm no panic and state is accessible
        let _ = store.unviewed_count();
        let _ = store.entries();
    }

    #[test]
    fn render_error_entry_text_includes_core_fields() {
        let entry = ErrorLogEntry {
            id: 42,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Auth,
            source: "anthropic / claude".to_string(),
            message: "401 unauthorized".to_string(),
            raw_detail: Some("{".to_string()),
            conversation_title: Some("Bug triage".to_string()),
            conversation_id: Some(uuid::Uuid::new_v4()),
            diagnostics: None,
        };

        let rendered = render_error_entry_text(&entry);
        assert!(rendered.contains("AUTH"));
        assert!(rendered.contains("Source: anthropic / claude"));
        assert!(rendered.contains("Conversation: Bug triage"));
        assert!(rendered.contains("Message:"));
        assert!(rendered.contains("401 unauthorized"));
        assert!(rendered.contains("Raw Detail:"));
    }

    #[test]
    fn render_error_entry_text_includes_sanitized_diagnostics() {
        let entry = ErrorLogEntry {
            id: 43,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "stream failed api_key=abc123".to_string(),
            raw_detail: Some("authorization: Bearer secret-token".to_string()),
            conversation_title: None,
            conversation_id: None,
            diagnostics: Some(ErrorLogDiagnosticContext {
                underlying_error: Some("provider failed token=abc123".to_string()),
                subsystem: Some("chat stream".to_string()),
                code_path: Some("services::chat_impl::streaming".to_string()),
                provider_id: Some("anthropic".to_string()),
                model_id: Some("claude".to_string()),
                base_url_host: Some("api.anthropic.com".to_string()),
                run_status: Some(ErrorLogRunStatus::Failed),
                stream_lifecycle: Some(ErrorLogStreamLifecycle::Failed),
                partial_assistant_response_len: Some(12),
                thinking_len: Some(4),
                tool_calls: vec![ErrorLogToolContext {
                    tool_name: "search".to_string(),
                    tool_call_id: Some("call-1".to_string()),
                    success: Some(false),
                    summary: Some("tool secret=abc failed".to_string()),
                }],
                ..ErrorLogDiagnosticContext::default()
            }),
        };

        let rendered = render_error_entry_text(&entry);
        assert!(rendered.contains("Diagnostics:"));
        assert!(rendered.contains("Underlying error: provider failed token=[REDACTED]"));
        assert!(rendered.contains("Partial assistant response length: 12"));
        assert!(rendered.contains("Tool calls:"));
        assert!(!rendered.contains("abc123"));
        assert!(!rendered.contains("Bearer secret-token"));
    }

    #[test]
    fn render_error_log_json_includes_structured_sanitized_diagnostics() {
        let entry = ErrorLogEntry {
            id: 44,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Connection,
            source: "chat".to_string(),
            message: "failed password=hunter2".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: Some(ErrorLogDiagnosticContext {
                underlying_error: Some("timeout access_token=xyz".to_string()),
                subsystem: Some("provider".to_string()),
                run_status: Some(ErrorLogRunStatus::Failed),
                ..ErrorLogDiagnosticContext::default()
            }),
        };

        let rendered = render_error_log_json(&[entry]).expect("json should render");
        let parsed: serde_json::Value = serde_json::from_str(&rendered).expect("json should parse");
        assert_eq!(parsed[0]["diagnostics"]["subsystem"], "provider");
        assert_eq!(
            parsed[0]["diagnostics"]["underlying_error"],
            "timeout access_token=[REDACTED]"
        );
        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("xyz"));
    }

    #[test]
    fn render_error_log_text_joins_entries_with_separator() {
        let first = ErrorLogEntry {
            id: 1,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "stream failed".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        };
        let second = ErrorLogEntry {
            id: 2,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Mcp,
            source: "mcp/server".to_string(),
            message: "tool error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        };

        let rendered = render_error_log_text(&[first, second]);
        assert!(rendered.contains("Source: chat"));
        assert!(rendered.contains("Source: mcp/server"));
        assert!(rendered.contains("----------------------------------------"));
    }

    #[test]
    fn render_error_log_text_handles_empty_entries() {
        assert_eq!(render_error_log_text(&[]), "No errors recorded");
    }

    // --- classify_error_severity ---

    #[test]
    fn test_classify_auth_patterns() {
        use super::classify_error_severity;
        assert_eq!(
            classify_error_severity("HTTP 401 Unauthorized"),
            ErrorSeverityTag::Auth
        );
        assert_eq!(
            classify_error_severity("403 Forbidden"),
            ErrorSeverityTag::Auth
        );
        assert_eq!(
            classify_error_severity("Request unauthorized"),
            ErrorSeverityTag::Auth
        );
        assert_eq!(
            classify_error_severity("Access forbidden to resource"),
            ErrorSeverityTag::Auth
        );
        assert_eq!(
            classify_error_severity("Invalid API key provided"),
            ErrorSeverityTag::Auth
        );
        assert_eq!(
            classify_error_severity("Authentication failed"),
            ErrorSeverityTag::Auth
        );
        assert_eq!(
            classify_error_severity("error: invalid_api_key"),
            ErrorSeverityTag::Auth
        );
    }

    #[test]
    fn test_classify_connection_patterns() {
        use super::classify_error_severity;
        assert_eq!(
            classify_error_severity("Request timed out after 30s"),
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("Connection refused on port 8080"),
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("ECONNREFUSED 127.0.0.1:3000"),
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("DNS resolution failed"),
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("Network unreachable"),
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("ETIMEDOUT while connecting"),
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("Connection reset by peer"),
            ErrorSeverityTag::Connection
        );
    }

    #[test]
    fn test_classify_generic_errors_match_internal() {
        use super::classify_error_severity;
        assert_eq!(
            classify_error_severity("Unexpected response from model"),
            ErrorSeverityTag::Internal
        );
        assert_eq!(
            classify_error_severity("Rate limit exceeded"),
            ErrorSeverityTag::Internal
        );
        assert_eq!(
            classify_error_severity("Something went wrong"),
            ErrorSeverityTag::Internal
        );
    }

    #[test]
    fn test_classify_empty_string_matches_internal() {
        use super::classify_error_severity;
        assert_eq!(classify_error_severity(""), ErrorSeverityTag::Internal);
    }

    // --- classify_error_severity: MCP branch ---

    #[test]
    fn test_classify_mcp_patterns() {
        use super::classify_error_severity;
        assert_eq!(
            classify_error_severity("mcp server failed to start"),
            ErrorSeverityTag::Mcp
        );
        assert_eq!(
            classify_error_severity("MCP server timed out"),
            // "timed out" is a connection keyword, so Connection wins over Mcp
            ErrorSeverityTag::Connection
        );
        assert_eq!(
            classify_error_severity("tool call error: invalid input"),
            ErrorSeverityTag::Mcp
        );
        assert_eq!(
            classify_error_severity("Tool Call returned unexpected result"),
            ErrorSeverityTag::Mcp
        );
    }

    #[test]
    fn test_classify_timed_out_pattern() {
        use super::classify_error_severity;
        // "timed out" should match Connection
        assert_eq!(
            classify_error_severity("operation timed out"),
            ErrorSeverityTag::Connection
        );
    }

    // --- ErrorSeverityTag Display ---

    #[test]
    fn test_severity_display_stream() {
        assert_eq!(ErrorSeverityTag::Stream.to_string(), "STREAM");
    }

    #[test]
    fn test_severity_display_auth() {
        assert_eq!(ErrorSeverityTag::Auth.to_string(), "AUTH");
    }

    #[test]
    fn test_severity_display_connection() {
        assert_eq!(ErrorSeverityTag::Connection.to_string(), "CONN");
    }

    #[test]
    fn test_severity_display_mcp() {
        assert_eq!(ErrorSeverityTag::Mcp.to_string(), "MCP");
    }

    #[test]
    fn test_severity_display_internal() {
        assert_eq!(ErrorSeverityTag::Internal.to_string(), "INTERNAL");
    }

    // --- ErrorLogEntry with all optional fields populated ---

    #[test]
    fn test_entry_with_all_fields_round_trips_through_store() {
        let store = fresh_store();
        let conv_id = uuid::Uuid::new_v4();

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Auth,
            source: "anthropic / claude-sonnet".to_string(),
            message: "401 Unauthorized".to_string(),
            raw_detail: Some(r#"{"error":"invalid_api_key"}"#.to_string()),
            conversation_title: Some("My Test Conversation".to_string()),
            conversation_id: Some(conv_id),
            diagnostics: None,
        });

        let entries = store.entries();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.severity, ErrorSeverityTag::Auth);
        assert_eq!(e.source, "anthropic / claude-sonnet");
        assert_eq!(e.message, "401 Unauthorized");
        assert_eq!(
            e.raw_detail.as_deref(),
            Some(r#"{"error":"invalid_api_key"}"#)
        );
        assert_eq!(
            e.conversation_title.as_deref(),
            Some("My Test Conversation")
        );
        assert_eq!(e.conversation_id, Some(conv_id));
    }

    #[test]
    fn test_entry_severity_variants_all_stored() {
        let severities = [
            ErrorSeverityTag::Stream,
            ErrorSeverityTag::Auth,
            ErrorSeverityTag::Connection,
            ErrorSeverityTag::Mcp,
            ErrorSeverityTag::Internal,
        ];

        for severity in severities {
            let store = fresh_store();
            let sev_clone = severity.clone();
            store.push(|id| ErrorLogEntry {
                id,
                timestamp: chrono::Utc::now(),
                severity: sev_clone,
                source: "test".to_string(),
                message: "test message".to_string(),
                raw_detail: None,
                conversation_title: None,
                conversation_id: None,
                diagnostics: None,
            });
            let entries = store.entries();
            assert_eq!(entries[0].severity, severity);
        }
    }

    #[test]
    fn test_entry_with_conversation_id_only_no_title() {
        let store = fresh_store();
        let conv_id = uuid::Uuid::new_v4();
        store.push(|id| ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "stream error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: Some(conv_id),
            diagnostics: None,
        });
        let entries = store.entries();
        assert_eq!(entries[0].conversation_id, Some(conv_id));
        assert!(entries[0].conversation_title.is_none());
    }

    #[test]
    fn test_entry_raw_detail_none_and_some() {
        let store = fresh_store();
        store.push(|id| ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Internal,
            source: "sys/a".to_string(),
            message: "error without detail".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        });
        store.push(|id| ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Internal,
            source: "sys/b".to_string(),
            message: "error with detail".to_string(),
            raw_detail: Some(
                "HTTP/1.1 500 Internal Server Error
body: {}"
                    .to_string(),
            ),
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        });
        let entries = store.entries();
        assert!(entries[0].raw_detail.is_some()); // newest first
        assert!(entries[1].raw_detail.is_none());
    }

    // --- Deduplication ---

    #[test]
    fn test_dedup_skips_same_source_and_conversation_within_window() {
        let store = fresh_store();
        let now = chrono::Utc::now();
        let conv = Some(uuid::Uuid::new_v4());

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "connection failed".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: conv,
            diagnostics: None,
        });

        // Different message but same source + conversation_id — should be deduped
        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "SerdesAi: connection failed".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: conv,
            diagnostics: None,
        });

        // Third variant with yet another wrapper — also deduped
        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "LLM error: connection failed".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: conv,
            diagnostics: None,
        });

        assert_eq!(store.entries().len(), 1, "triple-fire should collapse to 1");
        assert_eq!(
            store.unviewed_count(),
            1,
            "unviewed count should not increment for deduped entries"
        );
    }

    #[test]
    fn test_dedup_allows_different_sources() {
        let store = fresh_store();
        let now = chrono::Utc::now();

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "error A".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        });

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Mcp,
            source: "mcp/exa".to_string(),
            message: "error B".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        });

        assert_eq!(
            store.entries().len(),
            2,
            "different sources should not be deduped"
        );
    }

    #[test]
    fn test_dedup_allows_same_source_after_window() {
        let store = fresh_store();
        let old = chrono::Utc::now() - chrono::Duration::seconds(5);
        let now = chrono::Utc::now();

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: old,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "connection failed".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        });

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "connection failed".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        });

        assert_eq!(
            store.entries().len(),
            2,
            "same source outside window should not be deduped"
        );
    }

    #[test]
    fn test_dedup_allows_different_conversations_same_source() {
        let store = fresh_store();
        let now = chrono::Utc::now();

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: Some(uuid::Uuid::new_v4()),
            diagnostics: None,
        });

        store.push(|id| ErrorLogEntry {
            id,
            timestamp: now,
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: Some(uuid::Uuid::new_v4()),
            diagnostics: None,
        });

        assert_eq!(
            store.entries().len(),
            2,
            "different conversation_ids should not be deduped"
        );
    }
}
