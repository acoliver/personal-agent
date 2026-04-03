mod tests {
    use super::super::*;

    // Helper function to extract text from list item
    fn extract_item_text(item_blocks: &[MarkdownBlock]) -> String {
        let mut text = String::new();
        for block in item_blocks {
            if let MarkdownBlock::Paragraph { spans, .. } = block {
                for span in spans {
                    text.push_str(&span.text);
                }
            }
        }
        text
    }

    // ============================================================================
    // BLOCK-LEVEL PARSE TESTS
    // ============================================================================

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-002
    #[test]
    fn test_parse_single_paragraph() {
        let input = "Hello world";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                assert_eq!(spans.len(), 1);
                assert_eq!(spans[0].text, "Hello world");
                assert!(!spans[0].bold);
                assert!(!spans[0].italic);
                assert!(links.is_empty());
            }
            _ => panic!("Expected Paragraph block"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-002
    #[test]
    fn test_parse_multiple_paragraphs() {
        let input = "First paragraph\n\nSecond paragraph";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 2);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-003
    #[test]
    fn test_parse_heading_levels() {
        let input = "# H1\n## H2\n### H3";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0], MarkdownBlock::Heading { level: 1, .. }));
        assert!(matches!(blocks[1], MarkdownBlock::Heading { level: 2, .. }));
        assert!(matches!(blocks[2], MarkdownBlock::Heading { level: 3, .. }));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-004
    #[test]
    fn test_parse_fenced_code_block_with_language() {
        let input = "```rust\nfn main() {}\n```";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::CodeBlock { language, code } => {
                assert_eq!(language, &Some("rust".to_string()));
                assert_eq!(code, "fn main() {}\n");
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-005
    #[test]
    fn test_parse_indented_code_block() {
        let input = "    indented code";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::CodeBlock { language, code } => {
                assert_eq!(language, &None);
                assert!(code.contains("indented code"));
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-006
    #[test]
    fn test_parse_blockquote() {
        let input = "> quoted text";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::BlockQuote { blocks: children } => {
                assert!(!children.is_empty());
            }
            _ => panic!("Expected BlockQuote"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-007
    #[test]
    fn test_parse_unordered_list() {
        let input = "- item 1\n- item 2";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::List {
                ordered,
                start,
                items,
            } => {
                assert!(!ordered);
                assert_eq!(*start, 0);
                assert_eq!(items.len(), 2);
                assert_eq!(extract_item_text(&items[0]), "item 1");
                assert_eq!(extract_item_text(&items[1]), "item 2");
            }
            _ => panic!("Expected List"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-008
    #[test]
    fn test_parse_ordered_list() {
        let input = "3. item a\n4. item b";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::List {
                ordered,
                start,
                items,
            } => {
                assert!(*ordered);
                assert_eq!(*start, 3);
                assert_eq!(items.len(), 2);
                assert_eq!(extract_item_text(&items[0]), "item a");
                assert_eq!(extract_item_text(&items[1]), "item b");
            }
            _ => panic!("Expected List"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-009
    #[test]
    fn test_parse_table() {
        let input = "| A | B |\n|:---|---:|\n| 1 | 2 |";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Table {
                alignments,
                header,
                rows,
            } => {
                assert_eq!(alignments.len(), 2);
                assert_eq!(alignments[0], Alignment::Left);
                assert_eq!(alignments[1], Alignment::Right);
                assert_eq!(header.len(), 2);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].len(), 2);
            }
            _ => panic!("Expected Table"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-010
    #[test]
    fn test_parse_thematic_break() {
        let input = "---";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0], MarkdownBlock::ThematicBreak));
    }

    // ============================================================================
    // INLINE STYLE TESTS
    // ============================================================================

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-020
    #[test]
    fn test_parse_bold_text() {
        let input = "**bold**";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(!spans.is_empty());
                assert!(spans.iter().any(|s| s.bold));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-021
    #[test]
    fn test_parse_italic_text() {
        let input = "*italic*";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(!spans.is_empty());
                assert!(spans.iter().any(|s| s.italic));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-022
    #[test]
    fn test_parse_bold_italic_text() {
        let input = "***bolditalic***";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(!spans.is_empty());
                assert!(spans.iter().any(|s| s.bold && s.italic));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-023
    #[test]
    fn test_parse_strikethrough_text() {
        let input = "~~strike~~";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(!spans.is_empty());
                assert!(spans.iter().any(|s| s.strikethrough));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-024
    #[test]
    fn test_parse_inline_code() {
        let input = "`code`";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(!spans.is_empty());
                assert!(spans.iter().any(|s| s.code));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-024
    #[test]
    fn test_parse_inline_code_inside_bold_inherits_bold_style() {
        let input = "**prefix `code` suffix**";
        let blocks = parse_markdown_blocks(input);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let code_span = spans
                    .iter()
                    .find(|span| span.code)
                    .expect("Expected code span");
                assert_eq!(code_span.text, "code");
                assert!(code_span.bold);
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-024
    #[test]
    fn test_parse_inline_code_inside_link_inherits_link_url() {
        let input = "[prefix `code` suffix](https://example.com)";
        let blocks = parse_markdown_blocks(input);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                let code_span = spans
                    .iter()
                    .find(|span| span.code)
                    .expect("Expected code span");
                assert_eq!(code_span.text, "code");
                assert_eq!(code_span.link_url.as_deref(), Some("https://example.com"));
                assert_eq!(links.len(), 1);
                assert_eq!(links[0].1, "https://example.com");
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-025
    #[test]
    fn test_parse_link() {
        let input = "[link](https://example.com)";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                assert!(!spans.is_empty());
                assert!(!links.is_empty());
                assert_eq!(links[0].1, "https://example.com");
                assert!(spans.iter().any(|s| s.link_url.is_some()));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-026
    #[test]
    fn test_parse_task_list_marker() {
        let input = "- [x] done\n- [ ] todo";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::List { items, .. } => {
                assert_eq!(items.len(), 2);
                let item1_text = extract_item_text(&items[0]);
                let item2_text = extract_item_text(&items[1]);
                assert!(item1_text.starts_with('☑'), "item1={item1_text}");
                assert!(item2_text.starts_with('☐'), "item2={item2_text}");
            }
            _ => panic!("Expected List"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-027
    #[test]
    fn test_parse_nested_inline_styles() {
        let input = "**bold *italic* inside**";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(!spans.is_empty());
                // At least one span should have bold
                assert!(spans.iter().any(|s| s.bold));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-028
    #[test]
    fn test_parse_soft_break() {
        let input = "line1\nline2";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert!(text.contains("line1 line2") || text.contains("line1\nline2"));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-029
    #[test]
    fn test_parse_hard_break() {
        let input = "line1\\\nline2";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert!(text.contains('\n'));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    // ============================================================================
    // FALLBACK / SECURITY TESTS
    // ============================================================================

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-040
    #[test]
    fn test_parse_image_fallback() {
        let input = "![alt text](image.png)";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::ImageFallback { alt } => {
                assert_eq!(alt, "alt text");
            }
            _ => panic!("Expected ImageFallback"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-040
    #[test]
    fn test_parse_image_inside_heading_stays_in_heading_content() {
        let input = "# See ![alt text](image.png) now";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            MarkdownBlock::Heading { level, spans, .. } => {
                assert_eq!(*level, 1);
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert!(text.contains("See "));
                assert!(text.contains("[image: alt text]"));
                assert!(text.contains(" now"));
            }
            _ => panic!("Expected Heading"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-040
    #[test]
    fn test_parse_image_inside_table_cell_stays_in_cell_content() {
        let input = "| A |\n|---|\n| before ![alt text](image.png) after |";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            MarkdownBlock::Table { rows, .. } => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].len(), 1);
                let cell_text: String = rows[0][0].spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(cell_text, "before [image: alt text] after");
            }
            _ => panic!("Expected Table"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-043
    #[test]
    fn test_parse_html_in_blockquote_stays_nested() {
        let input = "> <div>nested html</div>";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            MarkdownBlock::BlockQuote { blocks: children } => {
                assert_eq!(children.len(), 1);
                match &children[0] {
                    MarkdownBlock::Paragraph { spans, .. } => {
                        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                        assert_eq!(text, "nested html");
                    }
                    _ => panic!("Expected nested Paragraph"),
                }
            }
            _ => panic!("Expected BlockQuote"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-041
    #[test]
    fn test_parse_footnote_definition() {
        let input = "[^1]: footnote text";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-041
    #[test]
    fn test_parse_footnote_definition_not_duplicated() {
        let input = "[^1]: footnote text";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(text, "[^1]: footnote text");
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-042
    #[test]
    fn test_parse_footnote_reference() {
        let input = "text[^1]";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-043
    #[test]
    fn test_parse_html_block_strip() {
        let input = "<div>content</div>";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
        // Should strip HTML tags
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert!(text.contains("content"));
                assert!(!text.contains("<div>"));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-044
    #[test]
    fn test_parse_inline_html_strip() {
        let input = "text <span>inline</span> text";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert!(text.contains("inline"));
                assert!(!text.contains("<span>"));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-045
    #[test]
    fn test_parse_script_style_strip() {
        let input = "<script>alert('xss')</script>safe";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert!(!text.contains("alert"));
                assert!(text.contains("safe"));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-046
    #[test]
    fn test_parse_inline_math_as_code() {
        let input = "$x^2$";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(spans.iter().any(|s| s.code));
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(text, "x^2");
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-047
    #[test]
    fn test_parse_display_math_as_code_block() {
        let input = "$$x^2$$";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::CodeBlock { language, code } => {
                assert_eq!(language.as_deref(), Some("math"));
                assert_eq!(code, "x^2");
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-048
    #[test]
    fn test_parse_superscript_subscript_plaintext() {
        let input = "x^2~n";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-049
    #[test]
    fn test_parse_metadata_block_skip() {
        let input = "---\ntitle: test\n---\ncontent";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-050
    #[test]
    fn test_parse_malformed_html_no_panic() {
        let input = "<div unclosed";
        let blocks = parse_markdown_blocks(input);
        // Should not panic and should preserve malformed tag text.
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(text, "<div unclosed");
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-051
    #[test]
    fn test_parse_definition_list_fallback() {
        let input = "Term\n: Definition";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
    }

    // ============================================================================
    // URL SAFETY TESTS
    // ============================================================================

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-001
    #[test]
    fn test_is_safe_url_accepts_http() {
        assert!(is_safe_url("http://example.com"));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-001
    #[test]
    fn test_is_safe_url_accepts_https() {
        assert!(is_safe_url("https://example.com"));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-002
    #[test]
    fn test_is_safe_url_rejects_javascript() {
        assert!(!is_safe_url("javascript:alert(1)"));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-002
    #[test]
    fn test_is_safe_url_rejects_file() {
        assert!(!is_safe_url("file:///etc/passwd"));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-003
    #[test]
    fn test_is_safe_url_rejects_malformed() {
        assert!(!is_safe_url("not a url"));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-006
    #[test]
    fn test_is_safe_url_rejects_relative() {
        assert!(!is_safe_url("/relative/path"));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-001
    #[test]
    fn test_is_safe_url_rejects_empty() {
        assert!(!is_safe_url(""));
        assert!(!is_safe_url("   "));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-SEC-001
    #[test]
    fn test_is_safe_url_trimmed() {
        assert!(is_safe_url("  https://example.com  "));
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-025
    #[test]
    fn test_link_range_offsets_are_byte_based() {
        let input = "é before [link](https://example.com) after";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                assert_eq!(links.len(), 1);
                let (range, url) = &links[0];
                assert_eq!(url, "https://example.com");
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(text.get(range.clone()), Some("link"));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-001
    #[test]
    fn test_parse_empty_input_returns_empty_blocks() {
        let blocks = parse_markdown_blocks("");
        assert!(blocks.is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-001
    #[test]
    fn test_parse_whitespace_input_returns_empty_or_paragraph() {
        let blocks = parse_markdown_blocks("   \n\n  ");
        // Accept either empty or a whitespace paragraph, but must not panic
        assert!(blocks.len() <= 1);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P04
    /// @requirement:REQ-MD-PARSE-065
    #[test]
    fn test_parser_no_panic_smoke() {
        let input = "normal text";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P06
    /// @requirement:REQ-MD-RENDER-041
    #[test]
    fn test_render_markdown_empty_returns_empty() {
        assert!(render_markdown("").is_empty());
    }

    /// @plan:PLAN-20260402-MARKDOWN.P08
    /// @requirement:REQ-MD-RENDER-001
    #[test]
    fn test_render_markdown_for_heading_paragraph_and_list() {
        let rendered = render_markdown("### Title\n\nParagraph\n\n1. one\n2. two");
        assert_eq!(rendered.len(), 3);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P08
    /// @requirement:REQ-MD-RENDER-005
    #[test]
    fn test_render_markdown_for_code_block() {
        let rendered = render_markdown("```rust\nfn main() {}\n```");
        assert_eq!(rendered.len(), 1);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P08
    /// @requirement:REQ-MD-RENDER-007
    #[test]
    fn test_render_markdown_for_blockquote() {
        let rendered = render_markdown("> quote");
        assert_eq!(rendered.len(), 1);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P08
    /// @requirement:REQ-MD-RENDER-009
    #[test]
    fn test_render_markdown_for_table() {
        let rendered = render_markdown("| A | B |\n|---|---|\n| 1 | 2 |");
        assert_eq!(rendered.len(), 1);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P08
    /// @requirement:REQ-MD-RENDER-010
    #[test]
    fn test_render_markdown_for_thematic_break() {
        let rendered = render_markdown("---");
        assert_eq!(rendered.len(), 1);
    }

    /// @plan:PLAN-20260402-MARKDOWN.P08
    /// @requirement:REQ-MD-RENDER-011
    #[test]
    fn test_render_markdown_for_image_fallback() {
        let rendered = render_markdown("![diagram](image.png)");
        assert_eq!(rendered.len(), 1);
    }
}
