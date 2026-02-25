//! Chat view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P04
//! @requirement REQ-GPUI-003

use gpui::{div, prelude::*, px, SharedString, MouseButton, FocusHandle, FontWeight};
use crate::ui_gpui::components::AssistantBubble;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::presentation::view_command::{ConversationSummary, ProfileSummary, ViewCommand};
use crate::events::types::UserEvent;
use std::sync::Arc;
use uuid::Uuid;

/// Represents a single message in the chat (for UI display)
#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
    pub model_id: Option<String>,
    pub timestamp: Option<u64>,
}

/// Message role enum
#[derive(Clone, Debug, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            thinking: None,
            model_id: None,
            timestamp: None,
        }
    }

    pub fn assistant(content: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            thinking: None,
            model_id: Some(model_id.into()),
            timestamp: None,
        }
    }

    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Streaming state for AI responses
#[derive(Clone, Debug, PartialEq)]
pub enum StreamingState {
    Idle,
    Streaming { content: String, done: bool },
    Error(String),
}

/// Main chat state container
#[derive(Clone)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub streaming: StreamingState,
    pub show_thinking: bool,
    pub thinking_content: Option<String>,
    pub input_text: String,
    pub conversation_title: String,
    pub conversations: Vec<ConversationSummary>,
    pub active_conversation_id: Option<Uuid>,
    pub conversation_dropdown_open: bool,
    pub conversation_dropdown_index: usize,
    pub conversation_title_editing: bool,
    pub conversation_title_input: String,
    pub current_model: String,
    pub profiles: Vec<ProfileSummary>,
    pub selected_profile_id: Option<Uuid>,
    pub profile_dropdown_open: bool,
    pub profile_dropdown_index: usize,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            streaming: StreamingState::Idle,
            show_thinking: false,
            thinking_content: None,
            input_text: String::new(),
            conversation_title: "New Conversation".to_string(),
            conversations: Vec::new(),
            active_conversation_id: None,
            conversation_dropdown_open: false,
            conversation_dropdown_index: 0,
            conversation_title_editing: false,
            conversation_title_input: String::new(),
            current_model: "No profile selected".to_string(),
            profiles: Vec::new(),
            selected_profile_id: None,
            profile_dropdown_open: false,
            profile_dropdown_index: 0,
        }
    }
}

impl ChatState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_streaming(mut self, state: StreamingState) -> Self {
        self.streaming = state;
        self
    }

    pub fn with_thinking(mut self, enabled: bool, content: Option<String>) -> Self {
        self.show_thinking = enabled;
        self.thinking_content = content;
        self
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    pub fn set_streaming(&mut self, state: StreamingState) {
        self.streaming = state;
    }

    pub fn set_thinking(&mut self, enabled: bool, content: Option<String>) {
        self.show_thinking = enabled;
        self.thinking_content = content;
    }

    fn selected_conversation(&self) -> Option<&ConversationSummary> {
        self.active_conversation_id
            .and_then(|id| self.conversations.iter().find(|conversation| conversation.id == id))
            .or_else(|| self.conversations.first())
    }

    fn sync_conversation_title_from_active(&mut self) {
        self.conversation_title = self
            .selected_conversation()
            .map(|conversation| {
                if conversation.title.trim().is_empty() {
                    "Untitled Conversation".to_string()
                } else {
                    conversation.title.clone()
                }
            })
            .unwrap_or_else(|| "New Conversation".to_string());
    }

    fn sync_conversation_dropdown_index(&mut self) {
        self.conversation_dropdown_index = self
            .active_conversation_id
            .and_then(|id| self.conversations.iter().position(|conversation| conversation.id == id))
            .unwrap_or(0)
            .min(self.conversations.len().saturating_sub(1));
    }

    fn selected_profile(&self) -> Option<&ProfileSummary> {
        self.selected_profile_id
            .and_then(|id| self.profiles.iter().find(|profile| profile.id == id))
            .or_else(|| self.profiles.iter().find(|profile| profile.is_default))
    }

    fn sync_current_model_from_profile(&mut self) {
        self.current_model = self
            .selected_profile()
            .map(|profile| profile.model_id.clone())
            .unwrap_or_else(|| "No profile selected".to_string());
    }

    fn sync_profile_dropdown_index(&mut self) {
        self.profile_dropdown_index = self
            .selected_profile_id
            .and_then(|id| self.profiles.iter().position(|profile| profile.id == id))
            .unwrap_or(0)
            .min(self.profiles.len().saturating_sub(1));
    }
}


