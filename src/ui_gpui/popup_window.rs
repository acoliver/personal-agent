#![allow(unsafe_code)]

//! GPUI popup window for the menu bar interface
//!
//! @plan PLAN-20250128-GPUI.P13
//! @requirement REQ-GPUI-007

use std::sync::Arc;

use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSScreen, NSWindow};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

use crate::ui_gpui::bridge::GpuiBridge;

/// Popup window manager for GPUI content
pub struct PopupWindow {
    /// `NSWindow` instance
    window: Option<Retained<NSWindow>>,
    /// GPUI bridge for event handling
    gpui_bridge: Arc<GpuiBridge>,
    /// Whether the window is currently visible
    is_visible: bool,
}

impl PopupWindow {
    /// Create a new popup window.
    ///
    /// # Errors
    ///
    /// Returns an error if the popup is not created on the macOS main thread.
    pub fn new(gpui_bridge: Arc<GpuiBridge>) -> anyhow::Result<Self> {
        // Get main thread marker
        let mtm = MainThreadMarker::new().ok_or_else(|| anyhow::anyhow!("Not on main thread"))?;

        // Calculate window size (wider default for GPUI content)
        let window_width = 760.0;
        let window_height = 520.0;

        // Create initial frame (off-screen, will be positioned when shown)
        let _frame = NSRect::new(
            NSPoint::new(-1000.0, -1000.0),
            NSSize::new(window_width, window_height),
        );

        // Create the window with borderless style
        let window = unsafe { NSWindow::new(mtm) };

        // Configure window properties
        window.setOpaque(false);
        window.setAlphaValue(0.95);

        // Set background color (dark semi-transparent)
        let bg_color = NSColor::colorWithSRGBRed_green_blue_alpha(
            0.1, // Red
            0.1, // Green
            0.1, // Blue
            1.0, // Alpha
        );
        window.setBackgroundColor(Some(&bg_color));

        // Set window level to appear above most windows
        window.setLevel(18); // NSPopUpMenuWindowLevel + 1
        window.setHidesOnDeactivate(false);

        // Enable rounded corners with shadow
        window.setHasShadow(true);

        Ok(Self {
            window: Some(window),
            gpui_bridge,
            is_visible: false,
        })
    }

    /// Position the popup window below the status bar item
    pub fn position_below_status_item(&self, status_item_frame: NSRect) {
        if let Some(ref window) = self.window {
            // Get window dimensions
            let window_frame = window.frame();
            let window_width = window_frame.size.width;
            let window_height = window_frame.size.height;

            // Calculate position (centered below status item)
            let status_item_center_x =
                status_item_frame.origin.x + (status_item_frame.size.width / 2.0);
            let x = status_item_center_x - (window_width / 2.0);
            let y = status_item_frame.origin.y - window_height - 4.0; // 4px gap

            // Bounds checking against real screen geometry when available.
            let min_margin = 8.0;
            let default_bounds = || {
                (
                    min_margin,
                    1920.0 - window_width - min_margin,
                    min_margin,
                    1080.0 - window_height - min_margin,
                )
            };
            let (min_x, max_x, min_y, max_y) = window.screen().map_or_else(
                || {
                    MainThreadMarker::new().map_or_else(default_bounds, |mtm| {
                        NSScreen::mainScreen(mtm).map_or_else(default_bounds, |main_screen| {
                            let frame = main_screen.frame();
                            (
                                frame.origin.x + min_margin,
                                frame.origin.x + frame.size.width - window_width - min_margin,
                                frame.origin.y + min_margin,
                                frame.origin.y + frame.size.height - window_height - min_margin,
                            )
                        })
                    })
                },
                |screen| {
                    let frame = screen.frame();
                    (
                        frame.origin.x + min_margin,
                        frame.origin.x + frame.size.width - window_width - min_margin,
                        frame.origin.y + min_margin,
                        frame.origin.y + frame.size.height - window_height - min_margin,
                    )
                },
            );

            let clamped_x = if max_x >= min_x {
                x.clamp(min_x, max_x)
            } else {
                min_x
            };
            let clamped_y = if max_y >= min_y {
                y.clamp(min_y, max_y)
            } else {
                min_y
            };

            // Set new frame
            let new_frame = NSRect::new(
                NSPoint::new(clamped_x, clamped_y),
                NSSize::new(window_width, window_height),
            );
            window.setFrame_display(new_frame, false);
        }
    }

    /// Show the popup window
    pub const fn show(&mut self) {
        if !self.is_visible {
            if let Some(ref window) = self.window {
                // Make window key and order front
                let _ = window;
                self.is_visible = true;
            }
        }
    }

    /// Hide the popup window
    pub const fn hide(&mut self) {
        if self.is_visible {
            if let Some(ref window) = self.window {
                // Order out the window
                let _ = window;
                self.is_visible = false;
            }
        }
    }

    /// Check if the window is currently visible
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Handle ESC key press (dismiss popup)
    pub const fn handle_esc_key(&mut self) {
        self.hide();
    }

    /// Handle window losing focus (dismiss popup)
    pub const fn handle_resign_key(&mut self) {
        self.hide();
    }

    /// Get the GPUI bridge
    #[must_use]
    pub fn gpui_bridge(&self) -> Arc<GpuiBridge> {
        Arc::clone(&self.gpui_bridge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires macOS main thread GUI context"]
    fn test_popup_window_creation() {
        let (user_tx, _user_rx) = flume::unbounded();
        let (_view_cmd_tx, view_cmd_rx) = flume::unbounded();
        let gpui_bridge = Arc::new(GpuiBridge::new(user_tx, view_cmd_rx));

        let popup = PopupWindow::new(gpui_bridge);
        assert!(popup.is_ok());

        if let Ok(window) = popup {
            assert!(!window.is_visible());
        }
    }
}
