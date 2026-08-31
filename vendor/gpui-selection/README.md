# Vendored GPUI selection engine

This package contains the text-selection engine from the `gpui-base` crate in [longbridge/gpui-component](https://github.com/longbridge/gpui-component) at commit `c5ade48`. The upstream code and this package are licensed under Apache-2.0. See `LICENSE-APACHE` and the notices in each source file.

PersonalAgent made these changes to the upstream source:

- adjusted crate-relative module paths for this repository;
- rewrote Rust 2024 let chains for Rust 2021;
- routed auto-scroll through participant callbacks because the pinned GPUI revision exposes a private return type from `Window::dispatch_event`;
- removed the unrelated `TextViewState` registry from `GlobalState`;
- gated tests that require newer GPUI test-support APIs;
- allowed dead code while the package remains unwired;
- added upstream's selectable-text participant with explicit surface colors, selected-glyph recoloring, scroll offsets, and participant-defined copy separators;
- marked the vendored source modules to skip rustfmt so `cargo fmt --all` preserves upstream formatting; and
- added Apache-2.0 section 4(b) modification notices to changed files.

## Re-syncing

Fetch the `gpui-base` selection sources from the intended upstream commit, compare them with `src/`, and copy in the upstream changes without formatting or splitting the files. Reapply the modifications listed above, preserve `LICENSE-APACHE`, the crate-level origin notice, and each per-file modification notice, then run the root repository's verification gates. Update the commit recorded in this README and the source notices only after the comparison is complete.
