//! Stream finalization: interrupted persistence (partial output saved with
//! an interrupted marker, no context-state update, no completion event),
//! normal completion, and outcome-based finalize dispatch (issue #193).

use super::*;
use crate::events::subscribe;
use crate::events::types::ChatEvent;

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
