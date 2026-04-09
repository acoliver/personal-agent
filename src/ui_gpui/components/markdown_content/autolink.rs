//! URL autolink detection for markdown rendering.
//!
//! Implements bare URL detection as a post-processing step on the markdown IR.
//! This allows URLs like `https://example.com` to be rendered as clickable links
//! without requiring explicit markdown link syntax `[text](url)`.
//!
//! @plan PLAN-20260402-ISSUE153.P01
//! @requirement REQ-MD-AUTOLINK-001

use regex::Regex;
use std::ops::Range;

use super::{MarkdownBlock, MarkdownInline};

/// Detect bare URLs in text and return their byte ranges with normalized URLs.
///
/// Matches:
/// - `https?://` URLs
/// - `www.` URLs (normalized to `https://`)
///
/// Strips trailing punctuation (`.`, `,`, `)`, `]`, `!`, `?`) from URLs.
///
/// @plan PLAN-20260402-ISSUE153.P01
/// @requirement REQ-MD-AUTOLINK-002
pub(crate) fn detect_bare_urls(text: &str) -> Vec<(Range<usize>, String)> {
    static HTTP_PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static WWW_PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

    let http_re = HTTP_PATTERN
        .get_or_init(|| Regex::new(r"https?://[^\s<>\[\]()]+").expect("invalid http URL regex"));

    let www_re = WWW_PATTERN
        .get_or_init(|| Regex::new(r"www\.[^\s<>\[\]()]+").expect("invalid www URL regex"));

    let mut results: Vec<(Range<usize>, String)> = Vec::new();

    // Collect http/https URLs
    for m in http_re.find_iter(text) {
        let url = strip_trailing_punctuation(m.as_str());
        results.push((m.start()..m.start() + url.len(), url.to_string()));
    }

    // Collect www URLs and normalize with https://
    for m in www_re.find_iter(text) {
        let url = strip_trailing_punctuation(m.as_str());
        let normalized = format!("https://{url}");
        results.push((m.start()..m.start() + url.len(), normalized));
    }

    // Sort by start position and remove overlaps (http takes precedence)
    results.sort_by_key(|(range, _)| range.start);

    // Remove overlapping matches (prefer earlier matches)
    let mut deduped: Vec<(Range<usize>, String)> = Vec::with_capacity(results.len());
    for item in results {
        if !deduped
            .iter()
            .any(|(r, _)| r.start < item.0.end && r.end > item.0.start)
        {
            deduped.push(item);
        }
    }

    deduped
}

/// Strip trailing punctuation characters from a URL.
///
/// Handles: `.`, `,`, `)`, `]`, `!`, `?`
///
/// @plan PLAN-20260402-ISSUE153.P01
/// @requirement REQ-MD-AUTOLINK-003
fn strip_trailing_punctuation(url: &str) -> &str {
    url.trim_end_matches(['.', ',', ')', ']', '!', '?'])
}

/// Apply autolink detection to markdown blocks.
///
/// Scans text spans for bare URLs and splits them into link spans.
/// This modifies blocks in place.
///
/// @plan PLAN-20260402-ISSUE153.P01
/// @requirement REQ-MD-AUTOLINK-004
pub(crate) fn apply_autolinks(blocks: &mut [MarkdownBlock]) {
    for block in blocks.iter_mut() {
        apply_autolinks_to_block(block);
    }
}

fn apply_autolinks_to_block(block: &mut MarkdownBlock) {
    match block {
        MarkdownBlock::Paragraph { spans, links } | MarkdownBlock::Heading { spans, links, .. } => {
            let new_links = apply_autolinks_to_spans(spans);
            links.extend(new_links);
        }
        MarkdownBlock::Table { header, rows, .. } => {
            for cell in header.iter_mut() {
                let new_links = apply_autolinks_to_spans(&mut cell.spans);
                cell.links.extend(new_links);
            }
            for row in rows.iter_mut() {
                for cell in row.iter_mut() {
                    let new_links = apply_autolinks_to_spans(&mut cell.spans);
                    cell.links.extend(new_links);
                }
            }
        }
        MarkdownBlock::BlockQuote { blocks } => {
            apply_autolinks(blocks);
        }
        MarkdownBlock::List { items, .. } => {
            for item_blocks in items.iter_mut() {
                apply_autolinks(item_blocks);
            }
        }
        // CodeBlock, ThematicBreak, ImageFallback: no autolinks
        _ => {}
    }
}

