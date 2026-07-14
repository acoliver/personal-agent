//! Unit tests for the per-message visible-document and selection model.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 1

#![cfg(test)]

use super::super::parse_markdown_blocks;
use super::{
    clamp_to_char_boundary, word_range_at, DocumentRange, MessageRevision, Selection,
    SelectionMode, SemanticBlock, VisibleDocument,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn doc_from(markdown: &str) -> VisibleDocument {
    VisibleDocument::from_blocks(&parse_markdown_blocks(markdown))
}

/// Return `true` when `byte_offset` lies on a UTF-8 character boundary in `text`.
fn is_char_boundary(text: &str, byte_offset: usize) -> bool {
    text.is_char_boundary(byte_offset)
}

// ===========================================================================
// MessageRevision freshness
// ===========================================================================

mod revision_tests {
    use super::*;

    #[test]
    fn same_fields_produce_equal_revisions() {
        let a = MessageRevision::new("msg-1", "hello", 0, false);
        let b = MessageRevision::new("msg-1", "hello", 0, false);
        assert_eq!(a, b);
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn different_message_id_rejects() {
        let a = MessageRevision::new("msg-1", "hello", 0, false);
        let b = MessageRevision::new("msg-2", "hello", 0, false);
        assert_ne!(a, b);
    }

    #[test]
    fn different_content_rejects() {
        let a = MessageRevision::new("msg-1", "hello", 0, false);
        let b = MessageRevision::new("msg-1", "world", 0, false);
        assert_ne!(a, b);
    }

    #[test]
    fn different_streaming_revision_rejects() {
        let a = MessageRevision::new("msg-1", "hello", 0, false);
        let b = MessageRevision::new("msg-1", "hello", 1, false);
        assert_ne!(a, b);
    }

    #[test]
    fn different_emoji_filter_rejects() {
        let a = MessageRevision::new("msg-1", "hello", 0, false);
        let b = MessageRevision::new("msg-1", "hello", 0, true);
        assert_ne!(a, b);
    }

    #[test]
    fn is_current_true_for_matching_revision() {
        let rev = MessageRevision::new("msg-1", "hello", 2, true);
        assert!(rev.is_current("msg-1", "hello", 2, true));
    }

    #[test]
    fn is_current_false_for_stale_revision() {
        let rev = MessageRevision::new("msg-1", "hello", 2, true);
        assert!(!rev.is_current("msg-1", "hello", 3, true));
        assert!(!rev.is_current("msg-1", "world", 2, true));
        assert!(!rev.is_current("msg-2", "hello", 2, true));
        assert!(!rev.is_current("msg-1", "hello", 2, false));
    }
}

// ===========================================================================
// VisibleDocument: text and separators
// ===========================================================================

mod document_text_tests {
    use super::*;

    #[test]
    fn paragraph_visible_text_excludes_markdown_syntax() {
        let doc = doc_from("Hello **world** and `code`");
        assert_eq!(doc.text(), "Hello world and code");
    }

    #[test]
    fn heading_visible_text_excludes_hash_prefix() {
        let doc = doc_from("# Title Here");
        assert_eq!(doc.text(), "Title Here");
    }

    #[test]
    fn code_block_visible_text_includes_rendered_language_label() {
        let doc = doc_from("```rust\nfn main() {}\n```");
        assert_eq!(doc.text(), "rust\nfn main() {}\n");
    }

    #[test]
    fn blockquote_visible_text_includes_nested_text() {
        let doc = doc_from("> quoted text");
        assert_eq!(doc.text(), "quoted text");
    }

    #[test]
    fn unordered_list_items_separated_by_newline() {
        let doc = doc_from("- one\n- two\n- three");
        assert_eq!(doc.text(), "• one\n• two\n• three");
    }

    #[test]
    fn ordered_list_items_include_marker_separator_via_newline() {
        let doc = doc_from("1. first\n2. second");
        assert_eq!(doc.text(), "1. first\n2. second");
    }

    #[test]
    fn table_cells_separated_by_tabs_and_rows_by_newlines() {
        let doc = doc_from("| A | B |\n|---|---|\n| 1 | 2 |");
        assert_eq!(doc.text(), "A\tB\n1\t2");
    }

    #[test]
    fn thematic_break_contributes_spacing_but_no_text() {
        let doc = doc_from("before\n\n---\n\nafter");
        assert_eq!(doc.text(), "before\n\nafter");
    }

    #[test]
    fn image_fallback_text_is_bracketed_alt() {
        let doc = doc_from("![alt text](https://example.com/img.png)");
        assert_eq!(doc.text(), "[image: alt text]");
    }

    #[test]
    fn image_without_alt_uses_generic_placeholder() {
        let doc = doc_from("![](https://example.com/img.png)");
        assert_eq!(doc.text(), "[image: ]");
    }

    #[test]
    fn task_marker_renders_checked_or_unchecked_box() {
        let doc = doc_from("- [x] done\n- [ ] todo");
        assert_eq!(doc.text(), "• \u{2611}  done\n• \u{2610}  todo");
    }

    #[test]
    fn mixed_formatting_paragraph_excludes_all_syntax() {
        let doc = doc_from("**bold** _italic_ ~~strike~~ `code` [link](https://x.com)");
        assert_eq!(doc.text(), "bold italic strike code link");
    }

    #[test]
    fn multiple_top_level_blocks_separated_by_newlines() {
        let doc = doc_from("para one\n\npara two");
        assert_eq!(doc.text(), "para one\npara two");
    }
}

// ===========================================================================
// VisibleDocument: links
// ===========================================================================

mod document_link_tests {
    use super::*;

    #[test]
    fn paragraph_link_range_covers_label_text() {
        let doc = doc_from("see [example](https://example.com) here");
        let links = doc.links();
        assert_eq!(links.len(), 1);
        let link = &links[0];
        assert_eq!(link.url, "https://example.com");
        assert_eq!(&doc.text()[link.range.start..link.range.end], "example");
    }

    #[test]
    fn heading_link_range_covers_label_text() {
        let doc = doc_from("# Title with [link](https://h.com)");
        let links = doc.links();
        assert_eq!(links.len(), 1);
        assert_eq!(
            &doc.text()[links[0].range.start..links[0].range.end],
            "link"
        );
    }

    #[test]
    fn autolinked_bare_url_becomes_link_range() {
        let doc = doc_from("visit https://example.com now");
        let links = doc.links();
        assert_eq!(links.len(), 1);
        assert_eq!(
            &doc.text()[links[0].range.start..links[0].range.end],
            "https://example.com"
        );
    }

    #[test]
    fn table_cell_links_are_tracked() {
        let doc = doc_from("| col |\n|---|\n| [cell link](https://c.com) |");
        let links = doc.links();
        assert_eq!(links.len(), 1);
        assert_eq!(
            &doc.text()[links[0].range.start..links[0].range.end],
            "cell link"
        );
    }

    #[test]
    fn no_links_for_plain_text() {
        let doc = doc_from("just plain text");
        assert!(doc.links().is_empty());
    }
}

// ===========================================================================
// VisibleDocument: semantic blocks (triple-click units)
// ===========================================================================

mod semantic_block_tests {
    use super::*;

    #[test]
    fn paragraph_is_single_semantic_block() {
        let doc = doc_from("a single paragraph");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "a single paragraph"
        );
    }

    #[test]
    fn each_paragraph_is_separate_block() {
        let doc = doc_from("first\n\nsecond");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "first"
        );
        assert_eq!(
            &doc.text()[blocks[1].range.start..blocks[1].range.end],
            "second"
        );
    }

    #[test]
    fn heading_is_single_block_without_hash() {
        let doc = doc_from("# Heading");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "Heading"
        );
    }

    #[test]
    fn code_block_is_single_block() {
        let doc = doc_from("```rust\nfn foo() {}\nbar\n```");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "rust\nfn foo() {}\nbar\n"
        );
    }

    #[test]
    fn list_items_are_individual_blocks() {
        let doc = doc_from("- one\n- two");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "• one"
        );
        assert_eq!(
            &doc.text()[blocks[1].range.start..blocks[1].range.end],
            "• two"
        );
    }

    #[test]
    fn table_cells_are_individual_blocks() {
        let doc = doc_from("| A | B |\n|---|---|\n| 1 | 2 |");
        let blocks = doc.semantic_blocks();
        // Header cells + body cells = 4 blocks
        assert_eq!(blocks.len(), 4);
        let texts: Vec<&str> = blocks
            .iter()
            .map(|b| &doc.text()[b.range.start..b.range.end])
            .collect();
        assert_eq!(texts, vec!["A", "B", "1", "2"]);
    }

    #[test]
    fn thematic_break_has_no_text_semantic_block() {
        let doc = doc_from("before\n\n---\n\nafter");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn image_fallback_is_single_block() {
        let doc = doc_from("![pic](https://e.com/i.png)");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "[image: pic]"
        );
    }

    #[test]
    fn blockquote_inner_paragraphs_are_blocks() {
        let doc = doc_from("> quote one\n>\n> quote two");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn task_marker_is_part_of_list_item_block() {
        let doc = doc_from("- [x] done task");
        let blocks = doc.semantic_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            &doc.text()[blocks[0].range.start..blocks[0].range.end],
            "• \u{2611}  done task"
        );
    }
}

