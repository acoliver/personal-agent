//! Leaf metadata: pairs each rendered inline-text leaf with its document byte
//! range and the actual `TextLayout` handle shared with the painted `StyledText`
//! child.
//!
//! A [`LeafRegistry`] is an `Rc<RefCell<...>>` collection that
//! Rich text leaves register
//! into during layout. After the rich child is laid out and painted, the
//! [`SelectableMarkdown`](super::selectable_markdown::SelectableMarkdown)
//! element reads the collected leaves for hit-testing and selection-quad
//! painting.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 2

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui::TextLayout;

/// Metadata for a single rendered inline-text leaf.
///
/// `doc_range` is a byte range into the visible-document text. `layout` is the
/// live `TextLayout` handle shared with the actual painted `StyledText` child.
///
/// @plan PLAN-20260713-ISSUE151 Phase 2
#[derive(Clone)]
pub struct LeafMeta {
    /// Byte range into the visible-document text covered by this leaf.
    pub doc_range: Range<usize>,
    /// Live layout handle shared with the painted `StyledText` child.
    pub layout: TextLayout,
}

/// A shared, mutable collection of [`LeafMeta`] populated during the rich
/// child's layout phase.
///
/// Each frame clears and re-populates the collection, so the layouts always
/// reflect the current-frame painted geometry.
///
/// @plan PLAN-20260713-ISSUE151 Phase 2
#[derive(Clone, Default)]
pub struct LeafRegistry {
    inner: Rc<RefCell<Vec<LeafMeta>>>,
}

impl LeafRegistry {
    /// Clear all leaves. Called at the start of each frame's layout phase.
    pub fn clear(&self) {
        self.inner.borrow_mut().clear();
    }

    /// Register a leaf. Called by each `SelectableText` during layout.
    pub fn register(&self, meta: LeafMeta) {
        self.inner.borrow_mut().push(meta);
    }

    /// Return a snapshot of all registered leaves.
    #[must_use]
    pub fn leaves(&self) -> Vec<LeafMeta> {
        self.inner.borrow().clone()
    }
}
