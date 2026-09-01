// NOTICE: PersonalAgent modified this file from gpui-component commit c5ade48.
// Changes: added this Apache-2.0 section 4(b) modification notice, and replaced
// the custom limited word classifier (ASCII/Latin/Cyrillic ranges plus a
// 128-character expansion cap) with Unicode UAX #29 default word boundaries
// through `unicode-segmentation`'s `split_word_bound_indices`, keeping the
// left-clipping behavior for byte offsets that are not character boundaries.
// The segments are cached per distinct run text in
// [`SelectableTextState`](super::text_selection::TextSelectionHandle) so drag
// updates binary-search the range under the pointer instead of re-segmenting
// the whole run from byte 0 on every update.

use std::{cmp, ops::Range};

use unicode_segmentation::UnicodeSegmentation;

/// The UAX #29 word segments of one text, built once per distinct run text.
///
/// Drag updates locate the segment under the pointer by binary search over
/// the cached ranges instead of re-segmenting from byte 0 on every update.
/// The cache's construction is the source of truth: callers invalidate it
/// exactly when the run text changes, so lookups never revalidate.
pub(crate) struct WordSegments {
    ranges: Vec<Range<usize>>,
}

impl WordSegments {
    /// Segments `text` into its UAX #29 word ranges.
    pub(crate) fn new(text: &str) -> Self {
        Self {
            ranges: text
                .split_word_bound_indices()
                .map(|(start, segment)| start..start + segment.len())
                .collect(),
        }
    }

