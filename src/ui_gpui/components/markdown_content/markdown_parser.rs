//! Markdown parser for assistant message rendering.
//!
//! Extracted from markdown_content to satisfy structural file-length gates.

#![allow(
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::module_name_repetitions,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

use super::{
    count_bytes_in_spans, create_inline_span, extract_language, strip_html_tags, Alignment,
    InlineStyle, MarkdownBlock, MarkdownInline, TableCell,
};
use std::ops::Range;

/// Parse markdown text into intermediate representation blocks.
///
/// Phase 1 of the two-phase IR pipeline. This function has no GPUI dependency
/// and produces a pure data structure that can be tested independently.
///
/// @plan:PLAN-20260402-MARKDOWN.P05
/// @requirement:REQ-MD-PARSE-001
/// @pseudocode parse-markdown-blocks.md lines 1-10
pub(crate) fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock> {
    ParseState::new().parse(content)
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

struct ParseState {
    blocks: Vec<MarkdownBlock>,
    text_buffer: String,
    current_spans: Vec<MarkdownInline>,
    current_links: Vec<(Range<usize>, String)>,
    current_heading_level: Option<u8>,
    inline_stack: Vec<InlineStyle>,
    link_start_offset: usize,
    current_url: Option<String>,
    container_stack: Vec<Container>,
    image_alt_buffer: String,
    in_image: bool,
    footnote_label: String,
    in_html_block: bool,
    html_buffer: String,
}

impl ParseState {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            text_buffer: String::new(),
            current_spans: Vec::new(),
            current_links: Vec::new(),
            current_heading_level: None,
            inline_stack: Vec::new(),
            link_start_offset: 0,
            current_url: None,
            container_stack: Vec::new(),
            image_alt_buffer: String::new(),
            in_image: false,
            footnote_label: String::new(),
            in_html_block: false,
            html_buffer: String::new(),
        }
    }

    fn parse(mut self, content: &str) -> Vec<MarkdownBlock> {
        use pulldown_cmark::{Options, Parser};

        let options = Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_MATH;
        for event in Parser::new_ext(content, options) {
            self.handle_event(event);
        }
        self.finish()
    }

    fn finish(mut self) -> Vec<MarkdownBlock> {
        self.flush_text_buffer();
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            let links = std::mem::take(&mut self.current_links);
            self.push_block(MarkdownBlock::Paragraph { spans, links });
        }
        self.flush_unclosed_containers();
        self.blocks
    }

    fn handle_event(&mut self, event: pulldown_cmark::Event<'_>) {
        use pulldown_cmark::Event;

        match event {
            Event::Start(tag) => self.handle_start_tag(tag),
            Event::End(tag_end) => self.handle_end_tag(tag_end),
            Event::Text(text) => self.handle_text(text.as_ref()),
            Event::Code(text) | Event::InlineMath(text) => self.push_code_span(text.as_ref()),
            Event::DisplayMath(text) => self.push_display_math_block(text.as_ref()),
            Event::SoftBreak => self.text_buffer.push(' '),
            Event::HardBreak => self.text_buffer.push('\n'),
            Event::Rule => self.push_block(MarkdownBlock::ThematicBreak),
            Event::Html(html) => self.handle_html(html.as_ref()),
            Event::InlineHtml(html) => {
                let stripped = strip_html_tags(html.as_ref());
                self.text_buffer.push_str(&stripped);
            }
            Event::TaskListMarker(checked) => self.push_task_marker(checked),
            Event::FootnoteReference(label) => self.push_footnote_reference(label.as_ref()),
        }
    }

    fn handle_start_tag(&mut self, tag: pulldown_cmark::Tag<'_>) {
        use pulldown_cmark::Tag;

        match tag {
            Tag::Paragraph | Tag::Item | Tag::TableCell => self.reset_inline_buffers(),
            Tag::Heading { level, .. } => self.begin_heading(level as u8),
            Tag::CodeBlock(kind) => self.begin_code_block(kind),
            Tag::BlockQuote(_) => self.begin_blockquote(),
            Tag::List(start_num) => self.begin_list(start_num),
            Tag::Table(alignments) => self.begin_table(alignments),
            Tag::TableHead => self.begin_table_head(),
            Tag::TableRow => self.begin_table_row(),
            Tag::Strong => self.push_inline_style(InlineStyle::Bold),
            Tag::Emphasis => self.push_inline_style(InlineStyle::Italic),
            Tag::Strikethrough => self.push_inline_style(InlineStyle::Strikethrough),
            Tag::Link { dest_url, .. } => self.begin_link(dest_url.as_ref()),
            Tag::HtmlBlock => self.begin_html_block(),
            Tag::Image { .. } => self.begin_image(),
            Tag::FootnoteDefinition(label) => self.begin_footnote_definition(label.as_ref()),
            _ => {}
        }
    }

    fn handle_end_tag(&mut self, tag_end: pulldown_cmark::TagEnd) {
        use pulldown_cmark::TagEnd;

        match tag_end {
            TagEnd::Paragraph => self.end_paragraph(),
            TagEnd::Heading(_) => self.end_heading(),
            TagEnd::CodeBlock => self.end_code_block(),
            TagEnd::BlockQuote(_) => self.end_blockquote(),
            TagEnd::List(_) => self.end_list(),
            TagEnd::Item => self.end_item(),
            TagEnd::TableRow => self.end_table_row(),
            TagEnd::TableCell => self.end_table_cell(),
            TagEnd::Table => self.end_table(),
            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough => self.pop_inline_style(),
            TagEnd::Link => self.end_link(),
            TagEnd::HtmlBlock => self.end_html_block(),
            TagEnd::Image => self.end_image(),
            TagEnd::FootnoteDefinition => self.end_footnote_definition(),
            _ => {}
        }
    }

    fn reset_inline_buffers(&mut self) {
        self.text_buffer.clear();
        self.current_spans.clear();
        self.current_links.clear();
    }

    fn begin_heading(&mut self, level: u8) {
        self.reset_inline_buffers();
        self.current_heading_level = Some(level);
    }

    fn begin_code_block(&mut self, kind: pulldown_cmark::CodeBlockKind<'_>) {
        let language = match kind {
            pulldown_cmark::CodeBlockKind::Fenced(info) => extract_language(info.as_ref()),
            pulldown_cmark::CodeBlockKind::Indented => None,
        };
        self.text_buffer.clear();
        self.container_stack.push(Container::CodeBlock { language });
    }

    fn begin_blockquote(&mut self) {
        self.container_stack
            .push(Container::BlockQuote { children: vec![] });
    }

    fn begin_list(&mut self, start_num: Option<u64>) {
        self.container_stack.push(Container::List {
            ordered: start_num.is_some(),
            start: start_num.unwrap_or(0),
            items: vec![],
            current_item: vec![],
        });
    }

    fn begin_table(&mut self, alignments: Vec<pulldown_cmark::Alignment>) {
        self.container_stack.push(Container::Table {
            alignments: alignments.into_iter().map(Alignment::from).collect(),
            header: vec![],
            rows: vec![],
            current_row: vec![],
            in_header: false,
        });
    }

    fn begin_table_head(&mut self) {
        if let Some(Container::Table { in_header, .. }) = self.container_stack.last_mut() {
            *in_header = true;
        }
    }

    fn begin_table_row(&mut self) {
        if let Some(Container::Table {
            in_header,
            header,
            rows,
            current_row,
            ..
        }) = self.container_stack.last_mut()
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

    fn push_inline_style(&mut self, style: InlineStyle) {
        self.flush_text_buffer();
        self.inline_stack.push(style);
    }

    fn pop_inline_style(&mut self) {
        self.flush_text_buffer();
        self.inline_stack.pop();
    }

    fn begin_link(&mut self, url: &str) {
        self.flush_text_buffer();
        self.link_start_offset = count_bytes_in_spans(&self.current_spans);
        self.current_url = Some(url.to_string());
        self.inline_stack.push(InlineStyle::Link(url.to_string()));
    }

    fn begin_html_block(&mut self) {
        self.in_html_block = true;
        self.html_buffer.clear();
    }

    fn begin_image(&mut self) {
        self.in_image = true;
        self.image_alt_buffer.clear();
    }

    fn begin_footnote_definition(&mut self, label: &str) {
        self.footnote_label = label.to_string();
        self.text_buffer.clear();
    }

    fn end_paragraph(&mut self) {
        self.flush_text_buffer();
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            let links = std::mem::take(&mut self.current_links);
            self.push_block(MarkdownBlock::Paragraph { spans, links });
        }
    }

    fn end_heading(&mut self) {
        self.flush_text_buffer();
        let spans = std::mem::take(&mut self.current_spans);
        let links = std::mem::take(&mut self.current_links);
        self.push_block(MarkdownBlock::Heading {
            level: self.current_heading_level.unwrap_or(1),
            spans,
            links,
        });
        self.current_heading_level = None;
    }

    fn end_code_block(&mut self) {
        let language = if let Some(Container::CodeBlock { language }) = self.container_stack.pop() {
            language
        } else {
            None
        };

        let code = std::mem::take(&mut self.text_buffer);
        self.push_block(MarkdownBlock::CodeBlock { language, code });
    }

    fn end_blockquote(&mut self) {
        if let Some(Container::BlockQuote { children }) = self.container_stack.pop() {
            self.push_block(MarkdownBlock::BlockQuote { blocks: children });
        }
    }

    fn end_list(&mut self) {
        if let Some(Container::List {
            ordered,
            start,
            mut items,
            current_item,
        }) = self.container_stack.pop()
        {
            if !current_item.is_empty() {
                items.push(current_item);
            }
            self.push_block(MarkdownBlock::List {
                ordered,
                start,
                items,
            });
        }
    }

    fn end_item(&mut self) {
        self.flush_text_buffer();
        let paragraph = if self.current_spans.is_empty() {
            None
        } else {
            let spans = std::mem::take(&mut self.current_spans);
            let links = std::mem::take(&mut self.current_links);
            Some(MarkdownBlock::Paragraph { spans, links })
        };

        if let Some(Container::List {
            items,
            current_item,
            ..
        }) = self.container_stack.last_mut()
        {
            if let Some(paragraph) = paragraph {
                current_item.push(paragraph);
            }

            if !current_item.is_empty() {
                items.push(std::mem::take(current_item));
            }
        }
    }

    fn end_table_row(&mut self) {
        if let Some(Container::Table {
            in_header,
            header,
            rows,
            current_row,
            ..
        }) = self.container_stack.last_mut()
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

    fn end_table_cell(&mut self) {
        self.flush_text_buffer();
        let spans = std::mem::take(&mut self.current_spans);
        let links = std::mem::take(&mut self.current_links);
        let cell = TableCell { spans, links };

        if let Some(Container::Table { current_row, .. }) = self.container_stack.last_mut() {
            current_row.push(cell);
        }
    }

    fn end_table(&mut self) {
        if let Some(Container::Table {
            alignments,
            header,
            rows,
            ..
        }) = self.container_stack.pop()
        {
            self.push_block(MarkdownBlock::Table {
                alignments,
                header,
                rows,
            });
        }
    }

    fn end_link(&mut self) {
        self.flush_text_buffer();
        if let Some(url) = self.current_url.take() {
            let link_end_offset = count_bytes_in_spans(&self.current_spans);
            self.current_links
                .push((self.link_start_offset..link_end_offset, url));
        }
        self.inline_stack.pop();
    }

    fn end_html_block(&mut self) {
        self.in_html_block = false;
        let stripped = strip_html_tags(&self.html_buffer);
        if !stripped.is_empty() {
            self.push_plain_paragraph_to_root(stripped);
        }
        self.html_buffer.clear();
    }

    fn end_image(&mut self) {
        self.in_image = false;
        let alt = std::mem::take(&mut self.image_alt_buffer);
        self.push_block(MarkdownBlock::ImageFallback { alt });
    }

    fn end_footnote_definition(&mut self) {
        if !self.text_buffer.is_empty() {
            let prefixed_text = format!("[^{}]: {}", self.footnote_label, self.text_buffer);
            self.push_plain_paragraph_to_root(prefixed_text);
        }
        self.footnote_label.clear();
    }

    fn handle_text(&mut self, text: &str) {
        if self.in_image {
            self.image_alt_buffer.push_str(text);
        } else {
            self.text_buffer.push_str(text);
        }
    }

    fn push_code_span(&mut self, text: &str) {
        self.current_spans.push(MarkdownInline {
            text: text.to_string(),
            bold: false,
            italic: false,
            strikethrough: false,
            code: true,
            link_url: None,
        });
    }

    fn push_display_math_block(&mut self, text: &str) {
        self.push_block(MarkdownBlock::CodeBlock {
            language: Some("math".to_string()),
            code: text.to_string(),
        });
    }

    fn handle_html(&mut self, html: &str) {
        if self.in_html_block {
            self.html_buffer.push_str(html);
            return;
        }

        let stripped = strip_html_tags(html);
        if !stripped.is_empty() {
            self.push_plain_paragraph_to_root(stripped);
        }
    }

    fn push_task_marker(&mut self, checked: bool) {
        self.text_buffer
            .push(if checked { '\u{2611}' } else { '\u{2610}' });
        self.text_buffer.push_str("  ");
    }

    fn push_footnote_reference(&mut self, label: &str) {
        use std::fmt::Write as _;
        let _ = write!(self.text_buffer, "[^{label}]");
    }

    fn push_plain_paragraph_to_root(&mut self, text: String) {
        self.blocks.push(MarkdownBlock::Paragraph {
            spans: vec![MarkdownInline::plain(text)],
            links: vec![],
        });
    }

    fn flush_text_buffer(&mut self) {
        if !self.text_buffer.is_empty() {
            let span = create_inline_span(&self.text_buffer, &self.inline_stack);
            self.current_spans.push(span);
            self.text_buffer.clear();
        }
    }

    fn push_block(&mut self, block: MarkdownBlock) {
        for container in self.container_stack.iter_mut().rev() {
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
        self.blocks.push(block);
    }

    fn flush_unclosed_containers(&mut self) {
        let mut remaining: Vec<MarkdownBlock> = vec![];
        while let Some(container) = self.container_stack.pop() {
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

        for block in remaining.into_iter().rev() {
            self.blocks.push(block);
        }
    }
}
