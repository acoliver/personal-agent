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

use super::error_log_diagnostics::{sanitize_optional, write_diagnostic_context_text};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

pub use super::error_log_diagnostics::{
    base_url_host, sanitize_text, ErrorLogDiagnosticContext, ErrorLogRunStatus,
    ErrorLogStreamLifecycle, ErrorLogToolContext,
};

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
