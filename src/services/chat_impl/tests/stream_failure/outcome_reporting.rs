//! Stream failure outcome recording: transcript error state, sanitized
//! diagnostics on the event bus, run-failure reporting, and the single-report
//! guarantee for duplicated failures (issue #193).

use super::*;
use crate::events::subscribe;
use crate::events::types::ChatEvent;

/// Extract the first `StreamError` diagnostics payload for this conversation.
fn stream_error_diagnostics(
    events: &[ChatEvent],
) -> Option<&crate::ui_gpui::error_log::ErrorLogDiagnosticContext> {
    events.iter().find_map(|event| match event {
        ChatEvent::StreamError { diagnostics, .. } => diagnostics.as_deref(),
        _ => None,
    })
}

#[tokio::test]
async fn error_event_marks_transcript_and_emits_sanitized_diagnostics() {
    let conversation_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let (tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut state = TranscriptState::new();

    state.apply(
        crate::llm::StreamEvent::ThinkingDelta("partial thinking".to_string()),
        conversation_id,
        &tx,
    );
    state.apply(
        crate::llm::StreamEvent::TextDelta("partial answer".to_string()),
        conversation_id,
        &tx,
    );
    state.apply(
        crate::llm::StreamEvent::Error("provider exploded token=abc123".to_string()),
        conversation_id,
        &tx,
    );

    assert_eq!(
        state.transcript.error.as_deref(),
        Some("provider exploded token=abc123"),
        "Error events must set the transcript error outcome"
    );
    assert!(
        !state.transcript.completed,
        "error event must not complete the stream"
    );

    match stream_rx.try_recv().expect("text token should be sent") {
        ChatStreamEvent::Token(token) => assert_eq!(token, "partial answer"),
        other => panic!("expected token event, got {other:?}"),
    }
    match stream_rx.try_recv().expect("stream error should be sent") {
        ChatStreamEvent::Error(error) => {
            assert_eq!(
                error.to_string(),
                ServiceError::Internal(STREAM_ERROR_MESSAGE.to_string()).to_string()
            );
        }
        other => panic!("expected stream error event, got {other:?}"),
    }

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    let diagnostics = stream_error_diagnostics(&events)
        .expect("StreamError with diagnostics should be emitted on the event bus");

    let underlying = diagnostics
        .underlying_error
        .as_deref()
        .expect("diagnostics should carry the underlying error");
    assert!(
        underlying.contains("provider exploded"),
        "underlying error should survive sanitization: {underlying}"
    );
    assert!(
        underlying.contains("token=[REDACTED]"),
        "credentials in the underlying error must be redacted: {underlying}"
    );
    assert_eq!(
        diagnostics.partial_assistant_response_len,
        Some("partial answer".len()),
        "diagnostics must report the partial response length"
    );
    assert_eq!(
        diagnostics.thinking_len,
        Some("partial thinking".len()),
        "diagnostics must report the partial thinking length"
    );
}

#[tokio::test]
async fn complete_after_error_clears_error_and_completes_transcript() {
    let conversation_id = Uuid::new_v4();
    let (tx, _stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut state = TranscriptState::new();

    state.apply(
        crate::llm::StreamEvent::Error("transient failure".to_string()),
        conversation_id,
        &tx,
    );
    state.apply(
        crate::llm::StreamEvent::Complete {
            input_tokens: Some(3),
            output_tokens: Some(5),
        },
        conversation_id,
        &tx,
    );

    assert!(
        state.transcript.completed,
        "a later Complete must complete the stream"
    );
    assert_eq!(
        state.transcript.error, None,
        "a later Complete must clear the error outcome so normal finalization runs"
    );
}

#[tokio::test]
async fn stream_run_failure_emits_sanitized_diagnostics_with_partial_lengths() {
    let conversation_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let (tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let context = streaming::StreamDiagnosticContext {
        profile_name: "Run Failure Profile".to_string(),
        provider_id: "provider".to_string(),
        model_id: "model".to_string(),
        ..streaming::StreamDiagnosticContext::default()
    };
    let transcript = streaming::StreamTranscript {
        response_text: "partial answer".to_string(),
        thinking_text: "partial thinking".to_string(),
        ..streaming::StreamTranscript::default()
    };

    streaming::handle_stream_run_failure(
        conversation_id,
        &context,
        &transcript,
        "connection reset api_key=supersecret",
        &tx,
    );

    match stream_rx.try_recv().expect("stream error should be sent") {
        ChatStreamEvent::Error(error) => {
            assert!(error.to_string().contains("interrupted the chat stream"));
        }
        other => panic!("expected stream error event, got {other:?}"),
    }

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    let diagnostics = stream_error_diagnostics(&events)
        .expect("StreamError with diagnostics should be emitted for run failures");

    let underlying = diagnostics
        .underlying_error
        .as_deref()
        .expect("run-failure diagnostics should carry the underlying error");
    assert!(
        underlying.contains("connection reset"),
        "underlying error should survive sanitization: {underlying}"
    );
    assert!(
        !underlying.contains("supersecret"),
        "api keys in the underlying error must be redacted: {underlying}"
    );
    assert_eq!(
        diagnostics.partial_assistant_response_len,
        Some("partial answer".len())
    );
    assert_eq!(
        diagnostics.thinking_len,
        Some("partial thinking".len()),
        "run-failure diagnostics must report the partial thinking length"
    );
}

/// `do_run_agent_stream` reports common mid-stream failures twice: an
/// `Error` event callback first, then a returned `Err` carrying the same
/// message. The first report must win — the transcript keeps the original
/// error and no second channel or bus error may be emitted (issue #193).
#[tokio::test]
async fn returned_error_after_error_event_does_not_duplicate_failure_report() {
    let conversation_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let (tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut state = TranscriptState::new();

    // Mid-stream failure as delivered by `do_run_agent_stream`: the Error
    // event fires first...
    state.apply(
        crate::llm::StreamEvent::Error("provider reset mid-stream".to_string()),
        conversation_id,
        &tx,
    );
    // ...then the run returns Err carrying the same message.
    streaming::record_stream_run_failure(
        &mut state.transcript,
        &crate::llm::error::LlmError::Stream("provider reset mid-stream".to_string()),
        conversation_id,
        &streaming::StreamDiagnosticContext::default(),
        &tx,
    );

    assert_eq!(
        state.transcript.error.as_deref(),
        Some("provider reset mid-stream"),
        "the error-event report must be preserved, not overwritten by the returned Err"
    );

    let mut channel_errors = 0;
    while let Ok(event) = stream_rx.try_recv() {
        match event {
            ChatStreamEvent::Error(_) => channel_errors += 1,
            other => panic!("unexpected channel event: {other:?}"),
        }
    }
    assert_eq!(
        channel_errors, 1,
        "a mid-stream failure must surface exactly one channel error"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ChatEvent::StreamError { .. }))
            .count(),
        1,
        "a mid-stream failure must emit exactly one StreamError on the event bus, got {events:?}"
    );
}

/// A returned `Err` that never fired an `Error` event (e.g. `AgentStream`
/// construction failure) must still be reported exactly once (issue #193).
#[tokio::test]
async fn eventless_returned_failure_is_still_reported() {
    let conversation_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let (tx, mut stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut transcript = streaming::StreamTranscript::default();

    streaming::record_stream_run_failure(
        &mut transcript,
        &crate::llm::error::LlmError::Stream("AgentStream creation failed".to_string()),
        conversation_id,
        &streaming::StreamDiagnosticContext::default(),
        &tx,
    );

    let recorded = transcript
        .error
        .as_deref()
        .expect("an event-less returned failure must still be recorded");
    assert!(
        recorded.contains("AgentStream creation failed"),
        "recorded error should carry the failure: {recorded}"
    );

    match stream_rx.try_recv().expect("run failure should be sent") {
        ChatStreamEvent::Error(_) => {}
        other => panic!("expected stream error event, got {other:?}"),
    }
    assert!(
        stream_rx.try_recv().is_err(),
        "exactly one channel error must be emitted for an event-less failure"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ChatEvent::StreamError { .. }))
            .count(),
        1,
        "an event-less returned failure must emit exactly one StreamError, got {events:?}"
    );
}