// ===========================================================================
// UTF-8 char-boundary safety
// ===========================================================================

mod utf8_boundary_tests {
    use super::*;

    #[test]
    fn ascii_every_byte_offset_is_boundary() {
        let text = "hello world";
        for i in 0..=text.len() {
            assert!(is_char_boundary(text, i));
        }
    }

    #[test]
    fn clamping_ascii_offsets_is_identity() {
        let text = "hello world";
        for i in 0..=text.len() {
            assert_eq!(clamp_to_char_boundary(text, i), i.min(text.len()));
        }
    }

    #[test]
    fn accented_chars_clamp_to_char_boundary_forward() {
        // "café" -> c(0) a(1) f(2) é(3..5)
        let text = "café";
        assert_eq!(text.len(), 5);
        for i in 0..=text.len() {
            let clamped = clamp_to_char_boundary(text, i);
            assert!(
                is_char_boundary(text, clamped),
                "offset {i} clamped to {clamped} which is not a char boundary"
            );
        }
    }

    #[test]
    fn cjk_chars_clamp_to_char_boundary_forward() {
        // Each CJK char is 3 bytes in UTF-8.
        let text = "日本語";
        assert_eq!(text.len(), 9);
        for i in 0..=text.len() {
            let clamped = clamp_to_char_boundary(text, i);
            assert!(
                is_char_boundary(text, clamped),
                "offset {i} clamped to {clamped} which is not a char boundary"
            );
        }
    }

