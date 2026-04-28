use super::error_log::*;
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
    use super::error_log::classify_error_severity;
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
    use super::error_log::classify_error_severity;
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
    use super::error_log::classify_error_severity;
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
    use super::error_log::classify_error_severity;
    assert_eq!(classify_error_severity(""), ErrorSeverityTag::Internal);
}

// --- classify_error_severity: MCP branch ---

#[test]
fn test_classify_mcp_patterns() {
    use super::error_log::classify_error_severity;
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
    use super::error_log::classify_error_severity;
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
