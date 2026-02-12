//! ViewCommandSink - wrapper for presenters to send ViewCommands
//!
//! @plan PLAN-20250128-GPUI.P05
//! @requirement REQ-GPUI-006.4
//! @requirement REQ-GPUI-006.5

use flume::{Sender, TrySendError};
use crate::presentation::ViewCommand;

/// Notifier handle to wake GPUI from tokio
///
/// @plan PLAN-20250128-GPUI.P05
pub trait GpuiNotifier: Send + Sync + Clone {
    /// Wake the GPUI thread to process ViewCommands
    fn notify(&self);
}

/// Sink for presenters to send ViewCommands to GPUI
///
/// Wraps a flume sender with a GPUI notifier.
/// After sending a command, calls notify() to wake GPUI.
///
/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006.4
pub struct ViewCommandSink<N: GpuiNotifier> {
    tx: Sender<ViewCommand>,
    notifier: N,
}

impl<N: GpuiNotifier> ViewCommandSink<N> {
    /// Create a new sink with the given sender and notifier
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn new(tx: Sender<ViewCommand>, notifier: N) -> Self {
        Self { tx, notifier }
    }

    /// Send a ViewCommand and wake GPUI (non-blocking)
    ///
    /// Uses `try_send` to avoid blocking the tokio task.
    /// Always calls notify() to ensure GPUI wakes up.
    ///
    /// @plan PLAN-20250128-GPUI.P05
    /// @requirement REQ-GPUI-006.5
    pub fn send(&self, cmd: ViewCommand) {
        match self.tx.try_send(cmd) {
            Ok(()) => {
                self.notifier.notify();
            }
            Err(TrySendError::Full(cmd)) => {
                tracing::warn!("ViewCommand channel full, dropping: {:?}", cmd);
                // Still notify in case GPUI is behind on draining
                self.notifier.notify();
            }
            Err(TrySendError::Disconnected(cmd)) => {
                tracing::info!("ViewCommand channel disconnected (GPUI closed), dropping: {:?}", cmd);
                // No point notifying if disconnected
            }
        }
    }

    /// Clone the sender (for passing to multiple presenters)
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn clone_sender(&self) -> Sender<ViewCommand> {
        self.tx.clone()
    }
}

impl<N: GpuiNotifier> Clone for ViewCommandSink<N> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            notifier: self.notifier.clone(),
        }
    }
}
