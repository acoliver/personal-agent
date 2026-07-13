//! Markdown content rendering for assistant messages.
//!
//! This module implements the two-phase IR architecture for markdown rendering:
//! 1. Phase 1: `parse_markdown_blocks()` converts markdown text to `Vec<MarkdownBlock>` (pure Rust)
//! 2. Phase 2: `blocks_to_elements()` converts IR to GPUI elements
//!
//! The public API `render_markdown()` composes both phases.

#![allow(
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::module_name_repetitions,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

/// @plan:PLAN-20260402-MARKDOWN.P03
use std::ops::Range;

use gpui::px;

/// A single inline text span with style flags.
///
/// Represents a segment of text with uniform styling. Multiple spans
/// compose the content of block-level elements.
///
/// @plan:PLAN-20260402-MARKDOWN.P03
/// @requirement:REQ-MD-PARSE-061
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownInline {
    /// The text content of this span.
    pub text: String,

    /// Bold style flag (from `**text**` or `__text__`).
    pub bold: bool,

    /// Italic style flag (from `*text*` or `_text_`).
    pub italic: bool,

    /// Strikethrough flag (from `~~text~~`).
    pub strikethrough: bool,

    /// Inline code flag (from `` `text` ``).
    /// When true, text should render with monospace font.
    pub code: bool,

    /// Link URL for clickable text (from `[text](url)`).
    /// When Some, this span is part of a link.
    pub link_url: Option<String>,
}

impl MarkdownInline {
    /// Create a plain text span with all flags false.
    ///
    /// @plan:PLAN-20260402-MARKDOWN.P03
    /// @requirement:REQ-MD-PARSE-061
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            strikethrough: false,
            code: false,
            link_url: None,
        }
    }
}

/// A block-level markdown element.
///
/// This enum represents all supported block-level constructs from the
/// markdown input. The parser produces a `Vec<MarkdownBlock>` from input text.
///
/// @plan:PLAN-20260402-MARKDOWN.P03
/// @requirement:REQ-MD-PARSE-062
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    /// A paragraph containing inline spans.
    Paragraph {
        /// The inline content of the paragraph.
        spans: Vec<MarkdownInline>,
        /// Clickable link ranges with their URLs.
        /// Each tuple is (byte_range, url) for InteractiveText.
        links: Vec<(Range<usize>, String)>,
    },

    /// A heading with level 1-6.
    Heading {
        /// Heading level (1 = H1, 6 = H6).
        level: u8,
        /// The inline content of the heading.
        spans: Vec<MarkdownInline>,
        /// Clickable link ranges with their URLs.
        links: Vec<(Range<usize>, String)>,
    },

    /// A code block (fenced or indented).
    CodeBlock {
        /// The language identifier from the fence (e.g., "rust").
        /// None for indented code blocks.
        language: Option<String>,
        /// The code content (raw text, not parsed as markdown).
        code: String,
    },

    /// A blockquote containing nested blocks.
    BlockQuote {
        /// The nested block content.
        blocks: Vec<MarkdownBlock>,
    },

    /// A list (ordered or unordered).
    List {
        /// true for numbered lists (1., 2., ...).
        /// false for bullet lists (-, *, +).
        ordered: bool,
        /// Starting number for ordered lists (default: 0 for unordered).
        start: u64,
        /// List items, where each item contains its block content.
        items: Vec<Vec<MarkdownBlock>>,
    },

    /// A table with header and body rows.
    Table {
        /// Column alignments (left, center, right, or none).
        /// Length equals column count.
        alignments: Vec<Alignment>,
        /// Header row cells, each containing inline content.
        header: Vec<TableCell>,
        /// Body rows, each containing cells with inline content.
        rows: Vec<Vec<TableCell>>,
    },

    /// A horizontal rule (thematic break).
    ThematicBreak,

    /// An image rendered as fallback text.
    /// Per REQ-MD-PARSE-040, images are not rendered.
    ImageFallback {
        /// The alt text extracted from the image markdown.
        alt: String,
    },
}

