//! System Tray Manager
//!
//! Manages the macOS menu bar status item, click detection via polling,
//! and popup window lifecycle.

use std::sync::atomic::{AtomicBool, Ordering};

use gpui::*;
use tracing::info;

use personal_agent::ui_gpui::views::main_panel::{MainPanel, MainPanelAppState};

use super::AppState;

// ============================================================================
// Thread-local storage for status item
// ============================================================================

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSImage, NSScreen, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSData, NSRect, NSSize, NSString};

#[cfg(target_os = "macos")]
thread_local! {
    static STATUS_ITEM: std::cell::Cell<Option<Retained<NSStatusItem>>> = const { std::cell::Cell::new(None) };
}

// Global flag for click detection
static TRAY_CLICKED: AtomicBool = AtomicBool::new(false);

// ============================================================================
// SystemTray struct + impls
// ============================================================================

/// System tray manager - holds tray state
pub struct SystemTray {
    /// Current popup window handle
    popup_window: Option<AnyWindowHandle>,
}

impl Global for SystemTray {}

impl Default for SystemTray {
    fn default() -> Self {
        Self { popup_window: None }
    }
}

#[cfg(target_os = "macos")]
impl SystemTray {
    /// Create a new system tray with menu bar icon
    pub fn new(mtm: MainThreadMarker) -> Self {
        // Set activation policy to Regular (normal app with dock icon)
        // Accessory mode prevents proper event handling in some cases
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        info!("Set activation policy to Regular");

        // Activate the application to ensure it receives events
        app.activate();
        info!("Application activated");

        // Create status item
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        // Set up icon
        if let Some(button) = status_item.button(mtm) {
            let icon_data = include_bytes!("../../assets/MenuBarIcon.imageset/icon-32.png");
            let data = NSData::with_bytes(icon_data);
            use objc2::AllocAnyThread;
            if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                image.setSize(NSSize::new(18.0, 18.0));
                button.setImage(Some(&image));
            } else {
                button.setTitle(&NSString::from_str("PA"));
            }
        }

        // Store status item
        STATUS_ITEM.set(Some(status_item));
        info!("Status item created");

        // Set up local event monitor for left mouse up
        // Local monitors catch events that are already targeted at our app
        Self::setup_local_event_monitor();

