//! Shared rich Markdown renderer with optional selectable-leaf metadata.
//!
//! Both the existing rendering wrappers and [`SelectableMarkdown`] call this
//! builder. Selection therefore retains the same block hierarchy, text runs,
//! spacing, and link styles as normal Markdown rendering.

use std::ops::Range;

use gpui::{div, prelude::*, px, AnyElement, Hsla, StyledText, TextRun};

use super::leaf_meta::{LeafMeta, LeafRegistry};
use super::visible_document::{Selection, VisibleDocument, VisibleLeaf};
use super::{Alignment, MarkdownBlock, MarkdownInline, TableCell};

/// Build the rich children used by normal Markdown rendering.
pub(super) fn build_rich_children(blocks: &[MarkdownBlock], text_color: Hsla) -> Vec<AnyElement> {
    let document = VisibleDocument::from_blocks(blocks);
    RichRenderer::new(&document, text_color, None, None, text_color, true).render_blocks(blocks)
}

/// Build the same rich hierarchy with tracked layouts and message-level links.
pub(super) fn build_selectable_rich_tree(
    blocks: &[MarkdownBlock],
    document: &VisibleDocument,
    text_color: Hsla,
    selection: Option<&Selection>,
    selection_text_color: Hsla,
    registry: &LeafRegistry,
) -> AnyElement {
    let selection_range = selection.map(Selection::ordered_range);
    let children = RichRenderer::new(
        document,
        text_color,
        Some(registry),
        selection_range,
        selection_text_color,
        false,
    )
    .render_blocks(blocks);
    div()
        .w_full()
        .min_w(px(0.0))
        .children(children)
        .into_any_element()
}

struct RichRenderer<'a> {
    document: &'a VisibleDocument,
    text_color: Hsla,
    registry: Option<&'a LeafRegistry>,
    selection_range: Option<Range<usize>>,
    selection_text_color: Hsla,
    interactive_links: bool,
    next_leaf: usize,
}

impl<'a> RichRenderer<'a> {
    fn new(
        document: &'a VisibleDocument,
        text_color: Hsla,
        registry: Option<&'a LeafRegistry>,
        selection_range: Option<Range<usize>>,
        selection_text_color: Hsla,
        interactive_links: bool,
    ) -> Self {
        Self {
            document,
            text_color,
            registry,
            selection_range,
            selection_text_color,
            interactive_links,
            next_leaf: 0,
        }
    }

    fn render_blocks(mut self, blocks: &[MarkdownBlock]) -> Vec<AnyElement> {
        let result = self.render_nested_blocks(blocks);
        debug_assert_eq!(
            self.next_leaf,
            self.document.leaves().len(),
            "rich renderer and visible document traversals diverged"
        );
        result
    }

    fn render_nested_blocks(&mut self, blocks: &[MarkdownBlock]) -> Vec<AnyElement> {
        blocks
            .iter()
            .map(|block| self.render_block(block))
            .collect()
    }

    fn render_block(&mut self, block: &MarkdownBlock) -> AnyElement {
        match block {
            MarkdownBlock::Paragraph { spans, links } => self.render_paragraph(spans, links),
            MarkdownBlock::Heading {
                level,
                spans,
                links,
            } => self.render_heading(*level, spans, links),
            MarkdownBlock::CodeBlock { language, code } => {
                self.render_code_block(language.as_deref(), code)
            }
            MarkdownBlock::BlockQuote { blocks } => self.render_blockquote(blocks),
            MarkdownBlock::List {
                ordered,
                start,
                items,
            } => self.render_list(*ordered, *start, items),
            MarkdownBlock::Table {
                alignments,
                header,
                rows,
            } => self.render_table(alignments, header, rows),
            MarkdownBlock::ThematicBreak => Self::render_thematic_break(),
            MarkdownBlock::ImageFallback { alt } => self.render_image_fallback(alt),
        }
    }

