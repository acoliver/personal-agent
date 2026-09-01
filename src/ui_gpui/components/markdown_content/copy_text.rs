//! Syntax-free copy leaves derived from markdown IR without layout.

use super::{MarkdownBlock, MarkdownInline, TableCell};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownCopyLeaf {
    pub text: String,
    pub separator_before: String,
}

#[must_use]
pub fn markdown_copy_leaves(blocks: &[MarkdownBlock]) -> Vec<MarkdownCopyLeaf> {
    let mut leaves = Vec::new();
    collect_blocks(blocks, "", &mut leaves);
    leaves
}

fn collect_blocks(
    blocks: &[MarkdownBlock],
    first_separator: &str,
    leaves: &mut Vec<MarkdownCopyLeaf>,
) {
    for (index, block) in blocks.iter().enumerate() {
        let separator = if index == 0 { first_separator } else { "\n\n" };
        collect_block(block, separator, leaves);
    }
}

fn collect_block(block: &MarkdownBlock, separator: &str, leaves: &mut Vec<MarkdownCopyLeaf>) {
    match block {
        MarkdownBlock::Paragraph { spans, .. } | MarkdownBlock::Heading { spans, .. } => {
            push_leaf(leaves, spans_text(spans), separator);
        }
        MarkdownBlock::CodeBlock { code, .. } => push_leaf(leaves, code.clone(), separator),
        MarkdownBlock::BlockQuote { blocks } => collect_blocks(blocks, separator, leaves),
        MarkdownBlock::List {
            ordered,
            start,
            items,
        } => collect_list(*ordered, *start, items, separator, leaves),
        MarkdownBlock::Table { header, rows, .. } => {
            collect_table(header, rows, separator, leaves);
        }
        MarkdownBlock::ThematicBreak => {}
        MarkdownBlock::ImageFallback { alt } => {
            push_leaf(leaves, format!("[image: {alt}]"), separator);
        }
    }
}

fn collect_list(
    ordered: bool,
    start: u64,
    items: &[Vec<MarkdownBlock>],
    separator: &str,
    leaves: &mut Vec<MarkdownCopyLeaf>,
) {
    for (index, item) in items.iter().enumerate() {
        let prefix = if ordered {
            format!("{}. ", start.saturating_add(index as u64))
        } else {
            "• ".to_string()
        };
        push_leaf(leaves, prefix, if index == 0 { separator } else { "\n" });
        collect_blocks(item, "", leaves);
    }
}

fn collect_table(
    header: &[TableCell],
    rows: &[Vec<TableCell>],
    separator: &str,
    leaves: &mut Vec<MarkdownCopyLeaf>,
) {
    for (column, cell) in header.iter().enumerate() {
        push_leaf(
            leaves,
            spans_text(&cell.spans),
            if column == 0 { separator } else { "\t" },
        );
    }
    for row in rows {
        for (column, cell) in row.iter().enumerate() {
            push_leaf(
                leaves,
                spans_text(&cell.spans),
                if column == 0 { "\n" } else { "\t" },
            );
        }
    }
}

fn spans_text(spans: &[MarkdownInline]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

fn push_leaf(leaves: &mut Vec<MarkdownCopyLeaf>, text: String, separator: &str) {
    leaves.push(MarkdownCopyLeaf {
        text,
        separator_before: separator.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_gpui::components::markdown_content::parse_markdown_blocks;

    fn copy_text(leaves: &[MarkdownCopyLeaf]) -> String {
        leaves
            .iter()
            .enumerate()
            .fold(String::new(), |mut text, (index, leaf)| {
                if index > 0 {
                    text.push_str(&leaf.separator_before);
                }
                text.push_str(&leaf.text);
                text
            })
    }

    #[test]
    fn copy_leaves_remove_markdown_fences_and_preserve_list_and_table_separators() {
        let blocks = parse_markdown_blocks(
            "Intro **bold**\n\n```rust\nfn main() {}\n```\n\n- one\n- two\n\n| A | B |\n| - | - |\n| x | y |",
        );

        assert_eq!(
            copy_text(&markdown_copy_leaves(&blocks)),
            "Intro bold\n\nfn main() {}\n\n\n• one\n• two\n\nA\tB\nx\ty"
        );
    }

    #[test]
    fn copy_leaves_match_nested_quote_list_link_and_image_fallback_text() {
        let mut link = MarkdownInline::plain("link");
        link.link_url = Some("https://example.com".to_string());
        let blocks = vec![MarkdownBlock::BlockQuote {
            blocks: vec![
                MarkdownBlock::Paragraph {
                    spans: vec![MarkdownInline::plain("quote "), link],
                    links: Vec::new(),
                },
                MarkdownBlock::List {
                    ordered: false,
                    start: 0,
                    items: vec![
                        vec![
                            MarkdownBlock::Paragraph {
                                spans: vec![MarkdownInline::plain("one")],
                                links: Vec::new(),
                            },
                            MarkdownBlock::List {
                                ordered: true,
                                start: 3,
                                items: vec![vec![MarkdownBlock::Paragraph {
                                    spans: vec![MarkdownInline::plain("nested")],
                                    links: Vec::new(),
                                }]],
                            },
                        ],
                        vec![MarkdownBlock::Paragraph {
                            spans: vec![MarkdownInline::plain("two")],
                            links: Vec::new(),
                        }],
                    ],
                },
                MarkdownBlock::ImageFallback {
                    alt: "diagram".to_string(),
                },
            ],
        }];

        assert_eq!(
            copy_text(&markdown_copy_leaves(&blocks)),
            "quote link\n\n• one\n\n3. nested\n• two\n\n[image: diagram]"
        );
    }
}
