//! `ChatView::handle_command` — `ViewCommand` dispatch.
//!
//! Store-managed commands (conversation list, activation, streaming,
//! thinking, profiles, etc.) are handled exclusively via
//! `apply_store_snapshot`. This dispatch is reserved for commands the
//! store does NOT own:
//!
//! - `ConversationCleared` — resets ephemeral UI state and emits
//!   `RefreshHistory` side-effect.
//! - `ToggleThinkingVisibility` — view-local toggle.
//! - Export feedback commands — view-local display state.
//!
//! @plan PLAN-20250130-GPUIREDUX.P04

use super::state::{ApprovalBubbleState, QueuedSteeringEntry, ToolApprovalBubble};
use super::ChatView;
use crate::events::types::{ToolApprovalResponseAction, UserEvent};
use crate::presentation::view_command::{ToolApprovalContext, ViewCommand};

impl ChatView {
    fn is_export_notification(message: &str) -> bool {
        message.contains("Conversation saved") || message.contains("No active conversation to save")
    }

    fn is_export_error(title: &str) -> bool {
        title == "Save Conversation"
    }

    fn handle_tool_approval_request(
        &mut self,
        conversation_id: uuid::Uuid,
        request_id: String,
        context: ToolApprovalContext,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.yolo_mode {
            self.emit(UserEvent::ToolApprovalResponse {
                request_id,
                decision: ToolApprovalResponseAction::ProceedOnce,
            });
            cx.notify();
            return;
        }

        let is_visible_conversation = self.state.active_conversation_id == Some(conversation_id);

        let bubbles = self
            .state
            .approval_bubbles
            .entry(conversation_id)
            .or_default();

        // Try to find an existing pending bubble to group with
        if let Some(existing) = bubbles.iter_mut().find(|b| b.can_group_with(&context)) {
            // Group with existing bubble
            let details = context.details.clone();
            existing.add_operation(request_id, details);
            if is_visible_conversation {
                cx.notify();
            }
            return;
        }

        // Create new bubble
        bubbles.push(ToolApprovalBubble::new(request_id, context));
        if is_visible_conversation {
            self.maybe_scroll_chat_to_bottom();
            cx.notify();
        }
    }

    /// Record the user's decision on a tool approval and retire the bubble.
    ///
    /// Extracted from `handle_command` unchanged; the dispatch had grown past
    /// the length the complexity gate allows.
    fn handle_tool_approval_resolved(
        &mut self,
        conversation_id: uuid::Uuid,
        request_id: &str,
        approved: bool,
        cx: &mut gpui::Context<Self>,
    ) {
        // Find the bubble containing this request_id in the owning conversation bucket.
        if let Some(bubbles) = self.state.approval_bubbles.get_mut(&conversation_id) {
            if let Some(bubble) = bubbles
                .iter_mut()
                .find(|b| b.request_ids.iter().any(|id| id == request_id))
            {
                bubble.state = if approved {
                    ApprovalBubbleState::Approved
                } else {
                    ApprovalBubbleState::Denied
                };
            }
            // Remove resolved bubbles so they don't accumulate.
            bubbles.retain(|b| b.state == ApprovalBubbleState::Pending);
            if bubbles.is_empty() {
                self.state.approval_bubbles.remove(&conversation_id);
            }
        }
        cx.notify();
    }

    /// Handle YOLO mode activation - auto-approve any pending tool approval bubbles.
    fn handle_yolo_mode_changed(&mut self, active: bool, cx: &mut gpui::Context<Self>) {
        self.state.yolo_mode = active;
        if active {
            // Retroactively auto-approve any bubbles that arrived before YOLO was confirmed
            // Use flat_map to emit for all request_ids in grouped bubbles
            let pending_ids: Vec<String> = self
                .state
                .approval_bubbles
                .values()
                .flat_map(|bubbles| bubbles.iter())
                .filter(|b| b.state == ApprovalBubbleState::Pending)
                .flat_map(|b| b.request_ids.clone())
                .collect();

            for request_id in pending_ids {
                self.emit(UserEvent::ToolApprovalResponse {
                    request_id,
                    decision: ToolApprovalResponseAction::ProceedOnce,
                });
            }

            // Drop all pending bubbles — they've been auto-approved
            for bubbles in self.state.approval_bubbles.values_mut() {
                bubbles.retain(|b| b.state != ApprovalBubbleState::Pending);
            }
            self.state
                .approval_bubbles
                .retain(|_, bubbles| !bubbles.is_empty());
        }
        cx.notify();
    }

