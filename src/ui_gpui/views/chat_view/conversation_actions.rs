use super::ChatView;
use crate::events::types::UserEvent;
use uuid::Uuid;

impl ChatView {
    pub fn toggle_conversation_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.conversation_dropdown_open = !self.state.conversation_dropdown_open;
        if self.state.conversation_dropdown_open {
            self.state.profile_dropdown_open = false;
            self.state.conversation_title_editing = false;
            self.state.sync_conversation_dropdown_index();
        }
        tracing::info!(
            open = self.state.conversation_dropdown_open,
            count = self.state.conversations.len(),
            selected_index = self.state.conversation_dropdown_index,
            "ChatView: toggled conversation dropdown"
        );
        cx.notify();
    }

    #[must_use]
    pub const fn conversation_dropdown_open(&self) -> bool {
        self.state.conversation_dropdown_open
    }

    pub fn move_conversation_dropdown_selection(
        &mut self,
        delta: isize,
        cx: &mut gpui::Context<Self>,
    ) {
        if !self.state.conversation_dropdown_open || self.state.conversations.is_empty() {
            return;
        }

        let len = self.state.conversations.len().cast_signed();
        let current = self.state.conversation_dropdown_index.cast_signed();
        let next = (current + delta).clamp(0, len - 1).cast_unsigned();
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

    pub fn select_conversation_by_id(
        &mut self,
        conversation_id: Uuid,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(index) = self
            .state
            .conversations
            .iter()
            .position(|conversation| conversation.id == conversation_id)
        {
            self.select_conversation_at_index(index, cx);
        }
    }

    pub fn start_rename_conversation(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(id) = self.current_or_active_conversation_id() {
            self.state.conversation_dropdown_open = false;
            self.state.conversation_title_editing = true;
            self.state.conversation_title_input = self.state.conversation_title.clone();
            self.state.rename_replace_on_next_char = true;
            self.state.active_conversation_id = Some(id);
            self.conversation_id = Some(id);
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
                self.state.rename_replace_on_next_char = false;
                self.state.sync_conversation_title_from_active();
                cx.notify();
                return;
            }

            self.state.conversation_title.clone_from(&title);
            if let Some(conversation) = self
                .state
                .conversations
                .iter_mut()
                .find(|conversation| conversation.id == id)
            {
                conversation.title.clone_from(&title);
            }

            self.state.conversation_title_editing = false;
            self.state.conversation_title_input.clear();
            self.state.rename_replace_on_next_char = false;
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
        self.state.rename_replace_on_next_char = false;
        self.state.sync_conversation_title_from_active();
        self.emit(UserEvent::CancelRenameConversation);
        cx.notify();
    }

    pub fn handle_rename_backspace(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.state.conversation_title_editing {
            return;
        }
        if self.state.rename_replace_on_next_char {
            self.state.conversation_title_input.clear();
            self.state.rename_replace_on_next_char = false;
        } else {
            self.state.conversation_title_input.pop();
        }
        cx.notify();
    }

    #[must_use]
    pub const fn conversation_title_editing(&self) -> bool {
        self.state.conversation_title_editing
    }
}
