//! Selectable text component with traditional text selection behavior.
//!
//! This component implements standard macOS-style text selection:
//! - Click and drag to select text
//! - Double-click to select word
//! - Triple-click to select paragraph
//! - Visual highlighting of selected text
//! - Cmd+C to copy selected text
//!
//! @plan PLAN-20260406-ISSUE151.P01
//! @requirement REQ-TEXT-SELECT-001

#![allow(clippy::missing_const_for_fn, clippy::must_use_candidate)]

use gpui::{
    div, prelude::*, px, ClipboardItem, Context, FocusHandle, Focusable, IntoElement, Render,
    Styled, Window,
};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use crate::ui_gpui::theme::Theme;

/// Selection state for tracking drag selection.
///
/// @plan PLAN-20260406-ISSUE151.P01
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionState {
    /// The range of selected text (byte offsets in UTF-8).
    pub range: Option<Range<usize>>,
    /// Whether we're currently in a drag selection operation.
    pub is_dragging: bool,
    /// The start position of drag selection (byte offset).
    pub drag_start: Option<usize>,
}

impl SelectionState {
    /// Create a new empty selection state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there's an active selection.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.range.as_ref().is_some_and(|r| !r.is_empty())
    }

    /// Get the selected text range if any.
    #[must_use]
    pub fn selection_range(&self) -> Option<Range<usize>> {
        self.range.clone()
    }

    /// Start a drag selection at the given position.
    pub fn start_drag(&mut self, position: usize) {
        self.is_dragging = true;
        self.drag_start = Some(position);
        self.range = Some(position..position);
    }

    /// Update drag selection to new position.
    pub fn update_drag(&mut self, position: usize) {
        if let Some(start) = self.drag_start {
            let range = if position < start {
                position..start
            } else {
                start..position
            };
            self.range = Some(range);
        }
    }

    /// End drag selection.
    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        // Keep the selection range
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.range = None;
        self.is_dragging = false;
        self.drag_start = None;
    }

    /// Select all text.
    pub fn select_all(&mut self, total_len: usize) {
        self.range = Some(0..total_len);
        self.is_dragging = false;
        self.drag_start = None;
    }

    /// Select a word around the given position.
    pub fn select_word(&mut self, text: &str, position: usize) {
        let range = find_word_boundaries(text, position);
        self.range = Some(range);
        self.is_dragging = false;
        self.drag_start = None;
    }

    /// Select the entire paragraph around the given position.
    pub fn select_paragraph(&mut self, text: &str, position: usize) {
        let range = find_paragraph_boundaries(text, position);
        self.range = Some(range);
        self.is_dragging = false;
        self.drag_start = None;
    }
}

/// Find word boundaries around a position.
///
/// @plan PLAN-20260406-ISSUE151.P01
pub fn find_word_boundaries(text: &str, position: usize) -> Range<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let pos = position.min(len);

    // If we're not at a word character, return empty range
    if pos < len && !is_word_char(bytes[pos] as char) {
        return pos..pos;
    }
    // If we're past the end, return empty range
    if pos >= len {
        return pos..pos;
    }

    // Find start of word
    let mut start = pos;
    while start > 0 && is_word_char(bytes[start - 1] as char) {
        start -= 1;
    }

    // Find end of word
    let mut end = pos;
    while end < len && is_word_char(bytes[end] as char) {
        end += 1;
    }

    start..end
}

/// Find paragraph boundaries around a position.
///
/// @plan PLAN-20260406-ISSUE151.P01
pub fn find_paragraph_boundaries(text: &str, position: usize) -> Range<usize> {
    let len = text.len();
    let pos = position.min(len);

    // Find start of paragraph (after newline or at beginning)
    let mut start = pos;
    while start > 0 {
        let ch = text.as_bytes()[start - 1] as char;
        if ch == '\n' || ch == '\r' {
            break;
        }
        start -= 1;
    }

    // Find end of paragraph (before newline or at end)
    let mut end = pos;
    while end < len {
        let ch = text.as_bytes()[end] as char;
        if ch == '\n' || ch == '\r' {
            break;
        }
        end += 1;
    }

    start..end
}