    fn handle_conversation_cleared(&mut self, cx: &mut gpui::Context<Self>) {
        let cleared_conversation_id = self.state.active_conversation_id;
        self.state.messages.clear();
        self.state.streaming = super::state::StreamingState::Idle;
        self.state.thinking_content = None;
        self.state.conversation_dropdown_open = false;
        self.state.conversation_title_editing = false;
        self.state.conversation_title_input.clear();
        self.state.export_feedback_message = None;
        self.state.export_feedback_is_error = false;
        self.state.export_feedback_path = None;
        if let Some(conversation_id) = cleared_conversation_id {
            self.state.approval_bubbles.remove(&conversation_id);
        }
        // Queued steers belong to the turn that was just cleared away.
        // @plan PLAN-20260903-ISSUE222.P04
        // @requirement REQ-222-003
        self.state.queued_steering.clear();
        self.state.chat_autoscroll_enabled = true;
        self.scroll_transcript_to_bottom();
        self.state.sync_conversation_title_from_active();
        self.refresh_transcript_selection_revisions();
        cx.notify();
    }

    /// Show a steering message as waiting for the running turn's boundary.
    ///
    /// Scoped to the conversation on screen the way the approval commands
    /// are: `queued_steering` describes the transcript being rendered, and a
    /// steer accepted for another conversation is not part of it.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-003
    fn handle_steering_queued(
        &mut self,
        conversation_id: uuid::Uuid,
        steer_id: uuid::Uuid,
        text: String,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.active_conversation_id != Some(conversation_id) {
            return;
        }
        self.state
            .queued_steering
            .push(QueuedSteeringEntry::new(steer_id, text));
        self.maybe_scroll_chat_to_bottom();
        cx.notify();
    }

    /// Stop showing a steering message as waiting: it reached the model.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @requirement REQ-222-003
    fn handle_steering_delivered(
        &mut self,
        conversation_id: uuid::Uuid,
        steer_id: uuid::Uuid,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.active_conversation_id != Some(conversation_id) {
            return;
        }
        self.state
            .queued_steering
            .retain(|entry| entry.id != steer_id);
        cx.notify();
    }

    /// Stop showing a steering message as waiting: the turn it was waiting on
    /// ended, so it is never reaching the model.
    ///
    /// The removal is the one delivery performs, because the entry's job is
    /// the same either way: it says an instruction is still pending. Once
    /// that stops being true the entry has to go, or it waits on screen for
    /// the rest of the session.
    ///
    /// @plan PLAN-20260903-ISSUE222.P06
    /// @requirement REQ-222-003
    fn handle_steering_discarded(
        &mut self,
        conversation_id: uuid::Uuid,
        steer_id: uuid::Uuid,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.active_conversation_id != Some(conversation_id) {
            return;
        }
        self.state
            .queued_steering
            .retain(|entry| entry.id != steer_id);
        cx.notify();
    }

    /// A steer the service refused.
    ///
    /// This withdraws no queued entry. A refusal means the service queued
    /// nothing, so there is none under it to remove, and removing one anyway
    /// would take away a steer that is still waiting. The accompanying
    /// `ShowError` is what the user sees.
    ///
    /// What it does do is give the words back. Submitting clears the
    /// composer without waiting for an answer, so a refused steer would
    /// otherwise be typed, taken away and never seen again. The text goes
    /// back only into the conversation it was typed in, and only into a
    /// composer that is still empty: the round trip leaves room for the
    /// user to have moved to another conversation or started something
    /// new, and whatever is on their screen now outranks the draft the
    /// service turned down.
    ///
    /// @plan PLAN-20260903-ISSUE222.P04
    /// @plan PLAN-20260903-ISSUE222.P08
    /// @requirement REQ-222-002
    /// @requirement REQ-222-004
    fn handle_steering_rejected(
        &mut self,
        conversation_id: uuid::Uuid,
        error: &str,
        text: String,
        cx: &mut gpui::Context<Self>,
    ) {
        tracing::warn!(%conversation_id, "Steering message refused: {error}");
        if self.state.active_conversation_id != Some(conversation_id)
            || !self.state.input_text.is_empty()
        {
            return;
        }
        self.state.input_text = text;
        self.state.cursor_position = self.state.input_text.len();
        cx.notify();
    }

    fn handle_conversation_search_results(
        &mut self,
        results: Vec<crate::presentation::view_command::ConversationSearchResult>,
        cx: &mut gpui::Context<Self>,
    ) {
        if results.is_empty() && self.state.sidebar_search_query.is_empty() {
            self.state.sidebar_search_results = None;
        } else {
            self.state.sidebar_search_results = Some(results);
        }
        cx.notify();
    }

