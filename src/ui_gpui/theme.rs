//! GPUI runtime theme engine.
//!
//! Provides runtime-backed token accessors that resolve colors from the active
//! theme slug stored in thread-safe global state.  The active slug is set via
//! [`set_active_theme_slug`] and read via [`active_theme_slug`].
//! Unknown slugs fall back to the "green-screen" palette.
//!
//! @plan ISSUE12.P02
//! @plan ISSUE12.P04

use std::sync::{OnceLock, RwLock};

use gpui::{hsla, rgb, FontFeatures, Hsla, Rgba, SharedString};

use crate::ui_gpui::mac_native::{self, MacNativePalette, MAC_NATIVE_SLUG};
use crate::ui_gpui::theme_catalog::{ThemeCatalog, ThemeColors, ThemeDefinition, ThemeKind};

// ── Default slug constant ────────────────────────────────────────────────────

const DEFAULT_SLUG: &str = "green-screen";
const DEFAULT_THEME_NAME: &str = "Green Screen";

// ── Font size and family constants ───────────────────────────────────────────

/// Default font size in points used when no persisted value is found.
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

/// Minimum allowed font size in points.
pub const MIN_FONT_SIZE: f32 = 10.0;

/// Maximum allowed font size in points.
pub const MAX_FONT_SIZE: f32 = 24.0;

/// Default monospace font family name.
pub const DEFAULT_MONO_FONT_FAMILY: &str = "Menlo";

/// Setting key for persisting font size.
pub const SETTING_KEY_FONT_SIZE: &str = "font_size";

/// Setting key for persisting the UI (proportional) font family.
pub const SETTING_KEY_UI_FONT_FAMILY: &str = "ui_font_family";

/// Setting key for persisting the monospace font family.
pub const SETTING_KEY_MONO_FONT_FAMILY: &str = "mono_font_family";

/// Setting key for persisting the mono-ligatures toggle.
pub const SETTING_KEY_MONO_LIGATURES: &str = "mono_ligatures";

// ── Global active theme slug ─────────────────────────────────────────────────

static ACTIVE_THEME_SLUG: RwLock<String> = RwLock::new(String::new());

// ── Global active font settings ───────────────────────────────────────────────

