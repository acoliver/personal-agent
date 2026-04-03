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

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Severity classification for a logged error.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    let conversation_label = entry
        .conversation_title
        .clone()
        .or_else(|| entry.conversation_id.as_ref().map(ToString::to_string));
    if let Some(conversation) = conversation_label {
        let _ = writeln!(output, "Conversation: {conversation}");
    }

    let _ = writeln!(output, "Message:");
    let _ = writeln!(output, "{}", entry.message.trim_end());

    if let Some(raw_detail) = entry.raw_detail.as_deref().map(str::trim) {
        if !raw_detail.is_empty() {
            let _ = writeln!(output);
            let _ = writeln!(output, "Raw Detail:");
            let _ = writeln!(output, "{raw_detail}");
        }
    }

    output.trim_end().to_string()
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
        });

        assert_eq!(
            store.entries().len(),
            2,
            "different conversation_ids should not be deduped"
        );
    }
}
