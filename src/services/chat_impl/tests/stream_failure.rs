//! Behavioral tests for chat stream failure handling.
//!
//! Covers issue #193: stream failures must carry explicit outcome state,
//! finalize as interrupted (partial output persisted with an interrupted
//! marker, no context-state update, no completion event), and retain
//! actionable diagnostics.

use super::*;
use crate::events::types::ChatEvent;
use crate::events::{subscribe, AppEvent};

/// Conversation id carried by the chat event (every variant has one).
fn chat_event_conversation_id(event: &ChatEvent) -> Uuid {
    match event {
        ChatEvent::StreamStarted {
            conversation_id, ..
        }
        | ChatEvent::TextDelta {
            conversation_id, ..
        }
        | ChatEvent::ThinkingDelta {
            conversation_id, ..
        }
        | ChatEvent::ToolCallStarted {
            conversation_id, ..
        }
        | ChatEvent::ToolCallCompleted {
            conversation_id, ..
        }
        | ChatEvent::StreamCompleted {
            conversation_id, ..
        }
        | ChatEvent::StreamCancelled {
            conversation_id, ..
        }
        | ChatEvent::StreamError {
            conversation_id, ..
        }
        | ChatEvent::MessageSaved {
            conversation_id, ..
        } => *conversation_id,
    }
}

/// Collect chat events addressing `conversation_id` within a short window.
///
/// The global event bus is shared by concurrently running tests, so every
/// assertion must filter by a conversation UUID unique to this test.
async fn collect_chat_events(
    rx: &mut tokio::sync::broadcast::Receiver<AppEvent>,
    conversation_id: Uuid,
    window: std::time::Duration,
) -> Vec<ChatEvent> {
    let mut matched = Vec::new();
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(AppEvent::Chat(event))) => {
                if chat_event_conversation_id(&event) == conversation_id {
                    matched.push(event);
                }
            }
            // Other subsystems' events and lagged receivers (which just
            // missed other tests' events) are skipped; keep draining.
            Ok(Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
            Err(_elapsed) => break,
        }
    }
    matched
}

/// Drive one `LlmStreamEvent` through `handle_llm_stream_event` using a local
/// transcript, mirroring how `stream_agent_response` tracks a stream.
struct TranscriptState {
    transcript: streaming::StreamTranscript,
}

impl TranscriptState {
    fn new() -> Self {
        Self {
            transcript: streaming::StreamTranscript::default(),
        }
    }

    fn apply(
        &mut self,
        event: crate::llm::StreamEvent,
        conversation_id: Uuid,
        tx: &tokio::sync::mpsc::UnboundedSender<ChatStreamEvent>,
    ) {
        streaming::handle_llm_stream_event(
            &streaming::StreamDiagnosticContext::default(),
            event,
            conversation_id,
            tx,
            &mut self.transcript,
        );
    }
}

/// Extract the first `StreamError` diagnostics payload for this conversation.
fn stream_error_diagnostics(
    events: &[ChatEvent],
) -> Option<&crate::ui_gpui::error_log::ErrorLogDiagnosticContext> {
    events.iter().find_map(|event| match event {
        ChatEvent::StreamError { diagnostics, .. } => diagnostics.as_deref(),
        _ => None,
    })
}

fn active_stream_entry(stream_id: Uuid) -> ActiveStream {
    ActiveStream {
        stream_id,
        task: None,
        cancel: CancellationToken::new(),
        state: StreamLifecycle::Running,
    }
}

/// Bundle the stream-scoped handles the dispatch tests exercise.
fn finalize_context<'a>(
    conversation_service: &'a Arc<dyn crate::services::ConversationService>,
    conversation_id: Uuid,
    stream_id: Uuid,
    active_streams: &'a Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
) -> streaming::StreamFinalizeContext<'a> {
    streaming::StreamFinalizeContext {
        conversation_service,
        conversation_id,
        stream_id,
        active_streams,
    }
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

#[tokio::test]
async fn interrupted_finalize_persists_partial_output_as_interrupted() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let stream_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(conversation_id, active_stream_entry(stream_id));

    let transcript = streaming::StreamTranscript {
        response_text: "partial answer".to_string(),
        thinking_text: "partial thinking".to_string(),
        error: Some("connection reset".to_string()),
        ..streaming::StreamTranscript::default()
    };

    streaming::finalize_interrupted_stream(
        &conversation_service,
        conversation_id,
        stream_id,
        &transcript,
        &active_streams,
        "Interrupted Model",
    )
    .await;

    let messages = conversation_service_impl.messages.read().await.clone();
    assert_eq!(
        messages.len(),
        1,
        "interrupted finalize must persist exactly one assistant message"
    );
    assert_eq!(messages[0].content, "partial answer");
    assert_eq!(
        messages[0].thinking_content.as_deref(),
        Some("partial thinking")
    );
    assert_eq!(messages[0].model_id.as_deref(), Some("Interrupted Model"));
    assert!(
        messages[0].interrupted,
        "interrupted finalize must persist the interrupted marker"
    );

    assert!(
        conversation_service_impl
            .context_state
            .read()
            .await
            .is_none(),
        "interrupted finalize must not update context state"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ChatEvent::StreamCompleted { .. })),
        "interrupted finalize must not emit StreamCompleted, got {events:?}"
    );

    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&conversation_id),
        "interrupted finalize must clear the matching active stream"
    );
}

