//! macOS-native appearance resolver for the `mac-native` pseudo-theme.
//!
//! On macOS builds, this module reads semantic system colors from `AppKit`
//! (`NSColor`) and detects light/dark mode via `NSAppearance`.  On non-macOS
//! targets the entire public API compiles to no-ops that return `None`,
//! allowing callers to fall through to the deterministic default-theme
//! fallback path.
//!
//! @plan ISSUE12.P04

use gpui::Hsla;

// ── Slug constant ─────────────────────────────────────────────────────────────

/// The slug used to identify the mac-native pseudo-theme everywhere in the
/// codebase.
pub const MAC_NATIVE_SLUG: &str = "mac-native";

/// Human-readable display name shown in the settings dropdown.
pub const MAC_NATIVE_NAME: &str = "Mac Native";

// ── Resolved palette ─────────────────────────────────────────────────────────

/// A fully-resolved set of GPUI color tokens derived from macOS system colors.
///
/// All fields are `Hsla` values ready for direct use by `Theme` accessor
/// methods.  The struct intentionally mirrors the token names used in the JSON
/// theme catalog (`ThemeColors`) so the mapping in `theme.rs` stays obvious.
#[derive(Debug, Clone)]
pub struct MacNativePalette {
    pub background: Hsla,
    pub panel_bg: Hsla,
    pub input_bg: Hsla,
    pub panel_header_bg: Hsla,
    pub text_primary: Hsla,
    pub text_muted: Hsla,
    pub accent_primary: Hsla,
    pub accent_secondary: Hsla,
    pub accent_error: Hsla,
    pub accent_warning: Hsla,
    pub accent_success: Hsla,
    pub border: Hsla,
    pub user_bubble: Hsla,
}