/// Chat view component with event handling
/// 
/// @plan PLAN-20250130-GPUIREDUX.P04
pub struct ChatView {
    pub state: ChatState,
    focus_handle: FocusHandle,
    bridge: Option<Arc<GpuiBridge>>,
    conversation_id: Option<Uuid>,
}

impl ChatView {
    pub fn new(state: ChatState, cx: &mut gpui::Context<Self>) -> Self {
        Self { 
            state,
            focus_handle: cx.focus_handle(),
            bridge: None,
            conversation_id: None,
        }
    }

    /// Set the bridge for event communication
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set the current conversation ID
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn set_conversation_id(&mut self, id: Uuid) {
        self.conversation_id = Some(id);
        self.state.active_conversation_id = Some(id);
        self.state.sync_conversation_dropdown_index();
        self.state.sync_conversation_title_from_active();
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P04
    fn emit(&self, event: UserEvent) {
        tracing::info!("ChatView::emit called with event: {:?}", event);
        if let Some(bridge) = &self.bridge {
            tracing::info!("ChatView::emit - bridge is Some, calling bridge.emit");
            bridge.emit(event);
        } else {
            tracing::warn!("ChatView: No bridge set, cannot emit event: {:?}", event);
        }
    }

    fn current_or_active_conversation_id(&self) -> Option<Uuid> {
        self.state
            .active_conversation_id
            .or(self.conversation_id)
            .or_else(|| self.state.conversations.first().map(|conversation| conversation.id))
    }

    fn select_conversation_at_index(&mut self, index: usize, cx: &mut gpui::Context<Self>) {
        if self.state.conversations.is_empty() {
            return;
        }

        let bounded = index.min(self.state.conversations.len() - 1);
        let conversation_id = self.state.conversations[bounded].id;
        self.state.conversation_dropdown_index = bounded;
        self.state.active_conversation_id = Some(conversation_id);
        self.conversation_id = Some(conversation_id);
        self.state.conversation_dropdown_open = false;
        self.state.conversation_title_editing = false;
        self.state.sync_conversation_title_from_active();
        self.emit(UserEvent::SelectConversation { id: conversation_id });
        cx.notify();
    }

    pub fn toggle_conversation_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.conversation_dropdown_open = !self.state.conversation_dropdown_open;
        if self.state.conversation_dropdown_open {
            self.state.profile_dropdown_open = false;
            self.state.conversation_title_editing = false;
            self.state.sync_conversation_dropdown_index();
        }
        cx.notify();
    }

    pub fn conversation_dropdown_open(&self) -> bool {
        self.state.conversation_dropdown_open
    }

    pub fn move_conversation_dropdown_selection(&mut self, delta: isize, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_dropdown_open || self.state.conversations.is_empty() {
            return;
        }

        let len = self.state.conversations.len() as isize;
        let current = self.state.conversation_dropdown_index as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
        if next != self.state.conversation_dropdown_index {
            self.state.conversation_dropdown_index = next;
            cx.notify();
        }
    }

    pub fn confirm_conversation_dropdown_selection(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_dropdown_open {
            return;
        }
        self.select_conversation_at_index(self.state.conversation_dropdown_index, cx);
    }

