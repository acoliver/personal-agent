use personal_agent::ui_gpui::mac_native::{MAC_NATIVE_NAME, MAC_NATIVE_SLUG};
use personal_agent::ui_gpui::theme::{
    active_theme_slug, available_theme_options, set_active_theme_slug, Theme,
};
use personal_agent::ui_gpui::theme_catalog::ThemeCatalog;

type TestResult = Result<(), Box<dyn std::error::Error>>;

// Safety: tests that mutate global theme state are serialized with a mutex so
// they don't interfere with each other. ThemeSwitchGuard restores the previous
// slug on drop, even during unwinding.
use std::sync::{Mutex, MutexGuard};

static THEME_SWITCH_LOCK: Mutex<()> = Mutex::new(());

struct ThemeSwitchGuard {
    _lock: MutexGuard<'static, ()>,
    prev_slug: String,
}

impl ThemeSwitchGuard {
    fn acquire() -> Self {
        let lock = THEME_SWITCH_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let prev_slug = active_theme_slug();
        Self {
            _lock: lock,
            prev_slug,
        }
    }
}

impl Drop for ThemeSwitchGuard {
    fn drop(&mut self) {
        set_active_theme_slug(&self.prev_slug);
    }
}

/// Float comparison for GPUI Hsla channel values.
/// Both values come from the same deterministic hex-to-hsla conversion path, so
/// exact equality holds.  We use a tiny epsilon to be robust against any
/// floating-point reordering while still catching genuine mismatches.
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

fn colors_differ(a: gpui::Hsla, b: gpui::Hsla) -> bool {
    !approx_eq(a.h, b.h) || !approx_eq(a.s, b.s) || !approx_eq(a.l, b.l) || !approx_eq(a.a, b.a)
}

// ── existing baseline test (kept intact) ────────────────────────────────────

#[test]
fn static_theme_primary_text_matches_green_screen_theme_file() -> TestResult {
    let catalog = ThemeCatalog::load_bundled()?;
    let green = catalog
        .get("green-screen")
        .expect("green-screen theme is required");

    assert_eq!(green.colors.text.primary.to_ascii_lowercase(), "#6a9955");

    let _guard = ThemeSwitchGuard::acquire();
    set_active_theme_slug("green-screen");

    let from_theme = Theme::text_primary();
    let expected = Theme::rgb_color(0x006A_9955);

    assert!(
        approx_eq(from_theme.h, expected.h),
        "hue mismatch: {} vs {}",
        from_theme.h,
        expected.h
    );
    assert!(
        approx_eq(from_theme.s, expected.s),
        "saturation mismatch: {} vs {}",
        from_theme.s,
        expected.s
    );
    assert!(
        approx_eq(from_theme.l, expected.l),
        "lightness mismatch: {} vs {}",
        from_theme.l,
        expected.l
    );

    Ok(())
}

// ── Phase 02: runtime slug switching changes returned colors ─────────────────

#[test]
fn switching_slug_changes_text_primary_color() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("default");
    let default_primary = Theme::text_primary();

    set_active_theme_slug("green-screen");
    let green_primary = Theme::text_primary();

    // default (#e5e7eb) and green-screen (#6a9955) have very different hue/lightness
    assert!(
        !approx_eq(default_primary.l, green_primary.l),
        "switching theme slug must change text_primary lightness"
    );
}

#[test]
fn switching_slug_changes_bg_base_color() -> TestResult {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("default");
    let default_bg = Theme::bg_base();

    set_active_theme_slug("green-screen");
    let green_bg = Theme::bg_base();

    let catalog = ThemeCatalog::load_bundled()?;
    let default_theme = catalog.get("default").expect("default theme must exist");
    let green_theme = catalog
        .get("green-screen")
        .expect("green-screen theme must exist");

    let expected_default_bg = Theme::hex_to_hsla(&default_theme.colors.background)?;
    let expected_green_bg = Theme::hex_to_hsla(&green_theme.colors.background)?;

    assert!(
        approx_eq(default_bg.h, expected_default_bg.h),
        "bg_base hue for default should match catalog"
    );
    assert!(
        approx_eq(default_bg.l, expected_default_bg.l),
        "bg_base lightness for default should match catalog"
    );
    assert!(
        approx_eq(green_bg.h, expected_green_bg.h),
        "bg_base hue for green-screen should match catalog"
    );
    assert!(
        approx_eq(green_bg.l, expected_green_bg.l),
        "bg_base lightness for green-screen should match catalog"
    );

    Ok(())
}

