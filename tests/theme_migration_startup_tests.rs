//! Phase 05 — Integration/migration mapping and regression guards.
//!
//! Tests cover:
//! - Legacy slug migration mapping (`dark`, `light`, `auto` → canonical slugs).
//! - Passthrough for canonical slugs and unknown values.
//! - Startup-restore behaviour: persisted slug applied before first render.
//! - Full select → persist → restore cycle via mock settings service.

use personal_agent::ui_gpui::mac_native::MAC_NATIVE_SLUG;
use personal_agent::ui_gpui::theme::{
    active_theme_slug, available_theme_options, migrate_legacy_theme_slug, set_active_theme_slug,
};

use std::sync::Mutex;

static MIGRATION_SWITCH_LOCK: Mutex<()> = Mutex::new(());

// ── Migration mapping unit tests ─────────────────────────────────────────────

#[test]
fn legacy_dark_maps_to_default() {
    assert_eq!(migrate_legacy_theme_slug("dark"), "default");
}

#[test]
fn legacy_light_maps_to_default_light() {
    assert_eq!(migrate_legacy_theme_slug("light"), "default-light");
}

#[test]
fn legacy_auto_maps_to_mac_native() {
    assert_eq!(migrate_legacy_theme_slug("auto"), MAC_NATIVE_SLUG);
    assert_eq!(migrate_legacy_theme_slug("auto"), "mac-native");
}

#[test]
fn canonical_default_passes_through_unchanged() {
    assert_eq!(migrate_legacy_theme_slug("default"), "default");
}

#[test]
fn canonical_default_light_passes_through_unchanged() {
    assert_eq!(migrate_legacy_theme_slug("default-light"), "default-light");
}

#[test]
fn canonical_mac_native_passes_through_unchanged() {
    assert_eq!(migrate_legacy_theme_slug("mac-native"), "mac-native");
}

#[test]
fn canonical_green_screen_passes_through_unchanged() {
    assert_eq!(migrate_legacy_theme_slug("green-screen"), "green-screen");
}

#[test]
fn canonical_dracula_passes_through_unchanged() {
    assert_eq!(migrate_legacy_theme_slug("dracula"), "dracula");
}

#[test]
fn unknown_slug_passes_through_unchanged() {
    assert_eq!(
        migrate_legacy_theme_slug("completely-unknown-slug"),
        "completely-unknown-slug"
    );
}

#[test]
fn empty_string_passes_through_unchanged() {
    assert_eq!(migrate_legacy_theme_slug(""), "");
}

#[test]
fn migration_is_idempotent_for_all_three_legacy_values() {
    // Applying migration twice must produce the same result as applying once.
    for legacy in &["dark", "light", "auto"] {
        let once = migrate_legacy_theme_slug(legacy);
        let twice = migrate_legacy_theme_slug(once);
        assert_eq!(
            once, twice,
            "migration is not idempotent for '{legacy}': once→'{once}', twice→'{twice}'"
        );
    }
}

// ── Startup-restore tests ─────────────────────────────────────────────────────

/// Simulates the startup path: load saved slug → migrate → apply to theme engine.
/// Confirms the active slug is what was persisted (after migration).
#[test]
fn startup_restore_applies_persisted_slug() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    // Simulate persisting "green-screen" and then restoring it on next launch.
    let persisted = "green-screen";
    let migrated = migrate_legacy_theme_slug(persisted);
    set_active_theme_slug(migrated);

    assert_eq!(
        active_theme_slug(),
        "green-screen",
        "active slug must match the persisted value after startup restore"
    );

    set_active_theme_slug(&prev);
}

/// Simulates the startup path for the legacy `"dark"` value.
#[test]
fn startup_restore_migrates_legacy_dark_to_default() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    // Simulate an old settings file that stored "dark".
    let persisted = "dark";
    let migrated = migrate_legacy_theme_slug(persisted);
    set_active_theme_slug(migrated);

    assert_eq!(
        active_theme_slug(),
        "default",
        "startup with legacy 'dark' must activate the 'default' theme"
    );

    set_active_theme_slug(&prev);
}

/// Simulates the startup path for the legacy `"light"` value.
#[test]
fn startup_restore_migrates_legacy_light_to_default_light() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    let persisted = "light";
    let migrated = migrate_legacy_theme_slug(persisted);
    set_active_theme_slug(migrated);

    assert_eq!(
        active_theme_slug(),
        "default-light",
        "startup with legacy 'light' must activate the 'default-light' theme"
    );

    set_active_theme_slug(&prev);
}

