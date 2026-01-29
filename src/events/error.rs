// EventBus Error Types
//
// Defines error types for the EventBus.
//
// @plan PLAN-20250125-REFACTOR.P04
// @pseudocode event-bus.md lines 160-162

use thiserror::Error;

/// EventBus error types
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @pseudocode event-bus.md lines 160-162
#[derive(Debug, Error)]
pub enum EventBusError {
    /// No subscribers are currently listening for events
    #[error("No subscribers")]
    NoSubscribers,

    /// The broadcast channel has been closed
    #[error("Channel closed")]
    ChannelClosed,
}
