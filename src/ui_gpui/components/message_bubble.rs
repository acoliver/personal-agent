//! Message bubble components for chat
//!
//! @plan PLAN-20250128-GPUI.P06
//! @requirement REQ-GPUI-003

use crate::ui_gpui::components::markdown_content::{
    blocks_to_elements_with_leaf_factory, parse_markdown_blocks, MarkdownBlock, MarkdownLeaf,
    MarkdownLeafFactory,
};
use crate::ui_gpui::components::transcript_selection::{
    TranscriptSelectionContext, TranscriptSelectionLeafFactory, THINKING_BODY_SEPARATOR,
};
use gpui::{div, prelude::*, px, IntoElement};
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
    selection: TranscriptSelectionContext,
}

impl AssistantBubble {
    /// Create a new assistant bubble with the given content.
    ///
    /// Accepts `Arc<String>` or any type that can be converted to it,
    /// allowing callers to pass `Arc::clone()` without allocation.
    ///
    /// @plan PLAN-20260407-ISSUE172.P10
    pub(crate) fn new(
        content: impl Into<Arc<String>>,
        selection: TranscriptSelectionContext,
    ) -> Self {
        Self {
            content: content.into(),
            cached_blocks: None,
            model_id: None,
            thinking: None,
            show_thinking: false,
            is_streaming: false,
            selection,
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

/// Builds the selectable leaf carrying a bubble's thinking text.
///
/// Only the reasoning text participates in selection; the "Thinking:"
/// label is chrome, matching the exclusion of role labels and
/// "via <model>".
fn thinking_leaf(text: &str, document_order: u64, copy_separator_before: &str) -> MarkdownLeaf {
    MarkdownLeaf {
        plain_text: gpui::SharedString::from(text.to_string()),
        links: Vec::new(),
        text_runs: vec![gpui::TextRun {
            len: text.len(),
            color: crate::ui_gpui::theme::Theme::text_muted(),
            ..gpui::TextRun::default()
        }],
        document_order,
        copy_separator_before: copy_separator_before.to_string().into(),
        surface_background: crate::ui_gpui::theme::Theme::bg_dark(),
        surface_foreground: crate::ui_gpui::theme::Theme::text_muted(),
    }
}

/// Builds the thinking badge with its selectable leaf.
///
/// The leaf takes the row's entry document order, ahead of every content
/// leaf, so logical reading order is stable regardless of paint order.
/// Returns the badge, the next content document order, and the separator
/// the first content leaf must use so copied thinking and body text never
/// concatenate.
fn thinking_badge(
    thinking_text: &str,
    document_order: u64,
    first_copy_separator: &'static str,
    factory: &mut TranscriptSelectionLeafFactory,
) -> (gpui::Div, u64, &'static str) {
    use crate::ui_gpui::theme::Theme;

    let leaf = thinking_leaf(thinking_text, document_order, first_copy_separator);
    let badge = Theme::badge(
        div()
            .w_full()
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_MD))
            .text_sm()
            .flex()
            .items_start()
            .gap(px(Theme::SPACING_XS))
            .child(div().flex_shrink_0().child("Thinking:"))
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_1()
                    .child(factory.create_leaf(leaf)),
            ),
    );
    (badge, document_order + 1, THINKING_BODY_SEPARATOR)
}

impl IntoElement for AssistantBubble {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        use crate::ui_gpui::theme::Theme;

        let thinking = self
            .show_thinking
            .then_some(self.thinking.as_deref())
            .flatten()
            .filter(|text| !text.trim().is_empty());

        let mut bubble = div()
            .flex()
            .flex_col()
            .items_start()
            .w_full()
            .gap(px(Theme::SPACING_SM));

        let mut factory = TranscriptSelectionLeafFactory::new(
            self.selection.scroll_offset,
            self.selection.content_key,
            Arc::clone(&self.selection.copy_document),
        );
        let mut document_order = self.selection.document_order;
        let mut first_content_separator = self.selection.first_copy_separator;

        // The thinking leaf takes the row's entry document order, ahead of
        // every content leaf, so logical reading order is stable regardless
        // of paint order.
        if let Some(thinking_text) = thinking {
            let (badge, next_order, separator_after_thinking) = thinking_badge(
                thinking_text,
                document_order,
                self.selection.first_copy_separator,
                &mut factory,
            );
            document_order = next_order;
            first_content_separator = separator_after_thinking;
            bubble = bubble.child(badge);
        }

        let content_text = rendered_content_text(&self.content, self.is_streaming);

        // @plan:PLAN-20260402-MARKDOWN.P11
        // @plan:PLAN-20260407-ISSUE172.P10 (cached blocks)
        // @requirement:REQ-MD-INTEGRATE-002
        let blocks: Vec<MarkdownBlock> = if self.is_streaming {
            // Streaming: parse fresh since content changes
            parse_markdown_blocks(&content_text)
        } else if let Some(cached) = &self.cached_blocks {
            // Finalized with cache: use cached blocks
            // Dereference Arc<Vec<_>> to &Vec<_>, then clone the Vec
            cached.as_ref().clone()
        } else {
            // No cache available: parse fresh
            parse_markdown_blocks(&content_text)
        };
        let rendered = blocks_to_elements_with_leaf_factory(
            &blocks,
            Theme::text_primary(),
            Theme::assistant_bubble_bg(),
            &mut factory,
            &mut document_order,
            first_content_separator,
        );

        bubble = bubble.child(Theme::assistant_bubble(
            div()
                .w_full()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .rounded(px(Theme::RADIUS_LG))
                .children(rendered),
        ));

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
    fn test_streaming_cursor_only_during_streaming() {
        assert_eq!(rendered_content_text("Hello", true), "Hello▋");
        assert_eq!(rendered_content_text("Hello", false), "Hello");
    }

    #[test]
    fn thinking_leaf_excludes_label_chrome_and_uses_entry_document_order() {
        let leaf = thinking_leaf(
            "chain of thought",
            7,
            "

",
        );

        assert_eq!(leaf.plain_text.as_ref(), "chain of thought");
        assert_eq!(leaf.document_order, 7);
        assert_eq!(
            leaf.copy_separator_before.as_ref(),
            "

"
        );
        assert!(leaf.links.is_empty());
        assert_eq!(
            leaf.text_runs.iter().map(|run| run.len).sum::<usize>(),
            "chain of thought".len(),
            "run lengths must cover the exact leaf text"
        );
        assert!(!leaf.plain_text.contains("Thinking"));
    }
}