    fn render_paragraph(
        &mut self,
        spans: &[MarkdownInline],
        links: &[(Range<usize>, String)],
    ) -> AnyElement {
        div()
            .text_size(px(crate::ui_gpui::theme::Theme::font_size_body()))
            .child(self.render_inline_leaf(spans, links))
            .into_any_element()
    }

    fn render_heading(
        &mut self,
        level: u8,
        spans: &[MarkdownInline],
        links: &[(Range<usize>, String)],
    ) -> AnyElement {
        let size = match level {
            1 => crate::ui_gpui::theme::Theme::font_size_h1(),
            2 => crate::ui_gpui::theme::Theme::font_size_h2(),
            3 => crate::ui_gpui::theme::Theme::font_size_h3(),
            4 => crate::ui_gpui::theme::Theme::font_size_body(),
            5 => crate::ui_gpui::theme::Theme::font_size_mono(),
            _ => crate::ui_gpui::theme::Theme::font_size_ui(),
        };

        div()
            .w_full()
            .min_w(px(0.0))
            .text_size(px(size))
            .font_weight(gpui::FontWeight::BOLD)
            .child(self.render_inline_leaf(spans, links))
            .into_any_element()
    }

    fn render_code_block(&mut self, language: Option<&str>, code: &str) -> AnyElement {
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
            .font_family(crate::ui_gpui::theme::Theme::mono_font_family_name())
            .font_features(crate::ui_gpui::theme::Theme::mono_font_features())
            .text_size(px(crate::ui_gpui::theme::Theme::font_size_mono()));

        if let Some(language) = language {
            block = block.child(
                div()
                    .text_size(px(crate::ui_gpui::theme::Theme::font_size_ui()))
                    .text_color(crate::ui_gpui::theme::Theme::text_muted())
                    .child(self.render_text_leaf(language, Vec::new(), Vec::new())),
            );
        }

        block
            .child(self.render_text_leaf(code, Vec::new(), Vec::new()))
            .into_any_element()
    }

    fn render_blockquote(&mut self, blocks: &[MarkdownBlock]) -> AnyElement {
        let children = self.render_nested_blocks(blocks);
        div()
            .w_full()
            .border_l_2()
            .border_color(crate::ui_gpui::theme::Theme::accent())
            .pl(px(crate::ui_gpui::theme::Theme::SPACING_SM))
            .py(px(crate::ui_gpui::theme::Theme::SPACING_XS))
            .bg(crate::ui_gpui::theme::Theme::bg_base())
            .children(children)
            .into_any_element()
    }

