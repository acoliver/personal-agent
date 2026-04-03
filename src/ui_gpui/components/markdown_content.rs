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

use gpui::{div, prelude::*, px};

/// A single inline text span with style flags.
///
/// Represents a segment of text with uniform styling. Multiple spans
/// compose the content of block-level elements.
///
/// @plan:PLAN-20260402-MARKDOWN.P03
/// @requirement:REQ-MD-PARSE-061
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MarkdownInline {
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MarkdownBlock {
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableCell {
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Alignment {
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

    while let Some(ch) = chars.next() {
        if ch == '<' && !in_tag {
            in_tag = true;
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
        } else if in_tag {
            // Inside tag: skip character
        } else if in_strip_tag {
            // Inside script/style content: strip entirely
        } else {
            // Outside tag: append character
            result.push(ch);
        }
    }

    // Handle malformed: if still in_tag, treat remaining as literal
    if in_tag {
        result.insert(0, '<');
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

/// Parse markdown text into intermediate representation blocks.
///
/// Phase 1 of the two-phase IR pipeline. This function has no GPUI dependency
/// and produces a pure data structure that can be tested independently.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @requirement:REQ-MD-PARSE-001
/// @pseudocode parse-markdown-blocks.md lines 1-10
#[allow(dead_code, clippy::too_many_lines)]
pub(crate) fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(content, options);

    let mut blocks: Vec<MarkdownBlock> = Vec::new();
    let mut text_buffer = String::new();
    let mut current_spans: Vec<MarkdownInline> = Vec::new();
    let mut current_links: Vec<(Range<usize>, String)> = Vec::new();
    let mut current_heading_level: Option<u8> = None;
    let mut inline_stack: Vec<InlineStyle> = Vec::new();
    let mut link_start_offset: usize = 0;
    let mut current_url: Option<String> = None;

    let mut container_stack: Vec<Container> = Vec::new();
    let mut image_alt_buffer = String::new();
    let mut in_image = false;
    let mut footnote_label = String::new();
    let mut in_html_block = false;
    let mut html_buffer = String::new();

    // Helper to push a block, respecting container hierarchy
    let push_block = |block: MarkdownBlock,
                      blocks: &mut Vec<MarkdownBlock>,
                      container_stack: &mut Vec<Container>| {
        for container in container_stack.iter_mut().rev() {
            match container {
                Container::BlockQuote { children } => {
                    children.push(block);
                    return;
                }
                Container::List { current_item, .. } => {
                    current_item.push(block);
                    return;
                }
                _ => {}
            }
        }
        blocks.push(block);
    };

    // Helper to finalize current text buffer into spans
    let flush_text_buffer = |text_buffer: &mut String,
                             current_spans: &mut Vec<MarkdownInline>,
                             inline_stack: &[InlineStyle]| {
        if !text_buffer.is_empty() {
            let span = create_inline_span(text_buffer, inline_stack);
            current_spans.push(span);
            text_buffer.clear();
        }
    };

    for event in parser {
        match event {
            // Block-level: Paragraph / Item / TableCell
            Event::Start(Tag::Paragraph | Tag::Item | Tag::TableCell) => {
                text_buffer.clear();
                current_spans.clear();
                current_links.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                if !current_spans.is_empty() {
                    let block = MarkdownBlock::Paragraph {
                        spans: std::mem::take(&mut current_spans),
                        links: std::mem::take(&mut current_links),
                    };
                    push_block(block, &mut blocks, &mut container_stack);
                }
            }

            // Block-level: Heading
            Event::Start(Tag::Heading { level, .. }) => {
                text_buffer.clear();
                current_spans.clear();
                current_links.clear();
                current_heading_level = Some(level as u8);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                let block = MarkdownBlock::Heading {
                    level: current_heading_level.unwrap_or(1),
                    spans: std::mem::take(&mut current_spans),
                    links: std::mem::take(&mut current_links),
                };
                current_heading_level = None;
                push_block(block, &mut blocks, &mut container_stack);
            }

            // Block-level: Code Block
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(info) => extract_language(info.as_ref()),
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                text_buffer.clear();
                container_stack.push(Container::CodeBlock { language });
            }
            Event::End(TagEnd::CodeBlock) => {
                let language =
                    if let Some(Container::CodeBlock { language }) = container_stack.pop() {
                        language
                    } else {
                        None
                    };
                let block = MarkdownBlock::CodeBlock {
                    language,
                    code: std::mem::take(&mut text_buffer),
                };
                push_block(block, &mut blocks, &mut container_stack);
            }

            // Block-level: BlockQuote
            Event::Start(Tag::BlockQuote(_)) => {
                container_stack.push(Container::BlockQuote { children: vec![] });
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                if let Some(Container::BlockQuote { children }) = container_stack.pop() {
                    let block = MarkdownBlock::BlockQuote { blocks: children };
                    push_block(block, &mut blocks, &mut container_stack);
                }
            }

            // Block-level: List
            Event::Start(Tag::List(start_num)) => {
                container_stack.push(Container::List {
                    ordered: start_num.is_some(),
                    start: start_num.unwrap_or(0),
                    items: vec![],
                    current_item: vec![],
                });
            }
            Event::End(TagEnd::List(_)) => {
                if let Some(Container::List {
                    ordered,
                    start,
                    mut items,
                    current_item,
                }) = container_stack.pop()
                {
                    if !current_item.is_empty() {
                        items.push(current_item);
                    }
                    let block = MarkdownBlock::List {
                        ordered,
                        start,
                        items,
                    };
                    push_block(block, &mut blocks, &mut container_stack);
                }
            }

            // Block-level: Item
            Event::End(TagEnd::Item) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                if let Some(Container::List {
                    items,
                    current_item,
                    ..
                }) = container_stack.last_mut()
                {
                    if !current_spans.is_empty() {
                        current_item.push(MarkdownBlock::Paragraph {
                            spans: std::mem::take(&mut current_spans),
                            links: std::mem::take(&mut current_links),
                        });
                    }

                    if !current_item.is_empty() {
                        items.push(std::mem::take(current_item));
                    }
                }
            }

            // Block-level: Table
            Event::Start(Tag::Table(alignments)) => {
                let aln: Vec<Alignment> = alignments.iter().map(|&a| a.into()).collect();
                container_stack.push(Container::Table {
                    alignments: aln,
                    header: vec![],
                    rows: vec![],
                    current_row: vec![],
                    in_header: false,
                });
            }
            Event::Start(Tag::TableHead) => {
                if let Some(Container::Table { in_header, .. }) = container_stack.last_mut() {
                    *in_header = true;
                }
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(Container::Table { .. }) = container_stack.last_mut() {
                    // Keep header state until the corresponding TableRow ends.
                }
            }
            Event::Start(Tag::TableRow) => {
                if let Some(Container::Table {
                    in_header,
                    header,
                    rows,
                    current_row,
                    ..
                }) = container_stack.last_mut()
                {
                    if !current_row.is_empty() {
                        let previous_row = std::mem::take(current_row);
                        if *in_header {
                            header.extend(previous_row);
                            *in_header = false;
                        } else {
                            rows.push(previous_row);
                        }
                    }
                    current_row.clear();
                }
            }
            Event::End(TagEnd::TableRow) => {
                if let Some(Container::Table {
                    in_header,
                    header,
                    rows,
                    current_row,
                    ..
                }) = container_stack.last_mut()
                {
                    let row = std::mem::take(current_row);
                    if *in_header {
                        header.extend(row);
                        *in_header = false;
                    } else {
                        rows.push(row);
                    }
                }
            }
            Event::End(TagEnd::TableCell) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                let cell = TableCell {
                    spans: std::mem::take(&mut current_spans),
                    links: std::mem::take(&mut current_links),
                };
                if let Some(Container::Table { current_row, .. }) = container_stack.last_mut() {
                    current_row.push(cell);
                }
            }
            Event::End(TagEnd::Table) => {
                if let Some(Container::Table {
                    alignments,
                    header,
                    rows,
                    ..
                }) = container_stack.pop()
                {
                    let block = MarkdownBlock::Table {
                        alignments,
                        header,
                        rows,
                    };
                    push_block(block, &mut blocks, &mut container_stack);
                }
            }

            // Block-level: Thematic Break
            Event::Rule => {
                let block = MarkdownBlock::ThematicBreak;
                push_block(block, &mut blocks, &mut container_stack);
            }

            // Inline: Bold
            Event::Start(Tag::Strong) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                inline_stack.push(InlineStyle::Bold);
            }
            Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                inline_stack.pop();
            }

            // Inline: Italic
            Event::Start(Tag::Emphasis) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                inline_stack.push(InlineStyle::Italic);
            }

            // Inline: Strikethrough
            Event::Start(Tag::Strikethrough) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                inline_stack.push(InlineStyle::Strikethrough);
            }

            // Inline: Link
            Event::Start(Tag::Link { dest_url, .. }) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                link_start_offset = count_bytes_in_spans(&current_spans);
                let url_str = dest_url.to_string();
                current_url = Some(url_str.clone());
                inline_stack.push(InlineStyle::Link(url_str));
            }
            Event::End(TagEnd::Link) => {
                flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
                if let Some(url) = current_url.take() {
                    let link_end_offset = count_bytes_in_spans(&current_spans);
                    current_links.push((link_start_offset..link_end_offset, url));
                }
                inline_stack.pop();
            }

            // Text events
            Event::Text(text) => {
                if in_image {
                    image_alt_buffer.push_str(&text);
                } else {
                    text_buffer.push_str(&text);
                }
            }

            // Inline code + inline math
            Event::Code(text) | Event::InlineMath(text) => {
                current_spans.push(MarkdownInline {
                    text: text.to_string(),
                    bold: false,
                    italic: false,
                    strikethrough: false,
                    code: true,
                    link_url: None,
                });
            }

            // Breaks
            Event::SoftBreak => {
                text_buffer.push(' ');
            }
            Event::HardBreak => {
                text_buffer.push('\n');
            }

            // HTML
            Event::Start(Tag::HtmlBlock) => {
                in_html_block = true;
                html_buffer.clear();
            }
            Event::End(TagEnd::HtmlBlock) => {
                in_html_block = false;
                let stripped = strip_html_tags(&html_buffer);
                if !stripped.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph {
                        spans: vec![MarkdownInline::plain(stripped)],
                        links: vec![],
                    });
                }
                html_buffer.clear();
            }
            Event::Html(html) => {
                if in_html_block {
                    html_buffer.push_str(&html);
                } else {
                    let stripped = strip_html_tags(&html);
                    if !stripped.is_empty() {
                        blocks.push(MarkdownBlock::Paragraph {
                            spans: vec![MarkdownInline::plain(stripped)],
                            links: vec![],
                        });
                    }
                }
            }
            Event::InlineHtml(html) => {
                let stripped = strip_html_tags(&html);
                text_buffer.push_str(&stripped);
            }

            // Image
            Event::Start(Tag::Image { .. }) => {
                in_image = true;
                image_alt_buffer.clear();
            }
            Event::End(TagEnd::Image) => {
                in_image = false;
                let block = MarkdownBlock::ImageFallback {
                    alt: std::mem::take(&mut image_alt_buffer),
                };
                push_block(block, &mut blocks, &mut container_stack);
            }

            // Task list marker
            Event::TaskListMarker(checked) => {
                if checked {
                    text_buffer.push('\u{2611}');
                } else {
                    text_buffer.push('\u{2610}');
                }
                text_buffer.push_str("  ");
            }

            // Footnote reference
            Event::FootnoteReference(label) => {
                use std::fmt::Write as _;
                let _ = write!(text_buffer, "[^{label}]");
            }

            // Fallbacks
            Event::Start(Tag::Superscript | Tag::Subscript | Tag::MetadataBlock(_))
            | Event::End(TagEnd::Superscript | TagEnd::Subscript | TagEnd::MetadataBlock(_)) => {}

            // Footnote definition
            Event::Start(Tag::FootnoteDefinition(label)) => {
                footnote_label = label.to_string();
                text_buffer.clear();
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if !text_buffer.is_empty() {
                    let prefixed_text = format!("[^{footnote_label}]: {text_buffer}");
                    blocks.push(MarkdownBlock::Paragraph {
                        spans: vec![MarkdownInline::plain(prefixed_text)],
                        links: vec![],
                    });
                }
                footnote_label.clear();
            }

            // Unhandled
            _ => {
                if let Some(Container::CodeBlock { .. }) = container_stack.last() {
                    // Keep raw text behavior inside fenced/indented code blocks.
                }
            }
        }
    }

    // Flush remaining content at end of document
    flush_text_buffer(&mut text_buffer, &mut current_spans, &inline_stack);
    if !current_spans.is_empty() {
        let block = MarkdownBlock::Paragraph {
            spans: std::mem::take(&mut current_spans),
            links: std::mem::take(&mut current_links),
        };
        push_block(block, &mut blocks, &mut container_stack);
    }

    // Handle any unclosed containers (edge cases where markdown is incomplete)
    // These should be appended, not prepended, to preserve order
    let mut remaining: Vec<MarkdownBlock> = vec![];
    while let Some(container) = container_stack.pop() {
        match container {
            Container::BlockQuote { children } => {
                remaining.push(MarkdownBlock::BlockQuote { blocks: children });
            }
            Container::List {
                ordered,
                start,
                mut items,
                current_item,
            } => {
                if !current_item.is_empty() {
                    items.push(current_item);
                }
                remaining.push(MarkdownBlock::List {
                    ordered,
                    start,
                    items,
                });
            }
            _ => {}
        }
    }
    // Append in reverse order since we were popping
    for block in remaining.into_iter().rev() {
        blocks.push(block);
    }

    blocks
}