    #[test]
    fn combining_chars_clamp_to_char_boundary_forward() {
        // "n" + combining tilde = 2 code points, 3 bytes total.
        let text = "n\u{0303}";
        assert_eq!(text.len(), 3);
        for i in 0..=text.len() {
            let clamped = clamp_to_char_boundary(text, i);
            assert!(is_char_boundary(text, clamped));
        }
    }

    #[test]
    fn emoji_chars_clamp_to_char_boundary_forward() {
        // 😀 is U+1F600, 4 bytes in UTF-8.
        let text = "a😀b";
        assert_eq!(text.len(), 6);
        for i in 0..=text.len() {
            let clamped = clamp_to_char_boundary(text, i);
            assert!(
                is_char_boundary(text, clamped),
                "offset {i} clamped to {clamped} which is not a char boundary"
            );
        }
    }

    #[test]
    fn clamping_past_end_clamps_to_end() {
        let text = "café";
        assert_eq!(clamp_to_char_boundary(text, 100), text.len());
    }

    #[test]
    fn clamping_mid_multibyte_rounds_forward() {
        // "café": é occupies bytes 3..5. Offset 4 is mid-char.
        let text = "café";
        assert_eq!(clamp_to_char_boundary(text, 4), 5);
    }

    #[test]
    fn clamping_mid_cjk_rounds_forward() {
        let text = "日本語";
        // Second char starts at byte 3, ends at 6. Byte 4 is mid-char.
        assert_eq!(clamp_to_char_boundary(text, 4), 6);
    }

    #[test]
    fn clamping_mid_emoji_rounds_forward() {
        let text = "a😀b";
        // 😀 occupies bytes 1..5. Byte 3 is mid-char.
        assert_eq!(clamp_to_char_boundary(text, 3), 5);
    }
}

// ===========================================================================
// Unicode-safe word boundaries
// ===========================================================================

mod word_boundary_tests {
    use super::*;

    #[test]
    fn ascii_word_in_middle() {
        let text = "hello world";
        let range = word_range_at(text, 2); // 'l' in "hello"
        assert_eq!(&text[range], "hello");
    }

    #[test]
    fn ascii_word_at_boundaries() {
        let text = "hello world";
        let range = word_range_at(text, 7); // 'o' in "world"
        assert_eq!(&text[range], "world");
    }

    #[test]
    fn unicode_word_with_accents() {
        let text = "café résumé";
        let range = word_range_at(text, 1); // 'a' in "café"
        assert_eq!(&text[range], "café");
        let range2 = word_range_at(text, 6); // 'é' in "résumé"
        assert_eq!(&text[range2], "résumé");
    }

    #[test]
    fn unicode_word_cjk_treated_as_word() {
        // CJK characters are alphanumeric under char::is_alphanumeric.
        let text = "hello 日本語 world";
        let range = word_range_at(text, 7); // inside "日"
        assert_eq!(&text[range], "日本語");
    }

