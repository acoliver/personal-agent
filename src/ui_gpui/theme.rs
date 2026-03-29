//! GPUI runtime theme engine.
//!
//! Provides runtime-backed token accessors that resolve colors from the active
//! theme slug stored in thread-safe global state.  The active slug is set via
//! [`set_active_theme_slug`] and read via [`active_theme_slug`].
//! Unknown slugs fall back to the "default" palette.
//!
//! @plan ISSUE12.P02
//! @plan ISSUE12.P04

use std::sync::RwLock;

use gpui::{hsla, rgb, Hsla, Rgba};

use crate::ui_gpui::mac_native::{self, MacNativePalette, MAC_NATIVE_SLUG};
use crate::ui_gpui::theme_catalog::{ThemeCatalog, ThemeColors, ThemeDefinition};

// ── Default slug constant ────────────────────────────────────────────────────

const DEFAULT_SLUG: &str = "default";

// ── Global active theme slug ─────────────────────────────────────────────────

static ACTIVE_THEME_SLUG: RwLock<String> = RwLock::new(String::new());

fn get_active_slug() -> String {
    let guard = ACTIVE_THEME_SLUG
        .read()
        .expect("theme slug rwlock poisoned");
    if guard.is_empty() {
        DEFAULT_SLUG.to_string()
    } else {
        guard.clone()
    }
}

// ── Legacy slug migration ────────────────────────────────────────────────────

/// Map a legacy theme slug to its canonical modern equivalent.
///
/// Older app versions stored one of these three values in `app_settings.json`:
///
/// | Legacy value | Canonical slug    | Reason                               |
/// |--------------|-------------------|--------------------------------------|
/// | `"dark"`     | `"default"`       | Old name for the bundled dark theme  |
/// | `"light"`    | `"default-light"` | Old name for the bundled light theme |
/// | `"auto"`     | `"mac-native"`    | Old name for the OS-appearance mode  |
///
/// Any other value is returned unchanged.  Callers (startup, tests) are
/// responsible for applying the result to [`set_active_theme_slug`].
#[must_use]
pub fn migrate_legacy_theme_slug(slug: &str) -> &str {
    match slug {
        "dark" => "default",
        "light" => "default-light",
        "auto" => "mac-native",
        other => other,
    }
}

// ── Public runtime API ───────────────────────────────────────────────────────

/// A theme entry returned by [`available_theme_options`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeOption {
    pub name: String,
    pub slug: String,
    pub kind: crate::ui_gpui::theme_catalog::ThemeKind,
}

/// Set the active theme slug used by all [`Theme`] color accessors.
///
/// Returns `true` if the slug actually changed, `false` if it was already set
/// to the same value.  Unknown slugs are stored as-is; accessors will fall back
/// to the "default" palette when the catalog contains no entry for the stored
/// slug.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned (only possible if another thread
/// panicked while holding a write lock).
pub fn set_active_theme_slug(slug: &str) -> bool {
    let mut guard = ACTIVE_THEME_SLUG
        .write()
        .expect("theme slug rwlock poisoned");
    let current = if guard.is_empty() {
        DEFAULT_SLUG
    } else {
        guard.as_str()
    };
    if current == slug {
        return false;
    }
    *guard = slug.to_string();
    true
}

/// Returns the currently active theme slug (defaults to `"default"`).
#[must_use]
pub fn active_theme_slug() -> String {
    get_active_slug()
}

/// Returns theme metadata for all bundled catalog themes plus `mac-native`.
///
/// The synthetic `mac-native` pseudo-entry is appended at the end.
/// Falls back to only the mac-native entry on catalog load failure.
#[must_use]
pub fn available_theme_options() -> Vec<ThemeOption> {
    let mac_native_entry = ThemeOption {
        name: mac_native::MAC_NATIVE_NAME.to_string(),
        slug: MAC_NATIVE_SLUG.to_string(),
        kind: crate::ui_gpui::theme_catalog::ThemeKind::Dark,
    };

    let mut options = ThemeCatalog::load_bundled().map_or_else(
        |_| Vec::new(),
        |catalog| {
            catalog
                .slugs()
                .into_iter()
                .filter_map(|slug| catalog.get(slug))
                .map(|def| ThemeOption {
                    name: def.name.clone(),
                    slug: def.slug.clone(),
                    kind: def.kind.clone(),
                })
                .collect()
        },
    );

    options.push(mac_native_entry);
    options
}

// ── Catalog helpers ──────────────────────────────────────────────────────────

