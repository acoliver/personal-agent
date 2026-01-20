//! Shared UI helpers extracted from the main app for testing.

use eframe::egui;
use tray_icon::{
    menu::{Menu, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// Width of the application panel in pixels
pub const PANEL_WIDTH: f32 = 400.0;

/// Height of the application panel in pixels
pub const PANEL_HEIGHT: f32 = 500.0;

/// Create the tray menu with Quit option
///
/// # Errors
///
/// Returns an error if the menu item cannot be appended.
pub fn create_menu() -> Result<Menu, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&quit_item)?;
    Ok(menu)
}

/// Create tray icon from menu and icon
///
/// # Errors
///
/// Returns an error if the tray icon cannot be built.
pub fn create_tray_icon_with_menu(menu: Menu) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let icon = load_icon()?;

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_icon_as_template(true)
        .with_tooltip("PersonalAgent")
        .build()?;

    Ok(tray_icon)
}

/// Load the tray icon from embedded asset
/// Uses the 32px icon which works well for standard and retina displays
///
/// # Errors
///
/// Returns an error if the icon bytes cannot be decoded.
pub fn load_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    load_icon_from_bytes(include_bytes!("../assets/MenuIcon.imageset/icon-32.png"))
}

/// Load icon from raw PNG bytes
///
/// # Errors
///
/// Returns an error if the icon bytes cannot be decoded.
pub fn load_icon_from_bytes(icon_data: &[u8]) -> Result<Icon, Box<dyn std::error::Error>> {
    let image = image::load_from_memory(icon_data)?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).map_err(Into::into)
}

/// Create native window options for the panel
#[must_use]
pub fn create_native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([PANEL_WIDTH, PANEL_HEIGHT])
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_visible(false),
        ..Default::default()
    }
}

/// Create the dark theme style
#[must_use]
pub fn create_dark_style() -> egui::Style {
    egui::Style {
        visuals: egui::Visuals {
            window_fill: egui::Color32::from_rgb(13, 13, 13),
            ..egui::Visuals::dark()
        },
        ..egui::Style::default()
    }
}