        Self { popup_window: None }
    }

    /// Set up local event monitor - not used currently, relying on polling
    fn setup_local_event_monitor() {
        // Local monitors only work for events targeted at the app
        // For menu bar apps with Accessory policy, the status item button
        // doesn't route through the normal event loop
        // We rely on polling instead
        info!("Event monitoring via polling (local monitor not applicable for Accessory apps)");
    }

    /// Start polling for clicks on status item
    #[allow(clippy::option_if_let_else)]
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn start_click_listener(&self, cx: &mut App) {
        cx.spawn(async move |cx| {
            let mut last_buttons: usize = 0;

            loop {
                smol::Timer::after(std::time::Duration::from_millis(50)).await;

                // Check mouse button state
                let current_buttons = NSEvent::pressedMouseButtons();
                let was_down = (last_buttons & 1) != 0;
                let is_down = (current_buttons & 1) != 0;
                last_buttons = current_buttons;

                // Detect mouse up (was pressed, now released)
                if was_down && !is_down {
                    // Check if mouse is over our status item
                    let mouse_loc = NSEvent::mouseLocation();

                    let status_item = STATUS_ITEM.take();
                    let is_our_click = if let Some(ref item) = status_item {
                        if let Some(mtm) = MainThreadMarker::new() {
                            if let Some(button) = item.button(mtm) {
                                if let Some(window) = button.window() {
                                    let button_bounds = button.bounds();
                                    let button_in_window =
                                        button.convertRect_toView(button_bounds, None);
                                    let button_on_screen =
                                        window.convertRectToScreen(button_in_window);

                                    let in_x = mouse_loc.x >= button_on_screen.origin.x
                                        && mouse_loc.x
                                            <= button_on_screen.origin.x
                                                + button_on_screen.size.width;
                                    let in_y = mouse_loc.y >= button_on_screen.origin.y
                                        && mouse_loc.y
                                            <= button_on_screen.origin.y
                                                + button_on_screen.size.height;
                                    in_x && in_y
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    STATUS_ITEM.set(status_item);

                    if is_our_click {
                        info!(
                            mouse_x = mouse_loc.x,
                            mouse_y = mouse_loc.y,
                            "Tray click detected on status item"
                        );
                        let _ = cx.update_global::<Self, _>(|tray, cx| {
                            tray.toggle_popup(cx);
                        });
                    }
                }
            }
        })
        .detach();

        info!("Click polling started");
    }

    /// Toggle the popup window
    pub fn toggle_popup(&mut self, cx: &mut App) {
        if self.popup_window.is_some() {
            info!("Closing popup...");
            self.close_popup(cx);
        } else {
            info!("Opening popup...");
            self.open_popup(cx);
        }
    }

    /// Open the popup window
    #[allow(clippy::option_if_let_else)]
    fn open_popup(&mut self, cx: &mut App) {
        self.close_popup(cx);

        // A tray click is an explicit user intent to interact with this app right now.
        // Force app activation so the popup is not created behind the current foreground app.
        cx.activate(true);

        let menu_width = 780.0_f32;
        let menu_height = 600.0_f32;

        let (origin_x, origin_y) = self.get_popup_position(menu_width, menu_height);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point {
                    x: px(origin_x),
                    y: px(origin_y),
                },
                size: Size {
                    width: px(menu_width),
                    height: px(menu_height),
                },
            })),
            kind: WindowKind::Normal, // Use Normal instead of PopUp to allow interaction
            focus: true,
            show: true,
            display_id: None,
            titlebar: None,
            window_background: WindowBackgroundAppearance::Opaque,
            app_id: Some("com.personalagent.gpui".to_string()),
            window_min_size: None,
            window_decorations: Some(WindowDecorations::Client),
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            tabbing_identifier: None,
        };

        match cx.open_window(window_options, |_window, cx| {
            cx.new(|cx| MainPanel::new(cx))
        }) {
            Ok(handle) => {
                let any_handle: AnyWindowHandle = handle.into();
                self.popup_window = Some(any_handle);
                if let Some(state) = cx.try_global::<MainPanelAppState>().cloned() {
                    cx.set_global(MainPanelAppState {
                        gpui_bridge: state.gpui_bridge,
                        popup_window: Some(handle),
                        app_store: state.app_store,
                    });
                }
                let _ = handle.update(cx, |main_panel, window, cx| {
                    window.activate_window();
                    if !main_panel.is_runtime_started() {
                        tracing::info!("MainPanel: starting runtime from open_popup");
                        main_panel.start_runtime(cx);
                    }
                });
                // Re-emit MCP snapshot so the new settings view is populated.
                // The one-shot startup emission was consumed by the previous
                // (now-dead) MainPanel; this replays it into the flume channel
                // that the new pump will drain.
                if let Some(app_state) = cx.try_global::<AppState>().cloned() {
                    super::emit_mcp_snapshot_to_flume(&app_state.view_cmd_tx);
                }
                info!(x = origin_x, y = origin_y, "Popup opened");
            }
            Err(e) => {
                tracing::warn!(error = ?e, "Failed to open popup");
            }
        }
    }

    /// Close the popup window
    fn close_popup(&mut self, cx: &mut App) {
        if let Some(handle) = self.popup_window.take() {
            let _ = handle.update(cx, |_, window, _cx| {
                window.remove_window();
            });
        }
    }

    /// Get position for popup window (below status item)
    #[allow(clippy::option_if_let_else)]
    fn get_popup_position(&self, menu_width: f32, menu_height: f32) -> (f32, f32) {
        if std::env::var("PA_TEST_POPUP_ONSCREEN").ok().as_deref() == Some("1") {
            // Keep automation popup visible near the top-right on the main screen.
            // This avoids tray-coordinate edge cases during test startup.
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(main_screen) = NSScreen::mainScreen(mtm) {
                    let frame = main_screen.frame();
                    let x = (frame.size.width as f32 - menu_width - 24.0).max(0.0);
                    return (x, 36.0);
                }
            }

            return (100.0, 30.0);
        }

        let status_item = STATUS_ITEM.take();
        let result = if let Some(ref item) = status_item {
            if let Some(mtm) = MainThreadMarker::new() {
                if let Some(button) = item.button(mtm) {
                    if let Some(window) = button.window() {
                        let button_bounds = button.bounds();
                        let button_in_window = button.convertRect_toView(button_bounds, None);
                        let button_on_screen = window.convertRectToScreen(button_in_window);

                        let icon_center_x =
                            button_on_screen.origin.x + (button_on_screen.size.width / 2.0);
                        let icon_bottom_y = button_on_screen.origin.y;

                        // GPUI expects window origins in display-relative top-left coordinates.
                        // AppKit screen coordinates are bottom-left based, so convert accordingly.
                        if let Some(screen) = window.screen() {
                            let screen_frame = screen.frame();

                            let popup_left = icon_center_x - (menu_width as f64 / 2.0);
                            let popup_bottom = icon_bottom_y - menu_height as f64 - 6.0;

                            let x = (popup_left - screen_frame.origin.x) as f32;
                            let y = (screen_frame.origin.y + screen_frame.size.height
                                - (popup_bottom + menu_height as f64))
                                as f32;

                            let max_x = (screen_frame.size.width as f32 - menu_width).max(0.0);
                            let max_y = (screen_frame.size.height as f32 - menu_height).max(0.0);
                            let clamped_x = x.clamp(0.0, max_x);
                            let clamped_y = y.clamp(0.0, max_y);

                            info!(
                                screen_x = screen_frame.origin.x,
                                screen_y = screen_frame.origin.y,
                                screen_w = screen_frame.size.width,
                                screen_h = screen_frame.size.height,
                                icon_x = button_on_screen.origin.x,
                                icon_y = button_on_screen.origin.y,
                                icon_w = button_on_screen.size.width,
                                icon_h = button_on_screen.size.height,
                                raw_x = x,
                                raw_y = y,
                                clamped_x,
                                clamped_y,
                                "Computed popup position from tray icon"
                            );

                            (clamped_x, clamped_y)
                        } else {
                            info!("No screen on status item window; using fallback popup position");
                            let x = icon_center_x as f32 - (menu_width / 2.0);
                            let y = icon_bottom_y as f32 - menu_height - 6.0;
                            (x, y)
                        }
                    } else {
                        info!("No window on status item button; using fallback popup position");
                        (100.0, 30.0)
                    }
                } else {
                    info!("No status item button; using fallback popup position");
                    (100.0, 30.0)
                }
            } else {
                info!("No main thread marker; using fallback popup position");
                (100.0, 30.0)
            }
        } else {
            info!("No status item available; using fallback popup position");
            (100.0, 30.0)
        };
        STATUS_ITEM.set(status_item);
        result
    }
}

#[cfg(not(target_os = "macos"))]
impl SystemTray {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_click_listener(&self, _cx: &mut App) {}
    pub fn toggle_popup(&mut self, _cx: &mut App) {}
}
