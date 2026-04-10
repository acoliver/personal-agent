//! Message bubble components for chat
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use crate::ui_gpui::components::markdown_content::{
    blocks_to_elements, blocks_to_elements_with_color, parse_markdown_blocks, MarkdownBlock,
};
use gpui::{div, prelude::*, px, IntoElement, MouseButton, StyledText};
use std::ops::Range;

pub struct UserBubble {
    content: String,
    selection: Option<Range<usize>>,
}

impl UserBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            selection: None,
        }
    }

    #[must_use]
    pub const fn selection(mut self, range: Option<Range<usize>>) -> Self {
        self.selection = range;
        self
    }
}

impl IntoElement for UserBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        // Selection mode: render the raw text with the selection highlight
        // and skip markdown / click-to-copy so the visible glyphs match the
        // transcript backing string byte-for-byte.
        if let Some(range) = self.selection.clone() {
            let content = render_text_with_selection(&self.content, range).cursor_text();
            return div()
                .flex()
                .justify_end()
                .w_full()
                .child(Theme::user_bubble(
                    div()
                        .w(px(400.0))
                        .px(px(Theme::SPACING_MD))
                        .py(px(Theme::SPACING_SM))
                        .rounded(px(Theme::RADIUS_LG))
                        .child(content),
                ));
        }

        // No selection: route through the markdown pipeline so links are
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

/// Render text with selection highlight.
fn render_text_with_selection(text: &str, selection: Range<usize>) -> gpui::Div {
    let Some(styled) = render_selection_styled_text(text, &selection, false) else {
        return div().child(text.to_string());
    };

    div().child(styled)
}

pub struct AssistantBubble {
    content: String,
    model_id: Option<String>,
    thinking: Option<String>,
    show_thinking: bool,
    is_streaming: bool,
    selection: Option<Range<usize>>,
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

/// Render text with selection highlight.
///
/// @plan PLAN-20260406-ISSUE151.P01
fn render_selection_styled_text(
    text: &str,
    range: &Range<usize>,
    preserve_assistant_bubble_text: bool,
) -> Option<gpui::StyledText> {
    use crate::ui_gpui::theme::Theme;

    if range.is_empty() {
        return None;
    }

    // Snap selection bounds to UTF-8 char boundaries to avoid panics on
    // multi-byte characters when callers pass byte offsets that fall mid-char.
    let mut start = range.start.min(text.len());
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = range.end.min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    if start >= end {
        return None;
    }

    let before = &text[..start];
    let selected = &text[start..end];
    let after = &text[end..];

    let base_color = if preserve_assistant_bubble_text {
        Theme::text_primary()
    } else {
        Theme::user_bubble_text()
    };

    let mut runs = Vec::with_capacity(3);
    if !before.is_empty() {
        runs.push(gpui::TextRun {
            len: before.len(),
            color: base_color,
            ..Default::default()
        });
    }
    runs.push(gpui::TextRun {
        len: selected.len(),
        color: Theme::selection_fg(),
        background_color: Some(Theme::selection_bg()),
        ..Default::default()
    });
    if !after.is_empty() {
        runs.push(gpui::TextRun {
            len: after.len(),
            color: base_color,
            ..Default::default()
        });
    }

    // The reconstructed string is byte-identical to the original input, so
    // reuse it directly instead of allocating via `format!`.
    Some(StyledText::new(text.to_string()).with_runs(runs))
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
            if let Some(thinking_content) = self.thinking {
                bubble = bubble.child(Theme::badge(
                    div()
                        .w_full()
                        .px(px(Theme::SPACING_MD))
                        .py(px(Theme::SPACING_SM))
                        .rounded(px(Theme::RADIUS_MD))
                        .text_sm()
                        .child(format!("Thinking: {thinking_content}")),
                ));
            }
        }

        let content_text = rendered_content_text(&self.content, self.is_streaming);

        let mut main_content = Theme::assistant_bubble(
            div()
                .w_full()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .rounded(px(Theme::RADIUS_LG)),
        )
        .cursor_text();

        // Check for selection highlight or normal markdown/copy behavior.
        // @plan PLAN-20260406-ISSUE151.P01
        if let Some(ref range) = self.selection {
            if let Some(styled) = render_selection_styled_text(&self.content, range, true) {
                main_content = main_content.child(styled);
            } else {
                main_content = main_content.child(self.content.clone());
            }
        } else {
            // @plan:PLAN-20260402-MARKDOWN.P11
            // @requirement:REQ-MD-INTEGRATE-002
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
