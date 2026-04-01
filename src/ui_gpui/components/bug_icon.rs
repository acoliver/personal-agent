//! Bug icon component for the error log indicator.
//!
//! Renders a beetle SVG via GPUI's `svg()` element, loading the embedded
//! `icons/bug.svg` asset registered through [`crate::ui_gpui::app_assets::AppAssets`].

use gpui::{prelude::*, px, svg, Svg};

/// Asset path for the embedded bug SVG icon.
const BUG_SVG_PATH: &str = "icons/bug.svg";

/// Create a bug icon SVG element at the given pixel size.
///
/// The SVG inherits `text_color` from its parent for fill/stroke coloring.
#[must_use]
pub fn bug_icon(size_px: f32) -> Svg {
    svg().path(BUG_SVG_PATH).size(px(size_px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bug_svg_path_is_non_empty() {
        assert!(!BUG_SVG_PATH.is_empty());
    }

    #[test]
    fn bug_svg_path_ends_with_svg_extension() {
        assert!(std::path::Path::new(BUG_SVG_PATH)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg")));
    }
}