fn load_active_theme() -> Option<ThemeDefinition> {
    let catalog = ThemeCatalog::load_bundled().ok()?;
    let slug = get_active_slug();
    // mac-native is synthetic and not stored in the catalog; skip catalog lookup.
    if slug == MAC_NATIVE_SLUG {
        return catalog.get(DEFAULT_SLUG).cloned();
    }
    catalog
        .get(&slug)
        .or_else(|| catalog.get(DEFAULT_SLUG))
        .cloned()
}

fn with_active_colors<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&ThemeColors) -> T,
{
    load_active_theme().map(|def| f(&def.colors))
}

/// Attempt to resolve the mac-native palette, falling back to `None` if
/// `AppKit` is unavailable or the active slug is not mac-native.
fn try_mac_native_palette() -> Option<MacNativePalette> {
    if get_active_slug() == MAC_NATIVE_SLUG {
        mac_native::resolve_palette()
    } else {
        None
    }
}

// ── Hex parsing helpers ──────────────────────────────────────────────────────

fn parse_hex6(hex: &str) -> Option<(f32, f32, f32)> {
    let h = hex.trim().strip_prefix('#').unwrap_or_else(|| hex.trim());
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    // Component values 0..=255 – precision loss in u8->f32 is intentional and
    // negligible for display colors.
    #[allow(clippy::cast_precision_loss)]
    Some((
        ((n >> 16) & 0xff) as f32 / 255.0,
        ((n >> 8) & 0xff) as f32 / 255.0,
        (n & 0xff) as f32 / 255.0,
    ))
}

