//! Message bubble components for chat
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use crate::ui_gpui::components::markdown_content::{
    blocks_to_elements, parse_markdown_blocks, MarkdownBlock,
};
use gpui::{div, prelude::*, px, IntoElement, MouseButton};
use std::sync::Arc;

pub struct UserBubble {
    content: String,
}

impl UserBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

impl IntoElement for UserBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        div()
            .flex()
            .justify_end()
            .w_full()
            .child(Theme::user_bubble(
                div()
                    .w(px(400.0))
                    .px(px(Theme::SPACING_MD))
                    .py(px(Theme::SPACING_SM))
                    .rounded(px(Theme::RADIUS_LG))
                    .child(self.content),
            ))
    }
}

/// Assistant message bubble with markdown rendering.
///
/// Stores `Arc<String>` to allow cheap sharing of message content
/// via `Arc::clone()` without heap allocation during renders.
/// Also accepts optional pre-parsed markdown blocks to avoid
/// re-parsing finalized messages on every render.
///
/// @plan PLAN-20260407-ISSUE172.P10
pub struct AssistantBubble {
    /// Arc-wrapped content for cheap sharing across renders.
    content: Arc<String>,
    /// Optional pre-parsed markdown blocks for finalized messages.
    /// Streaming messages should NOT provide this since content changes.
    cached_blocks: Option<Arc<Vec<MarkdownBlock>>>,
    model_id: Option<String>,
    thinking: Option<String>,
    show_thinking: bool,
    is_streaming: bool,
}

impl AssistantBubble {
    /// Create a new assistant bubble with the given content.
    ///
    /// Accepts `Arc<String>` or any type that can be converted to it,
    /// allowing callers to pass `Arc::clone()` without allocation.
    ///
    /// @plan PLAN-20260407-ISSUE172.P10
    pub fn new(content: impl Into<Arc<String>>) -> Self {
        Self {
            content: content.into(),
            cached_blocks: None,
            model_id: None,
            thinking: None,
            show_thinking: false,
            is_streaming: false,
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

    /// Provide pre-parsed markdown blocks to avoid re-parsing.
    ///
    /// Only use this for finalized messages where content won't change.
    /// Streaming messages should NOT provide cached blocks.
    ///
    /// @plan PLAN-20260407-ISSUE172.P10
    #[must_use]
    pub fn with_cached_blocks(mut self, blocks: Arc<Vec<MarkdownBlock>>) -> Self {
        self.cached_blocks = Some(blocks);
        self
    }

    /// Returns a reference to the content string slice.
    #[must_use]
    pub fn content_str(&self) -> &str {
        &self.content
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

        // @plan:PLAN-20260402-MARKDOWN.P11
        // @plan:PLAN-20260407-ISSUE172.P10 (cached blocks)
        // @requirement:REQ-MD-INTEGRATE-002
        let blocks: Vec<MarkdownBlock> = if self.is_streaming || self.cached_blocks.is_none() {
            // Streaming or no cache: parse fresh
            parse_markdown_blocks(&content_text)
        } else {
            // Finalized with cache: use cached blocks (only if no emoji filtering)
            // Since content_text is same as self.content, we can use cache
            (*self.cached_blocks.unwrap()).clone()
        };
        let rendered = blocks_to_elements(&blocks);

        let mut main_content = Theme::assistant_bubble(
            div()
                .w_full()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .rounded(px(Theme::RADIUS_LG))
                .children(rendered),
        );

        if should_enable_bubble_copy(&blocks, self.is_streaming) {
            // Clone the Arc, not the String - defer allocation to click time
            let raw_markdown = Arc::clone(&self.content);
            main_content =
                main_content
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            (*raw_markdown).clone(),
                        ));
                    });
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
