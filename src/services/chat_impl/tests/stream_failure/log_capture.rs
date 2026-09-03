//! Log capture: the tracing output emitted by stream failure paths must
//! redact credentials before logging and record persistence failures with
//! their outcome state (issue #193).

use super::*;

/// Buffer receiving the scoped subscriber's output for the current thread.
type CaptureBuffer = Arc<StdMutex<Vec<u8>>>;

thread_local! {
    static CURRENT_CAPTURE: std::cell::RefCell<Option<CaptureBuffer>> =
        const { std::cell::RefCell::new(None) };
}

/// Forwards formatted log output to the current thread's capture buffer;
/// threads without an active capture discard the output.
#[derive(Clone, Copy)]
struct ThreadCaptureSink;

impl std::io::Write for ThreadCaptureSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        CURRENT_CAPTURE.with(|slot| {
            if let Some(buffer) = slot.borrow().as_ref() {
                buffer
                    .lock()
                    .expect("log capture poisoned")
                    .extend_from_slice(buf);
            }
            Ok(buf.len())
        })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ThreadCaptureSink {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        Self
    }
}

/// Install the capture subscriber as the process-wide default, once.
///
/// Callsites that first fire while no default subscriber exists cache
/// `Interest::never()` in the global callsite registry, and a later scoped
/// `with_default` does not rebuild that cache, so their events would be
/// skipped entirely. A permanently registered default keeps the cache
/// stable while tests run in parallel. Threads without an active capture
/// just discard the formatted output.
fn ensure_global_capture_default() {
    static GLOBAL: std::sync::Once = std::sync::Once::new();
    GLOBAL.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThreadCaptureSink)
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect(
            "stream_failure tests must install the process-global capture subscriber before any other default",
        );
    });
}

/// Run `f` and return the log output captured for the current thread. The
/// buffer is thread-local, so concurrently running tests are unaffected.
fn with_captured_logs(f: impl FnOnce()) -> String {
    ensure_global_capture_default();
    let buffer: CaptureBuffer = Arc::new(StdMutex::new(Vec::new()));
    CURRENT_CAPTURE.with(|slot| *slot.borrow_mut() = Some(buffer.clone()));
    f();
    CURRENT_CAPTURE.with(|slot| *slot.borrow_mut() = None);
    let bytes = buffer.lock().expect("log capture poisoned").clone();
    String::from_utf8(bytes).expect("captured logs must be valid UTF-8")
}

/// The error-event log line must carry the sanitized error, never the raw
/// provider value; credentials may only reach storage/export paths, which
/// apply their own redaction (issue #193 OCR follow-up).
#[test]
fn error_event_log_sanitizes_credentials_before_tracing() {
    let conversation_id = Uuid::new_v4();
    let (tx, _stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut state = TranscriptState::new();

    let captured = with_captured_logs(|| {
        state.apply(
            crate::llm::StreamEvent::Error("provider exploded token=abc123".to_string()),
            conversation_id,
            &tx,
        );
    });

    assert!(
        captured.contains("LLM stream event error"),
        "the error-event log line should be captured: {captured}"
    );
    assert!(
        captured.contains("token=[REDACTED]"),
        "credentials must be redacted before tracing: {captured}"
    );
    assert!(
        !captured.contains("abc123"),
        "the raw credential must never reach the log: {captured}"
    );
}

/// A failing `add_message` must not be silently discarded: the failure is
/// logged with the conversation id and the interrupted outcome (issue #193
/// OCR follow-up).
#[tokio::test]
async fn persist_assistant_response_logs_add_message_failure_with_outcome_state() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    conversation_service_impl
        .set_add_message_failure(true)
        .await;
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let transcript = streaming::StreamTranscript {
        response_text: "partial answer".to_string(),
        error: Some("connection reset".to_string()),
        ..streaming::StreamTranscript::default()
    };

    let captured = with_captured_logs(|| {
        // The mock's awaits resolve immediately, so the future can be driven
        // to completion on this thread inside the scoped subscriber.
        futures::executor::block_on(streaming::persist_assistant_response(
            &conversation_service,
            conversation_id,
            &transcript,
            "Interrupted Model",
            true,
        ));
    });
    assert!(
        captured.contains("Failed to persist assistant response"),
        "the add_message failure must be logged: {captured}"
    );
    assert!(
        captured.contains(&conversation_id.to_string()),
        "the failure log must carry the conversation id: {captured}"
    );
    assert!(
        captured.contains("interrupted=true"),
        "the failure log must record the interrupted outcome: {captured}"
    );
    assert!(
        captured.contains("simulated add_message persistence failure"),
        "the failure log must carry the ServiceError display: {captured}"
    );

    assert!(
        conversation_service_impl.messages.read().await.is_empty(),
        "failed persistence must not record the message"
    );
}
