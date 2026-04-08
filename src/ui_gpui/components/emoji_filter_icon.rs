//! Emoji filter toggle icons.
//!
//! Renders SVG icons via GPUI's `svg()` element, loading the embedded
//! assets registered through [`crate::ui_gpui::app_assets::AppAssets`].

use gpui::{prelude::*, px, svg, Svg};

const SMILE_SVG_PATH: &str = "icons/smile.svg";
const SMILE_X_SVG_PATH: &str = "icons/smile-x.svg";

/// Smiley face icon (emoji filter enabled).
#[must_use]
pub fn smile_icon(size_px: f32) -> Svg {
    svg().path(SMILE_SVG_PATH).size(px(size_px))
}

/// Smiley face with X overlay (emoji filter disabled).
#[must_use]
pub fn smile_x_icon(size_px: f32) -> Svg {
    svg().path(SMILE_X_SVG_PATH).size(px(size_px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_paths_are_non_empty() {
        assert!(!SMILE_SVG_PATH.is_empty());
        assert!(!SMILE_X_SVG_PATH.is_empty());
    }

    #[test]
    fn icon_builders_construct_svg_elements() {
        let _ = smile_icon(16.0);
        let _ = smile_x_icon(16.0);
    }
}