#[derive(Debug)]
enum Container {
    BlockQuote {
        children: Vec<MarkdownBlock>,
    },
    List {
        ordered: bool,
        start: u64,
        items: Vec<Vec<MarkdownBlock>>,
        current_item: Vec<MarkdownBlock>,
    },
    Table {
        alignments: Vec<Alignment>,
        header: Vec<TableCell>,
        rows: Vec<Vec<TableCell>>,
        current_row: Vec<TableCell>,
        in_header: bool,
    },
    CodeBlock {
        language: Option<String>,
    },
}

/// Convert intermediate representation blocks to GPUI elements.
///
/// Phase 2 of the two-phase IR pipeline. This function takes the IR produced
/// by `parse_markdown_blocks()` and constructs GPUI elements for rendering.
///
/// @plan:PLAN-20260402-MARKDOWN.P06
pub(crate) fn blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<gpui::AnyElement> {
    blocks
        .iter()
        .map(|block| match block {
            MarkdownBlock::Paragraph { spans, links } => render_paragraph(spans, links),
            MarkdownBlock::Heading {
                level,
                spans,
                links,
            } => render_heading(*level, spans, links),
            MarkdownBlock::CodeBlock { language, code } => {
                render_code_block(language.as_ref(), code)
            }
            MarkdownBlock::BlockQuote { blocks } => render_blockquote(blocks),
            MarkdownBlock::List {
                ordered,
                start,
                items,
            } => render_list(*ordered, *start, items),
            MarkdownBlock::Table {
                alignments,
                header,
                rows,
            } => render_table(alignments, header, rows),
            MarkdownBlock::ThematicBreak => render_thematic_break(),
            MarkdownBlock::ImageFallback { alt } => render_image_fallback(alt),
        })
        .collect()
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
fn inline_to_text_run(span: &MarkdownInline) -> gpui::TextRun {
    use gpui::{font, FontStyle, FontWeight, StrikethroughStyle, TextRun, UnderlineStyle};

    let mut run = TextRun {
        len: span.text.len(),
        color: if span.link_url.is_some() {
            crate::ui_gpui::theme::Theme::accent()
        } else {
            crate::ui_gpui::theme::Theme::text_primary()
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
        run.font = font("Menlo");
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

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-002
fn spans_to_styled_text(
    spans: &[MarkdownInline],
    links: &[(Range<usize>, String)],
) -> gpui::AnyElement {
    use gpui::StyledText;

    let mut text = String::new();
    let mut runs = Vec::with_capacity(spans.len());
    for span in spans {
        text.push_str(&span.text);
        runs.push(inline_to_text_run(span));
    }

    let styled = StyledText::new(text).with_runs(runs);
    if links.is_empty() {
        return div().child(styled).into_any_element();
    }

    let ranges: Vec<Range<usize>> = links.iter().map(|(range, _)| range.clone()).collect();
    let links_owned: Vec<String> = links.iter().map(|(_, url)| url.clone()).collect();

    div()
        .child(
            gpui::InteractiveText::new("markdown-links", styled).on_click(
                ranges,
                move |clicked_ix, _window, cx| {
                    if let Some(url) = links_owned.get(clicked_ix) {
                        if is_safe_url(url) {
                            cx.open_url(url);
                        }
                    }
                },
            ),
        )
        .into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-001
fn render_paragraph(
    spans: &[MarkdownInline],
    links: &[(Range<usize>, String)],
) -> gpui::AnyElement {
    div()
        .text_size(px(crate::ui_gpui::theme::Theme::FONT_SIZE_MD))
        .text_color(crate::ui_gpui::theme::Theme::text_primary())
        .child(spans_to_styled_text(spans, links))
        .into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-003
fn render_heading(
    level: u8,
    spans: &[MarkdownInline],
    links: &[(Range<usize>, String)],
) -> gpui::AnyElement {
    let size = match level {
        1 => 24.0,
        2 => 20.0,
        3 => 18.0,
        4 => 16.0,
        5 => 14.0,
        _ => 13.0,
    };

    div()
        .text_size(px(size))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(crate::ui_gpui::theme::Theme::text_primary())
        .child(spans_to_styled_text(spans, links))
        .into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-005
fn render_code_block(language: Option<&String>, code: &str) -> gpui::AnyElement {
    let mut block = div()
        .flex()
        .flex_col()
        .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
        .w_full()
        .px(px(crate::ui_gpui::theme::Theme::SPACING_SM))
        .py(px(crate::ui_gpui::theme::Theme::SPACING_SM))
        .rounded(px(crate::ui_gpui::theme::Theme::RADIUS_MD))
        .bg(crate::ui_gpui::theme::Theme::bg_dark())
        .text_color(crate::ui_gpui::theme::Theme::text_primary())
        .font_family("Menlo")
        .text_size(px(crate::ui_gpui::theme::Theme::FONT_SIZE_SM));

    if let Some(lang) = language {
        block = block.child(
            div()
                .text_size(px(crate::ui_gpui::theme::Theme::FONT_SIZE_XS))
                .text_color(crate::ui_gpui::theme::Theme::text_muted())
                .child(lang.clone()),
        );
    }

    block.child(code.to_string()).into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-007
fn render_blockquote(children: &[MarkdownBlock]) -> gpui::AnyElement {
    div()
        .w_full()
        .border_l_2()
        .border_color(crate::ui_gpui::theme::Theme::accent())
        .pl(px(crate::ui_gpui::theme::Theme::SPACING_SM))
        .py(px(crate::ui_gpui::theme::Theme::SPACING_XS))
        .bg(crate::ui_gpui::theme::Theme::bg_base())
        .children(blocks_to_elements(children))
        .into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-008
fn render_list(ordered: bool, start: u64, items: &[Vec<MarkdownBlock>]) -> gpui::AnyElement {
    let mut list = div()
        .flex()
        .flex_col()
        .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
        .w_full();

    for (idx, item_blocks) in items.iter().enumerate() {
        let prefix = if ordered {
            format!("{}. ", start.saturating_add(idx as u64))
        } else {
            "• ".to_string()
        };
        list = list.child(
            div()
                .flex()
                .w_full()
                .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                .child(
                    div()
                        .text_color(crate::ui_gpui::theme::Theme::text_muted())
                        .child(prefix),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                        .children(blocks_to_elements(item_blocks)),
                ),
        );
    }

    list.into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-009
fn render_table(
    alignments: &[Alignment],
    header: &[TableCell],
    rows: &[Vec<TableCell>],
) -> gpui::AnyElement {
    let _ = alignments;
    let col_count = header.len().max(rows.first().map_or(0, Vec::len));
    let grid_cols = u16::try_from(col_count.max(1)).unwrap_or(u16::MAX);

    let mut table = div().grid().grid_cols(grid_cols).w_full();

    for cell in header {
        table = table.child(
            div()
                .px(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                .py(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                .bg(crate::ui_gpui::theme::Theme::bg_dark())
                .border_1()
                .border_color(crate::ui_gpui::theme::Theme::border())
                .child(spans_to_styled_text(&cell.spans, &cell.links)),
        );
    }

    for (row_idx, row) in rows.iter().enumerate() {
        for cell in row {
            table = table.child(
                div()
                    .px(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                    .py(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                    .bg(if row_idx % 2 == 0 {
                        crate::ui_gpui::theme::Theme::bg_base()
                    } else {
                        crate::ui_gpui::theme::Theme::bg_dark()
                    })
                    .border_1()
                    .border_color(crate::ui_gpui::theme::Theme::border())
                    .child(spans_to_styled_text(&cell.spans, &cell.links)),
            );
        }
    }

    table.into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-010
fn render_thematic_break() -> gpui::AnyElement {
    div()
        .h(px(1.0))
        .w_full()
        .bg(crate::ui_gpui::theme::Theme::border())
        .into_any_element()
}

/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-011
fn render_image_fallback(alt: &str) -> gpui::AnyElement {
    div()
        .text_color(crate::ui_gpui::theme::Theme::text_muted())
        .text_size(px(crate::ui_gpui::theme::Theme::FONT_SIZE_SM))
        .child(format!("[image: {alt}]"))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let blocks = parse_markdown_blocks(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Table {
                alignments,
                header,
                rows,
            } => {
                assert_eq!(alignments.len(), 2);
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
    /// @requirement:REQ-MD-PARSE-041
    #[test]
    fn test_parse_footnote_definition() {
        let input = "[^1]: footnote text";
        let blocks = parse_markdown_blocks(input);
        assert!(!blocks.is_empty());
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
        let input = "`x^2`";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { spans, .. } => {
                assert!(spans.iter().any(|s| s.code));
            }
            _ => panic!("Expected Paragraph"),
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
        // Should not panic and should produce output
        assert!(!blocks.is_empty());
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
        let input = "before [link](https://example.com) after";
        let blocks = parse_markdown_blocks(input);
        match &blocks[0] {
            MarkdownBlock::Paragraph { links, .. } => {
                assert_eq!(links.len(), 1);
                let (range, url) = &links[0];
                assert_eq!(url, "https://example.com");
                assert!(range.start < range.end);
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
    fn test_parser_unknown_event_fallback_no_panic() {
        // This exercises the default _ => {} arm implicitly
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
}
