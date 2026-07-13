//! Visible-document builder.
//!
//! Walks the [`MarkdownBlock`] IR to produce the canonical rendered text and
//! ranges consumed by both rich rendering and selection.

use std::ops::Range;

use super::super::{MarkdownBlock, MarkdownInline, TableCell};
use super::{DocumentRange, VisibleLeaf, CELL_SEPARATOR};

/// A semantic block range used for triple-click selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticBlock {
    /// Byte range into the visible document text.
    pub range: Range<usize>,
}

/// The canonical visible document for a single message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleDocument {
    text: String,
    leaves: Vec<VisibleLeaf>,
    links: Vec<DocumentRange>,
    blocks: Vec<SemanticBlock>,
}

impl VisibleDocument {
    /// Build a visible document from parsed markdown blocks.
    #[must_use]
    pub fn from_blocks(blocks: &[MarkdownBlock]) -> Self {
        let mut context = BuildContext::new();
        for (index, block) in blocks.iter().enumerate() {
            if index > 0 {
                context.push_block_separator();
            }
            context.emit_block(block);
        }
        context.finish()
    }

    /// Return the visible plain text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Return the ranges belonging to actual rendered text leaves.
    #[must_use]
    pub fn leaves(&self) -> &[VisibleLeaf] {
        &self.leaves
    }

    /// Return the link ranges.
    #[must_use]
    pub fn links(&self) -> &[DocumentRange] {
        &self.links
    }

    /// Return the semantic block ranges.
    #[must_use]
    pub fn semantic_blocks(&self) -> &[SemanticBlock] {
        &self.blocks
    }

    /// Return the visible text covered by a selection, clamped to the document.
    #[must_use]
    pub fn selected_text(&self, selection: &super::Selection) -> String {
        let range = selection.clamped(&self.text).ordered_range();
        self.text[range].to_string()
    }
}

struct BuildContext {
    text: String,
    leaves: Vec<VisibleLeaf>,
    links: Vec<DocumentRange>,
    blocks: Vec<SemanticBlock>,
}

impl BuildContext {
    fn new() -> Self {
        Self {
            text: String::new(),
            leaves: Vec::new(),
            links: Vec::new(),
            blocks: Vec::new(),
        }
    }

    fn finish(self) -> VisibleDocument {
        VisibleDocument {
            text: self.text,
            leaves: self.leaves,
            links: self.links,
            blocks: self.blocks,
        }
    }

    fn push_block_separator(&mut self) {
        self.text.push('\n');
    }

    fn push_sibling_separator(&mut self) {
        self.text.push('\n');
    }

    fn push_leaf(&mut self, text: &str) -> Range<usize> {
        let start = self.text.len();
        self.text.push_str(text);
        let range = start..self.text.len();
        self.leaves.push(VisibleLeaf {
            range: range.clone(),
        });
        range
    }

    fn emit_block(&mut self, block: &MarkdownBlock) {
        match block {
            MarkdownBlock::Paragraph { spans, links }
            | MarkdownBlock::Heading { spans, links, .. } => self.emit_inline_block(spans, links),
            MarkdownBlock::CodeBlock { language, code } => {
                self.emit_code_block(language.as_deref(), code);
            }
            MarkdownBlock::BlockQuote { blocks } => self.emit_blockquote(blocks),
            MarkdownBlock::List {
                ordered,
                start,
                items,
            } => self.emit_list(*ordered, *start, items),
            MarkdownBlock::Table { header, rows, .. } => self.emit_table(header, rows),
            MarkdownBlock::ThematicBreak => {}
            MarkdownBlock::ImageFallback { alt } => self.emit_image_fallback(alt),
        }
    }

    fn emit_inline_block(&mut self, spans: &[MarkdownInline], links: &[(Range<usize>, String)]) {
        let start = self.text.len();
        self.emit_inline_leaf(spans, links);
        self.push_semantic_block(start);
    }