    /// Handle incoming `ViewCommands` that are NOT store-managed.
    ///
    /// All shared state commands arrive exclusively through
    /// `apply_store_snapshot` via the store subscription. This method
    /// only handles ephemeral / view-local commands.
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ConversationCleared | ViewCommand::ClearActiveConversation => {
                self.handle_conversation_cleared(cx);
            }
            ViewCommand::ToggleThinkingVisibility => {
                self.state.show_thinking = !self.state.show_thinking;
                self.refresh_transcript_selection_revisions();
                cx.notify();
            }
            cmd @ (ViewCommand::CodexReauthRequired { .. }
            | ViewCommand::CodexSignInCompleted { .. }) => {
                self.apply_codex_command(cmd, cx);
            }
            ViewCommand::SetEmojiFilterVisibility { enabled } => {
                tracing::info!(
                    "ChatView: SetEmojiFilterVisibility received, enabled={}",
                    enabled
                );
                self.state.filter_emoji = enabled;
                self.refresh_transcript_selection_revisions();
                cx.notify();
            }
            ViewCommand::ShowConversationExportFormat { format } => {
                self.state.conversation_export_format = format;
                cx.notify();
            }
            ViewCommand::ExportCompleted { path, format_label } => {
                self.state.export_feedback_message =
                    Some(format!("Conversation saved as {path} ({format_label})"));
                self.state.export_feedback_is_error = false;
                self.state.export_feedback_path = Some(path);
                cx.notify();
            }
            ViewCommand::ShowNotification { message } if Self::is_export_notification(&message) => {
                self.state.export_feedback_message = Some(message);
                self.state.export_feedback_is_error = false;
                self.state.export_feedback_path = None;
                cx.notify();
            }
            ViewCommand::ShowError {
                title,
                message,
                severity: _,
            } if Self::is_export_error(&title) => {
                self.state.export_feedback_message = Some(format!("{title}: {message}"));
                self.state.export_feedback_is_error = true;
                self.state.export_feedback_path = None;
                cx.notify();
            }
            ViewCommand::ToolApprovalRequest {
                conversation_id,
                request_id,
                context,
            } => {
                self.handle_tool_approval_request(conversation_id, request_id, context, cx);
            }
            ViewCommand::ToolApprovalResolved {
                conversation_id,
                request_id,
                approved,
            } => {
                self.handle_tool_approval_resolved(conversation_id, &request_id, approved, cx);
            }
            ViewCommand::SteeringQueued {
                conversation_id,
                steer_id,
                text,
            } => {
                self.handle_steering_queued(conversation_id, steer_id, text, cx);
            }
            ViewCommand::SteeringDelivered {
                conversation_id,
                steer_id,
            } => {
                self.handle_steering_delivered(conversation_id, steer_id, cx);
            }
            ViewCommand::SteeringDiscarded {
                conversation_id,
                steer_id,
            } => {
                self.handle_steering_discarded(conversation_id, steer_id, cx);
            }
            ViewCommand::SteeringRejected {
                conversation_id,
                error,
                text,
            } => {
                self.handle_steering_rejected(conversation_id, &error, text, cx);
            }
            ViewCommand::YoloModeChanged { active } => {
                self.handle_yolo_mode_changed(active, cx);
            }
            ViewCommand::ConversationSearchResults { results } => {
                self.handle_conversation_search_results(results, cx);
            }
            _ => {}
        }
    }

    /// Toggle sidebar visibility (popout mode).
    pub fn toggle_sidebar(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.sidebar_visible = !self.state.sidebar_visible;
        cx.notify();
    }

    /// Emit a search event for the current sidebar search query.
    pub fn trigger_sidebar_search(&mut self, cx: &mut gpui::Context<Self>) {
        let query = self.state.sidebar_search_query.clone();
        if query.trim().is_empty() {
            self.state.sidebar_search_results = None;
        } else {
            self.emit(crate::events::types::UserEvent::SearchConversations { query });
        }
        cx.notify();
    }

    /// Drive the expired-session banner.
    fn apply_codex_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::CodexReauthRequired { account } => {
                self.state.codex_reauth_account = Some(account);
                cx.notify();
            }
            // The banner names one account; clear it only when that account is
            // the one that just signed back in.
            ViewCommand::CodexSignInCompleted { account, .. }
                if self.state.codex_reauth_account.as_deref() == Some(account.as_str()) =>
            {
                self.state.codex_reauth_account = None;
                cx.notify();
            }
            _ => {}
        }
    }
}