/// Simulates the startup path for the legacy `"auto"` value.
#[test]
fn startup_restore_migrates_legacy_auto_to_mac_native() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    let persisted = "auto";
    let migrated = migrate_legacy_theme_slug(persisted);
    set_active_theme_slug(migrated);

    assert_eq!(
        active_theme_slug(),
        MAC_NATIVE_SLUG,
        "startup with legacy 'auto' must activate the mac-native theme"
    );

    set_active_theme_slug(&prev);
}

/// Simulates the startup path when no theme is persisted (uses default).
#[test]
fn startup_restore_uses_default_when_no_slug_persisted() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    // AppSettingsService returns None → startup falls back to "default"
    let fallback = "default";
    let migrated = migrate_legacy_theme_slug(fallback);
    set_active_theme_slug(migrated);

    assert_eq!(
        active_theme_slug(),
        "default",
        "startup without a persisted slug must activate the 'default' theme"
    );

    set_active_theme_slug(&prev);
}

// ── Select → persist → restore cycle ────────────────────────────────────────

/// Full round-trip: user selects a theme, the slug is stored as-is in
/// settings, and on next startup the slug is migrated (no-op for a canonical
/// slug) and restored to the engine.
#[test]
fn select_persist_restore_cycle_for_canonical_slugs() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    // Simulate user selecting "green-screen" in settings.
    let selected = "green-screen";
    // Presenter calls AppSettingsService::set_theme(selected) — persisted as-is.
    // On next startup AppSettingsService::get_theme() returns selected.
    let restored = migrate_legacy_theme_slug(selected);
    set_active_theme_slug(restored);

    assert_eq!(
        active_theme_slug(),
        selected,
        "canonical slug must survive a full select→persist→restore cycle"
    );

    set_active_theme_slug(&prev);
}

/// Round-trip for all canonical bundled slugs: each must survive a
/// persist→migrate→restore cycle unchanged.
#[test]
fn all_bundled_slugs_survive_migration_round_trip() {
    let _lock = MIGRATION_SWITCH_LOCK.lock().unwrap();
    let prev = active_theme_slug();

    let options = available_theme_options();
    for option in &options {
        let slug = option.slug.as_str();
        let migrated = migrate_legacy_theme_slug(slug);
        // For canonical slugs the migrated value must be unchanged.
        assert_eq!(
            migrated, slug,
            "canonical slug '{slug}' must not be altered by migration"
        );
        // And setting + reading back must be stable.
        set_active_theme_slug(migrated);
        assert_eq!(
            active_theme_slug(),
            migrated,
            "active slug must equal migrated slug for '{slug}'"
        );
    }

    set_active_theme_slug(&prev);
}

// ── Regression guards ────────────────────────────────────────────────────────

/// The three legacy slugs must NOT appear in `available_theme_options`; they are
/// only valid as persisted values from older app versions and are mapped away
/// at startup.
#[test]
fn legacy_slugs_are_not_in_available_theme_options() {
    let options = available_theme_options();
    let slugs: Vec<&str> = options.iter().map(|o| o.slug.as_str()).collect();

    for legacy in &["dark", "light", "auto"] {
        assert!(
            !slugs.contains(legacy),
            "legacy slug '{legacy}' must not appear in available_theme_options — \
             it is a migration alias only"
        );
    }
}

/// After migration all three legacy values must resolve to a slug that IS
/// present in `available_theme_options`.
#[test]
fn migrated_legacy_slugs_are_present_in_available_theme_options() {
    let options = available_theme_options();
    let slugs: Vec<&str> = options.iter().map(|o| o.slug.as_str()).collect();

    let cases = [
        ("dark", "default"),
        ("light", "default-light"),
        ("auto", "mac-native"),
    ];

    for (legacy, expected_canonical) in &cases {
        let migrated = migrate_legacy_theme_slug(legacy);
        assert_eq!(
            migrated, *expected_canonical,
            "legacy '{legacy}' should migrate to '{expected_canonical}'"
        );
        assert!(
            slugs.contains(&migrated),
            "migrated slug '{migrated}' (from legacy '{legacy}') must be in available_theme_options; \
             available: {slugs:?}"
        );
    }
}
