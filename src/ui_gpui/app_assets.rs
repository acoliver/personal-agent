//! Embedded asset source for GPUI SVG icons.

use gpui::AssetSource;
use std::borrow::Cow;

/// Compile-time embedded assets for the `PersonalAgent` GPUI application.
///
/// Uses `include_bytes!` so icons are baked into the binary and require
/// no runtime filesystem access.
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icons/bug.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/bug.svg"
            )))),
            "icons/popout.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/popout.svg"
            )))),
            "icons/popin.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/popin.svg"
            )))),
            "icons/sidebar.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/sidebar.svg"
            )))),
            "icons/smile.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/smile.svg"
            )))),
            "icons/smile-x.svg" => Ok(Some(Cow::Borrowed(include_bytes!(
                "../../assets/icons/smile-x.svg"
            )))),
            _ => Ok(None),
        }
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(vec![])
    }
}