    fn emit_inline_leaf(&mut self, spans: &[MarkdownInline], links: &[(Range<usize>, String)]) {
        let text: String = spans.iter().map(|span| span.text.as_str()).collect();
        let base = self.text.len();
        self.push_leaf(&text);
        self.record_links(links, base);
    }

    fn emit_code_block(&mut self, language: Option<&str>, code: &str) {
        let start = self.text.len();
        if let Some(language) = language {
            self.push_leaf(language);
            self.text.push('\n');
        }
        self.push_leaf(code);
        self.push_semantic_block(start);
    }

    fn emit_image_fallback(&mut self, alt: &str) {
        let start = self.text.len();
        self.push_leaf(&format!("[image: {alt}]"));
        self.push_semantic_block(start);
    }

    fn emit_blockquote(&mut self, blocks: &[MarkdownBlock]) {
        for (index, child) in blocks.iter().enumerate() {
            if index > 0 {
                self.push_sibling_separator();
            }
            self.emit_block(child);
        }
    }

    fn emit_list(&mut self, ordered: bool, start: u64, items: &[Vec<MarkdownBlock>]) {
        for (index, item_blocks) in items.iter().enumerate() {
            if index > 0 {
                self.push_sibling_separator();
            }
            let prefix = if ordered {
                format!("{}. ", start.saturating_add(index as u64))
            } else {
                "• ".to_string()
            };
            self.emit_list_item(&prefix, item_blocks);
        }
    }

    fn emit_list_item(&mut self, prefix: &str, item_blocks: &[MarkdownBlock]) {
        let start = self.text.len();
        self.push_leaf(prefix);

        let Some((first, remaining)) = item_blocks.split_first() else {
            self.push_semantic_block(start);
            return;
        };

        match first {
            MarkdownBlock::Paragraph { spans, links }
            | MarkdownBlock::Heading { spans, links, .. } => {
                self.emit_inline_leaf(spans, links);
                self.push_semantic_block(start);
            }
            other => {
                self.push_semantic_block(start);
                self.emit_block(other);
            }
        }

        for child in remaining {
            self.push_sibling_separator();
            self.emit_block(child);
        }
    }

    fn emit_table(&mut self, header: &[TableCell], rows: &[Vec<TableCell>]) {
        let mut emitted_row = if header.is_empty() {
            false
        } else {
            self.emit_table_row(header);
            true
        };
        for row in rows {
            if emitted_row {
                self.push_sibling_separator();
            }
            self.emit_table_row(row);
            emitted_row = true;
        }
    }

    fn emit_table_row(&mut self, cells: &[TableCell]) {
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                self.text.push(CELL_SEPARATOR);
            }
            let start = self.text.len();
            self.emit_inline_leaf(&cell.spans, &cell.links);
            self.push_semantic_block(start);
        }
    }

    fn push_semantic_block(&mut self, start: usize) {
        if start < self.text.len() {
            self.blocks.push(SemanticBlock {
                range: start..self.text.len(),
            });
        }
    }

    fn record_links(&mut self, links: &[(Range<usize>, String)], base_offset: usize) {
        self.links
            .extend(links.iter().map(|(range, url)| DocumentRange {
                range: (base_offset + range.start)..(base_offset + range.end),
                url: url.clone(),
            }));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::parse_markdown_blocks;
    use super::*;

    #[test]
    fn empty_document() {
        let doc = VisibleDocument::from_blocks(&[]);
        assert_eq!(doc.text(), "");
        assert!(doc.leaves().is_empty());
        assert!(doc.links().is_empty());
        assert!(doc.semantic_blocks().is_empty());
    }

    #[test]
    fn single_paragraph() {
        let doc = VisibleDocument::from_blocks(&parse_markdown_blocks("hello"));
        assert_eq!(doc.text(), "hello");
        assert_eq!(doc.leaves().len(), 1);
        assert_eq!(doc.semantic_blocks().len(), 1);
    }
}
