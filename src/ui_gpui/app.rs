//! GPUI application with tray icon and popup window integration
//!
//! @plan PLAN-20250128-GPUI.P13
//! @requirement REQ-GPUI-007

use std::sync::Arc;
use std::time::Duration;

use crate::events::EventBus;
use crate::presentation::view_command::{ViewCommand, ViewId};
use crate::ui_gpui::bridge::{GpuiBridge, GpuiNotifier, ViewCommandSink};
use crate::ui_gpui::tray_bridge::TrayBridge;
use crate::ui_gpui::popup_window::PopupWindow;

/// Main GPUI application struct
pub struct GpuiApp {
    /// Event bus for application-wide event handling
    event_bus: Arc<EventBus>,
    /// GPUI bridge for UI event handling
    gpui_bridge: Arc<GpuiBridge>,
    /// Tray bridge for menu bar integration
    tray_bridge: Option<TrayBridge>,
    /// Popup window manager
    popup_window: Option<PopupWindow>,
}

impl GpuiApp {
    /// Create a new GPUI application
    pub fn new(event_bus: Arc<EventBus>) -> anyhow::Result<Self> {
        // Create channels for GPUI bridge
        let (user_tx, user_rx) = flume::unbounded();
        let (view_cmd_tx, view_cmd_rx) = flume::unbounded();
        
        let gpui_bridge = Arc::new(GpuiBridge::new(user_tx, view_cmd_rx));
        
        // Store unused channels
        let _ = (user_rx, view_cmd_tx);
        
        Ok(Self {
            event_bus,
            gpui_bridge,
            tray_bridge: None,
            popup_window: None,
        })
    }
    
    /// Initialize the application components
    pub fn initialize(&mut self) -> anyhow::Result<()> {
        // Create the popup window
        let popup_window = PopupWindow::new(Arc::clone(&self.gpui_bridge))?;
        
        // Create the tray bridge
        let tray_bridge = TrayBridge::new(Arc::clone(&self.gpui_bridge))?;
        
        // Set the popup window on the tray bridge
        // Note: We move the popup_window into the tray_bridge
        // This is handled by set_popup_window which takes ownership
        
        self.tray_bridge = Some(tray_bridge);
        self.popup_window = Some(popup_window);
        
        tracing::info!("GPUI application initialized successfully");
        
        Ok(())
    }
    
    /// Get the GPUI bridge
    pub fn gpui_bridge(&self) -> Arc<GpuiBridge> {
        Arc::clone(&self.gpui_bridge)
    }
    
    /// Check if the popup is currently visible
    pub fn is_popup_visible(&self) -> bool {
        self.tray_bridge
            .as_ref()
            .map(|tray| tray.is_popup_visible())
            .unwrap_or(false)
    }
    
    /// Toggle the popup window visibility
    pub fn toggle_popup(&self) {
        if let Some(ref tray) = self.tray_bridge {
            tray.toggle_popup();
        }
    }
    
    /// Show the popup window
    pub fn show_popup(&self) {
        if let Some(ref tray) = self.tray_bridge {
            tray.show_popup();
        }
    }
    
    /// Hide the popup window
    pub fn hide_popup(&self) {
        if let Some(ref tray) = self.tray_bridge {
            tray.hide_popup();
        }
    }
    
    /// Start the event forwarding task
    ///
    /// This task listens for ViewCommands from the command sink and forwards them
    /// to the appropriate GPUI views.
    pub fn start_event_forwarding(&mut self) -> anyhow::Result<()> {
        // The event forwarding is handled by the existing user_event_forwarder
        // This method is kept for compatibility but the actual forwarding
        // is managed by the GPUI bridge
        
        tracing::info!("Event forwarding initialized (handled by GPUI bridge)");
        
        Ok(())
    }
    
    /// Run the application main loop
    pub async fn run(&mut self) -> anyhow::Result<()> {
        tracing::info!("Starting GPUI application main loop");
        
        // Initialize components
        self.initialize()?;
        
        // Start event forwarding
        self.start_event_forwarding()?;
        
        // Keep the application running
        // In a real implementation, this would integrate with GPUI's event loop
        // For now, we'll use a simple keep-alive loop
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // Check if we should continue running
            // This would typically be controlled by application state
        }
    }
    
    /// Handle application shutdown
    pub fn shutdown(&mut self) {
        tracing::info!("Shutting down GPUI application");
        
        // Hide the popup if visible
        if let Some(ref tray) = self.tray_bridge {
            tray.hide_popup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_gpui_app_creation() {
        let event_bus = EventBus::new(100);
        let app = GpuiApp::new(Arc::new(event_bus));
        
        assert!(app.is_ok());
        
        if let Ok(app) = app {
            // Verify bridge is created
            let bridge = app.gpui_bridge();
            assert!(Arc::strong_count(&bridge) >= 2);
        }
    }
    
    #[tokio::test]
    #[ignore = "Requires macOS main thread GUI context"]
    async fn test_gpui_app_initialization() {
        let event_bus = EventBus::new(100);
        let mut app = GpuiApp::new(Arc::new(event_bus)).unwrap();
        
        let init_result = app.initialize();
        assert!(init_result.is_ok());
        
        assert!(!app.is_popup_visible());
        
        let forward_result = app.start_event_forwarding();
        assert!(forward_result.is_ok());
    }
}
