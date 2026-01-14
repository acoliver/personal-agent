//! Debug version of main.rs with enhanced tray icon visibility testing
//!
//! This version includes:
//! - Non-template icon mode (full color, more visible)
//! - Enhanced logging at every step
//! - Verification that NSStatusItem is actually in menu bar
//! - Visual feedback for all operations

use eframe::egui;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

#[cfg(target_os = "macos")]
mod popover;

const PANEL_WIDTH: f32 = 400.0;
const PANEL_HEIGHT: f32 = 500.0;

#[derive(Default)]
struct PersonalAgentApp {
    _tray_icon: Option<TrayIcon>,
    popover_initialized: bool,
}

impl PersonalAgentApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        tracing::info!("=== PersonalAgentApp::new called ===");
        
        #[cfg(target_os = "macos")]
        {
            tracing::info!("Initializing popover state...");
            popover::initialize_popover_state();
            tracing::info!("Popover state initialized");
        }
        
        tracing::info!("Creating tray icon...");
        let tray_icon_result = create_tray_icon();
        
        match &tray_icon_result {
            Ok(tray_icon) => {
                tracing::info!("[OK] Tray icon created successfully!");
                
                #[cfg(target_os = "macos")]
                {
                    // Verify NSStatusItem
                    if let Some(ns_status_item) = tray_icon.ns_status_item() {
                        tracing::info!("[OK] NSStatusItem obtained from tray icon");
                        
                        use objc2_foundation::MainThreadMarker;
                        if let Some(mtm) = MainThreadMarker::new() {
                            if let Some(button) = ns_status_item.button(mtm) {
                                tracing::info!("[OK] NSStatusBarButton obtained");
                                tracing::info!("   Button frame: {:?}", button.frame());
                                tracing::info!("   Button bounds: {:?}", button.bounds());
                            } else {
                                tracing::error!(" Failed to get NSStatusBarButton - ICON WILL NOT BE VISIBLE");
                            }
                        } else {
                            tracing::error!(" Not on main thread!");
                        }
                    } else {
                        tracing::error!(" Failed to get NSStatusItem - ICON WILL NOT BE VISIBLE");
                    }
                }
            }
            Err(e) => {
                tracing::error!(" Failed to create tray icon: {}", e);
                tracing::error!("   Icon will NOT be visible in menu bar");
            }
        }
        
        tracing::info!("=== PersonalAgentApp initialization complete ===");
        
        Self {
            _tray_icon: tray_icon_result.ok(),
            popover_initialized: false,
        }
    }
}

impl eframe::App for PersonalAgentApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        if !self.popover_initialized {
            tracing::info!("First frame - initializing popover...");
            if let Err(e) = popover::setup_native_popover(frame) {
                tracing::error!("Failed to setup popover: {}", e);
            } else {
                self.popover_initialized = true;
                tracing::info!("[OK] Popover initialized successfully");
            }
        }

        #[cfg(target_os = "macos")]
        self.handle_tray_events_popover();
        
        #[cfg(not(target_os = "macos"))]
        Self::handle_tray_events(ctx);
        
        Self::handle_menu_events(ctx);
        Self::render_ui(ctx);
    }
}