// ── Phase 02: unknown slug falls back to the runtime default slug ────────────

#[test]
fn unknown_slug_falls_back_to_green_screen_deterministically() {
    let _guard = ThemeSwitchGuard::acquire();

    // The key contract: accessors must not panic and must return a valid color.
    set_active_theme_slug("this-slug-does-not-exist");
    let color = Theme::text_primary();
    assert!(
        color.l >= 0.0 && color.l <= 1.0,
        "fallback color lightness must be 0..=1"
    );

    // When slug is unknown, accessor must use the green-screen fallback palette.
    set_active_theme_slug("green-screen");
    let green_primary = Theme::text_primary();

    set_active_theme_slug("this-slug-does-not-exist");
    let unknown_primary = Theme::text_primary();

    assert!(
        approx_eq(green_primary.h, unknown_primary.h)
            && approx_eq(green_primary.s, unknown_primary.s)
            && approx_eq(green_primary.l, unknown_primary.l),
        "unknown slug must fall back to green-screen palette colors"
    );
}

#[test]
fn set_active_theme_slug_returns_true_when_slug_changes() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("default");
    let changed = set_active_theme_slug("green-screen");
    assert!(changed, "changing to a different slug must return true");

    let not_changed = set_active_theme_slug("green-screen");
    assert!(!not_changed, "setting same slug must return false");
}

// ── Phase 02: active_theme_slug() returns current selection ──────────────────

#[test]
fn active_theme_slug_reflects_set_value() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("default");
    assert_eq!(active_theme_slug(), "default");

    set_active_theme_slug("green-screen");
    assert_eq!(active_theme_slug(), "green-screen");
}

// ── Phase 02: available_theme_options returns loaded file-backed themes ───────

#[test]
fn available_theme_options_contains_all_bundled_slugs() -> TestResult {
    let options = available_theme_options();
    let catalog = ThemeCatalog::load_bundled()?;

    assert!(
        options.len() >= catalog.len(),
        "available_theme_options must include all bundled themes (got {}, catalog has {})",
        options.len(),
        catalog.len()
    );

    let option_slugs: Vec<&str> = options.iter().map(|o| o.slug.as_str()).collect();
    for slug in catalog.slugs() {
        assert!(
            option_slugs.contains(&slug),
            "available_theme_options missing bundled slug '{slug}'"
        );
    }

    Ok(())
}

#[test]
fn available_theme_options_has_name_and_slug_for_each_entry() {
    let options = available_theme_options();
    for option in &options {
        assert!(
            !option.name.is_empty(),
            "theme option must have non-empty name"
        );
        assert!(
            !option.slug.is_empty(),
            "theme option must have non-empty slug"
        );
    }
}

// ── Phase 02: green-screen values come from file-backed palette ───────────────

#[test]
fn green_screen_text_primary_is_file_driven() -> TestResult {
    let _guard = ThemeSwitchGuard::acquire();

    let catalog = ThemeCatalog::load_bundled()?;
    let green = catalog
        .get("green-screen")
        .expect("green-screen must exist");

    set_active_theme_slug("green-screen");
    let from_theme = Theme::text_primary();
    let from_file = Theme::hex_to_hsla(&green.colors.text.primary)?;

    assert!(
        approx_eq(from_theme.h, from_file.h),
        "green-screen text_primary hue must come from file (got {}, want {})",
        from_theme.h,
        from_file.h
    );
    assert!(
        approx_eq(from_theme.l, from_file.l),
        "green-screen text_primary lightness must come from file (got {}, want {})",
        from_theme.l,
        from_file.l
    );

    Ok(())
}

#[test]
fn green_screen_background_is_file_driven() -> TestResult {
    let _guard = ThemeSwitchGuard::acquire();

    let catalog = ThemeCatalog::load_bundled()?;
    let green = catalog
        .get("green-screen")
        .expect("green-screen must exist");

    set_active_theme_slug("green-screen");
    let from_theme = Theme::bg_base();
    let from_file = Theme::hex_to_hsla(&green.colors.background)?;

    assert!(
        approx_eq(from_theme.l, from_file.l),
        "green-screen bg_base lightness must come from file (got {}, want {})",
        from_theme.l,
        from_file.l
    );

    Ok(())
}

