//! `PersonalAgent` - A macOS menu bar application with chat interface
//!
//! Phase 0: Minimal viable menu bar app
//! - Menu bar icon using tray-icon
//! - Empty egui panel (400x500px, dark background)
//! - Panel opens on tray icon click using native `NSPopover`
//! - Quit option in tray menu

use eframe::egui;
use tray_icon::{menu::MenuEvent, TrayIcon, TrayIconEvent};

mod main_utils;
#[cfg(target_os = "macos")]
mod popover;

use crate::main_utils::{create_dark_style, create_native_options, PANEL_WIDTH};

/// Main application state
#[derive(Default)]
struct PersonalAgentApp {
    tray_icon: Option<TrayIcon>,
}

impl PersonalAgentApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let tray_icon_result = create_tray_icon();
        if let Err(ref e) = tray_icon_result {
            tracing::error!("Failed to create tray icon: {}", e);
        } else {
            tracing::info!("Tray icon created successfully");
        }

        Self {
            tray_icon: tray_icon_result.ok(),
        }
    }
}

impl eframe::App for PersonalAgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::handle_tray_events(ctx, self.tray_icon.as_ref());
        Self::handle_menu_events(ctx);
        Self::render_ui(ctx);
    }
}

impl PersonalAgentApp {
    /// Handle tray icon click events - shows and focuses the window, positioned below the icon
    fn handle_tray_events(ctx: &egui::Context, _tray_icon: Option<&TrayIcon>) {
        if let Ok(TrayIconEvent::Click {
            rect,
            id,
            button,
            button_state,
            ..
        }) = TrayIconEvent::receiver().try_recv()
        {
            tracing::info!(
                "Tray click event: rect={:?}, id={:?}, button={:?}, button_state={:?}",
                rect,
                id,
                button,
                button_state
            );

            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let icon_x = rect.position.x as f32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let icon_y = rect.position.y as f32;
            #[allow(clippy::cast_precision_loss)]
            let icon_height = rect.size.height as f32;

            tracing::info!(
                "Icon position: x={}, y={}, height={}",
                icon_x,
                icon_y,
                icon_height
            );

            // Position window directly below the icon (centered)
            #[allow(clippy::cast_precision_loss)]
            let window_x = icon_x - (PANEL_WIDTH / 2.0) + (rect.size.width as f32 / 2.0);
            let window_y = icon_y + icon_height;

            tracing::info!("Setting window position to: x={}, y={}", window_x, window_y);

            // Apply positioning commands
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(
                window_x, window_y,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    /// Handle menu events (currently just Quit)
    fn handle_menu_events(ctx: &egui::Context) {
        if MenuEvent::receiver().try_recv().is_ok() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Render the main UI - currently just a heading
    fn render_ui(ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PersonalAgent");
        });
    }
}

/// Create the tray icon with menu
///
/// # Errors
///
/// Returns an error if the menu or tray icon cannot be created.
fn create_tray_icon() -> Result<TrayIcon, Box<dyn std::error::Error>> {
    crate::main_utils::create_tray_icon_with_menu(crate::main_utils::create_menu()?)
}

/// Application entry point
fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let options = create_native_options();
    run_app(options)?;
    Ok(())
}

/// Initialize tracing for logging
fn init_tracing() {
    tracing_subscriber::fmt::init();
}

/// Run the eframe application
fn run_app(options: eframe::NativeOptions) -> Result<(), Box<dyn std::error::Error>> {
    eframe::run_native(
        "PersonalAgent",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_style(create_dark_style());
            Ok(Box::new(PersonalAgentApp::new(cc)))
        }),
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_native_options_are_hidden_and_fixed_size() {
        let options = create_native_options();

        assert!(!options.viewport.visible.unwrap_or(true));
        assert_eq!(options.viewport.resizable, Some(false));
        assert_eq!(
            options.viewport.inner_size,
            Some(egui::Vec2::new(400.0, 500.0))
        );
    }

    #[test]
    fn create_dark_style_uses_expected_background() {
        let style = create_dark_style();
        assert_eq!(
            style.visuals.window_fill,
            egui::Color32::from_rgb(13, 13, 13)
        );
    }
}