#[tokio::test]
async fn interrupted_finalize_skips_persistence_when_no_assistant_output() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));

    let empty_conversation = Uuid::new_v4();
    let empty_stream = Uuid::new_v4();
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(empty_conversation, active_stream_entry(empty_stream));
    streaming::finalize_interrupted_stream(
        &conversation_service,
        empty_conversation,
        empty_stream,
        &streaming::StreamTranscript::default(),
        &active_streams,
        "Interrupted Model",
    )
    .await;

    let tool_only_conversation = Uuid::new_v4();
    let tool_only_stream = Uuid::new_v4();
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(
            tool_only_conversation,
            active_stream_entry(tool_only_stream),
        );
    let tool_only = streaming::StreamTranscript {
        tool_calls: vec![crate::llm::tools::ToolUse::new(
            "call-1",
            "web_search",
            serde_json::json!({"query":"rust"}),
        )],
        tool_results: vec![crate::llm::tools::ToolResult::success("call-1", "ok")],
        error: Some("connection reset".to_string()),
        ..streaming::StreamTranscript::default()
    };
    streaming::finalize_interrupted_stream(
        &conversation_service,
        tool_only_conversation,
        tool_only_stream,
        &tool_only,
        &active_streams,
        "Interrupted Model",
    )
    .await;

    assert!(
        conversation_service_impl.messages.read().await.is_empty(),
        "interrupted finalize must skip persistence when text and thinking are both empty"
    );
    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&empty_conversation),
        "interrupted finalize must clear streaming state even when nothing is persisted"
    );
    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&tool_only_conversation),
        "interrupted finalize must clear streaming state for tool-only transcripts"
    );
}

#[tokio::test]
async fn interrupted_finalize_persists_thinking_only_output() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let stream_id = Uuid::new_v4();
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(conversation_id, active_stream_entry(stream_id));

    let transcript = streaming::StreamTranscript {
        thinking_text: "thinking before the failure".to_string(),
        error: Some("connection reset".to_string()),
        ..streaming::StreamTranscript::default()
    };

    streaming::finalize_interrupted_stream(
        &conversation_service,
        conversation_id,
        stream_id,
        &transcript,
        &active_streams,
        "Interrupted Model",
    )
    .await;

    let messages = conversation_service_impl.messages.read().await.clone();
    assert_eq!(
        messages.len(),
        1,
        "thinking-only interrupted output must still persist"
    );
    assert_eq!(messages[0].content, "");
    assert_eq!(
        messages[0].thinking_content.as_deref(),
        Some("thinking before the failure")
    );
    assert!(messages[0].interrupted);
}

#[tokio::test]
async fn normal_finalize_persists_message_not_interrupted_and_emits_completion() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let stream_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(conversation_id, active_stream_entry(stream_id));

    let transcript = streaming::StreamTranscript {
        response_text: "complete answer".to_string(),
        completed: true,
        ..streaming::StreamTranscript::default()
    };

    streaming::finalize_stream_task(
        &conversation_service,
        conversation_id,
        stream_id,
        CompressionResult {
            messages: vec![],
            phase: crate::models::CompressionPhase::None,
            masked_tool_seqs: None,
            summary_range: None,
            preserved_facts: None,
            estimated_tokens: 0,
        },
        transcript,
        &active_streams,
        "Normal Model",
    )
    .await;

    let messages = conversation_service_impl.messages.read().await.clone();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "complete answer");
    assert_eq!(messages[0].model_id.as_deref(), Some("Normal Model"));
    assert!(
        !messages[0].interrupted,
        "normally finalized messages must not carry the interrupted marker"
    );

    assert!(
        conversation_service_impl
            .context_state
            .read()
            .await
            .is_some(),
        "normal finalize must persist context state"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ChatEvent::StreamCompleted { .. })),
        "normal finalize must emit StreamCompleted, got {events:?}"
    );
    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&conversation_id),
        "normal finalize must clear the active stream"
    );
}

/// A `CompressionResult` that leaves the conversation's context untouched
/// apart from what the finalize path itself writes.
fn no_compression() -> CompressionResult {
    CompressionResult {
        messages: vec![],
        phase: crate::models::CompressionPhase::None,
        masked_tool_seqs: None,
        summary_range: None,
        preserved_facts: None,
        estimated_tokens: 0,
    }
}