/// Check if a character is part of a word.
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// A text element that supports traditional selection behavior.
///
/// @plan PLAN-20260406-ISSUE151.P01
pub struct SelectableText {
    /// The text content.
    text: String,
    /// Shared selection state.
    selection: Rc<RefCell<SelectionState>>,
    /// Focus handle for keyboard events.
    focus_handle: FocusHandle,
}

impl SelectableText {
    /// Create a new selectable text component.
    pub fn new(text: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            text: text.into(),
            selection: Rc::new(RefCell::new(SelectionState::new())),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Get the text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the selection state.
    pub fn selection(&self) -> SelectionState {
        self.selection.borrow().clone()
    }

    /// Select all text.
    pub fn select_all(&self, cx: &mut Context<Self>) {
        let len = self.text.len();
        self.selection.borrow_mut().select_all(len);
        cx.notify();
    }

    /// Clear selection.
    pub fn clear_selection(&self, cx: &mut Context<Self>) {
        self.selection.borrow_mut().clear();
        cx.notify();
    }

    /// Copy selected text to clipboard.
    pub fn copy_selection(&self, cx: &mut Context<Self>) {
        if let Some(range) = self.selection.borrow().range.clone() {
            if !range.is_empty() {
                let selected = self.text[range].to_string();
                cx.write_to_clipboard(ClipboardItem::new_string(selected));
            }
        }
    }

    /// Get selected text if any.
    pub fn selected_text(&self) -> Option<String> {
        self.selection
            .borrow()
            .range
            .as_ref()
            .map(|r| self.text[r.clone()].to_string())
    }

    /// Handle mouse down event.
    pub fn handle_mouse_down(&self, click_count: usize, position: usize, cx: &mut Context<Self>) {
        match click_count {
            1 => {
                self.selection.borrow_mut().start_drag(position);
            }
            2 => {
                self.selection
                    .borrow_mut()
                    .select_word(&self.text, position);
            }
            3 => {
                self.selection
                    .borrow_mut()
                    .select_paragraph(&self.text, position);
            }
            _ => {
                self.selection.borrow_mut().clear();
            }
        }
        cx.notify();
    }

    /// Handle mouse drag event.
    pub fn handle_mouse_drag(&self, position: usize, cx: &mut Context<Self>) {
        self.selection.borrow_mut().update_drag(position);
        cx.notify();
    }

    /// Handle mouse up event.
    pub fn handle_mouse_up(&self, cx: &mut Context<Self>) {
        self.selection.borrow_mut().end_drag();
        cx.notify();
    }
}

impl Focusable for SelectableText {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectableText {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.text.clone();
        let selection = self.selection.borrow().clone();

        div()
            .id("selectable-text")
            .w_full()
            .min_h(px(20.0))
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_primary())
            .cursor_text()
            .child(render_text_with_selection(&text, selection.range))
    }
}

