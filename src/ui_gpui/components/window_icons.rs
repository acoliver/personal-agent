//! Window mode icons: popout, pop-in, and sidebar toggle.
//!
//! Renders SVG icons via GPUI's `svg()` element, loading the embedded
//! assets registered through [`crate::ui_gpui::app_assets::AppAssets`].

use gpui::{prelude::*, px, svg, Svg};

const POPOUT_SVG_PATH: &str = "icons/popout.svg";
const POPIN_SVG_PATH: &str = "icons/popin.svg";
const SIDEBAR_SVG_PATH: &str = "icons/sidebar.svg";

/// Pop-out icon (window with outward arrow).
#[must_use]
pub fn popout_icon(size_px: f32) -> Svg {
    svg().path(POPOUT_SVG_PATH).size(px(size_px))
}

/// Pop-in icon (window with inward arrow).
#[must_use]
pub fn popin_icon(size_px: f32) -> Svg {
    svg().path(POPIN_SVG_PATH).size(px(size_px))
}

/// Sidebar toggle icon (window with left panel and list rows).
#[must_use]
pub fn sidebar_icon(size_px: f32) -> Svg {
    svg().path(SIDEBAR_SVG_PATH).size(px(size_px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_paths_are_non_empty() {
        assert!(!POPOUT_SVG_PATH.is_empty());
        assert!(!POPIN_SVG_PATH.is_empty());
        assert!(!SIDEBAR_SVG_PATH.is_empty());
    }

    #[test]
    fn icon_builders_construct_svg_elements() {
        let _ = popout_icon(16.0);
        let _ = popin_icon(16.0);
        let _ = sidebar_icon(16.0);
    }
}