/// A single table cell containing inline content.
///
/// @plan:PLAN-20260402-MARKDOWN.P03
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    /// The inline content of the cell.
    pub spans: Vec<MarkdownInline>,
    /// Clickable link ranges with their URLs.
    pub links: Vec<(Range<usize>, String)>,
}

/// Text alignment for table columns.
///
/// Maps to pulldown-cmark's Alignment type.
///
/// @plan:PLAN-20260402-MARKDOWN.P03
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alignment {
    /// Default/no alignment specified.
    None,
    /// Left-aligned (:|:---).
    Left,
    /// Center-aligned (:|---:).
    Center,
    /// Right-aligned (---:|).
    Right,
}

impl From<pulldown_cmark::Alignment> for Alignment {
    fn from(a: pulldown_cmark::Alignment) -> Self {
        match a {
            pulldown_cmark::Alignment::None => Alignment::None,
            pulldown_cmark::Alignment::Left => Alignment::Left,
            pulldown_cmark::Alignment::Center => Alignment::Center,
            pulldown_cmark::Alignment::Right => Alignment::Right,
        }
    }
}

// ============================================================================
// BLOCK BUILDER TYPES
// ============================================================================

/// Internal builder enum for accumulating block content during parsing.
///
/// This tracks partially constructed blocks that are on the stack while
/// processing nested markdown structures.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// Inline style tracking for nested formatting.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
#[derive(Clone)]
enum InlineStyle {
    Bold,
    Italic,
    Strikethrough,
    Link(String),
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create an inline span with current style flags applied.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @pseudocode parse-markdown-blocks.md lines 515-521
fn create_inline_span(text: &str, stack: &[InlineStyle]) -> MarkdownInline {
    let bold = stack.iter().any(|s| matches!(s, InlineStyle::Bold));
    let italic = stack.iter().any(|s| matches!(s, InlineStyle::Italic));
    let strikethrough = stack
        .iter()
        .any(|s| matches!(s, InlineStyle::Strikethrough));
    let link_url = stack.iter().find_map(|s| {
        if let InlineStyle::Link(url) = s {
            Some(url.clone())
        } else {
            None
        }
    });
    MarkdownInline {
        text: text.to_string(),
        bold,
        italic,
        strikethrough,
        code: false,
        link_url,
    }
}

/// Count total bytes in all spans.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @pseudocode parse-markdown-blocks.md lines 522-528
fn count_bytes_in_spans(spans: &[MarkdownInline]) -> usize {
    spans.iter().map(|s| s.text.len()).sum()
}

/// Strip HTML tags from content, with special handling for script/style.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @pseudocode parse-markdown-blocks.md lines 529-591
fn strip_html_tags(html: &str) -> String {
    let mut in_tag = false;
    let mut in_strip_tag = false;
    let mut result = String::new();
    let mut chars = html.chars();
    let mut tag_buffer = String::new();

    while let Some(ch) = chars.next() {
        if ch == '<' && !in_tag {
            in_tag = true;
            tag_buffer.clear();
            // Check if this is a script or style tag
            let remaining: String = chars.clone().take(10).collect();
            let lower = remaining.to_ascii_lowercase();
            if lower.starts_with("script") || lower.starts_with("style") {
                in_strip_tag = true;
            } else if lower.starts_with("/script") || lower.starts_with("/style") {
                in_strip_tag = false;
            }
        } else if ch == '>' && in_tag {
            in_tag = false;
            tag_buffer.clear();
        } else if in_tag {
            tag_buffer.push(ch);
        } else if in_strip_tag {
            // Inside script/style content: strip entirely
        } else {
            // Outside tag: append character
            result.push(ch);
        }
    }

    // Handle malformed: append unclosed tag literal to preserve text order.
    if in_tag && !in_strip_tag {
        result.push('<');
        result.push_str(&tag_buffer);
    }

    result
}

/// Extract language from code block info string.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @pseudocode parse-markdown-blocks.md lines 571-579
fn extract_language(info: &str) -> Option<String> {
    let words: Vec<&str> = info.split_whitespace().collect();
    if words.is_empty() {
        None
    } else {
        Some(words[0].to_string())
    }
}

/// Convert intermediate representation blocks to GPUI elements.
///
/// Phase 2 of the two-phase IR pipeline. This function takes the IR produced
/// by `parse_markdown_blocks()` and constructs GPUI elements for rendering.
/// Uses text_primary() for text color.
///
/// @plan:PLAN-20260402-MARKDOWN.P06
pub(crate) fn blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<gpui::AnyElement> {
    blocks_to_elements_with_color(blocks, crate::ui_gpui::theme::Theme::text_primary())
}

/// Phase 2 variant that accepts a custom text color.
/// Used by user message bubbles which need user_bubble_text() color.
///
/// @plan:PLAN-20260402-ISSUE153.P02
pub(crate) fn blocks_to_elements_with_color(
    blocks: &[MarkdownBlock],
    text_color: gpui::Hsla,
) -> Vec<gpui::AnyElement> {
    rich_builder::build_rich_children(blocks, text_color)
}

/// Public API: Render markdown content to GPUI elements.
///
/// Composes `parse_markdown_blocks()` and `blocks_to_elements()` into a single
/// call. This is the entry point that `AssistantBubble` will use.
///
/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-040
#[allow(dead_code)]
pub fn render_markdown(content: &str) -> Vec<gpui::AnyElement> {
    let blocks = parse_markdown_blocks(content);
    blocks_to_elements(&blocks)
}

/// Check if a URL has a safe scheme (http or https only).
///
/// Per REQ-MD-SEC-001, only http and https schemes are allowed for link
/// click handling. All other schemes (javascript:, file:, etc.) are rejected.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @requirement:REQ-MD-SEC-001
/// @pseudocode render-markdown.md lines 59-82
pub(crate) fn is_safe_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }

    url::Url::parse(trimmed).is_ok_and(|url| {
        let scheme = url.scheme();
        scheme == "http" || scheme == "https"
    })
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-023
pub(crate) fn inline_to_text_run(span: &MarkdownInline, text_color: gpui::Hsla) -> gpui::TextRun {
    use gpui::{font, FontStyle, FontWeight, StrikethroughStyle, TextRun, UnderlineStyle};

    // For links, use accent color. For non-links, use the provided text_color
    let mut run = TextRun {
        len: span.text.len(),
        color: if span.link_url.is_some() {
            crate::ui_gpui::theme::Theme::accent()
        } else {
            text_color
        },
        ..Default::default()
    };

    if span.bold {
        run.font.weight = FontWeight::BOLD;
    }
    if span.italic {
        run.font.style = FontStyle::Italic;
    }
    if span.code {
        run.background_color = Some(crate::ui_gpui::theme::Theme::bg_dark());
        run.font = font(crate::ui_gpui::theme::Theme::mono_font_family_name());
        run.font.features = crate::ui_gpui::theme::Theme::mono_font_features();
    }
    if span.strikethrough {
        run.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.0),
            color: Some(crate::ui_gpui::theme::Theme::text_muted()),
        });
    }
    if span.link_url.is_some() {
        run.underline = Some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(crate::ui_gpui::theme::Theme::accent()),
            wavy: false,
        });
    }

    run
}

mod autolink;
mod leaf_meta;
mod markdown_parser;
mod rich_builder;
#[cfg(test)]
mod selectable_leaf;
pub mod selectable_markdown;
pub mod visible_document;

pub(crate) use autolink::apply_autolinks;
pub use markdown_parser::parse_markdown_blocks;

#[cfg(test)]
mod markdown_content_tests;
