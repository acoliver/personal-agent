//! GpuiBridge - main bridge struct for GPUI side
//!
//! @plan PLAN-20250128-GPUI.P05
//! @requirement REQ-GPUI-006.1
//! @requirement REQ-GPUI-006.2
//! @requirement REQ-GPUI-006.3

use flume::{Receiver, Sender, TrySendError};
use crate::events::types::UserEvent;
use crate::presentation::ViewCommand;

/// Bridge between GPUI (smol) and tokio runtimes
///
/// Owned by the GPUI main panel. Provides non-blocking
/// send/receive for cross-runtime communication.
///
/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006.1
pub struct GpuiBridge {
    user_tx: Sender<UserEvent>,
    view_rx: Receiver<ViewCommand>,
}

impl GpuiBridge {
    /// Create a new bridge with the given channels
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn new(user_tx: Sender<UserEvent>, view_rx: Receiver<ViewCommand>) -> Self {
        Self { user_tx, view_rx }
    }

    /// Emit a UserEvent to the tokio runtime (non-blocking)
    ///
    /// Uses `try_send` to avoid blocking the GPUI thread.
    /// Returns true if sent, false if channel full or disconnected.
    ///
    /// @plan PLAN-20250128-GPUI.P05
    /// @requirement REQ-GPUI-006.2
    pub fn emit(&self, event: UserEvent) -> bool {
        match self.user_tx.try_send(event) {
            Ok(()) => true,
            Err(TrySendError::Full(evt)) => {
                tracing::warn!("UserEvent channel full, dropping: {:?}", evt);
                false
            }
            Err(TrySendError::Disconnected(evt)) => {
                tracing::warn!("UserEvent channel disconnected, dropping: {:?}", evt);
                false
            }
        }
    }

    /// Drain all pending ViewCommands (non-blocking)
    ///
    /// Uses `try_recv` in a loop to collect all available commands.
    /// Returns empty vec if no commands pending.
    ///
    /// @plan PLAN-20250128-GPUI.P05
    /// @requirement REQ-GPUI-006.3
    pub fn drain_commands(&self) -> Vec<ViewCommand> {
        let mut commands = Vec::new();
        while let Ok(cmd) = self.view_rx.try_recv() {
            commands.push(cmd);
        }
        commands
    }

    /// Check if there are pending ViewCommands
    ///
    /// @plan PLAN-20250128-GPUI.P05
    pub fn has_pending_commands(&self) -> bool {
        !self.view_rx.is_empty()
    }
}
