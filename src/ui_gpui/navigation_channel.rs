//! Navigation channel for direct GPUI view navigation
//!
//! This provides a simple way for child views to request navigation
//! without going through the full EventBus→Presenter→ViewCommand path.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use crate::presentation::view_command::ViewId;

/// Callback to trigger GPUI notify
type NotifyCallback = Box<dyn Fn() + Send + Sync>;

/// Simple navigation request channel
pub struct NavigationChannel {
    pending_navigation: Mutex<Option<ViewId>>,
    has_request: AtomicBool,
    notify_callback: Mutex<Option<NotifyCallback>>,
}

impl NavigationChannel {
    pub fn new() -> Self {
        Self {
            pending_navigation: Mutex::new(None),
            has_request: AtomicBool::new(false),
            notify_callback: Mutex::new(None),
        }
    }
    
    /// Set a callback to trigger GPUI redraw when navigation is requested
    pub fn set_notify_callback(&self, callback: impl Fn() + Send + Sync + 'static) {
        if let Ok(mut guard) = self.notify_callback.lock() {
            *guard = Some(Box::new(callback));
        }
    }

    /// Request navigation to a view
    pub fn request_navigate(&self, to: ViewId) {
        println!(">>> NavigationChannel::request_navigate({:?}) <<<", to);
        if let Ok(mut guard) = self.pending_navigation.lock() {
            *guard = Some(to.clone());
            self.has_request.store(true, Ordering::SeqCst);
            println!(">>> Navigation stored, has_request=true <<<");
        }
        // Trigger notify callback to force GPUI redraw
        if let Ok(guard) = self.notify_callback.lock() {
            if let Some(ref callback) = *guard {
                println!(">>> Calling notify callback <<<");
                callback();
            } else {
                println!(">>> No notify callback set! <<<");
            }
        }
    }

    /// Request navigation back
    pub fn request_navigate_back(&self) {
        // Use Chat as sentinel for "back" (we'll handle this specially)
        // Actually, let's just not support back for now
    }

    /// Check if there's a pending navigation request
    pub fn has_pending(&self) -> bool {
        self.has_request.load(Ordering::SeqCst)
    }

    /// Take the pending navigation request (clears it)
    pub fn take_pending(&self) -> Option<ViewId> {
        if self.has_request.swap(false, Ordering::SeqCst) {
            if let Ok(mut guard) = self.pending_navigation.lock() {
                guard.take()
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Default for NavigationChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Global navigation channel
static NAVIGATION_CHANNEL: once_cell::sync::Lazy<NavigationChannel> = 
    once_cell::sync::Lazy::new(NavigationChannel::new);

/// Get the global navigation channel
pub fn navigation_channel() -> &'static NavigationChannel {
    &NAVIGATION_CHANNEL
}