#[test]
fn green_screen_accent_error_is_file_driven() -> TestResult {
    let _guard = ThemeSwitchGuard::acquire();

    let catalog = ThemeCatalog::load_bundled()?;
    let green = catalog
        .get("green-screen")
        .expect("green-screen must exist");

    set_active_theme_slug("green-screen");
    let from_theme = Theme::error();
    let from_file = Theme::hex_to_hsla(&green.colors.accent.error)?;

    assert!(
        approx_eq(from_theme.h, from_file.h),
        "green-screen error hue must come from file (got {}, want {})",
        from_theme.h,
        from_file.h
    );
    assert!(
        approx_eq(from_theme.l, from_file.l),
        "green-screen error lightness must come from file (got {}, want {})",
        from_theme.l,
        from_file.l
    );

    Ok(())
}

#[test]
fn green_screen_affirmative_and_selected_foregrounds_stay_distinct() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("green-screen");

    assert!(
        colors_differ(Theme::selection_bg(), Theme::selection_fg()),
        "selection foreground must differ from selection background in green-screen"
    );
    assert!(
        colors_differ(Theme::accent(), Theme::accent_fg()),
        "accent foreground must differ from accent background in green-screen"
    );
    assert!(
        colors_differ(Theme::success(), Theme::selection_fg()),
        "success status color should remain visually distinct from selected foreground in green-screen"
    );
}

#[test]
fn green_screen_selected_and_affirmative_foregrounds_are_black() -> TestResult {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("green-screen");

    let expected_black = Theme::hex_to_hsla("#000000")?;

    for (label, actual) in [
        ("selection_fg", Theme::selection_fg()),
        ("accent_fg", Theme::accent_fg()),
        ("error_fg", Theme::error_fg()),
        ("user_bubble_text", Theme::user_bubble_text()),
    ] {
        assert!(
            approx_eq(actual.h, expected_black.h)
                && approx_eq(actual.s, expected_black.s)
                && approx_eq(actual.l, expected_black.l)
                && approx_eq(actual.a, expected_black.a),
            "{label} must resolve to black in the green-screen theme"
        );
    }

    Ok(())
}

// ── Phase 02: accent/border/status tokens are runtime-backed ──────────────────

#[test]
fn switching_to_default_changes_accent_color_from_green_screen() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("green-screen");
    let gs_accent = Theme::accent();

    set_active_theme_slug("default");
    let def_accent = Theme::accent();

    assert!(
        !approx_eq(gs_accent.h, def_accent.h) || !approx_eq(gs_accent.l, def_accent.l),
        "accent color must differ between green-screen and default themes"
    );
}

// ── Phase 04: mac-native pseudo-theme ────────────────────────────────────────

#[test]
fn mac_native_slug_appears_in_available_theme_options() {
    let options = available_theme_options();
    let slugs: Vec<&str> = options.iter().map(|o| o.slug.as_str()).collect();
    assert!(
        slugs.contains(&MAC_NATIVE_SLUG),
        "available_theme_options must contain '{MAC_NATIVE_SLUG}'; got: {slugs:?}"
    );
}

#[test]
fn mac_native_entry_has_expected_name() {
    let options = available_theme_options();
    let entry = options
        .iter()
        .find(|o| o.slug == MAC_NATIVE_SLUG)
        .expect("mac-native must appear in available_theme_options");
    assert_eq!(
        entry.name, MAC_NATIVE_NAME,
        "mac-native display name must be '{MAC_NATIVE_NAME}'"
    );
}

#[test]
fn accent_and_error_foregrounds_are_distinct_in_catalog_fallback() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("totally-unknown-slug-99");

    let accent = Theme::accent();
    let accent_fg = Theme::accent_fg();
    assert!(
        !approx_eq(accent.h, accent_fg.h)
            || !approx_eq(accent.s, accent_fg.s)
            || !approx_eq(accent.l, accent_fg.l),
        "accent foreground should differ from accent background in fallback mode"
    );

    let error = Theme::error();
    let error_fg = Theme::error_fg();
    assert!(
        !approx_eq(error.h, error_fg.h)
            || !approx_eq(error.s, error_fg.s)
            || !approx_eq(error.l, error_fg.l),
        "error foreground should differ from error background in fallback mode"
    );
}

#[test]
fn mac_native_constants_match_expected_values() {
    // Contract: slug and display name constants are stable and used by tests and UI.
    assert_eq!(MAC_NATIVE_SLUG, "mac-native");
    assert_eq!(MAC_NATIVE_NAME, "Mac Native");
}

