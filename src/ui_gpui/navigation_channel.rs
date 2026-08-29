//! Navigation channel for direct GPUI view navigation
//!
//! This provides a simple way for child views to request navigation
//! without going through the full EventBus→Presenter→ViewCommand path.

use crate::presentation::view_command::ViewId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Callback to trigger GPUI notify
type NotifyCallback = Box<dyn Fn() + Send + Sync>;

/// Simple navigation request channel
pub struct NavigationChannel {
    pending_navigation: Mutex<Option<ViewId>>,
    has_request: AtomicBool,
    notify_callback: Mutex<Option<NotifyCallback>>,
}

impl NavigationChannel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_navigation: Mutex::new(None),
            has_request: AtomicBool::new(false),
            notify_callback: Mutex::new(None),
        }
    }

    /// Set a callback to trigger GPUI redraw when navigation is requested
    pub fn set_notify_callback(&self, callback: impl Fn() + Send + Sync + 'static) {
        // Recover from poisoned mutex by taking the inner value
        if let Ok(mut guard) = self.notify_callback.lock() {
            *guard = Some(Box::new(callback));
        }
    }

    /// Request navigation to a view
    pub fn request_navigate(&self, to: ViewId) {
        // Recover from poisoned mutex by taking the inner value
        if let Ok(mut guard) = self.pending_navigation.lock() {
            *guard = Some(to);
            self.has_request.store(true, Ordering::SeqCst);
        }
        // Trigger notify callback to force GPUI redraw
        if let Ok(guard) = self.notify_callback.lock() {
            if let Some(ref callback) = *guard {
                callback();
            }
        }
    }

    /// Request navigation back
    pub const fn request_navigate_back(&self) {
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
            self.pending_navigation
                .lock()
                .ok()
                .and_then(|mut guard| guard.take())
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

/// Global navigation channel used by the running app.
#[cfg(not(test))]
static NAVIGATION_CHANNEL: once_cell::sync::Lazy<NavigationChannel> =
    once_cell::sync::Lazy::new(NavigationChannel::new);

// One channel per test thread.
//
// The channel holds a single request, so tests sharing one instance clobber
// each other: one asks for McpConfigure, another drains it, and the first
// reads None. Each test runs on its own thread and drives its views
// synchronously there, so a channel per thread removes the sharing rather than
// guarding it. Locking was tried and self-deadlocked, because some tests take
// the helper twice.
#[cfg(test)]
thread_local! {
    static TEST_CHANNEL: &'static NavigationChannel =
        Box::leak(Box::new(NavigationChannel::new()));
}

/// Get the navigation channel.
///
/// One global instance in the app; one per thread under test.
#[must_use]
pub fn navigation_channel() -> &'static NavigationChannel {
    #[cfg(test)]
    {
        TEST_CHANNEL.with(|channel| *channel)
    }
    #[cfg(not(test))]
    {
        &NAVIGATION_CHANNEL
    }
}
