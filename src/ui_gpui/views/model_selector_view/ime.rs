//! IME `InputHandler` implementation for `ModelSelectorView`.

use super::ModelSelectorView;
use gpui::{Bounds, Pixels};
use std::ops::Range;

impl gpui::EntityInputHandler for ModelSelectorView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = &self.state.search_query;
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let start = range.start.min(utf16.len());
        let end = range.end.min(utf16.len());
        String::from_utf16(&utf16[start..end]).ok()
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        let len16 = self.state.search_query.encode_utf16().count();
        Some(gpui::UTF16Selection {
            range: len16..len16,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime_marked_byte_count > 0 {
            let q = &self.state.search_query;
            let len16: usize = q.encode_utf16().count();
            let start_utf8 = q.len().saturating_sub(self.ime_marked_byte_count);
            let start_utf16: usize = q[..start_utf8].encode_utf16().count();
            Some(start_utf16..len16)
        } else {
            None
        }
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) {
        self.ime_marked_byte_count = 0;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.ime_marked_byte_count > 0 {
            let len = self.state.search_query.len();
            self.state
                .search_query
                .truncate(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }
        if !text.is_empty() {
            self.state.search_query.push_str(text);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.ime_marked_byte_count > 0 {
            let len = self.state.search_query.len();
            self.state
                .search_query
                .truncate(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }
        if !new_text.is_empty() {
            self.state.search_query.push_str(new_text);
            self.ime_marked_byte_count = new_text.len();
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        None
    }
}
