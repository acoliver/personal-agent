use super::ChatView;

impl ChatView {
    #[cfg(test)]
    pub(super) fn composer_display_text_for_test(&self, cx: &gpui::App) -> String {
        self.composer_display_text(cx)
    }

    pub(super) fn composer_display_text(&self, cx: &gpui::App) -> String {
        Self::format_composer_display_text(
            &self.state.input_text,
            self.state.cursor_position,
            self.composer_has_focus(cx),
        )
    }

    fn format_composer_display_text(
        input_text: &str,
        cursor_position: usize,
        composer_focused: bool,
    ) -> String {
        if input_text.is_empty() {
            if composer_focused {
                "|".to_string()
            } else {
                "Type a message...".to_string()
            }
        } else if composer_focused {
            let cursor_pos = cursor_position.min(input_text.len());
            let before = &input_text[..cursor_pos];
            let after = &input_text[cursor_pos..];
            format!("{before}|{after}")
        } else {
            input_text.to_string()
        }
    }

    pub(super) fn composer_has_focus(&self, cx: &gpui::App) -> bool {
        self.state.composer_focused
            && !self.sidebar_search_focused(cx)
            && !self.state.conversation_title_editing
            && !self.state.conversation_dropdown_open
            && !self.state.profile_dropdown_open
    }

    pub(super) fn focus_composer(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.composer_focused = true;
        self.state.conversation_title_editing = false;
        self.state.conversation_dropdown_open = false;
        self.state.profile_dropdown_open = false;
        if self.sidebar_search_focused(cx) {
            self.set_sidebar_search_focused(false, cx);
        }
        cx.notify();
    }

    pub(super) const fn blur_composer(&mut self) {
        self.state.composer_focused = false;
    }
}
