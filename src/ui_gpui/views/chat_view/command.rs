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

use super::state::{ApprovalBubbleState, ToolApprovalBubble};
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

        // Try to find an existing pending bubble to group with
        if let Some(existing) = self
            .state
            .approval_bubbles
            .iter_mut()
            .find(|b| b.can_group_with(&context))
        {
            // Group with existing bubble
            let details = context.details.clone();
            existing.add_operation(request_id, details);
            cx.notify();
            return;
        }

        // Create new bubble
        self.state
            .approval_bubbles
            .push(ToolApprovalBubble::new(request_id, context));
        self.maybe_scroll_chat_to_bottom(cx);
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
                .iter()
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
            self.state
                .approval_bubbles
                .retain(|b| b.state != ApprovalBubbleState::Pending);
        }
        cx.notify();
    }

    /// Handle incoming `ViewCommands` that are NOT store-managed.
    ///
    /// All shared state commands arrive exclusively through
    /// `apply_store_snapshot` via the store subscription. This method
    /// only handles ephemeral / view-local commands.
    #[allow(clippy::too_many_lines)]
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ConversationCleared => {
                self.state.messages.clear();
                self.state.streaming = super::state::StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.export_feedback_message = None;
                self.state.export_feedback_is_error = false;
                self.state.export_feedback_path = None;
                self.state.approval_bubbles.clear();
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.state.sync_conversation_title_from_active();
                cx.notify();
            }
            ViewCommand::ToggleThinkingVisibility => {
                self.state.show_thinking = !self.state.show_thinking;
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
            ViewCommand::ShowNotification { message } => {
                if Self::is_export_notification(&message) {
                    self.state.export_feedback_message = Some(message);
                    self.state.export_feedback_is_error = false;
                    self.state.export_feedback_path = None;
                    cx.notify();
                }
            }
            ViewCommand::ShowError {
                title,
                message,
                severity: _,
            } => {
                if Self::is_export_error(&title) {
                    self.state.export_feedback_message = Some(format!("{title}: {message}"));
                    self.state.export_feedback_is_error = true;
                    self.state.export_feedback_path = None;
                    cx.notify();
                }
            }
            ViewCommand::ToolApprovalRequest {
                request_id,
                context,
            } => {
                self.handle_tool_approval_request(request_id, context, cx);
            }
            ViewCommand::ToolApprovalResolved {
                request_id,
                approved,
            } => {
                // Find the bubble containing this request_id
                if let Some(bubble) = self
                    .state
                    .approval_bubbles
                    .iter_mut()
                    .find(|b| b.request_ids.contains(&request_id))
                {
                    bubble.state = if approved {
                        ApprovalBubbleState::Approved
                    } else {
                        ApprovalBubbleState::Denied
                    };
                }
                // Remove resolved bubbles so they don't accumulate.
                self.state
                    .approval_bubbles
                    .retain(|b| b.state == ApprovalBubbleState::Pending);
                cx.notify();
            }
            ViewCommand::YoloModeChanged { active } => {
                self.handle_yolo_mode_changed(active, cx);
            }
            ViewCommand::ConversationSearchResults { results } => {
                if results.is_empty() && self.state.sidebar_search_query.is_empty() {
                    self.state.sidebar_search_results = None;
                } else {
                    self.state.sidebar_search_results = Some(results);
                }
                cx.notify();
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
}