    fn render_list(
        &mut self,
        ordered: bool,
        start: u64,
        items: &[Vec<MarkdownBlock>],
    ) -> AnyElement {
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
            .w_full();

        for (index, item_blocks) in items.iter().enumerate() {
            let prefix = if ordered {
                format!("{}. ", start.saturating_add(index as u64))
            } else {
                "• ".to_string()
            };
            let prefix_leaf = self.render_bare_text_leaf(&prefix, Vec::new(), Vec::new());
            let item_children = self.render_nested_blocks(item_blocks);
            list = list.child(
                div()
                    .flex()
                    .w_full()
                    .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                    .child(
                        div()
                            .text_color(crate::ui_gpui::theme::Theme::text_muted())
                            .child(prefix_leaf),
                    )
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(crate::ui_gpui::theme::Theme::SPACING_XS))
                            .children(item_children),
                    ),
            );
        }

        list.into_any_element()
    }

    fn render_table(
        &mut self,
        alignments: &[Alignment],
        header: &[TableCell],
        rows: &[Vec<TableCell>],
    ) -> AnyElement {
        let col_count = header
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0))
            .max(alignments.len());
        let grid_cols = u16::try_from(col_count.max(1)).unwrap_or(u16::MAX);
        let mut table = div().grid().grid_cols(grid_cols).w_full();

        for (column, cell) in header.iter().enumerate() {
            let content = self.render_inline_leaf(&cell.spans, &cell.links);
            table = table.child(Self::render_table_cell(
                content,
                alignments.get(column).unwrap_or(&Alignment::None),
                crate::ui_gpui::theme::Theme::bg_dark(),
            ));
        }

        for (row_index, row) in rows.iter().enumerate() {
            for (column, cell) in row.iter().enumerate() {
                let content = self.render_inline_leaf(&cell.spans, &cell.links);
                let background = if row_index % 2 == 0 {
                    crate::ui_gpui::theme::Theme::bg_base()
                } else {
                    crate::ui_gpui::theme::Theme::bg_dark()
                };
                table = table.child(Self::render_table_cell(
                    content,
                    alignments.get(column).unwrap_or(&Alignment::None),
                    background,
                ));
            }
        }

        div().w_full().child(table).into_any_element()
    }

    fn render_table_cell(
        content: AnyElement,
        alignment: &Alignment,
        background: Hsla,
    ) -> AnyElement {
        let aligned = match alignment {
            Alignment::Center => div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .justify_center()
                .child(content),
            Alignment::Right => div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .justify_end()
                .child(content),
            Alignment::Left | Alignment::None => div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .justify_start()
                .child(content),
        };

        div()
            .w_full()
            .min_w(px(120.0))
            .px(px(crate::ui_gpui::theme::Theme::SPACING_XS))
            .py(px(crate::ui_gpui::theme::Theme::SPACING_XS))
            .bg(background)
            .border_1()
            .border_color(crate::ui_gpui::theme::Theme::border())
            .child(aligned)
            .into_any_element()
    }

    fn render_thematic_break() -> AnyElement {
        div()
            .h(px(1.0))
            .w_full()
            .bg(crate::ui_gpui::theme::Theme::border())
            .into_any_element()
    }

    fn render_image_fallback(&mut self, alt: &str) -> AnyElement {
        let label = format!("[image: {alt}]");
        div()
            .text_color(crate::ui_gpui::theme::Theme::text_muted())
            .text_size(px(crate::ui_gpui::theme::Theme::font_size_mono()))
            .child(self.render_text_leaf(&label, Vec::new(), Vec::new()))
            .into_any_element()
    }

    fn render_inline_leaf(
        &mut self,
        spans: &[MarkdownInline],
        links: &[(Range<usize>, String)],
    ) -> AnyElement {
        let mut text = String::new();
        let mut runs = Vec::with_capacity(spans.len());
        for span in spans {
            text.push_str(&span.text);
            runs.push(super::inline_to_text_run(span, self.text_color));
        }
        self.render_text_leaf(&text, runs, links.to_vec())
    }

    fn render_text_leaf(
        &mut self,
        text: &str,
        runs: Vec<TextRun>,
        links: Vec<(Range<usize>, String)>,
    ) -> AnyElement {
        div()
            .w_full()
            .min_w(px(0.0))
            .child(self.render_bare_text_leaf(text, runs, links))
            .into_any_element()
    }

    fn render_bare_text_leaf(
        &mut self,
        text: &str,
        runs: Vec<TextRun>,
        links: Vec<(Range<usize>, String)>,
    ) -> AnyElement {
        let visible_leaf = self.consume_leaf(text);
        let runs = apply_selection_text_color(
            text,
            runs,
            &visible_leaf.range,
            self.selection_range.as_ref(),
            self.selection_text_color,
            self.text_color,
        );
        let mut styled = StyledText::new(text.to_string());
        if !runs.is_empty() {
            styled = styled.with_runs(runs);
        }

        if let Some(registry) = self.registry {
            registry.register(LeafMeta {
                doc_range: visible_leaf.range,
                layout: styled.layout().clone(),
            });
        }

        if self.interactive_links && !links.is_empty() {
            let ranges = links.iter().map(|(range, _)| range.clone()).collect();
            let destinations: Vec<String> = links.into_iter().map(|(_, url)| url).collect();
            gpui::InteractiveText::new("markdown-links", styled)
                .on_click(ranges, move |clicked, _window, cx| {
                    if let Some(url) = destinations.get(clicked) {
                        if super::is_safe_url(url) {
                            cx.open_url(url);
                        }
                    }
                })
                .into_any_element()
        } else {
            styled.into_any_element()
        }
    }

    fn consume_leaf(&mut self, rendered_text: &str) -> VisibleLeaf {
        let leaf = self
            .document
            .leaves()
            .get(self.next_leaf)
            .unwrap_or_else(|| panic!("missing visible leaf for {rendered_text:?}"))
            .clone();
        self.next_leaf += 1;
        debug_assert_eq!(
            self.document.text().get(leaf.range.clone()),
            Some(rendered_text),
            "rich leaf text differs from canonical visible document"
        );
        leaf
    }
}

