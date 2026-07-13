//! Selection model: anchor/head offsets, modes, and UTF-8-safe helpers.

use std::fmt;

use super::helpers::{clamp_to_char_boundary, word_range_at};
use super::SemanticBlock;

/// How a selection was created.
///
/// @plan PLAN-20260713-ISSUE151 Phase 1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Character-level (drag) selection.
    Char,
    /// Word selection (double-click).
    Word,
    /// Semantic-block selection (triple-click).
    Block,
}

/// A transient text selection within a single message's visible document.
///
/// `anchor` is the fixed end (where the gesture started); `head` is the active
/// end (where the cursor currently is). Forward selections have `anchor <=
/// head`; reverse selections have `anchor > head`.
///
/// All offsets are UTF-8 byte offsets into the visible document text. They
/// must be character boundaries (REQ-151-007).
///
/// @plan PLAN-20260713-ISSUE151 Phase 1
#[derive(Clone)]
pub struct Selection {
    anchor: usize,
    head: usize,
    mode: SelectionMode,
}

impl Selection {
    /// Create a new character-mode selection with the given anchor and head.
    #[must_use]
    pub fn new(anchor: usize, head: usize) -> Self {
        Self {
            anchor,
            head,
            mode: SelectionMode::Char,
        }
    }

    /// Create a caret (empty selection) at a single offset.
    #[must_use]
    pub fn char(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// Create a word-mode selection spanning `range`.
    #[must_use]
    pub fn word(range: std::ops::Range<usize>) -> Self {
        Self {
            anchor: range.start,
            head: range.end,
            mode: SelectionMode::Word,
        }
    }

    /// Create a block-mode selection spanning `range`.
    #[must_use]
    pub fn block(range: std::ops::Range<usize>) -> Self {
        Self {
            anchor: range.start,
            head: range.end,
            mode: SelectionMode::Block,
        }
    }

    /// Return the anchor offset.
    #[must_use]
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// Return the head offset.
    #[must_use]
    pub fn head(&self) -> usize {
        self.head
    }

    /// Return the selection mode.
    #[must_use]
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Return `true` when the selection covers no characters.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Return `true` when the head precedes the anchor (reverse selection).
    #[must_use]
    pub fn is_reverse(&self) -> bool {
        self.head < self.anchor
    }

    /// Return `true` when `offset` is within `[min(anchor,head),
    /// max(anchor,head))`.
    #[must_use]
    pub fn contains(&self, offset: usize) -> bool {
        let (start, end) = self.ordered_bounds();
        offset >= start && offset < end
    }

    /// Return the byte range as `start..end` regardless of direction.
    #[must_use]
    pub fn ordered_range(&self) -> std::ops::Range<usize> {
        let (start, end) = self.ordered_bounds();
        start..end
    }

    fn ordered_bounds(&self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// Return a new selection clamped to `text.len()` with all offsets snapped
    /// to UTF-8 character boundaries.
    #[must_use]
    pub fn clamped(&self, text: &str) -> Self {
        let anchor = clamp_to_char_boundary(text, self.anchor);
        let head = clamp_to_char_boundary(text, self.head);
        Self {
            anchor,
            head,
            mode: self.mode,
        }
    }

    /// Return a new word-mode selection expanded to the word boundaries around
    /// the current anchor.
    ///
    /// If the anchor is on a word, both anchor and head are moved to the word
    /// start/end respectively. If the anchor is on a separator, the selection
    /// remains empty.
    #[must_use]
    pub fn to_word(&self, text: &str) -> Self {
        let range = word_range_at(text, self.anchor);
        Self {
            anchor: range.start,
            head: range.end,
            mode: SelectionMode::Word,
        }
    }

    /// Return a new block-mode selection spanning exactly the given semantic
    /// block's range.
    #[must_use]
    pub fn to_block(_text: &str, block: &SemanticBlock) -> Self {
        Self {
            anchor: block.range.start,
            head: block.range.end,
            mode: SelectionMode::Block,
        }
    }
}

impl PartialEq for Selection {
    fn eq(&self, other: &Self) -> bool {
        self.anchor == other.anchor && self.head == other.head && self.mode == other.mode
    }
}

impl Eq for Selection {}

impl fmt::Debug for Selection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Selection")
            .field("anchor", &self.anchor)
            .field("head", &self.head)
            .field("mode", &self.mode)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_is_empty() {
        assert!(Selection::char(5).is_empty());
    }

    #[test]
    fn ordered_range_handles_reverse() {
        assert_eq!(Selection::new(8, 2).ordered_range(), 2..8);
    }
}
