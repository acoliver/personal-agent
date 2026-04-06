//! IME (Input Method Editor) and `Focusable` trait implementations for `ChatView`.
//!
//! Contains `impl gpui::EntityInputHandler for ChatView` and
//! `impl gpui::Focusable for ChatView`. These are self-contained protocol
//! implementations involving only UTF-8/UTF-16 index arithmetic and
//! `self.state.input_text` / `self.state.marked_range`.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use super::ChatView;
use gpui::{Bounds, FocusHandle, Pixels, UTF16Selection};
use std::ops::Range;

// ── UTF-8 ↔ UTF-16 helpers for InputHandler ──────────────────────────

pub(super) fn utf8_offset_to_utf16(text: &str, utf8_offset: usize) -> usize {
    text[..utf8_offset.min(text.len())].encode_utf16().count()
}

pub(super) fn utf16_offset_to_utf8(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (byte_idx, ch) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_idx;
        }
        utf16_count += ch.len_utf16();
    }
    text.len()
}

impl gpui::Focusable for ChatView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::EntityInputHandler for ChatView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = self.active_input_text();
        let start = utf16_offset_to_utf8(text, range.start);
        let end = utf16_offset_to_utf8(text, range.end);
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.active_input_text();
        let cursor_utf8 = self.active_cursor_position().min(text.len());
        let cursor_utf16 = utf8_offset_to_utf16(text, cursor_utf8);
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
        self.state.marked_range.clone()
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        self.state.marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.sidebar_search_focused {
            self.state.marked_range = None;
            self.state.sidebar_search_query.push_str(text);
            self.trigger_sidebar_search(cx);
            cx.notify();
            return;
        }

        if self.state.conversation_title_editing {
            self.state.marked_range = None;
            if self.state.rename_replace_on_next_char {
                self.state.conversation_title_input.clear();
                self.state.rename_replace_on_next_char = false;
            }
            self.state.conversation_title_input.push_str(text);
            cx.notify();
            return;
        }

        if self.state.conversation_dropdown_open || self.state.profile_dropdown_open {
            self.state.marked_range = None;
            return;
        }

        let effective_range = range.or_else(|| self.state.marked_range.take());
        let input = &mut self.state.input_text;
        let (start_utf8, end_utf8) = if let Some(r) = effective_range {
            (
                utf16_offset_to_utf8(input, r.start),
                utf16_offset_to_utf8(input, r.end),
            )
        } else {
            let pos = self.state.cursor_position.min(input.len());
            (pos, pos)
        };

        input.replace_range(start_utf8..end_utf8, text);
        self.state.cursor_position = start_utf8 + text.len();
        self.state.marked_range = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.sidebar_search_focused {
            self.state.marked_range = None;
            self.state.sidebar_search_query.push_str(new_text);
            self.trigger_sidebar_search(cx);
            cx.notify();
            return;
        }
        if self.state.conversation_dropdown_open || self.state.profile_dropdown_open {
            return;
        }
        if self.state.conversation_title_editing {
            self.state.conversation_title_input.push_str(new_text);
            cx.notify();
            return;
        }

        let input = &mut self.state.input_text;
        let (start_utf8, end_utf8) = if let Some(r) = range {
            (
                utf16_offset_to_utf8(input, r.start),
                utf16_offset_to_utf8(input, r.end),
            )
        } else if let Some(ref mr) = self.state.marked_range {
            (
                utf16_offset_to_utf8(input, mr.start),
                utf16_offset_to_utf8(input, mr.end),
            )
        } else {
            let pos = self.state.cursor_position.min(input.len());
            (pos, pos)
        };

        input.replace_range(start_utf8..end_utf8, new_text);
        self.state.cursor_position = start_utf8 + new_text.len();

        // Compute marked range in UTF-16 over the newly inserted text
        let mark_start_utf16 = utf8_offset_to_utf16(input, start_utf8);
        let mark_end_utf16 = mark_start_utf16 + new_text.encode_utf16().count();
        self.state.marked_range = Some(mark_start_utf16..mark_end_utf16);

        if let Some(sel) = new_selected_range {
            let sel_utf8 = utf16_offset_to_utf8(input, mark_start_utf16 + sel.start);
            self.state.cursor_position = sel_utf8;
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Return the element bounds so the IME candidate window appears near the input area
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        let text = self.active_input_text();
        Some(utf8_offset_to_utf16(text, self.active_cursor_position()))
    }
}