/// The completed branch of the dispatch must behave like normal finalization:
/// persist the message without the interrupted marker, update context state,
/// emit `StreamCompleted`, and clear the active stream (issue #193).
#[tokio::test]
async fn finalize_dispatch_completed_persists_updates_context_and_emits_completion() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let stream_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(conversation_id, active_stream_entry(stream_id));

    let transcript = streaming::StreamTranscript {
        response_text: "complete answer".to_string(),
        input_tokens: Some(3),
        output_tokens: Some(5),
        completed: true,
        ..streaming::StreamTranscript::default()
    };

    streaming::finalize_by_outcome(
        &finalize_context(
            &conversation_service,
            conversation_id,
            stream_id,
            &active_streams,
        ),
        no_compression(),
        transcript,
        "Dispatch Model",
    )
    .await;

    let messages = conversation_service_impl.messages.read().await.clone();
    assert_eq!(
        messages.len(),
        1,
        "completed dispatch must persist the message"
    );
    assert_eq!(messages[0].content, "complete answer");
    assert!(
        !messages[0].interrupted,
        "completed dispatch must not mark the message interrupted"
    );
    assert!(
        conversation_service_impl
            .context_state
            .read()
            .await
            .is_some(),
        "completed dispatch must persist context state"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    let total_tokens = events
        .iter()
        .find_map(|event| match event {
            ChatEvent::StreamCompleted { total_tokens, .. } => Some(*total_tokens),
            _ => None,
        })
        .expect("completed dispatch must emit StreamCompleted");
    assert_eq!(total_tokens, Some(8));
    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&conversation_id),
        "completed dispatch must clear the active stream"
    );
}

/// The errored-but-incomplete branch must persist partial output as
/// interrupted while skipping context state and `StreamCompleted`
/// (issue #193).
#[tokio::test]
async fn finalize_dispatch_errored_incomplete_persists_interrupted_without_context_or_completion() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let stream_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(conversation_id, active_stream_entry(stream_id));

    let transcript = streaming::StreamTranscript {
        response_text: "partial answer".to_string(),
        error: Some("provider reset the connection".to_string()),
        ..streaming::StreamTranscript::default()
    };

    streaming::finalize_by_outcome(
        &finalize_context(
            &conversation_service,
            conversation_id,
            stream_id,
            &active_streams,
        ),
        no_compression(),
        transcript,
        "Dispatch Model",
    )
    .await;

    let messages = conversation_service_impl.messages.read().await.clone();
    assert_eq!(
        messages.len(),
        1,
        "errored incomplete dispatch must persist the partial output"
    );
    assert_eq!(messages[0].content, "partial answer");
    assert!(
        messages[0].interrupted,
        "errored incomplete dispatch must mark the message interrupted"
    );
    assert!(
        conversation_service_impl
            .context_state
            .read()
            .await
            .is_none(),
        "errored incomplete dispatch must not update context state"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ChatEvent::StreamCompleted { .. })),
        "errored incomplete dispatch must not emit StreamCompleted, got {events:?}"
    );
    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&conversation_id),
        "errored incomplete dispatch must clear the active stream"
    );
}

/// The benign incomplete branch (e.g. user cancelled) must persist nothing,
/// emit no completion, and still clear the active stream (issue #193).
#[tokio::test]
async fn finalize_dispatch_benign_incomplete_persists_nothing_and_clears_stream_state() {
    let conversation_service_impl = Arc::new(MockConversationService::new(Uuid::new_v4()));
    let conversation_service =
        conversation_service_impl.clone() as Arc<dyn crate::services::ConversationService>;
    let conversation_id = Uuid::new_v4();
    let stream_id = Uuid::new_v4();
    let mut bus_rx = subscribe();
    let active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    active_streams
        .lock()
        .expect("active_streams poisoned")
        .insert(conversation_id, active_stream_entry(stream_id));

    let transcript = streaming::StreamTranscript {
        response_text: "user stopped this mid-flight".to_string(),
        ..streaming::StreamTranscript::default()
    };

    streaming::finalize_by_outcome(
        &finalize_context(
            &conversation_service,
            conversation_id,
            stream_id,
            &active_streams,
        ),
        no_compression(),
        transcript,
        "Dispatch Model",
    )
    .await;

    assert!(
        conversation_service_impl.messages.read().await.is_empty(),
        "benign incomplete dispatch must not persist any message"
    );
    assert!(
        conversation_service_impl
            .context_state
            .read()
            .await
            .is_none(),
        "benign incomplete dispatch must not update context state"
    );

    let events = collect_chat_events(
        &mut bus_rx,
        conversation_id,
        std::time::Duration::from_millis(250),
    )
    .await;
    assert!(
        events
            .iter()
            .all(|event| !matches!(event, ChatEvent::StreamCompleted { .. })),
        "benign incomplete dispatch must not emit StreamCompleted, got {events:?}"
    );
    assert!(
        !active_streams
            .lock()
            .expect("active_streams poisoned")
            .contains_key(&conversation_id),
        "benign incomplete dispatch must clear the active stream"
    );
}

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