/// Render text with selection highlight.
///
/// @plan PLAN-20260406-ISSUE151.P01
fn render_text_with_selection(text: &str, selection: Option<Range<usize>>) -> gpui::AnyElement {
    use gpui::StyledText;

    let Some(range) = selection else {
        // No selection, just render plain text
        return div().child(text.to_string()).into_any_element();
    };

    if range.is_empty() {
        // Empty selection, just render plain text
        return div().child(text.to_string()).into_any_element();
    }

    // Split text into three parts: before, selected, after
    let before = &text[..range.start];
    let selected = &text[range.clone()];
    let after = &text[range.end..];

    // Build styled text with selection highlight
    let mut parts = Vec::new();

    if !before.is_empty() {
        parts.push(gpui::TextRun {
            len: before.len(),
            color: Theme::text_primary(),
            ..Default::default()
        });
    }

    if !selected.is_empty() {
        parts.push(gpui::TextRun {
            len: selected.len(),
            color: Theme::selection_fg(),
            background_color: Some(Theme::selection_bg()),
            ..Default::default()
        });
    }

    if !after.is_empty() {
        parts.push(gpui::TextRun {
            len: after.len(),
            color: Theme::text_primary(),
            ..Default::default()
        });
    }

    let full_text = format!("{before}{selected}{after}");
    let styled = StyledText::new(full_text).with_runs(parts);

    div().child(styled).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_state_initial() {
        let state = SelectionState::new();
        assert!(!state.has_selection());
        assert!(state.selection_range().is_none());
    }

    #[test]
    fn test_selection_state_drag() {
        let mut state = SelectionState::new();
        state.start_drag(5);
        assert!(state.is_dragging);
        assert_eq!(state.drag_start, Some(5));
        assert_eq!(state.range, Some(5..5));
        assert!(!state.has_selection()); // Empty range

        state.update_drag(10);
        assert_eq!(state.range, Some(5..10));
        assert!(state.has_selection());

        state.end_drag();
        assert!(!state.is_dragging);
        assert_eq!(state.range, Some(5..10)); // Selection persists
    }

    #[test]
    fn test_selection_state_drag_reversed() {
        let mut state = SelectionState::new();
        state.start_drag(10);
        state.update_drag(5);
        assert_eq!(state.range, Some(5..10)); // Normalized to start < end
    }

    #[test]
    fn test_selection_state_clear() {
        let mut state = SelectionState::new();
        state.start_drag(0);
        state.update_drag(10);
        state.end_drag();
        assert!(state.has_selection());

        state.clear();
        assert!(!state.has_selection());
        assert!(!state.is_dragging);
    }

    #[test]
    fn test_selection_state_select_all() {
        let mut state = SelectionState::new();
        state.select_all(100);
        assert_eq!(state.range, Some(0..100));
        assert!(!state.is_dragging);
    }

    #[test]
    fn test_find_word_boundaries_middle() {
        let text = "Hello, world!";
        let range = find_word_boundaries(text, 8); // 'w' in 'world'
        assert_eq!(range, 7..12);
        assert_eq!(&text[range], "world");
    }

    #[test]
    fn test_find_word_boundaries_start() {
        let text = "Hello, world!";
        let range = find_word_boundaries(text, 0); // 'H'
        assert_eq!(range, 0..5);
        assert_eq!(&text[range], "Hello");
    }

    #[test]
    fn test_find_word_boundaries_end() {
        let text = "Hello";
        let range = find_word_boundaries(text, 4); // 'o'
        assert_eq!(range, 0..5);
    }

    #[test]
    fn test_find_word_boundaries_space() {
        let text = "Hello world";
        let range = find_word_boundaries(text, 5); // At space
                                                   // Space is not a word character, so should return empty range at position
        assert!(range.is_empty());
        assert_eq!(range.start, 5);
        assert_eq!(range.end, 5);
    }

    #[test]
    fn test_find_paragraph_boundaries_middle() {
        let text = "First line\nSecond line\nThird line";
        let range = find_paragraph_boundaries(text, 15); // In 'Second line'
        assert_eq!(range, 11..22);
        assert_eq!(&text[range], "Second line");
    }

    #[test]
    fn test_find_paragraph_boundaries_single() {
        let text = "Single paragraph";
        let range = find_paragraph_boundaries(text, 5);
        assert_eq!(range, 0..16);
    }

    #[test]
    fn test_find_paragraph_boundaries_first() {
        let text = "First\nSecond";
        let range = find_paragraph_boundaries(text, 2);
        assert_eq!(range, 0..5);
    }

    #[test]
    fn test_selection_state_select_word() {
        let mut state = SelectionState::new();
        let text = "Hello, world!";
        state.select_word(text, 8);
        assert_eq!(state.range, Some(7..12));
    }

    #[test]
    fn test_selection_state_select_paragraph() {
        let mut state = SelectionState::new();
        let text = "First line\nSecond line\nThird line";
        state.select_paragraph(text, 15);
        assert_eq!(state.range, Some(11..22));
    }
}