/// Apply autolinks to a span vector, splitting spans at URL boundaries.
///
/// Returns new link ranges (byte offsets in the combined text).
///
/// @plan PLAN-20260402-ISSUE153.P01
/// @requirement REQ-MD-AUTOLINK-005
fn apply_autolinks_to_spans(spans: &mut Vec<MarkdownInline>) -> Vec<(Range<usize>, String)> {
    let all_urls = collect_url_positions(spans);

    if all_urls.is_empty() {
        return Vec::new();
    }

    let (new_spans, new_links) = rebuild_spans_with_urls(spans);
    *spans = new_spans;
    new_links
}

/// Collect URL positions from all eligible spans.
///
/// @plan PLAN-20260402-ISSUE153.P01
fn collect_url_positions(spans: &[MarkdownInline]) -> Vec<(Range<usize>, String)> {
    let mut all_urls: Vec<(Range<usize>, String)> = Vec::new();
    let mut byte_offset = 0;

    for span in spans {
        // Skip code spans and already-linked spans
        if span.code || span.link_url.is_some() {
            byte_offset += span.text.len();
            continue;
        }

        let detected = detect_bare_urls(&span.text);
        for (local_range, url) in detected {
            let global_range = (byte_offset + local_range.start)..(byte_offset + local_range.end);
            all_urls.push((global_range, url));
        }
        byte_offset += span.text.len();
    }

    all_urls
}

/// Rebuild spans with URL splits.
///
/// @plan PLAN-20260402-ISSUE153.P01
fn rebuild_spans_with_urls(
    spans: &[MarkdownInline],
) -> (Vec<MarkdownInline>, Vec<(Range<usize>, String)>) {
    let mut new_spans: Vec<MarkdownInline> = Vec::new();
    let mut new_links: Vec<(Range<usize>, String)> = Vec::new();
    let mut byte_offset = 0;

    for span in spans {
        // Skip code spans - preserve as-is
        if span.code {
            new_spans.push(span.clone());
            byte_offset += span.text.len();
            continue;
        }

        // Skip spans that are already links - preserve as-is
        if span.link_url.is_some() {
            new_spans.push(span.clone());
            byte_offset += span.text.len();
            continue;
        }

        let detected = detect_bare_urls(&span.text);

        if detected.is_empty() {
            new_spans.push(span.clone());
        } else {
            split_span_at_urls(span, &detected, &mut new_spans, &mut new_links, byte_offset);
        }
        byte_offset += span.text.len();
    }

    (new_spans, new_links)
}