#[test]
fn setting_mac_native_slug_is_accepted_and_reflected() {
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug(MAC_NATIVE_SLUG);
    assert_eq!(
        active_theme_slug(),
        MAC_NATIVE_SLUG,
        "active_theme_slug must reflect mac-native after set"
    );
}

#[test]
fn mac_native_active_colors_are_valid_hsla() {
    // When mac-native is active, every Theme accessor must return a valid Hsla
    // (not panic, not return NaN).  On macOS the values come from AppKit; on
    // non-macOS or when resolution fails they fall through to the default theme.
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug(MAC_NATIVE_SLUG);

    #[allow(clippy::type_complexity)]
    let tokens: &[(&str, fn() -> gpui::Hsla)] = &[
        ("bg_base", Theme::bg_base),
        ("bg_darkest", Theme::bg_darkest),
        ("bg_darker", Theme::bg_darker),
        ("bg_dark", Theme::bg_dark),
        ("text_primary", Theme::text_primary),
        ("text_secondary", Theme::text_secondary),
        ("text_muted", Theme::text_muted),
        ("accent", Theme::accent),
        ("accent_hover", Theme::accent_hover),
        ("border", Theme::border),
        ("error", Theme::error),
        ("warning", Theme::warning),
        ("success", Theme::success),
    ];

    for (name, accessor) in tokens {
        let color = accessor();
        assert!(
            color.h >= 0.0 && color.h <= 1.0,
            "{name}: hue {:.4} out of range [0, 1]",
            color.h
        );
        assert!(
            color.s >= 0.0 && color.s <= 1.0,
            "{name}: saturation {:.4} out of range [0, 1]",
            color.s
        );
        assert!(
            color.l >= 0.0 && color.l <= 1.0,
            "{name}: lightness {:.4} out of range [0, 1]",
            color.l
        );
        assert!(
            color.a >= 0.0 && color.a <= 1.0,
            "{name}: alpha {:.4} out of range [0, 1]",
            color.a
        );
    }
}

#[test]
fn mac_native_colors_are_deterministic() {
    // On non-macOS targets (or when AppKit resolution fails), mac-native must
    // produce the same palette as the green-screen fallback theme — not panic,
    // not return garbage, and always be consistent on repeated calls.
    //
    // On macOS, AppKit colors are used and may differ from green-screen; but
    // they must still be reproducible on repeated calls (no non-determinism).
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug(MAC_NATIVE_SLUG);
    let first_call_text = Theme::text_primary();
    let second_call_text = Theme::text_primary();

    // Same active slug → same color on repeated calls.
    assert!(
        approx_eq(first_call_text.h, second_call_text.h)
            && approx_eq(first_call_text.s, second_call_text.s)
            && approx_eq(first_call_text.l, second_call_text.l),
        "mac-native colors must be deterministic (repeated calls must match)"
    );

    #[cfg(not(target_os = "macos"))]
    {
        set_active_theme_slug("green-screen");
        let green_primary = Theme::text_primary();

        assert!(
            approx_eq(first_call_text.h, green_primary.h)
                && approx_eq(first_call_text.s, green_primary.s)
                && approx_eq(first_call_text.l, green_primary.l),
            "non-mac mac-native fallback must match green-screen palette"
        );
    }
}

#[test]
fn mac_native_available_options_count_exceeds_catalog_count() {
    // available_theme_options must include all file-backed catalog themes PLUS
    // the mac-native pseudo-entry.
    let options = available_theme_options();
    let catalog = ThemeCatalog::load_bundled().expect("bundled catalog must load");

    assert!(
        options.len() > catalog.len(),
        "available_theme_options ({}) must have MORE entries than catalog ({}) because mac-native is synthetic",
        options.len(),
        catalog.len()
    );
}

#[test]
fn unknown_slug_fallback_not_affected_by_mac_native_presence() {
    // Confirms that adding mac-native did not break the unknown-slug-fallback
    // behavior that was tested in Phase 02.
    let _guard = ThemeSwitchGuard::acquire();

    set_active_theme_slug("green-screen");
    let green_primary = Theme::text_primary();

    set_active_theme_slug("totally-unknown-slug-99");
    let unknown_primary = Theme::text_primary();

    assert!(
        approx_eq(green_primary.h, unknown_primary.h)
            && approx_eq(green_primary.s, unknown_primary.s)
            && approx_eq(green_primary.l, unknown_primary.l),
        "unknown slug must still fall back to green-screen palette even after mac-native added"
    );
}
