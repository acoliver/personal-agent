//! `ChatView::handle_command` — `ViewCommand` dispatch.
//!
//! Each `ViewCommand` arm updates `self.state` in place and calls
//! `cx.notify()` to schedule a redraw. Streaming and conversation-list
//! commands are the two dominant groups here.
//!
//! @plan PLAN-20260325-ISSUE11B.P02
//! @plan PLAN-20250130-GPUIREDUX.P04

use super::state::{ChatMessage, StreamingState};
use super::ChatView;
use crate::presentation::view_command::{ConversationSummary, ViewCommand};
use uuid::Uuid;

impl ChatView {
    /// Handle incoming `ViewCommands`.
    ///
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        #[allow(clippy::enum_glob_use)]
        use ViewCommand::*;
        match cmd {
            // ── message load ────────────────────────────────────────────
            ConversationMessagesLoaded { .. } | MessageAppended { .. } => {
                self.handle_message_command(cmd, cx);
            }

            // ── thinking state ──────────────────────────────────────────
            ShowThinking { .. }
            | HideThinking { .. }
            | AppendThinking { .. }
            | ToggleThinkingVisibility => self.handle_thinking_command(cmd, cx),

            // ── stream lifecycle ─────────────────────────────────────────
            AppendStream { .. }
            | FinalizeStream { .. }
            | StreamCancelled { .. }
            | StreamError { .. } => self.handle_stream_lifecycle_command(cmd, cx),

            // ── conversation list / activation ──────────────────────────
            ConversationListRefreshed { .. }
            | ConversationActivated { .. }
            | ConversationCreated { .. } => self.handle_conversation_list_command(cmd, cx),

            // ── conversation mutations ──────────────────────────────────
            ConversationRenamed { .. }
            | ConversationTitleUpdated { .. }
            | ConversationDeleted { .. }
            | ConversationCleared => self.handle_conversation_update_command(cmd, cx),

            // ── profiles ────────────────────────────────────────────────
            ChatProfilesUpdated { .. } | ShowSettings { .. } | DefaultProfileChanged { .. } => {
                self.handle_profile_command(cmd, cx);
            }

            _ => {}
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────

    fn handle_message_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ConversationMessagesLoaded {
                conversation_id,
                selection_generation,
                messages,
            } => {
                if self.state.active_conversation_id != Some(conversation_id) {
                    tracing::info!(%conversation_id, "ChatView: ignoring ConversationMessagesLoaded for inactive conversation");
                    return;
                }
                if selection_generation != self.selection_generation {
                    tracing::info!(
                        %conversation_id,
                        selection_generation,
                        current_generation = self.selection_generation,
                        "ChatView: ignoring stale ConversationMessagesLoaded generation"
                    );
                    return;
                }
                let message_count = messages.len();
                let current_model = self.state.current_model.clone();
                self.state.messages = Self::messages_from_payload(messages, &current_model);
                tracing::info!(%conversation_id, message_count, "ChatView: applied ConversationMessagesLoaded");
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::MessageAppended {
                conversation_id,
                role,
                content,
            } => {
                if self.state.active_conversation_id != Some(conversation_id) {
                    return;
                }
                let chat_msg = match role {
                    crate::presentation::view_command::MessageRole::User => {
                        ChatMessage::user(content)
                    }
                    crate::presentation::view_command::MessageRole::Assistant => {
                        ChatMessage::assistant(content, self.state.current_model.clone())
                    }
                    _ => return,
                };
                self.state.messages.push(chat_msg);
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            _ => {}
        }
    }

    fn handle_thinking_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ShowThinking { conversation_id } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                if !matches!(self.state.streaming, StreamingState::Streaming { .. }) {
                    self.state.streaming = StreamingState::Streaming {
                        content: String::new(),
                        done: false,
                    };
                }
                self.state.thinking_content = Some(String::new());
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::HideThinking { conversation_id } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                self.state.thinking_content = None;
                cx.notify();
            }
            ViewCommand::AppendThinking {
                conversation_id,
                content,
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                self.state.thinking_content =
                    Some(self.state.thinking_content.clone().unwrap_or_default() + &content);
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::ToggleThinkingVisibility => {
                self.state.show_thinking = !self.state.show_thinking;
                cx.notify();
            }
            _ => {}
        }
    }

    fn handle_stream_lifecycle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::AppendStream {
                conversation_id,
                chunk,
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                match &mut self.state.streaming {
                    StreamingState::Streaming { content, .. } => {
                        content.push_str(&chunk);
                    }
                    StreamingState::Idle => {
                        self.state.streaming = StreamingState::Streaming {
                            content: chunk,
                            done: false,
                        };
                    }
                    StreamingState::Error(_) => {}
                }
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::FinalizeStream {
                conversation_id, ..
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                let thinking_content = self
                    .state
                    .thinking_content
                    .take()
                    .filter(|thinking| !thinking.is_empty());
                if let StreamingState::Streaming { content, .. } = &self.state.streaming {
                    let mut msg =
                        ChatMessage::assistant(content.clone(), self.state.current_model.clone());
                    if let Some(thinking) = thinking_content {
                        msg = msg.with_thinking(thinking);
                    }
                    self.state.messages.push(msg);
                }
                self.state.streaming = StreamingState::Idle;
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::StreamCancelled {
                conversation_id,
                partial_content,
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                if !partial_content.is_empty() {
                    let mut msg =
                        ChatMessage::assistant(partial_content, self.state.current_model.clone());
                    msg.content.push_str(" [cancelled]");
                    self.state.messages.push(msg);
                }
                self.state.streaming = StreamingState::Idle;
                self.maybe_scroll_chat_to_bottom(cx);
                cx.notify();
            }
            ViewCommand::StreamError {
                conversation_id,
                error,
                ..
            } => {
                if conversation_id != Uuid::nil()
                    && self.state.active_conversation_id != Some(conversation_id)
                {
                    return;
                }
                self.state.streaming = StreamingState::Error(error);
                cx.notify();
            }
            _ => {}
        }
    }

    fn handle_conversation_list_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ConversationListRefreshed { conversations } => {
                let conversation_count = conversations.len();
                tracing::info!(
                    count = conversation_count,
                    "ChatView: received ConversationListRefreshed"
                );
                let previous_active = self.state.active_conversation_id.or(self.conversation_id);
                self.state.conversations = conversations;
                self.apply_conversation_list_refresh(previous_active);
                cx.notify();
            }
            ViewCommand::ConversationActivated {
                id,
                selection_generation,
            } => {
                self.state.active_conversation_id = Some(id);
                self.conversation_id = Some(id);
                self.selection_generation = selection_generation;
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.profile_dropdown_anchor_x = None;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();

                if !self.state.conversation_title_editing {
                    self.state.conversation_title_input.clear();
                    if let Some(conversation) = self
                        .state
                        .conversations
                        .iter()
                        .find(|conversation| conversation.id == id)
                    {
                        self.state.conversation_title = if conversation.title.trim().is_empty() {
                            "Untitled Conversation".to_string()
                        } else {
                            conversation.title.clone()
                        };
                    }
                }

                self.state.sync_conversation_dropdown_index();
                cx.notify();
            }
            ViewCommand::ConversationCreated { id, .. } => {
                self.state.active_conversation_id = Some(id);
                self.conversation_id = Some(id);
                self.selection_generation = 0;
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.profile_dropdown_anchor_x = None;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.state.conversation_title = "New Conversation".to_string();

                if !self.state.conversations.iter().any(|c| c.id == id) {
                    let now = chrono::Utc::now();
                    self.state.conversations.insert(
                        0,
                        ConversationSummary {
                            id,
                            title: "New Conversation".to_string(),
                            updated_at: now,
                            message_count: 0,
                        },
                    );
                }

                self.state.sync_conversation_dropdown_index();
                cx.notify();
            }
            _ => {}
        }
    }

    /// Post-refresh logic for `ConversationListRefreshed`.
    fn apply_conversation_list_refresh(&mut self, previous_active: Option<Uuid>) {
        if self.state.conversations.is_empty() {
            self.state.active_conversation_id = None;
            self.conversation_id = None;
            self.state.messages.clear();
            self.state.streaming = StreamingState::Idle;
            self.state.thinking_content = None;
            self.state.conversation_dropdown_open = false;
            self.state.conversation_dropdown_index = 0;
            if !self.state.conversation_title_editing {
                self.state.conversation_title = "New Conversation".to_string();
            }
        } else {
            let active_exists = previous_active
                .is_some_and(|id| self.state.conversations.iter().any(|c| c.id == id));
            if active_exists {
                self.state.active_conversation_id = previous_active;
                self.conversation_id = previous_active;
            } else {
                let fallback = self.state.conversations[0].id;
                self.state.active_conversation_id = Some(fallback);
                self.conversation_id = Some(fallback);
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.chat_autoscroll_enabled = true;
            }
            self.state.sync_conversation_dropdown_index();
            if !self.state.conversation_title_editing {
                self.state.sync_conversation_title_from_active();
            }
        }
    }

    fn handle_conversation_update_command(
        &mut self,
        cmd: ViewCommand,
        cx: &mut gpui::Context<Self>,
    ) {
        match cmd {
            ViewCommand::ConversationRenamed { id, new_title } => {
                if let Some(conversation) = self.state.conversations.iter_mut().find(|c| c.id == id)
                {
                    conversation.title.clone_from(&new_title);
                    conversation.updated_at = chrono::Utc::now();
                }
                if self.state.active_conversation_id == Some(id) {
                    self.state.conversation_title = new_title;
                }
                cx.notify();
            }
            ViewCommand::ConversationTitleUpdated { id, title } => {
                if let Some(conversation) = self.state.conversations.iter_mut().find(|c| c.id == id)
                {
                    conversation.title.clone_from(&title);
                    conversation.updated_at = chrono::Utc::now();
                }
                if self.state.active_conversation_id == Some(id) {
                    self.state.conversation_title = title;
                }
                cx.notify();
            }
            ViewCommand::ConversationDeleted { id } => {
                self.state.conversations.retain(|c| c.id != id);
                if self.state.active_conversation_id == Some(id) {
                    self.state.active_conversation_id =
                        self.state.conversations.first().map(|c| c.id);
                    self.conversation_id = self.state.active_conversation_id;
                    self.state.messages.clear();
                    self.state.streaming = StreamingState::Idle;
                    self.state.thinking_content = None;
                }
                self.state.sync_conversation_dropdown_index();
                self.state.sync_conversation_title_from_active();
                self.profile_dropdown_anchor_x = None;
                cx.notify();
            }
            ViewCommand::ConversationCleared => {
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.profile_dropdown_anchor_x = None;
                self.state.chat_autoscroll_enabled = true;
                self.chat_scroll_handle.scroll_to_bottom();
                self.state.sync_conversation_title_from_active();
                cx.notify();
            }
            _ => {}
        }
    }

    fn handle_profile_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ChatProfilesUpdated {
                profiles,
                selected_profile_id,
            }
            | ViewCommand::ShowSettings {
                profiles,
                selected_profile_id,
            } => {
                let profile_count = profiles.len();
                self.state.profiles = profiles;
                self.state.selected_profile_id = selected_profile_id.or_else(|| {
                    self.state
                        .profiles
                        .iter()
                        .find(|p| p.is_default)
                        .map(|p| p.id)
                });
                tracing::info!(
                    count = profile_count,
                    selected = ?self.state.selected_profile_id,
                    "ChatView: received profile snapshot"
                );
                self.state.sync_current_model_from_profile();
                self.state.sync_profile_dropdown_index();
                cx.notify();
            }
            ViewCommand::DefaultProfileChanged { profile_id } => {
                self.state.selected_profile_id = profile_id;
                for profile in &mut self.state.profiles {
                    profile.is_default = Some(profile.id) == profile_id;
                }
                self.state.sync_current_model_from_profile();
                self.state.sync_profile_dropdown_index();
                cx.notify();
            }
            _ => {}
        }
    }
}