/// Split a span at URL boundaries.
///
/// @plan PLAN-20260402-ISSUE153.P01
fn split_span_at_urls(
    span: &MarkdownInline,
    detected: &[(Range<usize>, String)],
    new_spans: &mut Vec<MarkdownInline>,
    new_links: &mut Vec<(Range<usize>, String)>,
    byte_offset: usize,
) {
    let mut last_end = 0;

    for (local_range, url) in detected {
        // Text before URL
        if local_range.start > last_end {
            let before = &span.text[last_end..local_range.start];
            if !before.is_empty() {
                let mut new_span = span.clone();
                new_span.text = before.to_string();
                new_spans.push(new_span);
            }
        }

        // URL as a link span
        let url_text = &span.text[local_range.clone()];
        let mut url_span = span.clone();
        url_span.text = url_text.to_string();
        url_span.link_url = Some(url.clone());
        new_spans.push(url_span);

        // Record the link range in the new combined text
        let link_start = byte_offset + last_end;
        let link_end = link_start + url_text.len();
        new_links.push((link_start..link_end, url.clone()));

        last_end = local_range.end;
    }

    // Text after last URL
    if last_end < span.text.len() {
        let after = &span.text[last_end..];
        if !after.is_empty() {
            let mut new_span = span.clone();
            new_span.text = after.to_string();
            new_spans.push(new_span);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // URL DETECTION TESTS
    // ============================================================================

    #[test]
    fn test_detect_https_url() {
        let text = "Visit https://example.com for more";
        let urls = detect_bare_urls(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, 6..25);
        assert_eq!(urls[0].1, "https://example.com");
    }

    #[test]
    fn test_detect_http_url() {
        let text = "Check http://example.com out";
        let urls = detect_bare_urls(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].1, "http://example.com");
    }

    #[test]
    fn test_detect_www_url() {
        let text = "See www.example.com for details";
        let urls = detect_bare_urls(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].1, "https://www.example.com");
    }

    #[test]
    fn test_detect_multiple_urls() {
        let text = "Visit https://example.com and www.test.org";
        let urls = detect_bare_urls(text);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].1, "https://example.com");
        assert_eq!(urls[1].1, "https://www.test.org");
    }

    #[test]
    fn test_strip_trailing_dot() {
        assert_eq!(
            strip_trailing_punctuation("https://example.com."),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_trailing_comma() {
        assert_eq!(
            strip_trailing_punctuation("https://example.com,"),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_trailing_paren() {
        assert_eq!(
            strip_trailing_punctuation("https://example.com)"),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_trailing_bracket() {
        assert_eq!(
            strip_trailing_punctuation("https://example.com]"),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_trailing_exclamation() {
        assert_eq!(
            strip_trailing_punctuation("https://example.com!"),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_trailing_question() {
        assert_eq!(
            strip_trailing_punctuation("https://example.com?"),
            "https://example.com"
        );
    }

    #[test]
    fn test_strip_multiple_trailing_punctuation() {
        // trim_end_matches strips ALL trailing matches
        assert_eq!(
            strip_trailing_punctuation("https://example.com!."),
            "https://example.com"
        );
    }

    #[test]
    fn test_url_in_sentence_with_punctuation() {
        let text = "Check out https://example.com, it's great!";
        let urls = detect_bare_urls(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].1, "https://example.com");
    }

    #[test]
    fn test_url_at_end_of_sentence() {
        let text = "Visit https://example.com.";
        let urls = detect_bare_urls(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, 6..25);
        assert_eq!(urls[0].1, "https://example.com");
    }

    // ============================================================================
    // AUTOLINK INTEGRATION TESTS
    // ============================================================================

    #[test]
    fn test_apply_autolinks_to_paragraph() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "Check https://example.com for info";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                assert!(!spans.is_empty());
                assert!(!links.is_empty(), "Should have detected URL link");
                assert!(spans.iter().any(|s| s.link_url.is_some()));
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_apply_autolinks_preserves_code_spans() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "URL in code: `https://example.com` should not link";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                // Code span should still have code=true and no link_url
                let code_span = spans.iter().find(|s| s.code);
                assert!(code_span.is_some());
                let code_span = code_span.unwrap();
                assert!(code_span.code);
                assert!(
                    code_span.link_url.is_none(),
                    "Code spans should NOT be autolinked"
                );

                // No links should be generated
                assert!(links.is_empty(), "Code URL should not generate link");
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_apply_autolinks_preserves_explicit_links() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "[click](https://example.com) and also https://test.org";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                // Should have both explicit link and autolink
                assert_eq!(links.len(), 2, "Should have explicit link and autolink");

                // Check that we have spans with link_url set
                assert_eq!(
                    spans.iter().filter(|s| s.link_url.is_some()).count(),
                    2,
                    "Should have two link spans"
                );
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_apply_autolinks_heading() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "# Visit https://example.com";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Heading { spans, links, .. } => {
                assert!(!links.is_empty(), "Heading should have autolink");
                assert!(spans.iter().any(|s| s.link_url.is_some()));
            }
            _ => panic!("Expected Heading"),
        }
    }

    #[test]
    fn test_apply_autolinks_table_cell() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "| URL |\n|---|\n| https://example.com |";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Table { rows, .. } => {
                assert_eq!(rows.len(), 1);
                assert!(
                    !rows[0][0].links.is_empty(),
                    "Table cell should have autolink"
                );
            }
            _ => panic!("Expected Table"),
        }
    }

    #[test]
    fn test_apply_autolinks_code_block_untouched() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "```\nhttps://example.com\n```";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::CodeBlock { code, .. } => {
                assert!(code.contains("https://example.com"));
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    #[test]
    fn test_apply_autolinks_www_normalized() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "Visit www.example.com for more";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                assert!(!links.is_empty());
                let url_span = spans.iter().find(|s| s.link_url.is_some());
                assert!(url_span.is_some());
                let url = url_span.unwrap().link_url.as_ref().unwrap();
                assert!(
                    url.starts_with("https://www."),
                    "www URL should be normalized to https"
                );
            }
            _ => panic!("Expected Paragraph"),
        }
    }

    #[test]
    fn test_apply_autolinks_splits_spans_correctly() {
        use super::super::{parse_markdown_blocks, MarkdownBlock};

        let input = "Before https://example.com After";
        let mut blocks = parse_markdown_blocks(input);
        apply_autolinks(&mut blocks);

        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, links } => {
                // Should have 3 spans: "Before ", URL, " After"
                assert!(spans.len() >= 3, "Should have at least 3 spans after split");
                assert_eq!(links.len(), 1);

                // Check text reconstruction
                let combined: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(combined, "Before https://example.com After");

                // Check that only the URL span has link_url
                let link_spans: Vec<_> = spans.iter().filter(|s| s.link_url.is_some()).collect();
                assert_eq!(link_spans.len(), 1);
                assert_eq!(link_spans[0].text, "https://example.com");
            }
            _ => panic!("Expected Paragraph"),
        }
    }
}
