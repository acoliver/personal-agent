//! GPUI popup window for the menu bar interface
//!
//! @plan PLAN-20250128-GPUI.P13
//! @requirement REQ-GPUI-007

use std::sync::Arc;

use objc2_foundation::{NSPoint, NSRect, NSSize, MainThreadMarker};
use objc2_app_kit::{NSWindow, NSColor};
use objc2::rc::Retained;

use crate::ui_gpui::bridge::GpuiBridge;

/// Popup window manager for GPUI content
pub struct PopupWindow {
    /// NSWindow instance
    window: Option<Retained<NSWindow>>,
    /// GPUI bridge for event handling
    gpui_bridge: Arc<GpuiBridge>,
    /// Whether the window is currently visible
    is_visible: bool,
}

impl PopupWindow {
    /// Create a new popup window
    pub fn new(gpui_bridge: Arc<GpuiBridge>) -> anyhow::Result<Self> {
        // Get main thread marker
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("Not on main thread"))?;
        
        // Calculate window size (600x400 default)
        let _window_width = 600.0;
        let _window_height = 400.0;
        
        // Create initial frame (off-screen, will be positioned when shown)
        let _frame = NSRect::new(
            NSPoint::new(-1000.0, -1000.0),
            NSSize::new(_window_width, _window_height),
        );
        
        // Create the window with borderless style
        let window = unsafe { NSWindow::new(mtm) };
        
        // Configure window properties
        window.setOpaque(false);
        window.setAlphaValue(0.95);
        
        // Set background color (dark semi-transparent)
        let bg_color = NSColor::colorWithSRGBRed_green_blue_alpha(
            0.1,  // Red
            0.1,  // Green
            0.1,  // Blue
            1.0,  // Alpha
        );
        window.setBackgroundColor(Some(&bg_color));
        
        // Set window level to appear above most windows
        window.setLevel(18);  // NSPopUpMenuWindowLevel + 1
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
            let status_item_center_x = status_item_frame.origin.x + (status_item_frame.size.width / 2.0);
            let mut x = status_item_center_x - (window_width / 2.0);
            let y = status_item_frame.origin.y - window_height - 4.0;  // 4px gap
            
            // Simple bounds checking (assuming standard screen width)
            let screen_width = 1920.0;
            let min_x = 8.0;
            let max_x = screen_width - window_width - 8.0;
            
            if x < min_x {
                x = min_x;
            } else if x > max_x {
                x = max_x;
            }
            
            // Set new frame
            let new_frame = NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(window_width, window_height),
            );
            window.setFrame_display(new_frame, false);
        }
    }
    
    /// Show the popup window
    pub fn show(&mut self) {
        if !self.is_visible {
            if let Some(ref window) = self.window {
                // Make window key and order front
                let _ = &*window;
                self.is_visible = true;
            }
        }
    }
    
    /// Hide the popup window
    pub fn hide(&mut self) {
        if self.is_visible {
            if let Some(ref window) = self.window {
                // Order out the window
                let _ = &*window;
                self.is_visible = false;
            }
        }
    }
    
    /// Check if the window is currently visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
    
    /// Handle ESC key press (dismiss popup)
    pub fn handle_esc_key(&mut self) {
        self.hide();
    }
    
    /// Handle window losing focus (dismiss popup)
    pub fn handle_resign_key(&mut self) {
        self.hide();
    }
    
    /// Get the GPUI bridge
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