    #[test]
    fn word_with_internal_apostrophe() {
        let text = "it's a test";
        let range = word_range_at(text, 1); // 't' in "it's"
        assert_eq!(&text[range], "it's");
    }

    #[test]
    fn word_at_whitespace_returns_empty() {
        let text = "hello   world";
        let range = word_range_at(text, 6); // middle space
        assert!(range.is_empty(), "expected empty range at whitespace");
    }

    #[test]
    fn word_at_empty_string_is_empty() {
        let range = word_range_at("", 0);
        assert!(range.is_empty());
    }

    #[test]
    fn word_at_punctuation_is_empty() {
        let text = "hello --- world";
        let range = word_range_at(text, 7); // '-'
        assert!(range.is_empty());
    }

    #[test]
    fn word_with_emoji() {
        let text = "a 😀 b";
        // 😀 is alphanumeric
        let range = word_range_at(text, 2); // inside emoji
        assert_eq!(&text[range], "😀");
    }

    #[test]
    fn word_range_offsets_are_char_boundaries() {
        let text = "café résumé";
        for i in 0..=text.len() {
            let range = word_range_at(text, i);
            assert!(is_char_boundary(text, range.start));
            assert!(is_char_boundary(text, range.end));
        }
    }
}

// ===========================================================================
// Selection: forward / reverse / empty
// ===========================================================================

mod selection_range_tests {
    use super::*;

    #[test]
    fn forward_selection_ordered_range() {
        let sel = Selection::new(2, 8);
        assert_eq!(sel.anchor(), 2);
        assert_eq!(sel.head(), 8);
        let ordered = sel.ordered_range();
        assert_eq!(ordered, 2..8);
    }

    #[test]
    fn reverse_selection_ordered_range() {
        let sel = Selection::new(8, 2);
        assert_eq!(sel.anchor(), 8);
        assert_eq!(sel.head(), 2);
        let ordered = sel.ordered_range();
        assert_eq!(ordered, 2..8);
    }

    #[test]
    fn empty_selection_is_caret() {
        let sel = Selection::new(5, 5);
        assert!(sel.is_empty());
        assert_eq!(sel.ordered_range(), 5..5);
    }

    #[test]
    fn forward_is_not_reverse() {
        let fwd = Selection::new(1, 3);
        let rev = Selection::new(3, 1);
        assert!(!fwd.is_reverse());
        assert!(rev.is_reverse());
    }

    #[test]
    fn contains_for_inclusive_start_exclusive_end() {
        let sel = Selection::new(2, 5);
        assert!(sel.contains(2));
        assert!(sel.contains(4));
        assert!(!sel.contains(5));
    }
}

// ===========================================================================
// Selection: clamping to document
// ===========================================================================

mod selection_clamp_tests {
    use super::*;

    #[test]
    fn clamping_keeps_valid_offsets() {
        let doc = doc_from("hello world");
        let sel = Selection::new(0, 5);
        let clamped = sel.clamped(doc.text());
        assert_eq!(clamped.ordered_range(), 0..5);
    }

    #[test]
    fn clamping_caps_oversized_offset() {
        let doc = doc_from("hi");
        let sel = Selection::new(0, 100);
        let clamped = sel.clamped(doc.text());
        assert_eq!(clamped.ordered_range(), 0..2);
    }

    #[test]
    fn clamping_is_char_boundary_safe_for_unicode() {
        let doc = doc_from("café");
        // Try to select offset 4 (mid-é).
        let sel = Selection::new(0, 4);
        let clamped = sel.clamped(doc.text());
        assert!(is_char_boundary(doc.text(), clamped.anchor()));
        assert!(is_char_boundary(doc.text(), clamped.head()));
    }
}

// ===========================================================================
// Selection: selected text extraction
// ===========================================================================

mod selected_text_tests {
    use super::*;

    #[test]
    fn selected_text_forward() {
        let doc = doc_from("hello world");
        let sel = Selection::new(0, 5);
        assert_eq!(doc.selected_text(&sel), "hello");
    }

    #[test]
    fn selected_text_reverse() {
        let doc = doc_from("hello world");
        let sel = Selection::new(5, 0);
        assert_eq!(doc.selected_text(&sel), "hello");
    }

    #[test]
    fn selected_text_empty_is_empty_string() {
        let doc = doc_from("hello world");
        let sel = Selection::new(3, 3);
        assert_eq!(doc.selected_text(&sel), "");
    }

