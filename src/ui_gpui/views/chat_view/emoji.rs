//! Emoji filtering for rendered chat content.
//!
//! Display-only: the underlying database storage always keeps the original text.

/// Strip emojis from a string, replacing them with empty string.
/// This only affects display, not the underlying database storage.
pub(super) fn strip_emojis(text: &str) -> String {
    text.chars().filter(|c| !is_emoji(*c)).collect()
}

/// Check if a character is an emoji.
/// Uses Unicode ranges for emoji blocks.
const fn is_emoji(c: char) -> bool {
    // Basic emoji ranges - these cover most emojis
    matches!(c,
        '\u{1F600}'..='\u{1F64F}' |  // Emoticons
        '\u{1F300}'..='\u{1F5FF}' |  // Misc Symbols and Pictographs
        '\u{1F680}'..='\u{1F6FF}' |  // Transport and Map
        '\u{1F1E0}'..='\u{1F1FF}' |  // Flags
        '\u{2600}'..='\u{26FF}'   |  // Misc symbols
        '\u{2700}'..='\u{27BF}'   |  // Dingbats
        '\u{1F900}'..='\u{1F9FF}' |  // Supplemental Symbols and Pictographs
        '\u{1FA00}'..='\u{1FA6F}' |  // Chess Symbols
        '\u{1FA70}'..='\u{1FAFF}' |  // Symbols and Pictographs Extended-A
        '\u{2B50}'                |  // Star
        '\u{2B55}'                |  // Circle
        '\u{25AA}'..='\u{25AB}'   |  // Small squares
        '\u{25B6}' | '\u{25C0}'   |  // Play buttons
        '\u{25FB}'..='\u{25FE}'   |  // Medium squares
        '\u{2934}'..='\u{2935}'   |  // Arrows
        '\u{2B05}'..='\u{2B07}'   |  // Arrows
        '\u{2B1B}'..='\u{2B1C}'   |  // Squares
        '\u{3030}'                |  // Wavy dash
        '\u{303D}'                |  // Part alternation mark
        '\u{3297}'                |  // Circled ideograph congratulation
        '\u{3299}'                |  // Circled ideograph secret
        '\u{FE0F}'                |  // Variation Selector-16
        '\u{20E3}'                |  // Combining enclosing keycap
        '\u{E0020}'..='\u{E007F}' // Tags for emoji sequences
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_emojis_removes_basic_emojis() {
        // Test basic emoji removal
        let input = "Hello \u{1F60A} World";
        assert_eq!(strip_emojis(input), "Hello  World");
        assert_eq!(strip_emojis("No emojis here"), "No emojis here");
        // Multiple emojis
        let emojis_only = "\u{1F389}\u{1F38A}\u{1F380}";
        assert_eq!(strip_emojis(emojis_only), "");
    }

    #[test]
    fn test_strip_emojis_handles_mixed_content() {
        let input = "Great news! \u{1F389} We shipped the feature \u{1F680}";
        assert_eq!(strip_emojis(input), "Great news!  We shipped the feature ");
    }

    #[test]
    fn test_strip_emojis_preserves_regular_characters() {
        // Test that regular punctuation and special chars are preserved
        let input = "Special chars: !@#$%^&*()_+-=[]{}|;':,./<>?";
        assert_eq!(strip_emojis(input), input);
    }

    #[test]
    fn test_strip_emojis_empty_string() {
        assert_eq!(strip_emojis(""), "");
    }

    #[test]
    fn test_is_emoji_detects_emoticons() {
        assert!(is_emoji('\u{1F60A}')); // smiling face
        assert!(is_emoji('\u{1F602}')); // face with tears of joy
        assert!(is_emoji('\u{2764}')); // heart
    }

    #[test]
    fn test_is_emoji_detects_symbols() {
        assert!(is_emoji('\u{2B50}')); // star
        assert!(is_emoji('\u{2705}')); // check mark
    }

    #[test]
    fn test_is_emoji_rejects_regular_chars() {
        assert!(!is_emoji('A'));
        assert!(!is_emoji('a'));
        assert!(!is_emoji('1'));
        assert!(!is_emoji('!'));
    }
}