fn apply_selection_text_color(
    text: &str,
    runs: Vec<TextRun>,
    leaf_range: &Range<usize>,
    selection_range: Option<&Range<usize>>,
    selection_color: Hsla,
    default_color: Hsla,
) -> Vec<TextRun> {
    let Some(selection) = selection_range else {
        return runs;
    };
    let selected_start = selection.start.max(leaf_range.start);
    let selected_end = selection.end.min(leaf_range.end);
    if selected_start >= selected_end {
        return runs;
    }

    let local_selection = (selected_start - leaf_range.start)..(selected_end - leaf_range.start);
    let source_runs = if runs.is_empty() {
        vec![TextRun {
            len: text.len(),
            color: default_color,
            ..Default::default()
        }]
    } else {
        runs
    };
    let mut result = Vec::with_capacity(source_runs.len() + 2);
    let mut run_start = 0;
    for run in source_runs {
        let run_end = run_start + run.len;
        if local_selection.end <= run_start || local_selection.start >= run_end {
            result.push(run);
            run_start = run_end;
            continue;
        }
        let selected_run_start = local_selection.start.max(run_start);
        let selected_run_end = local_selection.end.min(run_end);
        if run_start < selected_run_start {
            result.push(run_with_len(&run, selected_run_start - run_start));
        }
        let mut selected = run_with_len(&run, selected_run_end - selected_run_start);
        selected.color = selection_color;
        result.push(selected);
        if selected_run_end < run_end {
            result.push(run_with_len(&run, run_end - selected_run_end));
        }
        run_start = run_end;
    }
    result
}

fn run_with_len(run: &TextRun, len: usize) -> TextRun {
    let mut result = run.clone();
    result.len = len;
    result
}

#[cfg(test)]
mod selection_run_tests {
    use gpui::{font, FontStyle, FontWeight, UnderlineStyle};

    use super::*;

    #[test]
    fn selected_foreground_preserves_rich_run_styles() {
        let base_color = gpui::hsla(0.3, 0.8, 0.4, 1.0);
        let selected_color = gpui::hsla(0.0, 0.0, 0.0, 1.0);
        let source = TextRun {
            len: 6,
            font: font("monospace"),
            color: base_color,
            underline: Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(base_color),
                wavy: false,
            }),
            ..Default::default()
        };
        let mut styled_source = source;
        styled_source.font.weight = FontWeight::BOLD;
        styled_source.font.style = FontStyle::Italic;

        let runs = apply_selection_text_color(
            "abcdef",
            vec![styled_source.clone()],
            &(10..16),
            Some(&(12..15)),
            selected_color,
            base_color,
        );

        assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), 6);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[1].color, selected_color);
        assert_eq!(runs[1].font, styled_source.font);
        assert_eq!(runs[1].underline, styled_source.underline);
        assert_eq!(runs[0].color, base_color);
        assert_eq!(runs[2].color, base_color);
    }
}
