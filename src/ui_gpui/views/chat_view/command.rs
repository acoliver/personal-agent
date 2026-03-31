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
use crate::presentation::view_command::ViewCommand;

impl ChatView {
    fn is_export_notification(message: &str) -> bool {
        message.contains("Conversation saved") || message.contains("No active conversation to save")
    }

    fn is_export_error(title: &str) -> bool {
        title == "Save Conversation"
    }

    /// Handle incoming `ViewCommands` that are NOT store-managed.
    ///
    /// All shared state commands arrive exclusively through
    /// `apply_store_snapshot` via the store subscription. This method
    /// only handles ephemeral / view-local commands.
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
            ViewCommand::ShowNotification { message } => {
                if Self::is_export_notification(&message) {
                    self.state.export_feedback_message = Some(message);
                    self.state.export_feedback_is_error = false;
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
                    cx.notify();
                }
            }
            ViewCommand::ToolApprovalRequest {
                request_id,
                tool_name,
                tool_argument,
            } => {
                self.state.approval_bubbles.push(ToolApprovalBubble {
                    request_id,
                    tool_name,
                    tool_argument,
                    state: ApprovalBubbleState::Pending,
                });
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::ToolApprovalResolved {
                request_id,
                approved,
            } => {
                if let Some(bubble) = self
                    .state
                    .approval_bubbles
                    .iter_mut()
                    .find(|b| b.request_id == request_id)
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
                self.state.yolo_mode = active;
                cx.notify();
            }
            _ => {}
        }
    }
}
