//! GPUI theme color constants
//!
//! @plan PLAN-20250130-GPUIREDUX.P03

use gpui::{hsla, rgb, Hsla, Rgba};

/// Dark theme color constants for GPUI
/// 
/// All color values are extracted from src/ui/theme.rs to ensure
/// visual parity between AppKit and GPUI implementations.
pub struct Theme;

impl Theme {
    // RGB values from existing theme (as f32 for GPUI)
    // These MUST match src/ui/theme.rs exactly
    pub const BG_DARKEST: (f32, f32, f32) = (0.051, 0.051, 0.051); // #0d0d0d - main background
    pub const BG_DARKER: (f32, f32, f32) = (0.102, 0.102, 0.102); // #1a1a1a - input background
    pub const BG_DARK: (f32, f32, f32) = (0.141, 0.141, 0.141); // #242424 - message bubbles
    pub const BG_BASE: (f32, f32, f32) = (0.071, 0.071, 0.071); // #121212 - chat area background
    pub const TEXT_PRIMARY: (f32, f32, f32) = (0.898, 0.898, 0.898); // #e5e5e5 - main text
    pub const TEXT_SECONDARY: (f32, f32, f32) = (0.533, 0.533, 0.533); // #888888 - secondary text
    pub const TEXT_MUTED: (f32, f32, f32) = (0.333, 0.333, 0.333); // #555555 - muted text

    // Additional UI colors
    pub const ACCENT: (f32, f32, f32) = (0.0, 0.478, 0.882); // #007ae1 - primary accent
    pub const ACCENT_HOVER: (f32, f32, f32) = (0.0, 0.549, 1.0); // #008cff - hover state
    pub const BORDER: (f32, f32, f32) = (0.2, 0.2, 0.2); // #333333 - borders
    pub const USER_BUBBLE_BG: (f32, f32, f32) = (0.141, 0.141, 0.141); // #242424
    pub const ASSISTANT_BUBBLE_BG: (f32, f32, f32) = (0.102, 0.102, 0.102); // #1a1a1a
    pub const ERROR: (f32, f32, f32) = (0.937, 0.263, 0.263); // #ef4343 - errors
    pub const WARNING: (f32, f32, f32) = (1.0, 0.769, 0.0); // #ffc400 - warnings
    
    // Phase 03: Chat view specific colors
    pub const USER_BUBBLE: u32 = 0x2a4a2a; // Green tint for user messages
    pub const THINKING_BG: u32 = 0x1a1a2a;  // Blue tint for thinking blocks
    pub const DANGER: u32 = 0x4a2a2a;       // Red tint for stop/danger

    // Spacing constants (in pixels)
    pub const SPACING_XS: f32 = 4.0;
    pub const SPACING_SM: f32 = 8.0;
    pub const SPACING_MD: f32 = 12.0;
    pub const SPACING_LG: f32 = 16.0;
    pub const SPACING_XL: f32 = 24.0;

    // Border radius
    pub const RADIUS_SM: f32 = 4.0;
    pub const RADIUS_MD: f32 = 6.0;
    pub const RADIUS_LG: f32 = 8.0;

    // Font sizes
    pub const FONT_SIZE_XS: f32 = 11.0;
    pub const FONT_SIZE_SM: f32 = 12.0;
    pub const FONT_SIZE_MD: f32 = 13.0;
    pub const FONT_SIZE_BASE: f32 = 14.0;
    pub const FONT_SIZE_LG: f32 = 16.0;

    // === Background Colors ===
    
    pub fn bg_darkest() -> Hsla {
        hsla(Self::BG_DARKEST.0, Self::BG_DARKEST.1, Self::BG_DARKEST.2, 1.0)
    }

    pub fn bg_darker() -> Hsla {
        hsla(Self::BG_DARKER.0, Self::BG_DARKER.1, Self::BG_DARKER.2, 1.0)
    }

    pub fn bg_dark() -> Hsla {
        hsla(Self::BG_DARK.0, Self::BG_DARK.1, Self::BG_DARK.2, 1.0)
    }

    // === Text Colors ===
    
    pub fn text_primary() -> Hsla {
        hsla(Self::TEXT_PRIMARY.0, Self::TEXT_PRIMARY.1, Self::TEXT_PRIMARY.2, 1.0)
    }

    pub fn text_secondary() -> Hsla {
        hsla(Self::TEXT_SECONDARY.0, Self::TEXT_SECONDARY.1, Self::TEXT_SECONDARY.2, 1.0)
    }

    pub fn text_muted() -> Hsla {
        hsla(Self::TEXT_MUTED.0, Self::TEXT_MUTED.1, Self::TEXT_MUTED.2, 1.0)
    }

    // === Accent Colors ===
    
    pub fn accent() -> Hsla {
        hsla(Self::ACCENT.0, Self::ACCENT.1, Self::ACCENT.2, 1.0)
    }

    pub fn accent_hover() -> Hsla {
        hsla(Self::ACCENT_HOVER.0, Self::ACCENT_HOVER.1, Self::ACCENT_HOVER.2, 1.0)
    }

    // === UI Colors ===
    
    pub fn border() -> Hsla {
        hsla(Self::BORDER.0, Self::BORDER.1, Self::BORDER.2, 1.0)
    }

    pub fn user_bubble_bg() -> Hsla {
        hsla(Self::USER_BUBBLE_BG.0, Self::USER_BUBBLE_BG.1, Self::USER_BUBBLE_BG.2, 1.0)
    }

    pub fn assistant_bubble_bg() -> Hsla {
        hsla(Self::ASSISTANT_BUBBLE_BG.0, Self::ASSISTANT_BUBBLE_BG.1, Self::ASSISTANT_BUBBLE_BG.2, 1.0)
    }

    pub fn error() -> Hsla {
        hsla(Self::ERROR.0, Self::ERROR.1, Self::ERROR.2, 1.0)
    }

    pub fn warning() -> Hsla {
        hsla(Self::WARNING.0, Self::WARNING.1, Self::WARNING.2, 1.0)
    }

    // === Phase 03: Chat View Colors ===
    
    /// Chat area background (#121212)
    pub fn bg_base() -> Hsla {
        hsla(Self::BG_BASE.0, Self::BG_BASE.1, Self::BG_BASE.2, 1.0)
    }
    
    /// User message bubble - green tint (#2a4a2a)
    pub fn user_bubble() -> Rgba {
        rgb(Self::USER_BUBBLE)
    }
    
    /// Assistant message bubble - same as bg_darker
    pub fn assistant_bubble() -> Hsla {
        Self::bg_darker()
    }
    
    /// Thinking block background - blue tint (#1a1a2a)
    pub fn thinking_bg() -> Rgba {
        rgb(Self::THINKING_BG)
    }
    
    /// Danger/stop button background - red tint (#4a2a2a)
    pub fn danger() -> Rgba {
        rgb(Self::DANGER)
    }

    /// Success/running status - green (#4a9f4a)
    /// @plan PLAN-20250130-GPUIREDUX.P06
    pub fn success() -> Hsla {
        hsla(0.33, 0.4, 0.46, 1.0) // Green
    }
}
