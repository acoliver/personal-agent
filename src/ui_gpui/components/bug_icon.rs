//! Bug icon indicator for the error log.
//!
//! Uses the `"!"` exclamation character styled consistently with the existing
//! single-character toolbar button pattern (see `icon_btn!` in `render_bars.rs`).
//!
//! The character is surfaced as the public constant [`BUG_CHAR`] so that both
//! the title-bar button and the inline error indicator share a single source of
//! truth for the icon glyph.

/// The character used for the bug/error indicator throughout the UI.
///
/// Consistent with the codebase pattern of using single characters for toolbar
/// icons ("T", "Y", "R", "H", "!", …).
pub const BUG_CHAR: &str = "!";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bug_char_is_non_empty() {
        assert!(!BUG_CHAR.is_empty());
    }
}
