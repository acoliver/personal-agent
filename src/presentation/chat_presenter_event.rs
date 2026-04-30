use tokio::sync::mpsc;
use uuid::Uuid;

use super::chat_presenter::{ChatPresenter, ChatPresenterDeps, ChatPresenterState};
use super::ViewCommand;
use crate::events::types::{ToolApprovalResponseAction, UserEvent};
use crate::models::ConversationExportFormat;

impl ChatPresenter {
    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.1
    #[allow(clippy::too_many_lines)]
    pub(super) async fn handle_user_event(
        deps: &ChatPresenterDeps<'_>,
        state: &ChatPresenterState<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SendMessage {
                text,
                conversation_id,
            } => {
                Self::handle_send_message_for_event(deps, state, view_tx, text, conversation_id)
                    .await;
            }
            UserEvent::StopStreaming { conversation_id } => {
                Self::handle_stop_streaming(deps.chat_service, view_tx, conversation_id).await;
            }
            UserEvent::NewConversation => {
                Self::handle_new_conversation_for_event(deps, state, view_tx).await;
            }
            UserEvent::ToggleThinking => {
                Self::handle_toggle_thinking(view_tx).await;
            }
            UserEvent::ToggleEmojiFilter => {
                Self::handle_toggle_emoji_filter(state.app_settings_service, view_tx).await;
            }
            UserEvent::ConfirmRenameConversation { id, title } => {
                Self::handle_rename_conversation_for_event(deps, view_tx, id, title).await;
            }
            UserEvent::SelectConversation {
                id,
                selection_generation,
            } => {
                Self::handle_select_conversation_for_event(deps, view_tx, id, selection_generation)
                    .await;
            }
            UserEvent::RefreshHistory | UserEvent::RefreshConversations => {
                let _ = Self::emit_conversation_list(deps.conversation_service, view_tx).await;
            }
            UserEvent::SelectConversationExportFormat { format } => {
                Self::handle_select_export_format_for_event(state, view_tx, format).await;
            }
            UserEvent::SaveConversation => {
                Self::handle_save_conversation_for_event(deps, state, view_tx).await;
            }
            UserEvent::SaveErrorLog { format } => {
                Self::handle_save_error_log(state.app_settings_service, view_tx, format).await;
            }
            UserEvent::SelectChatProfile { id } => {
                Self::handle_select_chat_profile(deps.conversation_service, id).await;
            }
            UserEvent::SetExportDirectory { path } => {
                Self::handle_set_export_directory(state.app_settings_service, view_tx, path).await;
            }
            UserEvent::ToolApprovalResponse {
                request_id,
                decision,
            } => {
                Self::handle_tool_approval_response_for_event(deps, view_tx, request_id, decision)
                    .await;
            }
            UserEvent::ToggleWindowMode => {
                let _ = view_tx.send(ViewCommand::ToggleWindowMode).await;
            }
            UserEvent::SearchConversations { query } => {
                Self::handle_search_conversations(deps.conversation_service, view_tx, query).await;
            }
            _ => {}
        }
    }

    async fn handle_send_message_for_event(
        deps: &ChatPresenterDeps<'_>,
        state: &ChatPresenterState<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        text: String,
        conversation_id: Option<Uuid>,
    ) {
        Self::handle_send_message(
            deps.conversation_service,
            deps.chat_service,
            deps.profile_service,
            view_tx,
            text,
            conversation_id,
            state.pending_draft_conversation_id,
        )
        .await;
    }

    async fn handle_new_conversation_for_event(
        deps: &ChatPresenterDeps<'_>,
        state: &ChatPresenterState<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        Self::handle_new_conversation(
            deps.conversation_service,
            deps.profile_service,
            view_tx,
            state.pending_draft_conversation_id,
        )
        .await;
    }

    async fn handle_rename_conversation_for_event(
        deps: &ChatPresenterDeps<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
        title: String,
    ) {
        Self::handle_rename_conversation(deps.conversation_service, view_tx, id, title).await;
    }

    async fn handle_select_conversation_for_event(
        deps: &ChatPresenterDeps<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        id: Uuid,
        selection_generation: u64,
    ) {
        Self::handle_select_conversation(
            deps.conversation_service,
            view_tx,
            id,
            selection_generation,
        )
        .await;
    }

    async fn handle_select_export_format_for_event(
        state: &ChatPresenterState<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
        format: ConversationExportFormat,
    ) {
        Self::handle_select_conversation_export_format(
            state.app_settings_service,
            state.current_export_format,
            view_tx,
            format,
        )
        .await;
    }

    async fn handle_save_conversation_for_event(
        deps: &ChatPresenterDeps<'_>,
        state: &ChatPresenterState<'_>,
        view_tx: &mut mpsc::Sender<ViewCommand>,
    ) {
        Self::handle_save_conversation(
            deps.conversation_service,
            state.app_settings_service,
            state.current_export_format,
            view_tx,
        )
        .await;
    }

    async fn handle_tool_approval_response_for_event(
        deps: &ChatPresenterDeps<'_>,
        view_tx: &mpsc::Sender<ViewCommand>,
        request_id: String,
        decision: ToolApprovalResponseAction,
    ) {
        Self::handle_tool_approval_response(deps.chat_service, view_tx, request_id, decision).await;
    }
}
