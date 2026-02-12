//! Global EventBus Singleton
//!
//! Provides global access to the EventBus using OnceLock.
//!
//! @plan PLAN-20250125-REFACTOR.P06
//! @requirement REQ-021.4
//! @pseudocode event-bus.md lines 50-75, 150-156

use crate::events::{AppEvent, EventBus, EventBusError};
use std::sync::OnceLock;
use tokio::sync::broadcast;

/// Global EventBus singleton
///
/// Lazily initialized on first access.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 50-60
static GLOBAL_BUS: OnceLock<EventBus> = OnceLock::new();

/// Get or initialize the global EventBus
///
/// Internal helper function.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 150-156
fn get_or_init_event_bus() -> &'static EventBus {
    GLOBAL_BUS.get_or_init(|| EventBus::new(16))
}

/// Initialize the global EventBus
///
/// Returns the existing instance if already initialized,
/// otherwise creates a new one.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 55-60
pub fn init_event_bus() -> Result<(), EventBusError> {
    // Just ensure the bus is initialized
    let _ = get_or_init_event_bus();
    Ok(())
}

/// Emit an event via the global EventBus
///
/// Initializes the EventBus on first call if needed.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 65-69
pub fn emit(event: AppEvent) -> Result<(), EventBusError> {
    let bus = get_or_init_event_bus();
    match bus.publish(event) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Subscribe to events via the global EventBus
///
/// Initializes the EventBus on first call if needed.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 73-75
pub fn subscribe() -> broadcast::Receiver<AppEvent> {
    let bus = get_or_init_event_bus();
    bus.subscribe()
}

/// Get a clone of the global EventBus for use in Arc
///
/// This is used when you need to share the event bus across threads.
/// The underlying broadcast channel is shared.
pub fn get_event_bus_clone() -> EventBus {
    // Create a new EventBus that shares the same sender
    // We can't clone the static EventBus, so we subscribe to it
    let bus = get_or_init_event_bus();
    EventBus::from_sender(bus.sender().clone())
}
