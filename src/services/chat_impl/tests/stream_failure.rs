//! Behavioral tests for chat stream failure handling.
//!
//! Covers issue #193: stream failures must carry explicit outcome state,
//! finalize as interrupted (partial output persisted with an interrupted
//! marker, no context-state update, no completion event), and retain
//! actionable diagnostics.
//!
//! The tests are grouped by concern in child modules: outcome recording and
//! diagnostics, stream finalization, and log redaction. Helpers shared by
//! more than one group stay here.

use super::*;
use crate::events::types::ChatEvent;
use crate::events::AppEvent;

mod finalization;
mod log_capture;
mod outcome_reporting;

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
        }
        | ChatEvent::SteeringQueued {
            conversation_id, ..
        }
        | ChatEvent::SteeringDelivered {
            conversation_id, ..
        }
        | ChatEvent::SteeringDiscarded {
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
