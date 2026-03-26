//! IME `InputHandler` implementation for `ApiKeyManagerView`.

use super::ApiKeyManagerView;
use gpui::{Bounds, Pixels};
use std::ops::Range;

impl gpui::EntityInputHandler for ApiKeyManagerView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = self.active_text();
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
        let len = self.active_text().encode_utf16().count();
        Some(gpui::UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime_marked_byte_count > 0 {
            let text = self.active_text();
            let len16: usize = text.encode_utf16().count();
            let start_utf8 = text.len().saturating_sub(self.ime_marked_byte_count);
            let start_utf16: usize = text[..start_utf8].encode_utf16().count();
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
        if self.state.active_field.is_none() {
            return;
        }

        // Remove marked (composing) portion first
        if self.ime_marked_byte_count > 0 {
            let len = self.active_text_len();
            self.truncate_active_text(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }

        if !text.is_empty() {
            self.push_active_text(text);
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
        if self.state.active_field.is_none() {
            return;
        }

        if self.ime_marked_byte_count > 0 {
            let len = self.active_text_len();
            self.truncate_active_text(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }

        if !new_text.is_empty() {
            self.push_active_text(new_text);
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

// ── Key handling ──────────────────────────────────────────────────