    /// Returns the segment of `text` containing the byte at `offset`.
    ///
    /// `text` must be the text these segments were built from. Offsets that
    /// are not character boundaries clip left to the enclosing character; an
    /// offset at or past the end of the text resolves to no segment.
    pub(crate) fn range_at(&self, text: &str, offset: usize) -> Option<Range<usize>> {
        let offset = clip_offset_left(text, offset);
        self.ranges
            .binary_search_by(|range| {
                if offset < range.start {
                    cmp::Ordering::Greater
                } else if offset >= range.end {
                    cmp::Ordering::Less
                } else {
                    cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|index| self.ranges[index].clone())
    }
}

/// Returns the UAX #29 word segment containing the byte at `offset`.
///
/// Segments cover whitespace and punctuation runs as well as words, so
/// double-clicking either selects a word or the surrounding punctuation and
/// whitespace exactly as UAX #29 segments it. Offsets that are not character
/// boundaries clip left to the enclosing character; an offset at or past the
/// end of the text resolves to no segment.
pub(crate) fn word_range_at(text: &str, offset: usize) -> Option<Range<usize>> {
    WordSegments::new(text).range_at(text, offset)
}

pub(crate) fn line_range_at(text: &str, offset: usize) -> Range<usize> {
    let offset = clip_offset_left(text, offset);
    let start = text[..offset].rfind('\n').map_or(0, |newline| newline + 1);
    let end = text[offset..]
        .find('\n')
        .map_or(text.len(), |newline| offset + newline);
    start..end
}

fn clip_offset_left(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, offset: usize) -> Option<Range<usize>> {
        word_range_at(text, offset)
    }

    fn segment(text: &str, segment: &str) -> Option<Range<usize>> {
        let start = text.find(segment)?;
        Some(start..start + segment.len())
    }

    #[test]
    fn cached_binary_search_matches_a_linear_scan_for_every_offset() {
        let texts = [
            "",
            "word",
            "alpha beta, gamma!",
            "snake_case v1.2.3 29.3",
            "can't l’objectif",
            "cafe\u{0301}",
            "αβγδε привет مرحبا नमस्ते",
            "你好吗",
            "👨‍👩‍👧‍👦 👋🏽 🇺🇸",
            "a\r\nb",
            &"x".repeat(200),
        ];
        for text in texts.iter() {
            let cached = WordSegments::new(text);
            for offset in 0..=text.len() + 1 {
                let clipped = clip_offset_left(text, offset);
                let linear = text
                    .split_word_bound_indices()
                    .find(|(start, segment)| *start <= clipped && clipped < start + segment.len())
                    .map(|(start, segment)| start..start + segment.len());
                assert_eq!(
                    cached.range_at(text, offset),
                    linear,
                    "text {text:?} offset {offset}"
                );
            }
        }
    }

    #[test]
    fn rebuilding_segments_for_new_text_uses_the_new_boundaries() {
        let first = "alpha words beta";
        let second = "can't stop";
        let first_segments = WordSegments::new(first);
        assert_eq!(first_segments.range_at(first, 7), Some(6..11));
        // A cache rebuilt for changed text resolves against the new
        // segmentation instead of the stale ranges.
        let second_segments = WordSegments::new(second);
        assert_eq!(second_segments.range_at(second, 2), Some(0..5));
    }

    #[test]
    fn ascii_words_spaces_and_punctuation_are_separate_segments() {
        let text = "alpha beta, gamma!";
        assert_eq!(word(text, 0), Some(0..5));
        assert_eq!(word(text, 4), Some(0..5));
        assert_eq!(word(text, 5), Some(5..6));
        assert_eq!(word(text, 6), Some(6..10));
        assert_eq!(word(text, 10), Some(10..11));
        assert_eq!(word(text, 11), Some(11..12));
        assert_eq!(word(text, 12), Some(12..17));
        assert_eq!(word(text, 17), Some(17..18));
    }

    #[test]
    fn boundaries_are_right_biased_at_segment_edges() {
        let text = "end. next";
        // An offset exactly between two segments resolves to the segment
        // starting at the offset (the character under a click there).
        assert_eq!(word(text, 3), segment(text, "."));
        assert_eq!(word(text, 4), segment(text, " "));
        assert_eq!(word(text, 5), segment(text, "next"));
    }

    #[test]
    fn snake_case_and_digit_separated_numbers_stay_whole() {
        assert_eq!(word("snake_case_name", 7), Some(0..15));
        assert_eq!(word("29.3", 2), Some(0..4));
        assert_eq!(word("v1.2.3", 4), Some(0..6));
        // A period not between digits stays its own segment.
        assert_eq!(word("end. next", 3), segment("end. next", "."));
    }

    #[test]
    fn apostrophe_joined_words_stay_whole() {
        assert_eq!(word("can't", 2), Some(0..5));
        assert_eq!(word("l’objectif", 3), Some(0..12));
    }

    #[test]
    fn precomposed_and_decomposed_accents_are_one_word() {
        let precomposed = "café";
        assert_eq!(word(precomposed, 3), Some(0..5));

        let decomposed = "cafe\u{0301}";
        assert_eq!(word(decomposed, 3), Some(0..6));
    }

    #[test]
    fn greek_cyrillic_arabic_and_devanagari_words_are_whole() {
        assert_eq!(word("αβγδε", 2), Some(0..10));
        assert_eq!(word("привет", 4), Some(0..12));
        assert_eq!(word("مرحبا", 4), Some(0..10));
        // Devanagari vowel signs are extend characters and join the word.
        assert_eq!(word("नमस्ते", 6), Some(0..18));
    }

    #[test]
    fn cjk_ideographs_segment_individually() {
        let text = "你好吗";
        assert_eq!(word(text, 0), Some(0..3));
        assert_eq!(word(text, 3), Some(3..6));
        assert_eq!(word(text, 6), Some(6..9));
    }

    #[test]
    fn zwj_families_skin_tones_and_flags_stay_whole() {
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(word(family, 0), Some(0..25));
        assert_eq!(word(family, 8), Some(0..25));

        let skin_tone = "👋🏽";
        assert_eq!(word(skin_tone, 0), Some(0..8));

        let flag = "🇺🇸";
        assert_eq!(word(flag, 0), Some(0..8));
    }

    #[test]
    fn crlf_is_one_segment() {
        let text = "a\r\nb";
        assert_eq!(word(text, 1), Some(1..3));
        assert_eq!(word(text, 2), Some(1..3));
        assert_eq!(word(text, 3), Some(3..4));
    }

    #[test]
    fn long_identifiers_have_no_expansion_cap() {
        let identifier = "x".repeat(200);
        assert_eq!(word(&identifier, 150), Some(0..200));
        assert_eq!(word(&identifier, 199), Some(0..200));
    }

    #[test]
    fn mid_codepoint_offsets_clip_left_to_the_enclosing_character() {
        let text = "a🙂z";
        assert_eq!(word(text, 2), word(text, 1));
        assert_eq!(word(text, 4), word(text, 3));
    }

    #[test]
    fn empty_text_and_end_offsets_resolve_to_none() {
        assert_eq!(word("", 0), None);
        assert_eq!(word("word", 4), None);
        assert_eq!(word("word", 400), None);
    }
}
