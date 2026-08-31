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
//! - add Apache-2.0 section 4(b) modification notices to changed files.

#![allow(dead_code)]

mod auto_scroll;
mod global_state;
mod text_boundary;
mod text_selection;

pub use auto_scroll::AutoScroll;
pub use global_state::{DeferredPopover, GlobalState};
pub use text_selection::{
    TextSelection, TextSelectionContentKey, TextSelectionCoverage, TextSelectionEndpoint,
    TextSelectionEvent, TextSelectionHandle, TextSelectionLayer, TextSelectionProjection,
    TextSelectionRegistration, TextSelectionRun, TextSelectionScopeId, TextSelectionSnapshot,
    TextSelectionWindowPoints,
};