    #[test]
    fn selected_text_unicode() {
        let doc = doc_from("café résumé");
        let text = doc.text();
        let start = text.find("café").unwrap();
        let sel = Selection::new(start, start + "café".len());
        assert_eq!(doc.selected_text(&sel), "café");
    }

    #[test]
    fn selected_text_excludes_markdown_syntax() {
        let doc = doc_from("**bold** text");
        let text = doc.text();
        assert_eq!(text, "bold text");
        let start = text.find("bold").unwrap();
        let sel = Selection::new(start, start + "bold".len());
        assert_eq!(doc.selected_text(&sel), "bold");
    }
}

// ===========================================================================
// Selection modes: word and block
// ===========================================================================

mod selection_mode_tests {
    use super::*;

    #[test]
    fn word_mode_extends_to_word_boundaries() {
        let doc = doc_from("hello world");
        let sel = Selection::char(2).to_word(doc.text());
        assert_eq!(sel.mode(), SelectionMode::Word);
        assert_eq!(doc.selected_text(&sel), "hello");
    }

    #[test]
    fn block_mode_extends_to_semantic_block() {
        let doc = doc_from("first paragraph\n\nsecond paragraph");
        let sel = Selection::to_block(doc.text(), &doc.semantic_blocks()[0]);
        assert_eq!(sel.mode(), SelectionMode::Block);
        assert_eq!(doc.selected_text(&sel), "first paragraph");
    }

    #[test]
    fn word_mode_unicode_safe() {
        let doc = doc_from("café résumé");
        let text = doc.text();
        let pos = text.find("café").unwrap();
        let sel = Selection::char(pos + 1).to_word(text);
        assert_eq!(doc.selected_text(&sel), "café");
    }

    #[test]
    fn char_mode_default() {
        let sel = Selection::char(5);
        assert_eq!(sel.mode(), SelectionMode::Char);
    }
}

// ===========================================================================
// DocumentRange and SemanticBlock invariants
// ===========================================================================

mod range_invariant_tests {
    use super::*;

    #[test]
    fn document_ranges_are_char_aligned() {
        let doc = doc_from("café résumé");
        for link in doc.links() {
            assert!(is_char_boundary(doc.text(), link.range.start));
            assert!(is_char_boundary(doc.text(), link.range.end));
        }
        for block in doc.semantic_blocks() {
            assert!(is_char_boundary(doc.text(), block.range.start));
            assert!(is_char_boundary(doc.text(), block.range.end));
        }
    }

    #[test]
    fn semantic_blocks_are_non_overlapping_and_ordered() {
        let doc = doc_from("# H\n\npara\n\n- a\n- b\n\n```rs\ncode\n```");
        let blocks = doc.semantic_blocks();
        for window in blocks.windows(2) {
            assert!(
                window[0].range.end <= window[1].range.start,
                "blocks overlap: {:?} vs {:?}",
                window[0].range,
                window[1].range
            );
        }
    }

    #[test]
    fn link_ranges_within_document_bounds() {
        let doc = doc_from("[a](https://a.com) [b](https://b.com)");
        let len = doc.text().len();
        for link in doc.links() {
            assert!(link.range.start <= link.range.end);
            assert!(link.range.end <= len);
        }
    }

    #[test]
    fn empty_document_has_no_links_or_blocks() {
        let doc = doc_from("");
        assert_eq!(doc.text(), "");
        assert!(doc.links().is_empty());
        assert!(doc.semantic_blocks().is_empty());
    }
}

// ===========================================================================
// DocumentRange, SemanticBlock, SelectionMode trait impls
// ===========================================================================

mod type_tests {
    use super::*;

    #[test]
    fn document_range_clone_eq_debug() {
        let r1 = DocumentRange {
            range: 0..5,
            url: "https://x.com".to_string(),
        };
        let r2 = r1.clone();
        assert_eq!(r1, r2);
        let _s = format!("{r1:?}");
    }

    #[test]
    fn semantic_block_clone_eq_debug() {
        let b1 = SemanticBlock { range: 1..3 };
        let b2 = b1.clone();
        assert_eq!(b1, b2);
        let _s = format!("{b1:?}");
    }

    #[test]
    fn selection_mode_clone_eq() {
        assert_eq!(SelectionMode::Char, SelectionMode::Char.clone());
        assert_ne!(SelectionMode::Char, SelectionMode::Word);
    }

    #[test]
    fn selection_clone_eq_debug() {
        let s1 = Selection::new(1, 4);
        let s2 = s1.clone();
        assert_eq!(s1, s2);
        let _s = format!("{s1:?}");
    }
}
