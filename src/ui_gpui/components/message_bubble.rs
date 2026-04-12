//! Message bubble components for chat
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use crate::ui_gpui::components::markdown_content::{
    blocks_to_elements, blocks_to_elements_with_color, parse_markdown_blocks, MarkdownBlock,
};
use crate::ui_gpui::components::selectable_text::{build_selectable_styled_text, TextLayoutSink};
use gpui::{div, prelude::*, px, IntoElement, MouseButton};
use std::ops::Range;

pub struct UserBubble {
    content: String,
    selection: Option<Range<usize>>,
    selectable: bool,
    body_layout_sink: Option<TextLayoutSink>,
}

impl UserBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            selection: None,
            selectable: false,
            body_layout_sink: None,
        }
    }

    #[must_use]
    pub const fn selection(mut self, range: Option<Range<usize>>) -> Self {
        self.selection = range;
        self
    }

    #[must_use]
    pub const fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    #[must_use]
    pub fn body_layout_sink(mut self, sink: TextLayoutSink) -> Self {
        self.body_layout_sink = Some(sink);
        self
    }
}

impl IntoElement for UserBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        // Flat/selectable mode: render the raw text as StyledText (optionally
        // with selection highlight) and populate the layout sink for
        // hit-testing. Markdown and click-to-copy are suppressed so the
        // visible glyphs match the transcript backing string byte-for-byte.
        if self.selectable {
            let styled = build_selectable_styled_text(
                &self.content,
                self.selection.as_ref(),
                Theme::user_bubble_text(),
            );
            if let Some(ref sink) = self.body_layout_sink {
                *sink.borrow_mut() = Some(styled.layout().clone());
            }
            return div()
                .flex()
                .justify_end()
                .w_full()
                .child(Theme::user_bubble(
                    div()
                        .max_w(px(300.0))
                        .px(px(Theme::SPACING_MD))
                        .py(px(Theme::SPACING_SM))
                        .rounded(px(Theme::RADIUS_LG))
                        .cursor_text()
                        .child(styled),
                ));
        }

        // Markdown mode: route through the markdown pipeline so links are
        // clickable, and only enable click-to-copy when the bubble has none.
        // @plan:PLAN-20260402-ISSUE153.P02
        // @requirement:REQ-MSG-LINK-001
        let blocks = parse_markdown_blocks(&self.content);
        let text_color = Theme::user_bubble_text();
        let rendered = blocks_to_elements_with_color(&blocks, text_color);
        let has_links = has_any_links(&blocks);

        let raw_content = self.content;
        let mut bubble = div()
            .max_w(px(300.0))
            .px(px(10.0))
            .py(px(10.0))
            .rounded(px(12.0))
            .text_size(px(Theme::font_size_mono()))
            .children(rendered);

        if !has_links {
            bubble = bubble
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(raw_content.clone()));
                });
        }

        div()
            .flex()
            .justify_end()
            .w_full()
            .child(Theme::user_bubble(bubble))
    }
}

pub struct AssistantBubble {
    content: String,
    model_id: Option<String>,
    thinking: Option<String>,
    show_thinking: bool,
    is_streaming: bool,
    selection: Option<Range<usize>>,
    selectable: bool,
    body_layout_sink: Option<TextLayoutSink>,
    thinking_layout_sink: Option<TextLayoutSink>,
}

impl AssistantBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            model_id: None,
            thinking: None,
            show_thinking: false,
            is_streaming: false,
            selection: None,
            selectable: false,
            body_layout_sink: None,
            thinking_layout_sink: None,
        }
    }

    #[must_use]
    pub fn model_id(mut self, id: impl Into<String>) -> Self {
        self.model_id = Some(id.into());
        self
    }

    #[must_use]
    pub fn thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking = Some(thinking.into());
        self
    }

    #[must_use]
    pub const fn show_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    #[must_use]
    pub const fn streaming(mut self, is_streaming: bool) -> Self {
        self.is_streaming = is_streaming;
        self
    }

    #[must_use]
    pub const fn selection(mut self, range: Option<Range<usize>>) -> Self {
        self.selection = range;
        self
    }

    #[must_use]
    pub const fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }

    #[must_use]
    pub fn body_layout_sink(mut self, sink: TextLayoutSink) -> Self {
        self.body_layout_sink = Some(sink);
        self
    }

    #[must_use]
    pub fn thinking_layout_sink(mut self, sink: TextLayoutSink) -> Self {
        self.thinking_layout_sink = Some(sink);
        self
    }
}

fn rendered_content_text(content: &str, is_streaming: bool) -> String {
    if is_streaming {
        format!("{content}▋")
    } else {
        content.to_string()
    }
}