static ACTIVE_FONT_SIZE: RwLock<f32> = RwLock::new(DEFAULT_FONT_SIZE);
static ACTIVE_UI_FONT_FAMILY: RwLock<String> = RwLock::new(String::new());
static ACTIVE_MONO_FONT_FAMILY: RwLock<String> = RwLock::new(String::new());
static ACTIVE_MONO_LIGATURES: RwLock<bool> = RwLock::new(true);

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
/// | Legacy value | Canonical slug    | Reason                                         |
/// |--------------|-------------------|------------------------------------------------|
/// | `"dark"`     | `"green-screen"`  | Old name for the bundled dark behavior         |
/// | `"light"`    | `"default-light"` | Legacy light alias for the light bundled theme |
/// | `"auto"`     | `"mac-native"`    | Old name for the OS-appearance mode            |
///
/// Any other value is returned unchanged.  Callers (startup, tests) are
/// responsible for applying the result to [`set_active_theme_slug`].
#[must_use]
pub fn migrate_legacy_theme_slug(slug: &str) -> &str {
    match slug {
        "dark" => "green-screen",
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
/// to the same value. Unknown slugs are stored as-is; accessors will fall back
/// to the "green-screen" palette when the catalog contains no entry for the
/// stored slug.
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

/// Returns the currently active theme slug (defaults to `"green-screen"`).
#[must_use]
pub fn active_theme_slug() -> String {
    get_active_slug()
}

// ── Public font runtime API ───────────────────────────────────────────────────

/// Set the active font size, clamping to `[MIN_FONT_SIZE, MAX_FONT_SIZE]`.
///
/// Returns `true` if the value changed, `false` if it was already set to the
/// clamped value.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
pub fn set_active_font_size(size: f32) -> bool {
    let clamped = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    let mut guard = ACTIVE_FONT_SIZE.write().expect("font size rwlock poisoned");
    if (*guard - clamped).abs() < f32::EPSILON {
        return false;
    }
    *guard = clamped;
    true
}

/// Returns the currently active font size, defaulting to `DEFAULT_FONT_SIZE`.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
#[must_use]
pub fn active_font_size() -> f32 {
    let guard = ACTIVE_FONT_SIZE.read().expect("font size rwlock poisoned");
    *guard
}

/// Set the active UI (proportional) font family override.
///
/// Passing `None` or `Some(String::new())` clears the override, falling back
/// to the GPUI default.  Returns `true` if the stored value changed.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
pub fn set_active_ui_font_family(family: Option<String>) -> bool {
    let value = family.unwrap_or_default();
    let mut guard = ACTIVE_UI_FONT_FAMILY
        .write()
        .expect("ui font family rwlock poisoned");
    if *guard == value {
        return false;
    }
    *guard = value;
    true
}

/// Returns the active UI font family override, or `None` if none is set.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
#[must_use]
pub fn active_ui_font_family() -> Option<String> {
    let guard = ACTIVE_UI_FONT_FAMILY
        .read()
        .expect("ui font family rwlock poisoned");
    if guard.is_empty() {
        None
    } else {
        Some(guard.clone())
    }
}

/// Set the active monospace font family.
///
/// An empty string is stored as-is; [`active_mono_font_family`] will fall back
/// to `DEFAULT_MONO_FONT_FAMILY` when the stored value is empty.  Returns
/// `true` if the stored value changed.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
pub fn set_active_mono_font_family(family: impl AsRef<str>) -> bool {
    let value = family.as_ref().to_string();
    let mut guard = ACTIVE_MONO_FONT_FAMILY
        .write()
        .expect("mono font family rwlock poisoned");
    if *guard == value {
        return false;
    }
    *guard = value;
    true
}

/// Returns the active monospace font family, falling back to
/// `DEFAULT_MONO_FONT_FAMILY` when no override is set.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
#[must_use]
pub fn active_mono_font_family() -> String {
    let guard = ACTIVE_MONO_FONT_FAMILY
        .read()
        .expect("mono font family rwlock poisoned");
    if guard.is_empty() {
        DEFAULT_MONO_FONT_FAMILY.to_string()
    } else {
        guard.clone()
    }
}

/// Set whether monospace ligatures are enabled.
///
/// Returns `true` if the stored value changed.
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
pub fn set_active_mono_ligatures(enabled: bool) -> bool {
    let mut guard = ACTIVE_MONO_LIGATURES
        .write()
        .expect("mono ligatures rwlock poisoned");
    if *guard == enabled {
        return false;
    }
    *guard = enabled;
    true
}

/// Returns whether monospace ligatures are currently enabled (default: `true`).
///
/// # Panics
///
/// Panics if the internal `RwLock` is poisoned.
#[must_use]
pub fn active_mono_ligatures() -> bool {
    *ACTIVE_MONO_LIGATURES
        .read()
        .expect("mono ligatures rwlock poisoned")
}

/// Returns `true` when `slug` maps to a known selectable theme option.
///
/// This includes all bundled catalog slugs and the synthetic `mac-native`
/// option.
#[must_use]
pub fn is_valid_theme_slug(slug: &str) -> bool {
    if slug == MAC_NATIVE_SLUG {
        return true;
    }

    catalog_cache().map_or_else(
        || slug == DEFAULT_SLUG,
        |catalog| catalog.get(slug).is_some(),
    )
}

/// Returns theme metadata for all bundled catalog themes plus `mac-native`.
///
/// The synthetic `mac-native` pseudo-entry is appended at the end.
/// Falls back to the degraded default entry plus `mac-native` on catalog load failure.
#[must_use]
pub fn available_theme_options() -> Vec<ThemeOption> {
    let mac_native_entry = ThemeOption {
        name: mac_native::MAC_NATIVE_NAME.to_string(),
        slug: MAC_NATIVE_SLUG.to_string(),
        kind: crate::ui_gpui::theme_catalog::ThemeKind::System,
    };

    let degraded_default_entry = ThemeOption {
        name: DEFAULT_THEME_NAME.to_string(),
        slug: DEFAULT_SLUG.to_string(),
        kind: ThemeKind::Dark,
    };

    let mut options = catalog_cache().map_or_else(
        || vec![degraded_default_entry],
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

fn catalog_cache() -> Option<&'static ThemeCatalog> {
    static CATALOG_CACHE: OnceLock<Option<ThemeCatalog>> = OnceLock::new();
    CATALOG_CACHE
        .get_or_init(|| ThemeCatalog::load_bundled().ok())
        .as_ref()
}

fn load_active_theme_for_slug(slug: &str) -> Option<ThemeDefinition> {
    let catalog = catalog_cache()?;
    // mac-native is synthetic and not stored in the catalog; skip catalog lookup.
    if slug == MAC_NATIVE_SLUG {
        return catalog.get(DEFAULT_SLUG).cloned();
    }
    catalog
        .get(slug)
        .or_else(|| catalog.get(DEFAULT_SLUG))
        .cloned()
}

fn with_active_colors_for_slug<F, T>(slug: &str, f: F) -> Option<T>
where
    F: FnOnce(&ThemeColors) -> T,
{
    load_active_theme_for_slug(slug).map(|def| f(&def.colors))
}

/// Attempt to resolve the mac-native palette, falling back to `None` if
/// `AppKit` is unavailable or the active slug is not mac-native.
fn try_mac_native_palette_for_slug(slug: &str) -> Option<MacNativePalette> {
    if slug == MAC_NATIVE_SLUG {
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

fn parse_hex3(hex: &str) -> Option<(f32, f32, f32)> {
    let h = hex.trim().strip_prefix('#').unwrap_or_else(|| hex.trim());
    if h.len() != 3 {
        return None;
    }

    let mut chars = h.chars();
    let r = chars.next()?.to_digit(16)?;
    let g = chars.next()?.to_digit(16)?;
    let b = chars.next()?.to_digit(16)?;

    // Expand #RGB to #RRGGBB by multiplying each nibble by 17.
    #[allow(clippy::cast_precision_loss)]
    Some((
        (r * 17) as f32 / 255.0,
        (g * 17) as f32 / 255.0,
        (b * 17) as f32 / 255.0,
    ))
}

fn parse_named_color(name: &str) -> Option<(f32, f32, f32)> {
    let mapped_hex = match name.trim().to_ascii_lowercase().as_str() {
        "black" => "#000000",
        "white" => "#ffffff",
        "gray" | "grey" => "#808080",
        "blue" => "#0000ff",
        "bluebright" => "#5555ff",
        "red" => "#ff0000",
        "green" => "#00ff00",
        "yellow" => "#ffff00",
        "magenta" => "#ff00ff",
        "orange" => "#ffa500",
        "purple" => "#800080",
        _ => return None,
    };

    parse_hex6(mapped_hex)
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

fn parse_color_token(token: &str) -> Option<(f32, f32, f32)> {
    parse_hex6(token)
        .or_else(|| parse_hex3(token))
        .or_else(|| parse_named_color(token))
}

fn hex_str_to_hsla(hex: &str) -> Option<Hsla> {
    let (r, g, b) = parse_color_token(hex)?;
    Some(rgb_to_hsla(r, g, b, 1.0))
}

// ── Fallback palette: hard-coded green-screen colors for zero-catalog situations ──
// These values mirror assets/themes/green-screen.json and are used ONLY when
// the catalog itself cannot be loaded (e.g., missing assets directory in a
// test build). Normal runtime always loads from the catalog.

mod fallback {
    pub const BG: (f32, f32, f32) = (0.0, 0.0, 0.0); // #000000
    pub const TEXT_PRIMARY: (f32, f32, f32) = (0.416, 0.600, 0.333); // #6a9955
    pub const TEXT_MUTED: (f32, f32, f32) = (0.416, 0.600, 0.333); // #6a9955
    pub const ACCENT_PRIMARY: (f32, f32, f32) = (0.416, 0.600, 0.333); // #6a9955
    pub const ACCENT_FG: (f32, f32, f32) = (0.0, 0.0, 0.0); // #000000
    pub const ACCENT_ERROR: (f32, f32, f32) = (0.416, 0.600, 0.333); // #6a9955
    pub const ERROR_FG: (f32, f32, f32) = (0.0, 0.0, 0.0); // #000000
    pub const ACCENT_WARNING: (f32, f32, f32) = (0.416, 0.600, 0.333); // #6a9955
    pub const ACCENT_SUCCESS: (f32, f32, f32) = (0.0, 1.0, 0.0); // #00ff00
    pub const BORDER: (f32, f32, f32) = (0.416, 0.600, 0.333); // #6a9955
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
/// back to the default fallback palette on unknown slugs or catalog errors.
pub struct Theme;

impl Theme {
    // ── Public conversion helpers ────────────────────────────────────────────

    /// Parse a theme color token and convert it to `Hsla`.
    ///
    /// Supports 6-digit hex (`#RRGGBB`), 3-digit hex (`#RGB`), and a small
    /// named-color set used by the bundled llxprt themes (`white`, `black`,
    /// `gray`/`grey`, `blue`, `bluebright`, `red`, `green`, `yellow`,
    /// `magenta`, `orange`, `purple`).
    ///
    /// # Errors
    ///
    /// Returns an error string if `hex` is not a recognized color token.
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

    /// Resolve a color token with optional mac-native override.
    ///
    /// If the active slug is `mac-native` and `AppKit` color resolution succeeds,
    /// `mac_select` is applied to the resolved palette and that color is
    /// returned immediately. Otherwise resolves from the active catalog theme and
    /// falls back to the hard-coded `fb` triplet when parsing/catalog access fails.
    fn resolve_with_mac_native(
        mac_select: impl FnOnce(&MacNativePalette) -> Hsla,
        catalog_select: impl FnOnce(&ThemeColors) -> &str,
        fb: (f32, f32, f32),
    ) -> Hsla {
        let slug = get_active_slug();
        if let Some(palette) = try_mac_native_palette_for_slug(&slug) {
            return mac_select(&palette);
        }
        with_active_colors_for_slug(&slug, |colors| hex_str_to_hsla(catalog_select(colors)))
            .flatten()
            .unwrap_or_else(|| rgb_to_hsla(fb.0, fb.1, fb.2, 1.0))
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

    // ── Scaled font size accessors (dynamic, respects active font size) ──────

    /// Heading 1 font size (2× base).
    #[must_use]
    pub fn font_size_h1() -> f32 {
        active_font_size() * 2.0
    }

    /// Heading 2 font size (1.5× base).
    #[must_use]
    pub fn font_size_h2() -> f32 {
        active_font_size() * 1.5
    }

    /// Heading 3 font size (1.25× base).
    #[must_use]
    pub fn font_size_h3() -> f32 {
        active_font_size() * 1.25
    }

    /// Body font size (equals base).
    #[must_use]
    pub fn font_size_body() -> f32 {
        active_font_size()
    }

    /// Monospace font size (9/10 of base, truncated to nearest 0.5pt).
    #[must_use]
    pub fn font_size_mono() -> f32 {
        (active_font_size() * 9.0) / 10.0
    }

    /// UI label font size (17/20 of base).
    #[must_use]
    pub fn font_size_ui() -> f32 {
        (active_font_size() * 17.0) / 20.0
    }

    /// Small label font size (39/50 of base).
    #[must_use]
    pub fn font_size_small() -> f32 {
        (active_font_size() * 39.0) / 50.0
    }

    /// Returns the active monospace font family as a GPUI shared string.
    #[must_use]
    pub fn mono_font_family() -> SharedString {
        SharedString::from(active_mono_font_family())
    }

    /// Returns the active UI font family override as a GPUI shared string.
    ///
    /// When no override is set, returns `None` so GPUI falls back to
    /// `.SystemUIFont`.
    #[must_use]
    pub fn ui_font_family() -> Option<SharedString> {
        active_ui_font_family().map(SharedString::from)
    }

    /// Returns the active mono font OpenType features.
    ///
    /// Ligatures are enabled by default; when disabled, this sets `calt=0`.
    #[must_use]
    pub fn mono_font_features() -> FontFeatures {
        if active_mono_ligatures() {
            FontFeatures::default()
        } else {
            FontFeatures::disable_ligatures()
        }
    }

    // ── Font family / features convenience methods ───────────────────────────

    /// Returns the active monospace font family name.
    #[must_use]
    pub fn mono_font_family_name() -> String {
        active_mono_font_family()
    }

    /// Returns the active UI (proportional) font family override, or `None`.
    #[must_use]
    pub fn ui_font_family_name() -> Option<String> {
        active_ui_font_family()
    }

    /// Returns whether monospace ligatures are currently enabled.
    #[must_use]
    pub fn mono_ligatures_enabled() -> bool {
        active_mono_ligatures()
    }

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

    /// Selection background (`colors.selection.bg`).
    #[must_use]
    pub fn selection_bg() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.accent_primary,
            |c| c.selection.bg.as_str(),
            fallback::ACCENT_PRIMARY,
        )
    }

    /// Selection foreground (`colors.selection.fg`).
    #[must_use]
    pub fn selection_fg() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.text_primary,
            |c| c.selection.fg.as_str(),
            fallback::TEXT_PRIMARY,
        )
    }

    /// Accent button foreground.
    ///
    /// Uses `colors.selection.fg` for catalog-backed themes and a dedicated
    /// high-contrast fallback when the catalog is unavailable.
    #[must_use]
    pub fn accent_fg() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.text_primary,
            |c| c.selection.fg.as_str(),
            fallback::ACCENT_FG,
        )
    }

    /// Error button foreground.
    ///
    /// Uses `colors.selection.fg` for catalog-backed themes and a dedicated
    /// high-contrast fallback when the catalog is unavailable.
    #[must_use]
    pub fn error_fg() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.text_primary,
            |c| c.selection.fg.as_str(),
            fallback::ERROR_FG,
        )
    }

    /// User message bubble background – uses `colors.message.userBorder`.
    #[must_use]
    pub fn user_bubble_bg() -> Hsla {
        Self::resolve_with_mac_native(
            |p| p.user_bubble,
            |c| c.message.user_border.as_str(),
            fallback::ACCENT_PRIMARY,
        )
    }

    /// User message bubble foreground (`colors.selection.fg`) for high contrast.
    #[must_use]
    pub fn user_bubble_text() -> Hsla {
        Self::selection_fg()
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
