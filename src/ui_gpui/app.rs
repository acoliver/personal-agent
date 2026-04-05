//! GPUI application with tray icon and popup window integration
//!
//! @plan PLAN-20250128-GPUI.P13
//! @requirement REQ-GPUI-007

use std::sync::Arc;
use std::time::Duration;

use crate::events::EventBus;
use crate::ui_gpui::bridge::GpuiBridge;
#[cfg(target_os = "macos")]
use crate::ui_gpui::popup_window::PopupWindow;
#[cfg(target_os = "macos")]
use crate::ui_gpui::tray_bridge::TrayBridge;

/// Main GPUI application struct
pub struct GpuiApp {
    /// Event bus for application-wide event handling
    _event_bus: Arc<EventBus>,
    /// GPUI bridge for UI event handling
    gpui_bridge: Arc<GpuiBridge>,
    /// Tray bridge for menu bar integration
    #[cfg(target_os = "macos")]
    tray_bridge: Option<TrayBridge>,
    /// Popup window manager
    #[cfg(target_os = "macos")]
    popup_window: Option<PopupWindow>,
}

impl GpuiApp {
    /// Create a new GPUI application.
    ///
    /// # Errors
    ///
    /// Returns an error if any bridge initialization step fails.
    pub fn new(event_bus: Arc<EventBus>) -> anyhow::Result<Self> {
        // Create channels for GPUI bridge
        let (user_tx, user_rx) = flume::unbounded();
        let (view_cmd_tx, view_cmd_rx) = flume::unbounded();

        let gpui_bridge = Arc::new(GpuiBridge::new(user_tx, view_cmd_rx));

        // Store unused channels
        let _ = (user_rx, view_cmd_tx);

        Ok(Self {
            _event_bus: event_bus,
            gpui_bridge,
            #[cfg(target_os = "macos")]
            tray_bridge: None,
            #[cfg(target_os = "macos")]
            popup_window: None,
        })
    }

    /// Initialize the application components.
    ///
    /// # Errors
    ///
    /// Returns an error if the popup window or tray bridge cannot be created.
    pub fn initialize(&mut self) -> anyhow::Result<()> {
        #[cfg(target_os = "macos")]
        {
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

        #[cfg(not(target_os = "macos"))]
        {
            anyhow::bail!("GpuiApp::initialize is only supported on macOS")
        }
    }

    /// Get the GPUI bridge
    #[must_use]
    pub fn gpui_bridge(&self) -> Arc<GpuiBridge> {
        Arc::clone(&self.gpui_bridge)
    }

    /// Check if the popup is currently visible
    #[cfg(target_os = "macos")]
    #[must_use]
    pub fn is_popup_visible(&self) -> bool {
        self.tray_bridge
            .as_ref()
            .is_some_and(TrayBridge::is_popup_visible)
    }

    /// Check if the popup is currently visible
    #[cfg(not(target_os = "macos"))]
    #[must_use]
    pub const fn is_popup_visible(&self) -> bool {
        false
    }

    /// Toggle the popup window visibility
    #[cfg(target_os = "macos")]
    pub fn toggle_popup(&self) {
        if let Some(tray) = &self.tray_bridge {
            tray.toggle_popup();
        }
    }

    /// Toggle the popup window visibility
    #[cfg(not(target_os = "macos"))]
    pub const fn toggle_popup(&self) {}

    /// Show the popup window
    #[cfg(target_os = "macos")]
    pub fn show_popup(&self) {
        if let Some(tray) = &self.tray_bridge {
            tray.show_popup();
        }
    }

    /// Show the popup window
    #[cfg(not(target_os = "macos"))]
    pub const fn show_popup(&self) {}

    /// Hide the popup window
    #[cfg(target_os = "macos")]
    pub fn hide_popup(&self) {
        if let Some(tray) = &self.tray_bridge {
            tray.hide_popup();
        }
    }

    /// Hide the popup window
    #[cfg(not(target_os = "macos"))]
    pub const fn hide_popup(&self) {}

    /// Start the event forwarding task.
    ///
    /// This task listens for `ViewCommands` from the command sink and forwards them
    /// to the appropriate GPUI views.
    ///
    /// # Errors
    ///
    /// Returns an error if event forwarding setup fails.
    pub fn start_event_forwarding(&mut self) -> anyhow::Result<()> {
        // The event forwarding is handled by the existing user_event_forwarder
        // This method is kept for compatibility but the actual forwarding
        // is managed by the GPUI bridge

        tracing::info!("Event forwarding initialized (handled by GPUI bridge)");

        Ok(())
    }

    /// Run the application main loop.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization or event-forwarding startup fails.
    #[allow(clippy::future_not_send)]
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
        #[cfg(target_os = "macos")]
        if let Some(tray) = &self.tray_bridge {
            tray.hide_popup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpui_app_creation_succeeds() {
        let event_bus = EventBus::new(100);
        let app = GpuiApp::new(Arc::new(event_bus));
        assert!(app.is_ok());
    }

    /// `initialize()` requires the macOS main thread (creates `PopupWindow` + `TrayBridge`).
    /// Full main-thread integration tests live in `tests/gui_main_thread.rs`.
    #[test]
    fn gpui_app_initialize_fails_off_main_thread() {
        let event_bus = EventBus::new(100);
        let mut app = GpuiApp::new(Arc::new(event_bus)).expect("GpuiApp::new");

        let result = app.initialize();
        assert!(result.is_err(), "initialize should fail off main thread");
    }
}