/// @plan:PLAN-20260402-MARKDOWN.P11
/// @requirement:REQ-MD-INTEGRATE-024
fn has_any_links(blocks: &[MarkdownBlock]) -> bool {
    blocks.iter().any(|block| match block {
        MarkdownBlock::Paragraph { links, .. } | MarkdownBlock::Heading { links, .. } => {
            !links.is_empty()
        }
        MarkdownBlock::BlockQuote { blocks } => has_any_links(blocks),
        MarkdownBlock::List { items, .. } => {
            items.iter().any(|item_blocks| has_any_links(item_blocks))
        }
        MarkdownBlock::Table { header, rows, .. } => {
            let header_has_links = header.iter().any(|cell| {
                !cell.links.is_empty() || cell.spans.iter().any(|span| span.link_url.is_some())
            });
            let body_has_links = rows.iter().any(|row| {
                row.iter().any(|cell| {
                    !cell.links.is_empty() || cell.spans.iter().any(|span| span.link_url.is_some())
                })
            });
            header_has_links || body_has_links
        }
        _ => false,
    })
}

/// @plan:PLAN-20260402-MARKDOWN.P11
/// @requirement:REQ-MD-INTEGRATE-020
fn should_enable_bubble_copy(blocks: &[MarkdownBlock], is_streaming: bool) -> bool {
    !is_streaming && !has_any_links(blocks)
}

/// Render a thinking block in selectable (flat) or markdown (badge) mode.
fn render_thinking_block(
    thinking_content: &str,
    selectable: bool,
    thinking_layout_sink: Option<&TextLayoutSink>,
) -> gpui::AnyElement {
    use crate::ui_gpui::theme::Theme;

    if selectable {
        let thinking_styled =
            build_selectable_styled_text(thinking_content, None, Theme::text_muted());
        if let Some(sink) = thinking_layout_sink {
            *sink.borrow_mut() = Some(thinking_styled.layout().clone());
        }
        div()
            .max_w(px(300.0))
            .px(px(8.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .bg(Theme::thinking_bg())
            .border_l_2()
            .border_color(Theme::text_muted())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_muted())
                            .child("Thinking"),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .italic()
                            .cursor_text()
                            .child(thinking_styled),
                    ),
            )
            .into_any_element()
    } else {
        Theme::badge(
            div()
                .w_full()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .rounded(px(Theme::RADIUS_MD))
                .text_sm()
                .child(format!("Thinking: {thinking_content}")),
        )
        .into_any_element()
    }
}

impl IntoElement for AssistantBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        let mut bubble = div()
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .gap(px(Theme::SPACING_SM));

        if self.show_thinking {
            if let Some(thinking_content) = &self.thinking {
                if !thinking_content.is_empty() {
                    bubble = bubble.child(render_thinking_block(
                        thinking_content,
                        self.selectable,
                        self.thinking_layout_sink.as_ref(),
                    ));
                }
            }
        }

        let mut main_content = Theme::assistant_bubble(
            div()
                .w_full()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .rounded(px(Theme::RADIUS_LG)),
        )
        .cursor_text();

        // Flat/selectable mode: raw StyledText with optional selection highlight.
        // @plan PLAN-20260406-ISSUE151.P01
        if self.selectable {
            let styled = build_selectable_styled_text(
                &self.content,
                self.selection.as_ref(),
                Theme::text_primary(),
            );
            if let Some(ref sink) = self.body_layout_sink {
                *sink.borrow_mut() = Some(styled.layout().clone());
            }
            main_content = main_content.child(styled);
        } else {
            // @plan:PLAN-20260402-MARKDOWN.P11
            // @requirement:REQ-MD-INTEGRATE-002
            let content_text = rendered_content_text(&self.content, self.is_streaming);
            let blocks = parse_markdown_blocks(&content_text);
            let rendered = blocks_to_elements(&blocks);
            main_content = main_content.children(rendered);

            if should_enable_bubble_copy(&blocks, self.is_streaming) {
                let raw_markdown = self.content.clone();
                main_content = main_content.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            raw_markdown.clone(),
                        ));
                    },
                );
            }
        }

        bubble = bubble.child(main_content);

        if let Some(model_id) = self.model_id {
            bubble = bubble.child(
                div()
                    .text_sm()
                    .text_color(Theme::text_muted())
                    .child(format!("via {model_id}")),
            );
        }

        bubble
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_link_does_not_copy_to_clipboard() {
        let blocks = parse_markdown_blocks("[click](https://example.com)");
        assert!(has_any_links(&blocks));
        assert!(!should_enable_bubble_copy(&blocks, false));
    }

    #[test]
    fn test_streaming_cursor_only_during_streaming() {
        assert_eq!(rendered_content_text("Hello", true), "Hello▋");
        assert_eq!(rendered_content_text("Hello", false), "Hello");
    }

    #[test]
    fn test_table_cell_links_suppress_bubble_copy() {
        let markdown = "| Col |\n|---|\n| [link](https://example.com) |";
        let blocks = parse_markdown_blocks(markdown);

        assert!(has_any_links(&blocks));
        assert!(!should_enable_bubble_copy(&blocks, false));
    }
}
