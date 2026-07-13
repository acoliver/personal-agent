//! Per-message visible-document and selection model.
//!
//! This module is a **pure** data layer: it has no GPUI dependency and no
//! geometry. It derives a canonical visible document containing plain text,
//! link ranges, and semantic block ranges from the existing [`MarkdownBlock`]
//! IR. It also provides UTF-8-safe selection/word/block helpers and a
//! message-revision freshness token.
//!
//! The visible document is generated from the same IR as the real rendered
//! tree so that selection and copy always operate on the exact text the user
//! sees. Copy slices this document, never raw Markdown.
//!
//! @plan PLAN-20260713-ISSUE151 Phase 1
//! @requirement REQ-151-007

#![allow(
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::module_name_repetitions,
    clippy::redundant_pub_crate,
    clippy::use_self
)]

use std::ops::Range;

mod builder;
mod helpers;
mod revision;
mod selection;

pub use builder::{SemanticBlock, VisibleDocument};
pub use helpers::{clamp_to_char_boundary, word_range_at};
pub use revision::MessageRevision;
pub use selection::Selection;
#[cfg(test)]
pub use selection::SelectionMode;

/// A visible-document range paired with a link destination.
///
/// `range` is a byte range into the document's `text()`. The byte offsets are
/// always UTF-8 character boundaries.
///
/// @plan PLAN-20260713-ISSUE151 Phase 1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRange {
    /// Byte range into the visible document text.
    pub range: Range<usize>,
    /// Link destination URL.
    pub url: String,
}

/// Byte range covered by one actual rendered text leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLeaf {
    /// Byte range into the visible document text.
    pub range: Range<usize>,
}

/// Character used to separate cells within a rendered table row.
const CELL_SEPARATOR: char = '\t';

#[cfg(test)]
mod visible_document_tests;
