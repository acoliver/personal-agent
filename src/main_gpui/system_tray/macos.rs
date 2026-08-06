//! macOS system tray: status-item construction, click polling and popup positioning.
//!
//! Split out of the parent module so each platform's tray code lives together.

use std::sync::Arc;

use gpui::*;
use tracing::info;

use personal_agent::presentation::view_command::AppMode;

use super::SystemTray;

// ============================================================================
// macOS tray state
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
use objc2_foundation::{NSData, NSSize, NSString};

#[cfg(target_os = "macos")]
thread_local! {
    static STATUS_ITEM: std::cell::Cell<Option<Retained<NSStatusItem>>> = const { std::cell::Cell::new(None) };
}

// ============================================================================
// macOS constructor + click polling
// ============================================================================

#[cfg(target_os = "macos")]
impl SystemTray {
    /// Create a new system tray with menu bar icon.
    pub fn new(mtm: MainThreadMarker) -> Self {
        // Activation policy:
        // - Packaged builds get LSUIElement=true via Info.plist
        //   (scripts/release/package_macos_arm64.sh, Issue #177), which Launch
        //   Services applies before our process starts. We must NOT override
        //   that here: setting `Regular` would re-add the Dock icon and the
        //   Cmd-Tab tile, defeating the whole point of the agent app.
        // - Raw `cargo run` binaries have no Info.plist, so without any policy
        //   override they show up as a normal app. To match the packaged
        //   experience for local dev, fall back to `Accessory` (menu-bar
        //   only, no Dock, but windows can still take focus). Per-window
        //   `cx.activate(true)` calls in `open_popup` / `open_popout` handle
        //   the key-window / first-responder routing for accessory apps.
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        info!("Set activation policy to Accessory (LSUIElement-equivalent)");

        // Create status item.
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        // Set up icon.
        if let Some(button) = status_item.button(mtm) {
            let icon_data = include_bytes!("../../../assets/MenuBarIcon.imageset/icon-32.png");
            let data = NSData::with_bytes(icon_data);
            use objc2::AllocAnyThread;
            if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
                image.setSize(NSSize::new(18.0, 18.0));
                button.setImage(Some(&image));
            } else {
                button.setTitle(&NSString::from_str("PA"));
            }
        }

        STATUS_ITEM.set(Some(status_item));
        info!("Status item created");

        Self::setup_local_event_monitor();

        Self {
            popup_window: None,
            app_mode: AppMode::Popup,
        }
    }

    /// Set up local event monitor - currently informational (polling is used).
    fn setup_local_event_monitor() {
        info!("Event monitoring via polling (local monitor not used)");
    }

    /// Start polling for tray icon clicks.
    #[allow(clippy::option_if_let_else)]
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn start_click_listener(&self, cx: &mut App) {
        cx.spawn(async move |cx| {
            let mut last_buttons: usize = 0;

            loop {
                smol::Timer::after(std::time::Duration::from_millis(50)).await;

                let current_buttons = NSEvent::pressedMouseButtons();
                let was_down = (last_buttons & 1) != 0;
                let is_down = (current_buttons & 1) != 0;
                last_buttons = current_buttons;

                if was_down && !is_down {
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
}

// ============================================================================
// macOS popup positioning
// ============================================================================

#[cfg(target_os = "macos")]
impl SystemTray {
    /// Get position for popup window (below status item).
    #[allow(clippy::option_if_let_else)]
    pub(super) fn get_popup_position(
        &self,
        menu_width: f32,
        menu_height: f32,
        _cx: &App,
    ) -> (f32, f32) {
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
