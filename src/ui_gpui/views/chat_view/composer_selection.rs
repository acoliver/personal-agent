//! Composer text selection state and byte-offset helpers.
//!
//! This module owns the *transient* selection state for the `ChatView`
//! composer input. It is deliberately separate from both the message
//! transcript selection (`message_selection.rs`) and the custom GPUI
//! element (`composer_field.rs`) so that:
//!
//! - Pure selection arithmetic can be unit-tested without a GPUI window.
//! - The element only paints and hit-tests; it does not own policy.
//!
//! All offsets are **UTF-8 byte offsets** into `input_text`, matching the
//! convention used by `cursor_position` and `ChatState.input_text`.

/// A transient text selection in the composer.
///
/// `anchor` is where the drag started; `head` is where the pointer currently
/// sits. Either may be less than the other, giving the selection direction.
/// When `anchor == head` the selection is a caret (collapsed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ComposerSelection {
    pub anchor: usize,
    pub head: usize,
}

impl ComposerSelection {
    #[must_use]
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// A caret (collapsed selection) at the given position.
    #[must_use]
    pub const fn caret(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    /// `true` when `anchor == head` (no selected text).
    #[must_use]
    pub const fn is_collapsed(&self) -> bool {
        self.anchor == self.head
    }

    /// The inclusive start of the selected range (minimum of anchor/head).
    #[must_use]
    pub const fn start(&self) -> usize {
        if self.anchor <= self.head {
            self.anchor
        } else {
            self.head
        }
    }

    /// The exclusive end of the selected range (maximum of anchor/head).
    #[must_use]
    pub const fn end(&self) -> usize {
        if self.anchor >= self.head {
            self.anchor
        } else {
            self.head
        }
    }

    /// The byte length of the selected range.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end() - self.start()
    }

    /// `true` when the range has a non-zero length.
    #[must_use]
    pub const fn is_non_empty(&self) -> bool {
        self.len() > 0
    }

    /// Return a UTF-8-safe selection constrained to the current text.
    #[must_use]
    pub const fn clamped(self, text: &str) -> Self {
        Self::new(
            clamp_to_char_boundary(text, self.anchor),
            clamp_to_char_boundary(text, self.head),
        )
    }

    /// Whether the selection direction runs from a later offset to an earlier one.
    #[must_use]
    pub const fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }
}

/// Clamp a byte offset to a valid UTF-8 char boundary within `text`.
///
/// If the offset falls inside a multi-byte character it is rounded **down**
/// to the preceding boundary, matching native text-field behaviour where a
/// caret never splits a grapheme.
#[must_use]
pub const fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    if offset == 0 {
        return 0;
    }
    if text.is_char_boundary(offset) {
        return offset;
    }
    // Walk backward to the nearest boundary.
    let mut i = offset;
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Delete the selected range from `text` and return the new text plus the
/// caret position (at the deletion point).
#[must_use]
pub fn delete_range(text: &str, sel: ComposerSelection) -> (String, usize) {
    let sel = sel.clamped(text);
    if sel.is_collapsed() {
        return (text.to_string(), sel.head);
    }
    let start = sel.start();
    let end = sel.end();
    let mut result = String::with_capacity(text.len() - (end - start));
    result.push_str(&text[..start]);
    result.push_str(&text[end..]);
    (result, start)
}

