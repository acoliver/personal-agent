//! Composer selection operations on `ChatView`.
//!
//! These are the *policy* methods that bridge pointer-driven selection
//! state (`composer_selection`) with the existing cursor/IME/clipboard
//! handlers. The pure arithmetic lives in `composer_selection.rs`.

use super::composer_selection::{
    clamp_to_char_boundary, delete_range, replace_range, ComposerSelection,
};
use super::ChatView;

impl ChatView {
    /// Called by the `ComposerField` element when a pointer event changes
    /// the selection. Clamps to char boundaries and notifies.
    pub(super) fn set_composer_selection_from_pointer(
        &mut self,
        sel: ComposerSelection,
        cx: &mut gpui::Context<Self>,
    ) {
        let text = &self.state.input_text;
        let anchor = clamp_to_char_boundary(text, sel.anchor);
        let head = clamp_to_char_boundary(text, sel.head);
        self.composer_selection = ComposerSelection::new(anchor, head);
        self.active_message_selection = None;
        self.message_context_menu = None;
        // Keep cursor_position in sync so IME and arrow keys start from head.
        self.state.cursor_position = head;
        cx.notify();
    }

    /// Returns `true` if the composer currently has a non-empty text
    /// selection (and the composer is the active input, not sidebar search
    /// or title editing).
    pub(super) fn composer_has_selection(&self, cx: &gpui::App) -> bool {
        !self.sidebar_search_focused(cx)
            && !self.state.conversation_title_editing
            && self.composer_selection.is_non_empty()
    }

    /// The selected text in the composer, or empty string.
    pub(super) fn composer_selected_text(&self) -> String {
        let sel = self.composer_selection;
        if sel.is_collapsed() {
            return String::new();
        }
        let text = &self.state.input_text;
        let sel = sel.clamped(text);
        text[sel.start()..sel.end()].to_string()
    }

    /// Select all text in the composer.
    pub(super) fn select_all_composer(&mut self, cx: &mut gpui::Context<Self>) {
        let len = self.state.input_text.len();
        self.composer_selection = ComposerSelection::new(0, len);
        self.state.cursor_position = len;
        cx.notify();
    }

    /// Delete the currently selected range (used by Backspace/Delete).
    /// Returns `true` if a range was deleted.
    pub(super) fn delete_composer_selection(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        if self.composer_selection.is_collapsed() {
            return false;
        }
        let (new_text, caret) = delete_range(&self.state.input_text, self.composer_selection);
        self.state.input_text = new_text;
        self.state.cursor_position = caret;
        self.composer_selection = ComposerSelection::caret(caret);
        if let Some(conv_id) = self.conversation_id {
            super::save_draft(conv_id, &self.state.input_text);
        }
        cx.notify();
        true
    }

    /// Replace the selected range (or insert at caret) with `text`.
    /// Used by typing, paste, and IME commit.
    pub(super) fn replace_composer_selection(&mut self, text: &str, cx: &mut gpui::Context<Self>) {
        if self.state.conversation_dropdown_open || self.state.profile_dropdown_open {
            return;
        }
        let (new_text, caret) =
            replace_range(&self.state.input_text, self.composer_selection, text);
        self.state.input_text = new_text;
        self.state.cursor_position = caret;
        self.composer_selection = ComposerSelection::caret(caret);
        if let Some(conv_id) = self.conversation_id {
            super::save_draft(conv_id, &self.state.input_text);
        }
        cx.notify();
    }

    /// Copy the selected text to the clipboard. Returns `true` if text was
    /// copied.
    pub(super) fn copy_composer_selection(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        let text = self.composer_selected_text();
        if text.is_empty() {
            return false;
        }

        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        true
    }

    /// Cut: copy then delete. Returns `true` if something was cut.
    pub(super) fn cut_composer_selection(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        if !self.copy_composer_selection(cx) {
            return false;
        }
        self.delete_composer_selection(cx);
        true
    }

    /// Delete the selected range, or the Unicode scalar after the caret.
    pub(super) fn handle_delete(&mut self, cx: &mut gpui::Context<Self>) {
        if self.composer_has_selection(cx) {
            self.delete_composer_selection(cx);
            return;
        }
        let position = clamp_to_char_boundary(&self.state.input_text, self.state.cursor_position);
        if position >= self.state.input_text.len() {
            return;
        }
        let next = self.state.input_text[position..]
            .char_indices()
            .nth(1)
            .map_or(self.state.input_text.len(), |(offset, _)| position + offset);
        self.state.input_text.replace_range(position..next, "");
        self.composer_selection = ComposerSelection::caret(position);
        cx.notify();
    }

    /// Collapse selection to the left boundary (Shift+Left equivalent for a
    /// pre-existing selection). Returns `true` if a collapse happened.
    pub(super) fn collapse_selection_left(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        if self.composer_selection.is_collapsed() {
            return false;
        }
        let pos = self.composer_selection.start();
        self.composer_selection = ComposerSelection::caret(pos);
        self.state.cursor_position = pos;
        cx.notify();
        true
    }

    /// Collapse selection to the right boundary. Returns `true` if a
    /// collapse happened.
    pub(super) fn collapse_selection_right(&mut self, cx: &mut gpui::Context<Self>) -> bool {
        if self.composer_selection.is_collapsed() {
            return false;
        }
        let pos = self.composer_selection.end();
        self.composer_selection = ComposerSelection::caret(pos);
        self.state.cursor_position = pos;
        cx.notify();
        true
    }
}