impl PersonalAgentApp {
    #[cfg(target_os = "macos")]
    fn handle_tray_events_popover(&self) {
        use objc2_foundation::MainThreadMarker;
        
        match TrayIconEvent::receiver().try_recv() {
            Ok(TrayIconEvent::Click { rect, .. }) => {
                tracing::info!(">>> TRAY ICON CLICKED! <<<");
                tracing::info!("    Click rect: {:?}", rect);
            
                if popover::is_popover_shown() {
                    tracing::info!("Popover is visible, hiding it...");
                    if let Err(e) = popover::hide_popover() {
                        tracing::error!("Failed to hide popover: {}", e);
                    } else {
                        tracing::info!("[OK] Popover hidden");
                    }
                } else {
                    tracing::info!("Popover is hidden, showing it...");
                    
                    if let Some(tray_icon) = &self._tray_icon {
                        if let Some(ns_status_item) = tray_icon.ns_status_item() {
                            if let Some(mtm) = MainThreadMarker::new() {
                                if let Some(button) = ns_status_item.button(mtm) {
                                    let button_bounds = button.bounds();
                                    tracing::info!("Button bounds: {:?}", button_bounds);
                                    
                                    if let Err(e) = popover::show_popover_at_statusbar(button_bounds, &button) {
                                        tracing::error!("Failed to show popover: {}", e);
                                    } else {
                                        tracing::info!("[OK] Popover shown");
                                    }
                                } else {
                                    tracing::error!("Failed to get NSStatusBarButton");
                                }
                            } else {
                                tracing::error!("Not on main thread");
                            }
                        } else {
                            tracing::error!("Failed to get NSStatusItem");
                        }
                    } else {
                        tracing::error!("Tray icon not available");
                    }
                }
            }
            Ok(event) => {
                tracing::debug!("Other tray event: {:?}", event);
            }
            Err(_) => {
                // No event - this is normal
            }
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    fn handle_tray_events(ctx: &egui::Context) {
        if let Ok(TrayIconEvent::Click { rect, .. }) = TrayIconEvent::receiver().try_recv() {
            tracing::info!("Tray click event: rect={:?}", rect);
            
            let icon_x = rect.position.x as f32;
            let icon_y = rect.position.y as f32;
            let icon_height = rect.size.height as f32;
            
            let window_x = icon_x;
            let window_y = icon_y + icon_height;
            
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(
                window_x,
                window_y,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn handle_menu_events(ctx: &egui::Context) {
        if MenuEvent::receiver().try_recv().is_ok() {
            tracing::info!("Quit menu item clicked");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn render_ui(ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PersonalAgent");
            ui.separator();
            ui.label("Debug Mode - Enhanced Visibility");
        });
    }
}

fn create_tray_icon() -> Result<TrayIcon, Box<dyn std::error::Error>> {
    tracing::info!("Loading icon...");
    let icon = load_icon()?;
    tracing::info!("[OK] Icon loaded successfully");
    
    tracing::info!("Creating menu...");
    let menu = create_menu()?;
    tracing::info!("[OK] Menu created");
    
    tracing::info!("Building tray icon with settings:");
    tracing::info!("   - Template mode: FALSE (full color, more visible)");
    tracing::info!("   - Tooltip: PersonalAgent");
    
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_icon_as_template(false) // DEBUG: Use full color icon for visibility
        .with_tooltip("PersonalAgent")
        .build()?;

    tracing::info!("[OK] TrayIcon built successfully");
    
    Ok(tray_icon)
}

fn create_menu() -> Result<Menu, Box<dyn std::error::Error>> {
    let menu = Menu::new();
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&quit_item)?;
    tracing::debug!("Menu created with Quit item");
    Ok(menu)
}

fn load_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let icon_data = include_bytes!("../../assets/MenuIcon.imageset/icon-32.png");
    tracing::info!("Icon data size: {} bytes", icon_data.len());
    
    load_icon_from_bytes(icon_data)
}

fn load_icon_from_bytes(icon_data: &[u8]) -> Result<Icon, Box<dyn std::error::Error>> {
    tracing::debug!("Decoding image...");
    let image = image::load_from_memory(icon_data)?;
    
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    
    tracing::info!("Icon dimensions: {}x{}", width, height);
    
    Icon::from_rgba(rgba.into_raw(), width, height).map_err(Into::into)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    
    tracing::info!("===========================================");
    tracing::info!("PersonalAgent DEBUG MODE");
    tracing::info!("Enhanced tray icon visibility testing");
    tracing::info!("===========================================");
    
    let options = create_native_options();
    
    tracing::info!("Starting eframe application...");
    run_app(options)?;
    
    tracing::info!("Application exited normally");
    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
}

fn create_native_options() -> eframe::NativeOptions {
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

fn create_dark_style() -> egui::Style {
    egui::Style {
        visuals: egui::Visuals {
            window_fill: egui::Color32::from_rgb(13, 13, 13),
            ..egui::Visuals::dark()
        },
        ..egui::Style::default()
    }
}

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
