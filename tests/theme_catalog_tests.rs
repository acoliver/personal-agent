use std::path::Path;

use personal_agent::ui_gpui::theme_catalog::{ThemeCatalog, ThemeCatalogError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bundled_catalog_loads_all_theme_files() -> TestResult {
    let catalog = ThemeCatalog::load_bundled()?;
    assert_eq!(catalog.len(), 15, "expected 15 bundled theme files");
    assert!(catalog.get("green-screen").is_some());
    assert!(catalog.get("default").is_some());
    assert!(catalog.get("default-light").is_some());

    Ok(())
}

#[test]
fn bundled_catalog_contains_unique_slugs() -> TestResult {
    let catalog = ThemeCatalog::load_bundled()?;
    let mut slugs = catalog.slugs();
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), catalog.len());

    Ok(())
}

#[test]
fn bundled_catalog_parses_green_screen_metadata_and_color_groups() -> TestResult {
    let catalog = ThemeCatalog::load_bundled()?;
    let green = catalog
        .get("green-screen")
        .expect("green-screen theme must exist");

    assert_eq!(green.name, "Green Screen");
    assert_eq!(green.slug, "green-screen");
    assert_eq!(green.colors.background, "#000000");
    assert_eq!(green.colors.panel.border, "#6a9955");
    assert_eq!(green.colors.input.hint, "#3b7a3b");
    assert_eq!(green.colors.accent.success, "#00ff00");

    Ok(())
}

#[test]
fn loading_missing_theme_directory_returns_error() {
    let missing = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("themes")
        .join("does-not-exist");

    let err = ThemeCatalog::load_from_dir(&missing).expect_err("expected missing dir error");
    assert!(matches!(err, ThemeCatalogError::ThemeDirectoryMissing(_)));
}

#[test]
fn bundled_theme_dir_points_to_assets_themes() {
    let bundled_dir = ThemeCatalog::bundled_theme_dir();
    assert_eq!(
        bundled_dir.file_name().and_then(|s| s.to_str()),
        Some("themes")
    );
    assert!(bundled_dir.starts_with(Path::new(env!("CARGO_MANIFEST_DIR"))));
}
