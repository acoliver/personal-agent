//! IME/input handling for `ConversationListView`.
//!
//! The shared list is embedded directly in the popin History panel, so it must
//! own its own input handler rather than relying on `ChatView` to proxy typed
//! text from the popout sidebar.
//!
//! @plan PLAN-20260420-ISSUE180.P03
//! @requirement REQ-180-001

use std::ops::Range;

use gpui::{Bounds, Pixels, UTF16Selection};

use super::ConversationListView;

fn utf8_offset_to_utf16(text: &str, utf8_offset: usize) -> usize {
    text[..utf8_offset.min(text.len())].encode_utf16().count()
}

fn utf16_offset_to_utf8(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (byte_idx, ch) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_idx;
        }
        utf16_count += ch.len_utf16();
    }
    text.len()
}

impl gpui::EntityInputHandler for ConversationListView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = self.active_input_text().to_string();
        let start = utf16_offset_to_utf8(&text, range.start);
        let end = utf16_offset_to_utf8(&text, range.end);
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.active_input_text().to_string();
        let cursor_utf8 = self.active_cursor_position().min(text.len());
        let cursor_utf16 = utf8_offset_to_utf16(&text, cursor_utf8);
        Some(UTF16Selection {
            range: cursor_utf16..cursor_utf16,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.conversation_title_editing {
            if self.state.rename_replace_on_next_char {
                self.state.conversation_title_input.clear();
                self.state.rename_replace_on_next_char = false;
            }
            let title = &mut self.state.conversation_title_input;
            if let Some(r) = range {
                let start = utf16_offset_to_utf8(title, r.start);
                let end = utf16_offset_to_utf8(title, r.end);
                title.replace_range(start..end, text);
            } else {
                title.push_str(text);
            }
            cx.notify();
            return;
        }

        if self.state.sidebar_search_focused {
            let query = &mut self.state.sidebar_search_query;
            if let Some(r) = range {
                let start = utf16_offset_to_utf8(query, r.start);
                let end = utf16_offset_to_utf8(query, r.end);
                query.replace_range(start..end, text);
            } else {
                query.push_str(text);
            }
            self.trigger_sidebar_search(cx);
            cx.notify();
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.replace_text_in_range(range, new_text, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        let text = self.active_input_text().to_string();
        Some(utf8_offset_to_utf16(&text, self.active_cursor_position()))
    }
}
