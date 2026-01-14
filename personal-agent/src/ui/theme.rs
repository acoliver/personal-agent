//! Dark theme color definitions

use objc2::rc::Retained;
use objc2_app_kit::NSColor;

/// Dark theme colors matching the UI mockup
pub struct Theme;

impl Theme {
    // RGB values normalized to 0.0-1.0
    pub const BG_DARKEST: (f64, f64, f64) = (0.051, 0.051, 0.051); // #0d0d0d - main background
    pub const BG_DARKER: (f64, f64, f64) = (0.102, 0.102, 0.102); // #1a1a1a - input background
    pub const BG_DARK: (f64, f64, f64) = (0.141, 0.141, 0.141); // #242424 - message bubbles
    pub const TEXT_PRIMARY: (f64, f64, f64) = (0.898, 0.898, 0.898); // #e5e5e5 - main text
    pub const TEXT_SECONDARY: (f64, f64, f64) = (0.533, 0.533, 0.533); // #888888 - secondary text
    pub const TEXT_MUTED: (f64, f64, f64) = (0.333, 0.333, 0.333); // #555555 - muted text

    pub fn bg_darker() -> Retained<NSColor> {
        NSColor::colorWithCalibratedRed_green_blue_alpha(
            Self::BG_DARKER.0,
            Self::BG_DARKER.1,
            Self::BG_DARKER.2,
            1.0,
        )
    }

    pub fn text_primary() -> Retained<NSColor> {
        NSColor::colorWithCalibratedRed_green_blue_alpha(
            Self::TEXT_PRIMARY.0,
            Self::TEXT_PRIMARY.1,
            Self::TEXT_PRIMARY.2,
            1.0,
        )
    }

    pub fn text_secondary_color() -> Retained<NSColor> {
        NSColor::colorWithCalibratedRed_green_blue_alpha(
            Self::TEXT_MUTED.0,
            Self::TEXT_MUTED.1,
            Self::TEXT_MUTED.2,
            1.0,
        )
    }
}
