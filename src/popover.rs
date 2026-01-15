//! Native macOS NSPopover integration for egui
//!
//! This module provides a bridge between egui windows and native macOS NSPopover,
//! allowing the application to appear with a native popover arrow anchored to
//! the status bar icon, just like BarTranslate.
//!
//! Note: This module uses unsafe blocks to interface with Objective-C APIs.
//! Safety is ensured by following objc2 patterns and macOS threading requirements.

#![allow(unsafe_code)]

use objc2::rc::Retained;
use objc2::Message;
use objc2_app_kit::{NSPopover, NSView, NSViewController, NSWindow};
use objc2_foundation::{MainThreadMarker, NSRect, NSRectEdge};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use std::cell::RefCell;

thread_local! {
    /// Thread-local state for the popover bridge
    /// This is safe because NSPopover must only be accessed from the main thread
    static POPOVER_STATE: RefCell<Option<PopoverBridgeState>> = const { RefCell::new(None) };
}

/// Shared state for the popover bridge
struct PopoverBridgeState {
    popover: Retained<NSPopover>,
    #[allow(dead_code)] // Keep for future use
    egui_window: Retained<NSWindow>,
    #[allow(dead_code)] // Must be retained to keep the content view alive
    view_controller: Retained<NSViewController>,
    pending_operation: Option<PendingPopoverOperation>,
}

/// Deferred popover operations to avoid re-entrancy
#[derive(Clone)]
enum PendingPopoverOperation {
    Show { button_bounds: NSRect, status_button: Retained<NSView> },
    Hide,
}

/// Initialize the popover state (must be called on main thread)
pub fn initialize_popover_state() {
    POPOVER_STATE.with(|state| {
        *state.borrow_mut() = None;
    });
}

/// Setup the native NSPopover for the egui window
///
/// This function should be called after the egui window is created.
/// It wraps the window's content view inside a native NSPopover.
///
/// # Safety
/// Uses unsafe blocks to interface with Objective-C NSWindow APIs.
/// Must be called from the main thread.
#[allow(unsafe_code)]
pub fn setup_native_popover(window_handle: &dyn HasWindowHandle) -> Result<(), String> {
    let raw_handle = window_handle
        .window_handle()
        .map_err(|e| format!("Failed to get window handle: {e}"))?;

    let ns_window = match raw_handle.as_raw() {
        RawWindowHandle::AppKit(handle) => {
            // Get the NSView from the handle
            let ns_view_ptr = handle.ns_view.as_ptr() as *mut NSView;
            // SAFETY: We know this is a valid NSView pointer from eframe
            let ns_view = unsafe { Retained::retain(ns_view_ptr) }
                .ok_or_else(|| "Failed to retain NSView".to_string())?;
            
            // Get the window from the view
            ns_view.window()
                .ok_or_else(|| "Failed to get NSWindow from NSView".to_string())?
        }
        _ => return Err("Not running on macOS".to_string()),
    };

    // Get MainThreadMarker (safe because we're on the main thread in macOS GUI app)
    let mtm = MainThreadMarker::new()
        .ok_or("Not running on main thread")?;

    // Create the popover
    let popover = NSPopover::new(mtm);
    popover.setBehavior(objc2_app_kit::NSPopoverBehavior::Transient);
    popover.setAnimates(true);
    
    // Set the content size to match our panel dimensions
    let size = objc2_foundation::NSSize::new(400.0, 500.0);
    popover.setContentSize(size);

    // Get the window's content view
    let content_view = ns_window.contentView()
        .ok_or("Failed to get content view")?;
    
    // Create a proper NSViewController to host the content
    let view_controller = NSViewController::new(mtm);
    
    // Set the content view on the view controller
    view_controller.setView(&content_view);
    
    // Set the view controller as the popover's content
    popover.setContentViewController(Some(&view_controller));

    // Hide the original window (we'll show it through the popover)
    // Note: The window needs to remain in the window hierarchy for the popover to work
    // but we make it invisible and order it out
    tracing::info!("Hiding original window to prepare for popover display");
    ns_window.setAlphaValue(0.0);
    ns_window.orderOut(None);

    // Store references in thread-local storage
    POPOVER_STATE.with(|state| {
        *state.borrow_mut() = Some(PopoverBridgeState {
            popover,
            egui_window: ns_window,
            view_controller,
            pending_operation: None,
        });
    });

    Ok(())
}

/// Queue showing the popover anchored to the status bar button
///
/// This should be called when the tray icon is clicked. The actual show
/// operation is deferred to avoid re-entrancy with eframe's update cycle.
///
/// # Safety
/// Uses unsafe blocks to call NSPopover methods. Must be called from main thread.
#[allow(unsafe_code)]
pub fn show_popover_at_statusbar(button_bounds: NSRect, status_button: &NSView) -> Result<(), String> {
    tracing::info!("Queueing popover show operation");
    
    POPOVER_STATE.with(|state| {
        let mut state_mut = state.borrow_mut();
        let bridge_state = state_mut.as_mut()
            .ok_or("Popover not initialized")?;

        // Queue the operation instead of executing immediately
        // Clone the Retained pointer to store it
        bridge_state.pending_operation = Some(PendingPopoverOperation::Show {
            button_bounds,
            status_button: status_button.retain(),
        });

        tracing::info!("Popover show operation queued");
        Ok(())
    })
}

/// Queue hiding the popover
///
/// The actual hide operation is deferred to avoid re-entrancy.
///
/// # Safety
/// Uses unsafe blocks to call NSPopover methods. Must be called from main thread.
#[allow(unsafe_code)]
pub fn hide_popover() -> Result<(), String> {
    tracing::info!("Queueing popover hide operation");
    
    POPOVER_STATE.with(|state| {
        let mut state_mut = state.borrow_mut();
        let bridge_state = state_mut.as_mut()
            .ok_or("Popover not initialized")?;

        // Queue the operation
        bridge_state.pending_operation = Some(PendingPopoverOperation::Hide);

        tracing::info!("Popover hide operation queued");
        Ok(())
    })
}

/// Process any pending popover operations
///
/// This should be called at the end of each frame, after eframe's update() completes.
/// It executes the deferred show/hide operations outside the update loop.
///
/// # Safety
/// Uses unsafe blocks to call NSPopover methods. Must be called from main thread.
#[allow(unsafe_code)]
pub fn process_pending_operations() {
    POPOVER_STATE.with(|state| {
        let mut state_mut = state.borrow_mut();
        if let Some(bridge_state) = state_mut.as_mut() {
            if let Some(operation) = bridge_state.pending_operation.take() {
                match operation {
                    PendingPopoverOperation::Show { button_bounds, status_button } => {
                        tracing::info!("Processing queued show operation");
                        bridge_state.popover.showRelativeToRect_ofView_preferredEdge(
                            button_bounds,
                            &status_button,
                            NSRectEdge::MinY,
                        );
                        tracing::info!("Popover shown successfully");
                    }
                    PendingPopoverOperation::Hide => {
                        tracing::info!("Processing queued hide operation");
                        unsafe {
                            bridge_state.popover.performClose(None);
                        }
                        tracing::info!("Popover hidden successfully");
                    }
                }
            }
        }
    });
}

/// Check if the popover is currently shown
pub fn is_popover_shown() -> bool {
    POPOVER_STATE.with(|state| {
        state.borrow()
            .as_ref()
            .map(|s| s.popover.isShown())
            .unwrap_or(false)
    })
}
