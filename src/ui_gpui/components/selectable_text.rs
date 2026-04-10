//! Word/paragraph boundary helpers used by the chat-view selection logic.
//!
//! Earlier iterations of this module hosted a standalone `SelectableText`
//! GPUI element. The chat view ultimately implemented selection directly on
//! its own scroll container (see `chat_view::render`), so the element and its
//! `SelectionState` were removed and only the boundary helpers remain.
//!
//! @plan PLAN-20260406-ISSUE151.P01
//! @requirement REQ-TEXT-SELECT-001

use std::ops::Range;

/// Find the word boundaries surrounding `position` (UTF-8 byte offset).
///
/// Returns an empty range when `position` does not sit on a word character.
/// All returned indices are guaranteed to fall on `char` boundaries so the
/// returned range can be used directly to slice `text`.
///
/// @plan PLAN-20260406-ISSUE151.P01
#[must_use]
pub fn find_word_boundaries(text: &str, position: usize) -> Range<usize> {
    let len = text.len();
    let pos = position.min(len);

    if !text.is_char_boundary(pos) {
        let nearest = text
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= pos)
            .last()
            .unwrap_or(0);
        if nearest == pos || !text.is_char_boundary(pos) {
            return nearest..nearest;
        }
    }

    if let Some(ch) = text[pos..].chars().next() {
        if !is_word_char(ch) {
            return pos..pos;
        }
    } else if pos >= len {
        return pos..pos;
    }

    let mut start = pos;
    for (i, ch) in text.char_indices().rev() {
        if i >= pos {
            continue;
        }
        if is_word_char(ch) {
            start = i;
        } else {
            break;
        }
    }

    let mut end = pos;
    for (i, ch) in text.char_indices() {
        if i < pos {
            continue;
        }
        if is_word_char(ch) {
            end = i + ch.len_utf8();
        } else {
            break;
        }
    }

    start..end
}

/// Find the paragraph boundaries surrounding `position` (UTF-8 byte offset).
///
/// A paragraph is delimited by `\n` or `\r`. The returned range never
/// includes the delimiters and always falls on `char` boundaries.
///
/// @plan PLAN-20260406-ISSUE151.P01
#[must_use]
pub fn find_paragraph_boundaries(text: &str, position: usize) -> Range<usize> {
    let len = text.len();
    let pos = position.min(len);

    let mut start = pos;
    for (i, ch) in text.char_indices().rev() {
        if i >= pos {
            continue;
        }
        if ch == '\n' || ch == '\r' {
            start = i + ch.len_utf8();
            break;
        }
        start = i;
    }

    let mut end = pos;
    for (i, ch) in text.char_indices() {
        if i < pos {
            continue;
        }
        if ch == '\n' || ch == '\r' {
            break;
        }
        end = i + ch.len_utf8();
    }

    start..end
}

/// Returns `true` when `ch` is considered part of a word for double-click
/// word selection. Mirrors common editor heuristics: alphanumerics plus `_`.
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_word_boundaries_middle() {
        let text = "Hello, world!";
        let range = find_word_boundaries(text, 8);
        assert_eq!(range, 7..12);
        assert_eq!(&text[range], "world");
    }

    #[test]
    fn test_find_word_boundaries_start() {
        let text = "Hello, world!";
        let range = find_word_boundaries(text, 0);
        assert_eq!(range, 0..5);
        assert_eq!(&text[range], "Hello");
    }

    #[test]
    fn test_find_word_boundaries_end() {
        let text = "Hello";
        let range = find_word_boundaries(text, 4);
        assert_eq!(range, 0..5);
    }

    #[test]
    fn test_find_word_boundaries_space() {
        let text = "Hello world";
        let range = find_word_boundaries(text, 5);
        assert!(range.is_empty());
        assert_eq!(range.start, 5);
        assert_eq!(range.end, 5);
    }

    #[test]
    fn test_find_word_boundaries_multibyte() {
        // "café" — 'é' is two bytes (0xC3 0xA9). Position 2 sits on 'f'.
        let text = "café au lait";
        let range = find_word_boundaries(text, 2);
        // The first word is "café" which is 5 bytes (c=1, a=1, f=1, é=2).
        assert_eq!(range, 0..5);
        assert_eq!(&text[range], "café");
    }

    #[test]
    fn test_find_word_boundaries_position_inside_multibyte_char() {
        // Hitting the middle byte of 'é' should snap to a char boundary
        // instead of panicking.
        let text = "café";
        let range = find_word_boundaries(text, 4); // mid-'é'
                                                   // We expect the helper to return a non-panicking range whose
                                                   // endpoints are valid char boundaries.
        assert!(text.is_char_boundary(range.start));
        assert!(text.is_char_boundary(range.end));
    }

    #[test]
    fn test_find_paragraph_boundaries_middle() {
        let text = "First line\nSecond line\nThird line";
        let range = find_paragraph_boundaries(text, 15);
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
    fn test_find_paragraph_boundaries_multibyte() {
        let text = "Café\nNaïveté";
        let range = find_paragraph_boundaries(text, 1);
        // First paragraph is "Café" (5 bytes).
        assert_eq!(&text[range.clone()], "Café");
        assert!(text.is_char_boundary(range.start));
        assert!(text.is_char_boundary(range.end));
    }
}
