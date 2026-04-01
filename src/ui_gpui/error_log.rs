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
/// - Auth: "401", "403", "unauthorized", "forbidden", "invalid api key", "authentication"
/// - Connection: "timeout", "connection refused", "ECONNREFUSED", "dns", "network", "ETIMEDOUT", "connection reset"
/// - MCP: "mcp", "tool call"
/// - Stream: default for anything not matching above
#[must_use]
pub fn classify_error_severity(error_msg: &str) -> ErrorSeverityTag {
    let lower = error_msg.to_ascii_lowercase();

    if lower.contains("401")
        || lower.contains("403")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("invalid api key")
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
        ErrorSeverityTag::Stream
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Build a minimal `ErrorLogEntry` for testing, using the given `id`.
    fn make_entry(id: u64) -> ErrorLogEntry {
        ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "test / source".to_string(),
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
    fn test_classify_generic_errors_match_stream() {
        use super::classify_error_severity;
        assert_eq!(
            classify_error_severity("Unexpected response from model"),
            ErrorSeverityTag::Stream
        );
        assert_eq!(
            classify_error_severity("Rate limit exceeded"),
            ErrorSeverityTag::Stream
        );
        assert_eq!(
            classify_error_severity("Something went wrong"),
            ErrorSeverityTag::Stream
        );
    }

    #[test]
    fn test_classify_empty_string_matches_stream() {
        use super::classify_error_severity;
        assert_eq!(classify_error_severity(""), ErrorSeverityTag::Stream);
    }
}