    pub fn start_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(id) = self.current_or_active_conversation_id() {
            self.state.conversation_dropdown_open = false;
            self.state.conversation_title_editing = true;
            self.state.conversation_title_input = self.state.conversation_title.clone();
            self.state.active_conversation_id = Some(id);
            self.conversation_id = Some(id);
            self.emit(UserEvent::StartRenameConversation { id });
            cx.notify();
        }
    }

    pub fn submit_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }

        if let Some(id) = self.current_or_active_conversation_id() {
            let title = self.state.conversation_title_input.trim().to_string();
            if title.is_empty() {
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.sync_conversation_title_from_active();
                cx.notify();
                return;
            }

            self.state.conversation_title = title.clone();
            if let Some(conversation) = self.state.conversations.iter_mut().find(|conversation| conversation.id == id) {
                conversation.title = title.clone();
            }

            self.state.conversation_title_editing = false;
            self.state.conversation_title_input.clear();
            self.emit(UserEvent::ConfirmRenameConversation { id, title });
            cx.notify();
        }
    }

    pub fn cancel_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        self.state.conversation_title_editing = false;
        self.state.conversation_title_input.clear();
        self.state.sync_conversation_title_from_active();
        self.emit(UserEvent::CancelRenameConversation);
        cx.notify();
    }

    pub fn handle_rename_backspace(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        self.state.conversation_title_input.pop();
        cx.notify();
    }

    pub fn handle_rename_space(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        self.state.conversation_title_input.push(' ');
        cx.notify();
    }

    pub fn handle_rename_char(&mut self, key: &str, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        if let Some(c) = key.chars().next() {
            if c.is_ascii_graphic() {
                self.state.conversation_title_input.push(c);
                cx.notify();
            }
        }
    }

    pub fn conversation_title_editing(&self) -> bool {
        self.state.conversation_title_editing
    }

    fn select_profile_at_index(&mut self, index: usize, cx: &mut gpui::Context<Self>) {
        if self.state.profiles.is_empty() {
            return;
        }

        let bounded = index.min(self.state.profiles.len() - 1);
        let profile_id = self.state.profiles[bounded].id;
        self.state.profile_dropdown_index = bounded;
        self.state.selected_profile_id = Some(profile_id);
        self.state.profile_dropdown_open = false;
        self.state.sync_current_model_from_profile();
        self.emit(UserEvent::SelectChatProfile { id: profile_id });
        cx.notify();
    }

    pub fn toggle_profile_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.profile_dropdown_open = !self.state.profile_dropdown_open;
        if self.state.profile_dropdown_open {
            self.state.conversation_dropdown_open = false;
            self.state.sync_profile_dropdown_index();
        }
        cx.notify();
    }

    pub fn profile_dropdown_open(&self) -> bool {
        self.state.profile_dropdown_open
    }

    pub fn move_profile_dropdown_selection(&mut self, delta: isize, cx: &mut gpui::Context<Self>) {
        if !self.state.profile_dropdown_open || self.state.profiles.is_empty() {
            return;
        }

        let len = self.state.profiles.len() as isize;
        let current = self.state.profile_dropdown_index as isize;
        let next = (current + delta).clamp(0, len - 1) as usize;
        if next != self.state.profile_dropdown_index {
            self.state.profile_dropdown_index = next;
            cx.notify();
        }
    }

    pub fn confirm_profile_dropdown_selection(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.profile_dropdown_open {
            return;
        }
        self.select_profile_at_index(self.state.profile_dropdown_index, cx);
    }

    /// Handle backspace key (called from MainPanel)
    pub fn handle_backspace(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_title_editing {
            self.handle_rename_backspace(cx);
            return;
        }

        if self.state.conversation_dropdown_open {
            return;
        }

        if self.state.profile_dropdown_open {
            return;
        }
        self.state.input_text.pop();
        cx.notify();
    }

    /// Handle enter key (called from MainPanel)
    pub fn handle_enter(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_title_editing {
            self.submit_rename_conversation(cx);
            return;
        }

        if self.state.conversation_dropdown_open {
            self.confirm_conversation_dropdown_selection(cx);
            return;
        }

        if self.state.profile_dropdown_open {
            self.confirm_profile_dropdown_selection(cx);
            return;
        }

        if matches!(self.state.streaming, StreamingState::Streaming { .. }) {
            tracing::info!("ChatView::handle_enter ignored while stream is active");
            return;
        }

        if !self.state.input_text.is_empty() {
            let text = self.state.input_text.clone();
            tracing::info!("ChatView::handle_enter - emitting SendMessage: {}", text);
            self.emit(UserEvent::SendMessage { text });
            self.state.input_text.clear();
            self.state.streaming = StreamingState::Streaming {
                content: String::new(),
                done: false,
            };
            cx.notify();
        }
    }

    /// Handle space key (called from MainPanel)
    pub fn handle_space(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_title_editing {
            self.handle_rename_space(cx);
            return;
        }

        if self.state.conversation_dropdown_open {
            return;
        }

        if self.state.profile_dropdown_open {
            return;
        }

        self.state.input_text.push(' ');
        cx.notify();
    }

    /// Handle single character input (called from MainPanel)
    pub fn handle_char(&mut self, key: &str, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_title_editing {
            self.handle_rename_char(key, cx);
            return;
        }

        if self.state.conversation_dropdown_open {
            return;
        }

        if self.state.profile_dropdown_open {
            return;
        }

        if let Some(c) = key.chars().next() {
            if c.is_ascii_graphic() {
                self.state.input_text.push(c);
                cx.notify();
            }
        }
    }

    /// Handle incoming ViewCommands
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::MessageAppended { role, content, .. } => {
                let current_model = self.state.current_model.clone();
                let msg = match role {
                    crate::presentation::view_command::MessageRole::User => ChatMessage::user(content),
                    crate::presentation::view_command::MessageRole::Assistant => {
                        ChatMessage::assistant(content, current_model)
                    }
                    _ => return,
                };
                self.state.messages.push(msg);
                cx.notify();
            }
            ViewCommand::ShowThinking { .. } => {
                if !matches!(self.state.streaming, StreamingState::Streaming { .. }) {
                    self.state.streaming = StreamingState::Streaming {
                        content: String::new(),
                        done: false,
                    };
                }
                self.state.thinking_content = Some(String::new());
                cx.notify();
            }
            ViewCommand::HideThinking { .. } => {
                self.state.thinking_content = None;
                cx.notify();
            }
            ViewCommand::AppendStream { chunk, .. } => {
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
                    _ => {}
                }
                cx.notify();
            }
            ViewCommand::FinalizeStream { .. } => {
                if let StreamingState::Streaming { content, .. } = &self.state.streaming {
                    let msg = ChatMessage::assistant(content.clone(), self.state.current_model.clone());
                    self.state.messages.push(msg);
                }
                self.state.streaming = StreamingState::Idle;
                cx.notify();
            }
            ViewCommand::StreamCancelled { partial_content, .. } => {
                if !partial_content.is_empty() {
                    let mut msg = ChatMessage::assistant(partial_content, self.state.current_model.clone());
                    msg.content.push_str(" [cancelled]");
                    self.state.messages.push(msg);
                }
                self.state.streaming = StreamingState::Idle;
                cx.notify();
            }
            ViewCommand::StreamError { error, .. } => {
                self.state.streaming = StreamingState::Error(error);
                cx.notify();
            }
            ViewCommand::AppendThinking { content, .. } => {
                self.state.thinking_content = Some(
                    self.state.thinking_content.clone().unwrap_or_default() + &content
                );
                cx.notify();
            }
            ViewCommand::ToggleThinkingVisibility => {
                self.state.show_thinking = !self.state.show_thinking;
                cx.notify();
            }
            ViewCommand::ConversationRenamed { id, new_title } => {
                if let Some(conversation) = self
                    .state
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == id)
                {
                    conversation.title = new_title.clone();
                    conversation.updated_at = chrono::Utc::now();
                }

                if self.state.active_conversation_id == Some(id) {
                    self.state.conversation_title = new_title;
                }
                cx.notify();
            }
            ViewCommand::ConversationListRefreshed { conversations } => {
                self.state.conversations = conversations;

                if self.state.conversations.is_empty() {
                    self.state.active_conversation_id = None;
                    self.conversation_id = None;
                    self.state.conversation_dropdown_open = false;
                    self.state.conversation_dropdown_index = 0;
                    if !self.state.conversation_title_editing {
                        self.state.conversation_title = "New Conversation".to_string();
                    }
                } else {
                    let active_exists = self
                        .state
                        .active_conversation_id
                        .map(|id| self.state.conversations.iter().any(|conversation| conversation.id == id))
                        .unwrap_or(false);

                    if !active_exists {
                        let fallback = self.state.conversations[0].id;
                        self.state.active_conversation_id = Some(fallback);
                        self.conversation_id = Some(fallback);
                    }

                    self.state.sync_conversation_dropdown_index();
                    if !self.state.conversation_title_editing {
                        self.state.sync_conversation_title_from_active();
                    }
                }

                cx.notify();
            }
            ViewCommand::ConversationActivated { id } => {
                self.state.active_conversation_id = Some(id);
                self.conversation_id = Some(id);
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();

                if let Some(conversation) = self.state.conversations.iter().find(|conversation| conversation.id == id) {
                    self.state.conversation_title = if conversation.title.trim().is_empty() {
                        "Untitled Conversation".to_string()
                    } else {
                        conversation.title.clone()
                    };
                }

                self.state.sync_conversation_dropdown_index();
                cx.notify();
            }
            ViewCommand::ConversationCreated { id, .. } => {
                self.state.active_conversation_id = Some(id);
                self.conversation_id = Some(id);
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.conversation_title = "New Conversation".to_string();

                if !self.state.conversations.iter().any(|conversation| conversation.id == id) {
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
            ViewCommand::ConversationTitleUpdated { id, title } => {
                if let Some(conversation) = self
                    .state
                    .conversations
                    .iter_mut()
                    .find(|conversation| conversation.id == id)
                {
                    conversation.title = title.clone();
                    conversation.updated_at = chrono::Utc::now();
                }

                if self.state.active_conversation_id == Some(id) {
                    self.state.conversation_title = title;
                }

                cx.notify();
            }
            ViewCommand::ConversationDeleted { id } => {
                self.state.conversations.retain(|conversation| conversation.id != id);
                if self.state.active_conversation_id == Some(id) {
                    self.state.active_conversation_id = self.state.conversations.first().map(|conversation| conversation.id);
                    self.conversation_id = self.state.active_conversation_id;
                    self.state.messages.clear();
                    self.state.streaming = StreamingState::Idle;
                    self.state.thinking_content = None;
                }
                self.state.sync_conversation_dropdown_index();
                self.state.sync_conversation_title_from_active();
                cx.notify();
            }
            ViewCommand::ConversationCleared => {
                self.state.messages.clear();
                self.state.streaming = StreamingState::Idle;
                self.state.thinking_content = None;
                self.state.conversation_dropdown_open = false;
                self.state.conversation_title_editing = false;
                self.state.conversation_title_input.clear();
                self.state.sync_conversation_title_from_active();
                cx.notify();
            }
            ViewCommand::ChatProfilesUpdated {
                profiles,
                selected_profile_id,
            }
            | ViewCommand::ShowSettings {
                profiles,
                selected_profile_id,
            } => {
                self.state.profiles = profiles;
                self.state.selected_profile_id = selected_profile_id.or_else(|| {
                    self.state
                        .profiles
                        .iter()
                        .find(|profile| profile.is_default)
                        .map(|profile| profile.id)
                });
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
            _ => {
                // Other commands handled elsewhere
            }
        }
    }

    /// Render the top bar with icon, title, and toolbar buttons
    /// @plan PLAN-20250130-GPUIREDUX.P04
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let show_thinking = self.state.show_thinking;
        
        div()
            .id("chat-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            // Left: title
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("PersonalAgent")
                    )
            )
            // Right: buttons [T][S][H][+][Settings] with event emission
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // [T] Toggle thinking button
                    .child(
                        div()
                            .id("btn-thinking")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(show_thinking, |d| d.bg(Theme::bg_dark()))
                            .when(!show_thinking, |d| d.hover(|s| s.bg(Theme::bg_dark())))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("T")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                tracing::info!("Toggle thinking clicked - emitting UserEvent");
                                this.emit(UserEvent::ToggleThinking);
                                // Also update local state immediately for responsiveness
                                this.state.show_thinking = !this.state.show_thinking;
                                cx.notify();
                            }))
                    )
                    // [R] Rename conversation button
                    .child(
                        div()
                            .id("btn-rename")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("R")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                this.start_rename_conversation(cx);
                            }))
                    )
                    // [H] History button - navigate to history view
                    .child(
                        div()
                            .id("btn-history")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(Theme::bg_darker())
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .active(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("H")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                println!(">>> HISTORY BUTTON CLICKED - using navigation_channel <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::History
                                );
                            }))
                    )
                    // [+] New conversation button
                    .child(
                        div()
                            .id("btn-new")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("+")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                tracing::info!("New conversation clicked - emitting UserEvent");
                                this.emit(UserEvent::NewConversation);
                                // Clear local state immediately
                                this.state.messages.clear();
                                this.state.streaming = StreamingState::Idle;
                                this.state.conversation_title = "New Conversation".to_string();
                                this.state.conversation_dropdown_open = false;
                                this.state.conversation_title_editing = false;
                                this.state.conversation_title_input.clear();
                                this.state.profile_dropdown_open = false;
                                cx.notify();
                            }))
                    )
                    // Settings button (gear icon) - navigate to settings view
                    .child(
                        div()
                            .id("btn-settings")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(Theme::bg_darker())
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .active(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("\u{2699}")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                println!(">>> SETTINGS BUTTON CLICKED - using navigation_channel <<<");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::Settings
                                );
                            }))
                    )
            )
    }

    /// Render the title bar with conversation dropdown and model label
    /// @plan PLAN-20250130-GPUIREDUX.P03
    fn render_title_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let conversation_title = self.state.conversation_title.clone();
        let conversation_title_input = self.state.conversation_title_input.clone();
        let conversation_dropdown_open = self.state.conversation_dropdown_open;
        let conversation_title_editing = self.state.conversation_title_editing;
        let conversation_dropdown_index = self.state.conversation_dropdown_index;
        let active_conversation_id = self.state.active_conversation_id;
        let current_model = self.state.current_model.clone();
        let selected_profile = self
            .state
            .selected_profile()
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| "Select profile".to_string());
        let profile_dropdown_open = self.state.profile_dropdown_open;

        div()
            .id("chat-title-bar")
            .h(px(32.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        if conversation_title_editing {
                            div()
                                .id("conversation-title-input")
                                .min_w(px(220.0))
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .bg(Theme::bg_dark())
                                .border_1()
                                .border_color(Theme::accent())
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::text_primary())
                                        .child(if conversation_title_input.is_empty() {
                                            "Enter conversation name".to_string()
                                        } else {
                                            conversation_title_input
                                        })
                                )
                        } else {
                            div()
                                .id("conversation-dropdown")
                                .min_w(px(220.0))
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .bg(Theme::bg_dark())
                                .border_1()
                                .border_color(if conversation_dropdown_open { Theme::accent() } else { Theme::border() })
                                .flex()
                                .items_center()
                                .justify_between()
                                .cursor_pointer()
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::text_primary())
                                        .child(conversation_title)
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(Theme::text_primary())
                                        .child(if conversation_dropdown_open { "▲" } else { "▼" })
                                )
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                    this.toggle_conversation_dropdown(cx);
                                }))
                        }
                    )
                    .child(
                        div()
                            .id("chat-profile-dropdown")
                            .min_w(px(190.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(if profile_dropdown_open { Theme::accent() } else { Theme::border() })
                            .cursor_pointer()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(Theme::text_primary())
                                            .child(selected_profile)
                                    )
                                    .child(
                                        div()
                                            .text_size(px(9.0))
                                            .text_color(Theme::text_secondary())
                                            .child(if profile_dropdown_open { "▲" } else { "▼" })
                                    )
                            )
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                this.toggle_profile_dropdown(cx);
                            }))
                    )
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_secondary())
                    .child(current_model)
            )
            .when(conversation_dropdown_open, |d| {
                d.child(
                    div()
                        .id("chat-conversation-dropdown-menu")
                        .absolute()
                        .top(px(30.0))
                        .left(px(12.0))
                        .w(px(360.0))
                        .max_h(px(220.0))
                        .overflow_y_scroll()
                        .bg(Theme::bg_dark())
                        .border_1()
                        .border_color(Theme::border())
                        .rounded(px(4.0))
                        .children(self.state.conversations.iter().enumerate().map(|(index, conversation)| {
                            let conversation_id = conversation.id;
                            let selected = active_conversation_id == Some(conversation_id);
                            let highlighted = conversation_dropdown_index == index;
                            let title = if conversation.title.trim().is_empty() {
                                "Untitled Conversation".to_string()
                            } else {
                                conversation.title.clone()
                            };
                            let count_label = if conversation.message_count == 1 {
                                "1 message".to_string()
                            } else {
                                format!("{} messages", conversation.message_count)
                            };

                            div()
                                .id(SharedString::from(format!("chat-conversation-item-{}", conversation_id)))
                                .w_full()
                                .px(px(8.0))
                                .py(px(6.0))
                                .cursor_pointer()
                                .when(selected, |row| {
                                    row.bg(Theme::accent()).text_color(gpui::white())
                                })
                                .when(!selected && highlighted, |row| {
                                    row.bg(Theme::accent_hover()).text_color(gpui::white())
                                })
                                .when(!selected && !highlighted, |row| {
                                    row.hover(|s| s.bg(Theme::bg_darker())).text_color(Theme::text_primary())
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .child(title)
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(Theme::text_secondary())
                                                .child(count_label)
                                        )
                                )
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, cx| {
                                    this.select_conversation_at_index(index, cx);
                                }))
                        }))
                )
            })
    }


    /// Render the chat area with messages
    /// @plan PLAN-20250130-GPUIREDUX.P03
    fn render_chat_area(&self, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let messages = self.state.messages.clone();
        let streaming = self.state.streaming.clone();
        let show_thinking = self.state.show_thinking;
        div()
            .id("chat-area")
            .flex_1()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_y_scroll()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Empty state
            .when(messages.is_empty() && !matches!(streaming, StreamingState::Streaming { .. }), |d| {
                d.items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(Theme::text_secondary())
                            .child("No messages yet")
                    )
            })
            // Messages
            .when(!messages.is_empty(), |d| {
                d.children(
                    messages
                        .into_iter()
                        .enumerate()
                        .map(|(i, msg)| {
                            let id = SharedString::from(format!("msg-{}", i));
                            div()
                                .id(id)
                                .child(self.render_message(&msg, show_thinking))
                        })
                )
            })
            // Streaming message
            .when(matches!(streaming, StreamingState::Streaming { .. }), |d| {
                let (content, _done) = match &streaming {
                    StreamingState::Streaming { content, done } => (content.clone(), *done),
                    _ => (String::new(), false),
                };
                d.child(
                    div()
                        .id("streaming-msg")
                        .child(
                            AssistantBubble::new(content)
                                .model_id("streaming")
                                .show_thinking(show_thinking)
                                .streaming(true)
                        )
                )
            })
    }

    /// Render a single message
    /// @plan PLAN-20250130-GPUIREDUX.P03
    fn render_message(&self, msg: &ChatMessage, show_thinking: bool) -> impl IntoElement {
        match msg.role {
            MessageRole::User => self.render_user_message(&msg.content),
            MessageRole::Assistant => self.render_assistant_message(msg, show_thinking),
        }
    }

    /// Render user message - right aligned, green bubble
    fn render_user_message(&self, content: &str) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .justify_end()
            .child(
                div()
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::user_bubble())
                    .text_size(px(13.0))
                    .text_color(Theme::text_primary())
                    .child(content.to_string())
            )
            .into_any_element()
    }

    /// Render assistant message - left aligned, dark bubble with model label
    fn render_assistant_message(&self, msg: &ChatMessage, show_thinking: bool) -> gpui::AnyElement {
        let model_id = msg.model_id.clone().unwrap_or_else(|| "Assistant".to_string());
        
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(2.0))
            // Model label
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(Theme::text_muted())
                    .child(model_id)
            )
            // Thinking block (if present and visible)
            .when(msg.thinking.is_some() && show_thinking, |d| {
                d.child(self.render_thinking_block(msg.thinking.as_ref().unwrap()))
            })
            // Response bubble
            .child(
                div()
                    .max_w(px(300.0))
                    .px(px(10.0))
                    .py(px(10.0))
                    .rounded(px(12.0))
                    .bg(Theme::assistant_bubble())
                    .border_1()
                    .border_color(Theme::border())
                    .text_size(px(13.0))
                    .text_color(Theme::text_primary())
                    .child(msg.content.clone())
            )
            .into_any_element()
    }

    /// Render thinking block with blue tint
    fn render_thinking_block(&self, content: &str) -> impl IntoElement {
        div()
            .max_w(px(300.0))
            .px(px(8.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .bg(Theme::thinking_bg())
            .border_l_2()
            .border_color(Theme::text_muted())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(Theme::text_muted())
                            .child("Thinking")
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_muted())
                            .italic()
                            .child(content.to_string())
                    )
            )
    }

    fn render_input_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_streaming = matches!(self.state.streaming, StreamingState::Streaming { .. });
        let input_text = self.state.input_text.clone();
        let has_text = !input_text.is_empty();
        let focus_handle = self.focus_handle.clone();

        div()
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            .p(px(Theme::SPACING_MD))
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::bg_dark())
            // Text input field
            .child(
                div()
                    .id("input-field")
                    .flex_1()
                    .p(px(Theme::SPACING_SM))
                    .bg(Theme::bg_darkest())
                    .rounded(px(Theme::RADIUS_MD))
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, {
                        let focus_handle = focus_handle.clone();
                        move |_, window, cx| {
                            tracing::info!("Input field clicked - requesting focus");
                            window.focus(&focus_handle, cx);
                        }
                    })
                    .child(
                        if input_text.is_empty() {
                            div()
                                .text_color(Theme::text_secondary())
                                .child("Type a message...")
                        } else {
                            div()
                                .text_color(Theme::text_primary())
                                .child(input_text)
                        }
                    )
            )
            // Send/Stop button with event emission
            // @plan PLAN-20250130-GPUIREDUX.P04
            .child(
                div()
                    .id(if is_streaming { "stop-btn" } else { "send-btn" })
                    .px(px(Theme::SPACING_MD))
                    .py(px(Theme::SPACING_SM))
                    .rounded(px(Theme::RADIUS_MD))
                    .cursor_pointer()
                    .when(is_streaming, |d| {
                        d.bg(Theme::error())
                            .text_color(gpui::white())
                            .child("Stop")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                tracing::info!("Stop button clicked - emitting StopStreaming");
                                this.emit(UserEvent::StopStreaming);
                                this.state.streaming = StreamingState::Idle;
                                cx.notify();
                            }))
                    })
                    .when(!is_streaming && has_text, |d| {
                        d.bg(Theme::bg_dark())
                            .text_color(Theme::text_primary())
                            .hover(|s| s.bg(Theme::bg_darker()))
                            .child("Send")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                if matches!(this.state.streaming, StreamingState::Streaming { .. }) {
                                    tracing::info!("Send button ignored while stream is active");
                                    return;
                                }

                                let text = this.state.input_text.clone();
                                if !text.is_empty() {
                                    tracing::info!("Send button clicked - emitting SendMessage: {}", text);
                                    this.emit(UserEvent::SendMessage { text });
                                    this.state.input_text.clear();
                                    this.state.streaming = StreamingState::Streaming {
                                        content: String::new(),
                                        done: false,
                                    };
                                    cx.notify();
                                }
                            }))
                    })
                    .when(!is_streaming && !has_text, |d| {
                        d.bg(Theme::bg_dark())
                            .text_color(Theme::text_secondary())
                            .child("Send")
                    })
            )
    }
}

