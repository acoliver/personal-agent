//! Selectable markdown leaves used by the chat transcript.

use super::markdown_content::{MarkdownLeaf, MarkdownLeafFactory};
use gpui::{IntoElement, Pixels, Point, SharedString};
use gpui_selection_vendor::SelectableText;

#[derive(Clone, Copy)]
pub struct TranscriptSelectionContext {
    pub scroll_offset: Point<Pixels>,
    pub document_order: u64,
    pub first_copy_separator: &'static str,
}

pub struct TranscriptSelectionLeafFactory {
    scroll_offset: Point<Pixels>,
}

impl TranscriptSelectionLeafFactory {
    pub const fn new(scroll_offset: Point<Pixels>) -> Self {
        Self { scroll_offset }
    }
}

impl MarkdownLeafFactory for TranscriptSelectionLeafFactory {
    fn create_leaf(&mut self, leaf: MarkdownLeaf) -> gpui::AnyElement {
        let id = SharedString::from(format!("selection-leaf-{}", leaf.document_order));
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
        .into_any_element()
    }
}
