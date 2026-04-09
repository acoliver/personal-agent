//! Message bubble components for chat
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use crate::ui_gpui::components::copy_icons::copy_icon;
use crate::ui_gpui::components::markdown_content::{
    blocks_to_elements, parse_markdown_blocks, MarkdownBlock,
};
use gpui::{div, prelude::*, px, IntoElement, MouseButton, SharedString};

const COPY_BUTTON_ICON_SIZE: f32 = 14.0;

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

        let bubble_content = self.content.clone();
        let button_content = self.content.clone();
        let copy_button_id = button_content.clone();

        div().flex().justify_end().w_full().child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(Theme::SPACING_SM))
                .max_w(px(400.0))
                .child(Theme::user_bubble(
                    div()
                        .w_full()
                        .px(px(Theme::SPACING_MD))
                        .py(px(Theme::SPACING_SM))
                        .rounded(px(Theme::RADIUS_LG))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                bubble_content.clone(),
                            ));
                        })
                        .child(self.content),
                ))
                .child(render_copy_button(
                    None,
                    true,
                    copy_button_id,
                    move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            button_content.clone(),
                        ));
                    },
                )),
        )
    }
}

pub struct AssistantBubble {
    content: String,
    model_id: Option<String>,
    thinking: Option<String>,
    show_thinking: bool,
    is_streaming: bool,
}

impl AssistantBubble {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
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

const fn should_show_copy_button(is_streaming: bool) -> bool {
    !is_streaming
}

fn render_copy_button(
    leading_label: Option<String>,
    align_end: bool,
    id_suffix: impl Into<String>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    use crate::ui_gpui::theme::Theme;

    let mut footer = div()
        .flex()
        .items_center()
        .gap(px(Theme::SPACING_SM))
        .w_full();

    if align_end {
        footer = footer.justify_end();
    } else {
        footer = footer.justify_between();
    }

    if let Some(label) = leading_label {
        footer = footer.child(div().text_sm().text_color(Theme::text_muted()).child(label));
    }

    let button_id = format!("copy-message-{}", id_suffix.into());

    footer.child(
        div()
            .id(SharedString::from(button_id))
            .size(px(28.0))
            .rounded(px(Theme::RADIUS_SM))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_darker())
            .hover(|s| s.bg(Theme::bg_dark()))
            .active(|s| s.bg(Theme::bg_dark()))
            .child(copy_icon(COPY_BUTTON_ICON_SIZE).text_color(Theme::text_primary()))
            .on_mouse_down(MouseButton::Left, on_click),
    )
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
        // @requirement:REQ-MD-INTEGRATE-002
        let blocks = parse_markdown_blocks(&content_text);
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
            let raw_markdown = self.content.clone();
            main_content =
                main_content
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            raw_markdown.clone(),
                        ));
                    });
        }

        bubble = bubble.child(main_content);

        if should_show_copy_button(self.is_streaming) {
            let button_content = self.content.clone();
            bubble = bubble.child(render_copy_button(
                self.model_id.map(|model_id| format!("via {model_id}")),
                false,
                button_content.clone(),
                move |_, _, cx| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(button_content.clone()));
                },
            ));
        } else if let Some(model_id) = self.model_id {
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

    #[test]
    fn test_copy_button_hidden_for_streaming_messages() {
        assert!(!should_show_copy_button(true));
        assert!(should_show_copy_button(false));
    }
}