impl gpui::Focusable for ChatView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for ChatView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("chat-view")
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;

                // === KEYBOARD SHORTCUTS (Cmd+key) ===
                if modifiers.platform {
                    match key.as_str() {
                        "h" => {
                            // Cmd+H: Navigate to History
                            println!(">>> Cmd+H pressed - navigating to History <<<");
                            crate::ui_gpui::navigation_channel().request_navigate(
                                crate::presentation::view_command::ViewId::History
                            );
                            return;
                        }
                        "," => {
                            // Cmd+,: Navigate to Settings (standard macOS)
                            println!(">>> Cmd+, pressed - navigating to Settings <<<");
                            crate::ui_gpui::navigation_channel().request_navigate(
                                crate::presentation::view_command::ViewId::Settings
                            );
                            return;
                        }
                        "n" => {
                            // Cmd+N: New conversation
                            println!(">>> Cmd+N pressed - new conversation <<<");
                            this.emit(UserEvent::NewConversation);
                            this.state.messages.clear();
                            this.state.input_text.clear();
                            this.state.conversation_title = "New Conversation".to_string();
                            this.state.conversation_dropdown_open = false;
                            this.state.conversation_title_editing = false;
                            this.state.conversation_title_input.clear();
                            this.state.profile_dropdown_open = false;
                            cx.notify();
                            return;
                        }
                        "t" => {
                            // Cmd+T: Toggle thinking
                            println!(">>> Cmd+T pressed - toggle thinking <<<");
                            this.emit(UserEvent::ToggleThinking);
                            this.state.show_thinking = !this.state.show_thinking;
                            cx.notify();
                            return;
                        }
                        "p" => {
                            // Cmd+P: Toggle chat profile dropdown
                            this.toggle_profile_dropdown(cx);
                            return;
                        }
                        "r" => {
                            // Cmd+R: rename active conversation
                            this.start_rename_conversation(cx);
                            return;
                        }
                        _ => {}
                    }
                }

                if this.state.conversation_title_editing {
                    match key.as_str() {
                        "escape" => this.cancel_rename_conversation(cx),
                        "backspace" => this.handle_rename_backspace(cx),
                        "space" => this.handle_rename_space(cx),
                        "enter" => this.submit_rename_conversation(cx),
                        _ => {
                            if key.len() == 1 && !modifiers.platform && !modifiers.control {
                                this.handle_rename_char(key, cx);
                            }
                        }
                    }
                    return;
                }

                if this.state.conversation_dropdown_open {
                    match key.as_str() {
                        "escape" => {
                            this.state.conversation_dropdown_open = false;
                            cx.notify();
                        }
                        "up" => {
                            this.move_conversation_dropdown_selection(-1, cx);
                        }
                        "down" => {
                            this.move_conversation_dropdown_selection(1, cx);
                        }
                        "enter" => {
                            this.confirm_conversation_dropdown_selection(cx);
                        }
                        _ => {}
                    }
                    return;
                }

                if this.state.profile_dropdown_open {
                    match key.as_str() {
                        "escape" => {
                            this.state.profile_dropdown_open = false;
                            cx.notify();
                        }
                        "up" => {
                            this.move_profile_dropdown_selection(-1, cx);
                        }
                        "down" => {
                            this.move_profile_dropdown_selection(1, cx);
                        }
                        "enter" => {
                            this.confirm_profile_dropdown_selection(cx);
                        }
                        _ => {}
                    }
                    return;
                }

                // === ESCAPE KEY ===
                if key == "escape" {
                    // Stop streaming if active
                    if matches!(this.state.streaming, StreamingState::Streaming { .. }) {
                        println!(">>> Escape pressed - stopping stream <<<");
                        this.emit(UserEvent::StopStreaming);
                        this.state.streaming = StreamingState::Idle;
                        cx.notify();
                    }
                    return;
                }

                // Text entry and send are handled by MainPanel's key forwarding.
                // Keeping that as the single owner avoids duplicate SendMessage emissions
                // when both MainPanel and ChatView receive the same key event.
            }))
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Title bar (32px)
            .child(self.render_title_bar(cx))
            // Chat area (flex)
            .child(self.render_chat_area(cx))
            // Input bar (50px)
            .child(self.render_input_bar(cx))
    }
}
