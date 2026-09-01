//! NOTICE: Vendored from the `gpui-base` crate in
//! <https://github.com/longbridge/gpui-component> at commit `c5ade48`.
//! Licensed under Apache-2.0. Copyright 2024-2026 Longbridge.
//!
//! This is a vendored and MODIFIED copy. `PersonalAgent`'s modifications are:
//! - adjust crate-relative module paths for this repository;
//! - rewrite Rust 2024 let chains for this project's Rust 2021 edition;
//! - route auto-scroll through participant callbacks because pinned GPUI's
//!   `Window::dispatch_event` returns a private type;
//! - remove the unrelated `TextViewState` registry from `GlobalState`;
//! - gate out upstream tests that require newer GPUI test-support APIs;
//! - allow dead code while this compile-only spike remains unwired;
//! - add the upstream selectable-text participant with explicit surface colors,
//!   selected-glyph recoloring, scroll offsets, and copy separators;
//! - mark the source modules to skip rustfmt so the root package's
//!   `cargo fmt --all` preserves upstream formatting;
//! - add Apache-2.0 section 4(b) modification notices to changed files.

#![allow(dead_code)]
#![allow(clippy::cloned_ref_to_slice_refs)]
#![allow(clippy::filter_map_bool_then)]
#![allow(clippy::type_complexity)]

#[rustfmt::skip]
mod auto_scroll;
#[rustfmt::skip]
mod global_state;
mod selectable_text;
#[rustfmt::skip]
mod text_boundary;
#[rustfmt::skip]
mod text_selection;

pub use auto_scroll::AutoScroll;
pub use global_state::{DeferredPopover, GlobalState};
pub use selectable_text::SelectableText;
pub use text_selection::{
    AutoScrollLease, TextSelection, TextSelectionContentKey, TextSelectionCoverage,
    TextSelectionEndpoint, TextSelectionEvent, TextSelectionHandle, TextSelectionLayer,
    TextSelectionProjection, TextSelectionRegistration, TextSelectionRun, TextSelectionScopeId,
    TextSelectionSnapshot, TextSelectionWindowPoints,
};
