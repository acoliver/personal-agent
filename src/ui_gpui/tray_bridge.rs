//! Bridge between `AppKit` `NSStatusItem` and GPUI popup window
//!
//! @plan PLAN-20250128-GPUI.P13
//! @requirement REQ-GPUI-007

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use objc2::rc::Retained;
use objc2_app_kit::NSStatusBar;
use objc2_foundation::{MainThreadMarker, NSString};

use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::popup_window::PopupWindow;

/// Bridge between `NSStatusItem` and GPUI application
pub struct TrayBridge {
    /// `NSStatusItem` for the menu bar icon
    status_item: Retained<objc2_app_kit::NSStatusItem>,
    /// GPUI popup window (shown/hidden on click)
    popup_window: Rc<RefCell<Option<PopupWindow>>>,
    /// GPUI bridge for event handling
    gpui_bridge: Arc<GpuiBridge>,
    /// Whether the popup is currently visible
    is_visible: Arc<Mutex<bool>>,
    /// Main thread marker for UI operations
    mtm: MainThreadMarker,
}

impl TrayBridge {
    /// Create a new tray bridge with status item and popup window.
    ///
    /// # Errors
    ///
    /// Returns an error if the tray is not created on the macOS main thread.
    pub fn new(gpui_bridge: Arc<GpuiBridge>) -> anyhow::Result<Self> {
        // Get main thread marker
        let mtm = MainThreadMarker::new().ok_or_else(|| anyhow::anyhow!("Not on main thread"))?;

        // Get the system status bar
        let status_bar = NSStatusBar::systemStatusBar();

        // Create a status item with square length (standard for icons)
        let status_item = status_bar.statusItemWithLength(24.0);

        // Configure the status item button
        if let Some(button) = status_item.button(mtm) {
            // Set the button title
            let title = NSString::from_str("PA");
            button.setTitle(&title);
        }

        let tray_bridge = Self {
            status_item,
            popup_window: Rc::new(RefCell::new(None)),
            gpui_bridge,
            is_visible: Arc::new(Mutex::new(false)),
            mtm,
        };

        Ok(tray_bridge)
    }

    /// Toggle the popup window visibility.
    ///
    /// # Panics
    ///
    /// Panics if the visibility mutex is poisoned.
    pub fn toggle_popup(&self) {
        let mut visible = self.is_visible.lock().unwrap();

        if *visible {
            self.hide_popup();
            *visible = false;
        } else {
            self.show_popup();
            *visible = true;
        }
    }

    /// Show the popup window.
    pub fn show_popup(&self) {
        let mut popup_guard = self.popup_window.borrow_mut();
        if let Some(window) = popup_guard.as_mut() {
            // Get status item button frame for positioning
            if let Some(button) = self.status_item.button(self.mtm) {
                let button_frame = button.frame();
                window.position_below_status_item(button_frame);
                window.show();
            }
        }
    }

    /// Hide the popup window.
    pub fn hide_popup(&self) {
        if let Some(mut window) = self.popup_window.borrow_mut().take() {
            window.hide();
            // Put it back
            *self.popup_window.borrow_mut() = Some(window);
        }
    }

    /// Set the popup window.
    pub fn set_popup_window(&self, window: PopupWindow) {
        let mut popup = self.popup_window.borrow_mut();
        *popup = Some(window);
    }

    /// Check if popup is currently visible.
    ///
    /// # Panics
    ///
    /// Panics if the visibility mutex is poisoned.
    #[must_use]
    pub fn is_popup_visible(&self) -> bool {
        *self.is_visible.lock().unwrap()
    }

    /// Handle click outside the popup window to dismiss it.
    ///
    /// # Panics
    ///
    /// Panics if the visibility mutex is poisoned.
    pub fn handle_click_outside(&self) {
        if self.is_popup_visible() {
            self.hide_popup();
            *self.is_visible.lock().unwrap() = false;
        }
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

    /// Validate that `TrayBridge::new` fails gracefully when not on the main thread.
    /// Full main-thread integration tests live in `tests/gui_main_thread.rs`.
    #[test]
    fn tray_bridge_rejects_worker_thread() {
        let (user_tx, _) = flume::unbounded();
        let (_, view_rx) = flume::unbounded();
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));

        let result = TrayBridge::new(bridge);
        assert!(
            result.is_err(),
            "TrayBridge::new should fail off main thread"
        );
    }
}
