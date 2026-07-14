//! UTF-8-safe text helpers.
//!
//! Every selection boundary must be a valid UTF-8 byte boundary
//! (REQ-151-007). These helpers guarantee that invariant.

use std::ops::Range;

/// Clamp `byte_offset` to the nearest UTF-8 character boundary at or after
/// `byte_offset`, capped at `text.len()`.
///
/// This rounds **forward** so that a mid-character offset moves to the start
/// of the next character, and an offset past the end moves to the end.
///
/// @plan PLAN-20260713-ISSUE151 Phase 1
/// @requirement REQ-151-007
#[must_use]
pub fn clamp_to_char_boundary(text: &str, byte_offset: usize) -> usize {
    if byte_offset >= text.len() {
        return text.len();
    }
    if text.is_char_boundary(byte_offset) {
        return byte_offset;
    }
    // Round forward to the next boundary. `ceil_char_boundary` is stable as of
    // Rust 1.82+; we implement it manually for broader compatibility.
    let mut pos = byte_offset + 1;
    while pos < text.len() && !text.is_char_boundary(pos) {
        pos += 1;
    }
    pos.min(text.len())
}

/// Return the byte range of the Unicode word containing the character at
/// `byte_offset`.
///
/// A "word" is a maximal run of alphanumeric characters (including underscore,
/// apostrophe, and combining marks) as determined by `char::is_alphanumeric`.
/// Whitespace, punctuation, and control characters are word separators. If the
/// character at `byte_offset` is a separator or the text is empty, an empty
/// range at a clamped boundary is returned.
///
/// Both the start and end of the returned range are UTF-8 character
/// boundaries.
///
/// @plan PLAN-20260713-ISSUE151 Phase 1
/// @requirement REQ-151-004, REQ-151-007
#[must_use]
pub fn word_range_at(text: &str, byte_offset: usize) -> Range<usize> {
    let clamped = clamp_to_char_boundary(text, byte_offset);
    if clamped >= text.len() {
        return clamped..clamped;
    }

    // Find the character index at `clamped` and decide if it's a word char.
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let Some(idx) = chars.iter().position(|(pos, _)| *pos == clamped) else {
        return clamped..clamped;
    };

    if !is_word_char(chars[idx].1) {
        let ch = chars[idx].1;
        return if is_single_character_selection(ch) {
            clamped..clamped + ch.len_utf8()
        } else {
            clamped..clamped
        };
    }

    let start = scan_word_start(&chars, idx);
    let end = scan_word_end(&chars, idx);
    start..end
}

/// Determine whether a character is part of a word.
///
/// Alphanumeric characters, apostrophes (for contractions like "it's"), and
/// underscores are word characters. Combining marks attach to the preceding
/// character, so they are also word characters.
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '_' || is_combining_mark(ch)
}

/// Native text controls treat a standalone non-ASCII symbol, including emoji,
/// as a selectable unit on double-click. ASCII punctuation remains a separator.
fn is_single_character_selection(ch: char) -> bool {
    !ch.is_ascii() && !ch.is_whitespace() && !is_combining_mark(ch)
}

/// Return `true` when `ch` is a Unicode combining mark (Mn / Me categories).
fn is_combining_mark(ch: char) -> bool {
    // Common combining ranges: U+0300–U+036F (combining diacriticals) and the
    // variation selectors / combining symbols. We keep this intentionally
    // conservative — `is_alphanumeric` already covers letter+mark composition
    // for most grapheme clusters via NFC, but combining marks on their own
    // are not alphanumeric.
    let code = ch as u32;
    (0x0300..=0x036F).contains(&code)
        || (0x1AB0..=0x1AFF).contains(&code)
        || (0x1DC0..=0x1DFF).contains(&code)
        || (0x20D0..=0x20FF).contains(&code)
        || (0xFE20..=0xFE2F).contains(&code)
}

/// Scan backward from `idx` to the first word character in the run.
fn scan_word_start(chars: &[(usize, char)], idx: usize) -> usize {
    let mut start = idx;
    while start > 0 && is_word_char(chars[start - 1].1) {
        start -= 1;
    }
    chars[start].0
}

/// Scan forward from `idx` to one past the last word character in the run.
fn scan_word_end(chars: &[(usize, char)], idx: usize) -> usize {
    let mut end = idx;
    while end + 1 < chars.len() && is_word_char(chars[end + 1].1) {
        end += 1;
    }
    // Byte offset of the character *after* the last word char.
    let last_pos = chars[end].0;
    last_pos + chars[end].1.len_utf8()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_at_zero_is_zero() {
        assert_eq!(clamp_to_char_boundary("abc", 0), 0);
    }

    #[test]
    fn word_in_pure_ascii() {
        assert_eq!(&"hello world"[word_range_at("hello world", 0)], "hello");
    }
}
