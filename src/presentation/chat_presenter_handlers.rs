use super::view_command::ErrorSeverity;
use super::{ChatPresenter, ViewCommand};
use crate::events::types::ChatEvent;
use tokio::sync::mpsc;
use uuid::Uuid;

impl ChatPresenter {
    /// Handle chat events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    pub(crate) async fn handle_chat_event(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: ChatEvent,
    ) {
        match event {
            ChatEvent::StreamStarted {
                conversation_id, ..
            } => {
                Self::handle_stream_started(view_tx, conversation_id).await;
            }
            ChatEvent::TextDelta { text } => {
                Self::handle_text_delta(view_tx, text).await;
            }
            ChatEvent::ThinkingDelta { text } => {
                Self::handle_thinking_delta(view_tx, text).await;
            }
            ChatEvent::ToolCallStarted { tool_name, .. } => {
                Self::handle_tool_call_started(view_tx, tool_name).await;
            }
            ChatEvent::ToolCallCompleted {
                tool_name,
                success,
                result,
                duration_ms,
                ..
            } => {
                Self::handle_tool_call_completed(view_tx, tool_name, success, result, duration_ms)
                    .await;
            }
            ChatEvent::StreamCompleted {
                conversation_id,
                total_tokens,
                ..
            } => {
                Self::handle_stream_completed(view_tx, conversation_id, total_tokens).await;
            }
            ChatEvent::StreamCancelled {
                conversation_id,
                partial_content,
                ..
            } => {
                Self::handle_stream_cancelled(view_tx, conversation_id, partial_content).await;
            }
            ChatEvent::StreamError {
                conversation_id,
                error,
                recoverable,
            } => {
                Self::handle_stream_error(view_tx, conversation_id, error, recoverable).await;
            }
            ChatEvent::MessageSaved {
                conversation_id, ..
            } => {
                Self::handle_message_saved(view_tx, conversation_id).await;
            }
        }
    }

    async fn handle_stream_started(view_tx: &mut mpsc::Sender<ViewCommand>, conversation_id: Uuid) {
        let _ = view_tx
            .send(ViewCommand::ShowThinking { conversation_id })
            .await;
    }

    async fn handle_text_delta(view_tx: &mut mpsc::Sender<ViewCommand>, text: String) {
        let _ = view_tx
            .send(ViewCommand::AppendStream {
                conversation_id: Uuid::nil(),
                chunk: text,
            })
            .await;
    }

    async fn handle_thinking_delta(view_tx: &mut mpsc::Sender<ViewCommand>, text: String) {
        let _ = view_tx
            .send(ViewCommand::AppendThinking {
                conversation_id: Uuid::nil(),
                content: text,
            })
            .await;
    }

    async fn handle_tool_call_started(view_tx: &mut mpsc::Sender<ViewCommand>, tool_name: String) {
        let _ = view_tx
            .send(ViewCommand::ShowToolCall {
                conversation_id: Uuid::nil(),
                tool_name,
                status: "running".to_string(),
            })
            .await;
    }

    async fn handle_tool_call_completed(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        tool_name: String,
        success: bool,
        result: String,
        duration_ms: u64,
    ) {
        let status = if success {
            "completed".to_string()
        } else {
            "failed".to_string()
        };
        let _ = view_tx
            .send(ViewCommand::UpdateToolCall {
                conversation_id: Uuid::nil(),
                tool_name,
                status,
                result: Some(result),
                duration: Some(duration_ms),
            })
            .await;
    }

    async fn handle_stream_completed(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        conversation_id: Uuid,
        total_tokens: Option<u32>,
    ) {
        let _ = view_tx
            .send(ViewCommand::FinalizeStream {
                conversation_id,
                tokens: u64::from(total_tokens.unwrap_or(0)),
            })
            .await;
        let _ = view_tx
            .send(ViewCommand::HideThinking { conversation_id })
            .await;
    }

    async fn handle_stream_cancelled(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        conversation_id: Uuid,
        partial_content: String,
    ) {
        let _ = view_tx
            .send(ViewCommand::StreamCancelled {
                conversation_id,
                partial_content,
            })
            .await;
        let _ = view_tx
            .send(ViewCommand::HideThinking { conversation_id })
            .await;
    }

    async fn handle_stream_error(
        view_tx: &mut mpsc::Sender<ViewCommand>,
        conversation_id: Uuid,
        error: String,
        recoverable: bool,
    ) {
        let _ = view_tx
            .send(ViewCommand::StreamError {
                conversation_id,
                error: error.clone(),
                recoverable,
            })
            .await;
        let _ = view_tx
            .send(ViewCommand::ShowError {
                title: "Stream Error".to_string(),
                message: error,
                severity: if recoverable {
                    ErrorSeverity::Warning
                } else {
                    ErrorSeverity::Error
                },
            })
            .await;
    }

    async fn handle_message_saved(view_tx: &mut mpsc::Sender<ViewCommand>, conversation_id: Uuid) {
        let _ = view_tx
            .send(ViewCommand::MessageSaved { conversation_id })
            .await;
    }
}
