//! Selectable markdown leaves used by the chat transcript.

use super::markdown_content::{MarkdownLeaf, MarkdownLeafFactory};
use gpui::{IntoElement, Pixels, Point, SharedString};
use gpui_selection_vendor::{SelectableText, TextSelectionContentKey};

#[derive(Clone, Copy)]
pub struct TranscriptSelectionContext {
    pub scroll_offset: Point<Pixels>,
    pub document_order: u64,
    pub first_copy_separator: &'static str,
    pub content_key: TextSelectionContentKey,
}

pub struct TranscriptSelectionLeafFactory {
    scroll_offset: Point<Pixels>,
    content_key: TextSelectionContentKey,
}

impl TranscriptSelectionLeafFactory {
    pub const fn new(scroll_offset: Point<Pixels>, content_key: TextSelectionContentKey) -> Self {
        Self {
            scroll_offset,
            content_key,
        }
    }
}

impl MarkdownLeafFactory for TranscriptSelectionLeafFactory {
    fn create_leaf(&mut self, leaf: MarkdownLeaf) -> gpui::AnyElement {
        let id = SharedString::from(format!(
            "selection-leaf-{}-{}",
            self.content_key.value(),
            leaf.document_order
        ));
        SelectableText::new(
            id,
            leaf.plain_text,
            leaf.text_runs,
            leaf.surface_background,
            leaf.surface_foreground,
        )
        .links(leaf.links)
        .document_order(leaf.document_order)
        .scroll_offset(self.scroll_offset)
        .copy_separator_before(leaf.copy_separator_before)
        .content_key(self.content_key)
        .into_any_element()
    }
}
