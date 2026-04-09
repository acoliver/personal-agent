//! Copy action icons for message bubble footer controls.
//!
//! Renders SVG icons via GPUI's `svg()` element, loading the embedded
//! assets registered through [`crate::ui_gpui::app_assets::AppAssets`].

use gpui::{prelude::*, px, svg, Svg};

const COPY_SVG_PATH: &str = "icons/copy.svg";
const CHECK_SVG_PATH: &str = "icons/check.svg";

/// Clipboard copy icon.
#[must_use]
pub fn copy_icon(size_px: f32) -> Svg {
    svg().path(COPY_SVG_PATH).size(px(size_px))
}

/// Success check icon used after a copy action.
#[must_use]
pub fn check_icon(size_px: f32) -> Svg {
    svg().path(CHECK_SVG_PATH).size(px(size_px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_paths_are_non_empty() {
        assert!(!COPY_SVG_PATH.is_empty());
        assert!(!CHECK_SVG_PATH.is_empty());
    }

    #[test]
    fn icon_builders_construct_svg_elements() {
        let _ = copy_icon(16.0);
        let _ = check_icon(16.0);
    }
}