/// Replace the selected range with `replacement`, returning the new text and
/// the caret position (just after the replacement).
#[must_use]
pub fn replace_range(text: &str, sel: ComposerSelection, replacement: &str) -> (String, usize) {
    let sel = sel.clamped(text);
    let start = sel.start();
    let end = sel.end();
    let mut result = String::with_capacity(text.len() - (end - start) + replacement.len());
    result.push_str(&text[..start]);
    result.push_str(replacement);
    result.push_str(&text[end..]);
    let caret = clamp_to_char_boundary(&result, start + replacement.len());
    (result, caret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_selection_is_collapsed() {
        let sel = ComposerSelection::caret(5);
        assert!(sel.is_collapsed());
        assert!(!sel.is_non_empty());
        assert_eq!(sel.start(), 5);
        assert_eq!(sel.end(), 5);
        assert_eq!(sel.len(), 0);
    }

    #[test]
    fn forward_selection() {
        let sel = ComposerSelection::new(2, 8);
        assert!(!sel.is_collapsed());
        assert!(sel.is_non_empty());
        assert_eq!(sel.start(), 2);
        assert_eq!(sel.end(), 8);
        assert_eq!(sel.len(), 6);
    }

    #[test]
    fn reverse_selection_swaps_start_end() {
        let sel = ComposerSelection::new(8, 2);
        assert_eq!(sel.start(), 2);
        assert_eq!(sel.end(), 8);
        assert_eq!(sel.len(), 6);
    }

    #[test]
    fn clamp_to_boundary_rounds_down_inside_multibyte() {
        // 'é' is 2 bytes (0xC3 0xA9), occupying offsets 1..3.
        let text = "aéb";
        assert_eq!(clamp_to_char_boundary(text, 0), 0);
        assert_eq!(clamp_to_char_boundary(text, 1), 1);
        assert_eq!(clamp_to_char_boundary(text, 2), 1); // mid 'é' -> its start
        assert_eq!(clamp_to_char_boundary(text, 3), 3);
        assert_eq!(clamp_to_char_boundary(text, 100), text.len());
    }

    #[test]
    fn delete_range_forward() {
        let text = "hello world";
        let sel = ComposerSelection::new(0, 5);
        let (result, caret) = delete_range(text, sel);
        assert_eq!(result, " world");
        assert_eq!(caret, 0);
    }

    #[test]
    fn delete_range_reverse_same_result() {
        let text = "hello world";
        let sel = ComposerSelection::new(5, 0);
        let (result, caret) = delete_range(text, sel);
        assert_eq!(result, " world");
        assert_eq!(caret, 0);
    }

    #[test]
    fn delete_range_collapsed_is_noop() {
        let text = "hello";
        let sel = ComposerSelection::caret(3);
        let (result, caret) = delete_range(text, sel);
        assert_eq!(result, "hello");
        assert_eq!(caret, 3);
    }

    #[test]
    fn replace_range_inserts_text() {
        let text = "hello world";
        let sel = ComposerSelection::new(0, 5);
        let (result, caret) = replace_range(text, sel, "goodbye");
        assert_eq!(result, "goodbye world");
        assert_eq!(caret, "goodbye".len());
    }

    #[test]
    fn replace_range_unicode_safe() {
        // "café" — 'é' at bytes 3..5
        let text = "café!";
        let sel = ComposerSelection::new(3, 5); // replace 'é'
        let (result, caret) = replace_range(text, sel, "e");
        assert_eq!(result, "cafe!");
        assert_eq!(caret, 4); // after 'e'
    }

    #[test]
    fn replace_range_at_caret_is_insertion() {
        let text = "hello";
        let sel = ComposerSelection::caret(2);
        let (result, caret) = replace_range(text, sel, "XYZ");
        assert_eq!(result, "heXYZllo");
        assert_eq!(caret, 5);
    }

    #[test]
    fn delete_range_unicode_safe() {
        let text = "a😀b"; // '😀' is 4 bytes
        let sel = ComposerSelection::new(1, 5); // select '😀'
        let (result, caret) = delete_range(text, sel);
        assert_eq!(result, "ab");
        assert_eq!(caret, 1);
    }

    #[test]
    fn stale_mid_character_ranges_are_clamped_before_slicing() {
        let text = "aéb";

        let (deleted, caret) = delete_range(text, ComposerSelection::new(2, 100));
        assert_eq!(deleted, "a");
        assert_eq!(caret, 1);

        let (replaced, caret) = replace_range(text, ComposerSelection::new(2, 100), "z");
        assert_eq!(replaced, "az");
        assert_eq!(caret, 2);
    }
}