// ── macOS implementation ──────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use super::MacNativePalette;

    use gpui::{hsla, Hsla};
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSColor, NSColorType,
    };
    use objc2_foundation::NSArray;

    /// Returns `true` if the current drawing appearance is dark (Dark Aqua).
    ///
    /// Uses `NSAppearance.currentDrawingAppearance` and probes with
    /// `bestMatchFromAppearancesWithNames:` – the canonical dark-mode detection
    /// idiom recommended by Apple.
    #[allow(unsafe_code)]
    fn is_dark_appearance() -> bool {
        let appearance = NSAppearance::currentDrawingAppearance();

        // SAFETY: NSAppearanceNameAqua and NSAppearanceNameDarkAqua are valid
        // &'static NSString constants provided by the AppKit framework.
        // Rust 2024 requires an explicit unsafe block for `extern static` access.
        let (aqua, dark_aqua) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };

        // Build the candidate list from the static references.
        let names = NSArray::<objc2_foundation::NSString>::from_slice(&[aqua, dark_aqua]);

        appearance
            .bestMatchFromAppearancesWithNames(&names)
            .is_some_and(|matched| {
                // Disambiguate the `AsRef` impls by casting to &NSString explicitly.
                let matched_ns: &objc2_foundation::NSString = &matched;
                matched_ns == dark_aqua
            })
    }

    /// Extract (r, g, b) channel values in the sRGB color space from an
    /// `NSColor`.  Returns `None` if the conversion fails or the color
    /// object does not support component-based RGB.
    #[allow(unsafe_code)]
    fn nscolor_to_rgb(color: &NSColor) -> Option<(f32, f32, f32)> {
        // Convert to a component-based color so getRed:green:blue:alpha:
        // is guaranteed to succeed even for semantic/catalog colors.
        let component = color.colorUsingType(NSColorType::ComponentBased)?;

        // CGFloat is f64 on 64-bit macOS; we use f64 directly to avoid
        // importing objc2-core-foundation just for the type alias.
        let mut r: f64 = 0.0;
        let mut g: f64 = 0.0;
        let mut b: f64 = 0.0;
        let mut a: f64 = 0.0;

        // SAFETY: pointers are local stack variables initialized to 0.0;
        // the method writes valid f64 values into them.
        unsafe {
            component.getRed_green_blue_alpha(&raw mut r, &raw mut g, &raw mut b, &raw mut a);
        }

        // Truncate f64 -> f32 for Hsla; precision loss is negligible for
        // display colors and is intentional.
        #[allow(clippy::cast_possible_truncation)]
        Some((r as f32, g as f32, b as f32))
    }

    /// Convert (r, g, b) in 0..=1 to GPUI `Hsla`.
    fn rgb_to_hsla(r: f32, g: f32, b: f32) -> Hsla {
        let max = r.max(g.max(b));
        let min = r.min(g.min(b));
        let lightness = (max + min) * 0.5;
        let delta = max - min;

        if delta <= f32::EPSILON {
            return hsla(0.0, 0.0, lightness, 1.0);
        }

        let saturation = delta / (1.0 - 2.0f32.mul_add(lightness, -1.0).abs());

        let mut hue = if (max - r).abs() <= f32::EPSILON {
            ((g - b) / delta) % 6.0
        } else if (max - g).abs() <= f32::EPSILON {
            ((b - r) / delta) + 2.0
        } else {
            ((r - g) / delta) + 4.0
        } / 6.0;

        if hue < 0.0 {
            hue += 1.0;
        }

        hsla(hue, saturation, lightness, 1.0)
    }

    /// Read an `NSColor` system semantic color and convert to `Hsla`.
    /// Returns `None` on any conversion failure.
    fn resolve(color: &NSColor) -> Option<Hsla> {
        let (r, g, b) = nscolor_to_rgb(color)?;
        Some(rgb_to_hsla(r, g, b))
    }

    /// Resolve the full mac-native palette from the current macOS appearance.
    ///
    /// Returns `None` if any critical color resolution step fails.  The caller
    /// is expected to fall back to the default catalog theme on `None`.
    ///
    /// # Panics
    ///
    /// Does not panic; all fallible operations return `None` which is propagated
    /// to the caller.
    pub fn resolve_palette() -> Option<MacNativePalette> {
        // Individual resolution failures are propagated via `?`.
        // If any single system color is unavailable we fall back to default
        // rather than returning a partial palette.
        let background = resolve(&NSColor::windowBackgroundColor())?;
        let panel_bg = resolve(&NSColor::controlBackgroundColor())?;

        // Use a slightly different shade for input areas in dark mode.
        let is_dark = is_dark_appearance();
        let input_bg = if is_dark {
            // Slightly lighter than panel in dark mode
            let bg = resolve(&NSColor::controlBackgroundColor())?;
            hsla(bg.h, bg.s, (bg.l + 0.04).min(1.0), 1.0)
        } else {
            // Slightly lighter than panel in light mode
            let bg = resolve(&NSColor::windowBackgroundColor())?;
            hsla(bg.h, bg.s, (bg.l + 0.04).min(1.0), 1.0)
        };
        // Keep header/background aligned with the same semantic system surface
        // so panel chrome remains consistent across light/dark appearances.
        let panel_header_bg = resolve(&NSColor::controlBackgroundColor())?;
        let text_primary = resolve(&NSColor::labelColor())?;
        let text_muted = resolve(&NSColor::secondaryLabelColor())?;
        let accent_primary = resolve(&NSColor::controlAccentColor())?;
        let accent_secondary = {
            // A slightly lighter/darker version of the accent for hover states.
            let acc = resolve(&NSColor::controlAccentColor())?;
            if is_dark {
                hsla(acc.h, acc.s, (acc.l + 0.08).min(1.0), 1.0)
            } else {
                hsla(acc.h, acc.s, (acc.l - 0.08).max(0.0), 1.0)
            }
        };
        let accent_error = resolve(&NSColor::systemRedColor())?;
        let accent_warning = resolve(&NSColor::systemOrangeColor())?;
        let accent_success = resolve(&NSColor::systemGreenColor())?;
        let border = resolve(&NSColor::separatorColor())?;
        let user_bubble = resolve(&NSColor::selectedContentBackgroundColor())?;

        Some(MacNativePalette {
            background,
            panel_bg,
            input_bg,
            panel_header_bg,
            text_primary,
            text_muted,
            accent_primary,
            accent_secondary,
            accent_error,
            accent_warning,
            accent_success,
            border,
            user_bubble,
        })
    }
}

// ── Non-macOS stub ────────────────────────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::MacNativePalette;

    /// On non-macOS platforms there are no system colors to read.  Always
    /// returns `None` so callers fall back to the default catalog theme.
    pub const fn resolve_palette() -> Option<MacNativePalette> {
        None
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Attempt to resolve a [`MacNativePalette`] from the current macOS appearance.
///
/// On non-macOS targets or if `AppKit` color resolution fails, returns `None`.
/// Callers **must** treat `None` as a signal to fall back to the default
/// catalog theme; there is no partial fallback within this module.
#[allow(clippy::missing_const_for_fn)]
#[must_use]
pub fn resolve_palette() -> Option<MacNativePalette> {
    platform::resolve_palette()
}