fn rgb_to_hsla(r: f32, g: f32, b: f32, a: f32) -> Hsla {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let lightness = (max + min) * 0.5;
    let delta = max - min;

    if delta <= f32::EPSILON {
        return hsla(0.0, 0.0, lightness, a);
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

    hsla(hue, saturation, lightness, a)
}

fn hex_str_to_hsla(hex: &str) -> Option<Hsla> {
    let (r, g, b) = parse_hex6(hex)?;
    Some(rgb_to_hsla(r, g, b, 1.0))
}

// ── Fallback palette: "default" dark hard-coded for zero-catalog situations ──
// These values match the default.json theme file and are used ONLY when the
// catalog itself cannot be loaded (e.g., missing assets directory in a test
// build).  Normal runtime always loads from the catalog.

mod fallback {
    pub const BG: (f32, f32, f32) = (0.118, 0.118, 0.180); // #1E1E2E
    pub const TEXT_PRIMARY: (f32, f32, f32) = (0.898, 0.906, 0.922); // #e5e7eb
    pub const TEXT_MUTED: (f32, f32, f32) = (0.424, 0.439, 0.525); // #6C7086
    pub const ACCENT_PRIMARY: (f32, f32, f32) = (0.537, 0.706, 0.980); // #89B4FA
    pub const ACCENT_ERROR: (f32, f32, f32) = (0.953, 0.545, 0.659); // #F38BA8
    pub const ACCENT_WARNING: (f32, f32, f32) = (0.976, 0.886, 0.686); // #F9E2AF
    pub const ACCENT_SUCCESS: (f32, f32, f32) = (0.651, 0.890, 0.631); // #A6E3A1
    pub const BORDER: (f32, f32, f32) = (0.424, 0.439, 0.525); // #6C7086
}

// ── Hsla -> approximate u8 RGB (for Rgba callers) ───────────────────────────

fn hsla_to_rgb_bytes(color: Hsla) -> (u8, u8, u8) {
    let hue = color.h * 360.0;
    let sat = color.s;
    let lig = color.l;

    let chroma = (1.0 - 2.0f32.mul_add(lig, -1.0).abs()) * sat;
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = lig - chroma / 2.0;

    let (r1, g1, b1) = if hue < 60.0 {
        (chroma, x, 0.0)
    } else if hue < 120.0 {
        (x, chroma, 0.0)
    } else if hue < 180.0 {
        (0.0, chroma, x)
    } else if hue < 240.0 {
        (0.0, x, chroma)
    } else if hue < 300.0 {
        (x, 0.0, chroma)
    } else {
        (chroma, 0.0, x)
    };

    // The `.clamp(0.0, 1.0)` call guarantees the result of `* 255.0` is in
    // 0..=255 before truncation.  Clippy cannot see through the float math, so
    // we allow both related lints here rather than masking the whole function.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let clamp = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    (clamp(r1), clamp(g1), clamp(b1))
}

/// Dark theme color constants for GPUI
///
/// All color methods are runtime-backed: they resolve the active theme slug
/// from global state, look up the palette from the bundled catalog, and fall
/// back to the "default" theme on unknown slugs or catalog errors.
pub struct Theme;

impl Theme {
    // ── Public conversion helpers ────────────────────────────────────────────

    /// Parse a 6-digit hex color string (with or without leading `#`) and
    /// convert it to `Hsla`.
    ///
    /// # Errors
    ///
    /// Returns an error string if `hex` is not a valid 6-digit hex color.
    pub fn hex_to_hsla(hex: &str) -> Result<Hsla, String> {
        hex_str_to_hsla(hex).ok_or_else(|| format!("invalid hex color: {hex}"))
    }

    /// Convert a packed 24-bit RGB integer (`0x00RRGGBB`) to `Hsla`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn rgb_color(hex: u32) -> Hsla {
        let r = ((hex >> 16) & 0xff) as f32 / 255.0;
        let g = ((hex >> 8) & 0xff) as f32 / 255.0;
        let b = (hex & 0xff) as f32 / 255.0;
        rgb_to_hsla(r, g, b, 1.0)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Resolve a color token.
    ///
    /// Resolution order:
    /// 1. If the active slug is `mac-native` and `AppKit` is available, use the
    ///    mac-native palette field selected by `mac_select`.
    /// 2. Otherwise look up the hex string from the catalog via `select` and
    ///    parse it.
    /// 3. Fall back to the hard-coded `fb` RGB triplet if everything else fails.
    fn resolve_hex_with_fallback(
        select: impl FnOnce(&ThemeColors) -> &str,
        fb: (f32, f32, f32),
    ) -> Hsla {
        with_active_colors(|colors| hex_str_to_hsla(select(colors)))
            .flatten()
            .unwrap_or_else(|| rgb_to_hsla(fb.0, fb.1, fb.2, 1.0))
    }

    /// Resolve a color token with optional mac-native override.
    ///
    /// If the active slug is `mac-native` and `AppKit` color resolution succeeds,
    /// `mac_select` is applied to the resolved palette and that color is
    /// returned immediately.  Otherwise falls through to the catalog/fallback
    /// path identical to [`resolve_hex_with_fallback`].
    fn resolve_with_mac_native(
        mac_select: impl FnOnce(&MacNativePalette) -> Hsla,
        catalog_select: impl FnOnce(&ThemeColors) -> &str,
        fb: (f32, f32, f32),
    ) -> Hsla {
        if let Some(palette) = try_mac_native_palette() {
            return mac_select(&palette);
        }
        Self::resolve_hex_with_fallback(catalog_select, fb)
    }

    // ── Spacing, radius, and font-size constants (layout; not theme-dependent) ──

    pub const SPACING_XS: f32 = 4.0;
    pub const SPACING_SM: f32 = 8.0;
    pub const SPACING_MD: f32 = 12.0;
    pub const SPACING_LG: f32 = 16.0;
    pub const SPACING_XL: f32 = 24.0;

    pub const RADIUS_SM: f32 = 4.0;
    pub const RADIUS_MD: f32 = 6.0;
    pub const RADIUS_LG: f32 = 8.0;

    pub const FONT_SIZE_XS: f32 = 11.0;
    pub const FONT_SIZE_SM: f32 = 12.0;
    pub const FONT_SIZE_MD: f32 = 13.0;
    pub const FONT_SIZE_BASE: f32 = 14.0;
    pub const FONT_SIZE_LG: f32 = 16.0;

    // ── Background tokens ────────────────────────────────────────────────────

    /// Main content background (`colors.background`).
    #[must_use]
    pub fn bg_base() -> Hsla {
        Self::resolve_with_mac_native(|p| p.background, |c| c.background.as_str(), fallback::BG)
    }

    /// Panel background (`colors.panel.bg`).
    #[must_use]
    pub fn bg_darkest() -> Hsla {
        Self::resolve_with_mac_native(|p| p.panel_bg, |c| c.panel.bg.as_str(), fallback::BG)
    }

    /// Input area background (`colors.input.bg`).
    #[must_use]
    pub fn bg_darker() -> Hsla {
        Self::resolve_with_mac_native(|p| p.input_bg, |c| c.input.bg.as_str(), fallback::BG)
    }

    /// Panel header background (`colors.panel.headerBg`).
    #[must_use]
    pub fn bg_dark() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.panel_header_bg,
            |c| c.panel.header_bg.as_str(),
            fallback::BG,
        )
    }

    // ── Text tokens ──────────────────────────────────────────────────────────

    /// Primary foreground text (`colors.text.primary`).
    #[must_use]
    pub fn text_primary() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.text_primary,
            |c| c.text.primary.as_str(),
            fallback::TEXT_PRIMARY,
        )
    }

    /// Muted foreground text (`colors.text.muted`).
    #[must_use]
    pub fn text_secondary() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.text_muted,
            |c| c.text.muted.as_str(),
            fallback::TEXT_MUTED,
        )
    }

    /// Same as `text_secondary` – alias for dimmer labels.
    #[must_use]
    pub fn text_muted() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.text_muted,
            |c| c.text.muted.as_str(),
            fallback::TEXT_MUTED,
        )
    }

    // ── Accent tokens ────────────────────────────────────────────────────────

    /// Primary accent color (`colors.accent.primary`).
    #[must_use]
    pub fn accent() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.accent_primary,
            |c| c.accent.primary.as_str(),
            fallback::ACCENT_PRIMARY,
        )
    }

    /// Secondary accent (hover state) – uses `colors.accent.secondary`.
    #[must_use]
    pub fn accent_hover() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.accent_secondary,
            |c| c.accent.secondary.as_str(),
            fallback::ACCENT_PRIMARY,
        )
    }

    // ── UI element tokens ────────────────────────────────────────────────────

    /// Panel/input border (`colors.panel.border`).
    #[must_use]
    pub fn border() -> Hsla {
        Self::resolve_with_mac_native(|p| p.border, |c| c.panel.border.as_str(), fallback::BORDER)
    }

    /// User message bubble background – uses the user border color as a tint.
    #[must_use]
    pub fn user_bubble_bg() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.user_bubble,
            |c| c.message.user_border.as_str(),
            fallback::ACCENT_PRIMARY,
        )
    }

    /// Assistant message bubble background (`colors.input.bg`).
    #[must_use]
    pub fn assistant_bubble_bg() -> Hsla {
        Self::resolve_with_mac_native(|p| p.input_bg, |c| c.input.bg.as_str(), fallback::BG)
    }

    /// Error color (`colors.accent.error`).
    #[must_use]
    pub fn error() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.accent_error,
            |c| c.accent.error.as_str(),
            fallback::ACCENT_ERROR,
        )
    }

    /// Warning color (`colors.accent.warning`).
    #[must_use]
    pub fn warning() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.accent_warning,
            |c| c.accent.warning.as_str(),
            fallback::ACCENT_WARNING,
        )
    }

    /// Success / running status color (`colors.accent.success`).
    #[must_use]
    pub fn success() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.accent_success,
            |c| c.accent.success.as_str(),
            fallback::ACCENT_SUCCESS,
        )
    }

    // ── Phase 03 chat-view tokens ────────────────────────────────────────────

    /// User message bubble (Rgba) – packed from `colors.message.userBorder`.
    #[must_use]
    pub fn user_bubble() -> Rgba {
        let color = Self::resolve_with_mac_native(
            |p| p.user_bubble,
            |c| c.message.user_border.as_str(),
            fallback::ACCENT_PRIMARY,
        );
        let (r, g, b) = hsla_to_rgb_bytes(color);
        rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
    }

    /// Assistant message bubble – same as `bg_darker`.
    #[must_use]
    pub fn assistant_bubble() -> Hsla {
        Self::bg_darker()
    }

    /// Thinking block background – darkened `colors.text.thinking`.
    #[must_use]
    pub fn thinking_bg() -> Rgba {
        // mac-native: use input_bg as a proxy for thinking block background
        let color = Self::resolve_with_mac_native(
            |p| p.input_bg,
            |c| c.text.thinking.as_str(),
            fallback::BG,
        );
        let (r, g, b) = hsla_to_rgb_bytes(color);
        rgb((u32::from(r / 5) << 16) | (u32::from(g / 5) << 8) | u32::from(b / 5))
    }

    /// Danger/stop button background – darkened error color.
    #[must_use]
    pub fn danger() -> Rgba {
        let color = Self::resolve_with_mac_native(
            |p| p.accent_error,
            |c| c.accent.error.as_str(),
            fallback::ACCENT_ERROR,
        );
        let (r, g, b) = hsla_to_rgb_bytes(color);
        rgb((u32::from(r / 5) << 16) | (u32::from(g / 5) << 8) | u32::from(b / 5))
    }
}
