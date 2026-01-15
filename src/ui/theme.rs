//! Dark theme color definitions and common UI components

use objc2::rc::Retained;
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSColor, NSStackView};
use objc2_foundation::NSObjectProtocol;

/// Dark theme colors matching the UI mockup
pub struct Theme;

// =============================================================================
// FlippedStackView - A NSStackView subclass with flipped coordinates
// =============================================================================
// 
// macOS uses a non-flipped coordinate system by default (origin at bottom-left).
// This causes content in scroll views to appear at the BOTTOM and scroll position
// defaults to showing the bottom of content.
//
// By overriding isFlipped to return true, the coordinate system becomes:
// - Origin at TOP-LEFT
// - Y increases DOWNWARD
// - Content appears at TOP
// - scrollPoint(0,0) shows the TOP
//
// This matches iOS behavior and is more intuitive for most UI layouts.
// =============================================================================

pub struct FlippedStackViewIvars;

define_class!(
    #[unsafe(super(NSStackView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FlippedStackView"]
    #[ivars = FlippedStackViewIvars]
    pub struct FlippedStackView;

    unsafe impl NSObjectProtocol for FlippedStackView {}

    impl FlippedStackView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl FlippedStackView {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(FlippedStackViewIvars);
        unsafe { msg_send![super(this), init] }
    }
}

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
