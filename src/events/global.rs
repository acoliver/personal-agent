//! Global `EventBus` Singleton
//!
//! Provides global access to the `EventBus` using `OnceLock`.
//!
//! @plan PLAN-20250125-REFACTOR.P06
//! @requirement REQ-021.4
//! @pseudocode event-bus.md lines 50-75, 150-156

use crate::events::{AppEvent, EventBus, EventBusError};
use std::sync::OnceLock;
use tokio::sync::broadcast;

/// Global `EventBus` singleton
///
/// Lazily initialized on first access.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 50-60
static GLOBAL_BUS: OnceLock<EventBus> = OnceLock::new();

/// Capacity of the global event bus broadcast ring, in events.
///
/// Arithmetic: while a subscriber does not poll, the ring holds at most
/// `GLOBAL_BUS_CAPACITY` events; every event emitted past that evicts the
/// oldest one. A subscriber that resumes polling after `k` events were
/// emitted receives `RecvError::Lagged(k - GLOBAL_BUS_CAPACITY)` and the
/// skipped events are gone for good. So nothing is lost as long as a
/// subscriber falls at most `GLOBAL_BUS_CAPACITY` events behind; with the
/// previous value of 16, any pause spanning 17 or more emissions (a stalled
/// UI task, or the parallel test harness sharing one process-wide ring)
/// silently dropped data. 1024 keeps the loss threshold far enough out that
/// a receiver must miss over a thousand events before losing any.
pub const GLOBAL_BUS_CAPACITY: usize = 1024;

/// Get or initialize the global `EventBus`
///
/// Internal helper function.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 150-156
fn get_or_init_event_bus() -> &'static EventBus {
    GLOBAL_BUS.get_or_init(|| EventBus::new(GLOBAL_BUS_CAPACITY))
}

/// Initialize the global `EventBus`
///
/// Returns the existing instance if already initialized,
/// otherwise creates a new one.
///
/// # Errors
///
/// Returns `EventBusError` if initialization becomes fallible in the future.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @requirement REQ-021.4
/// @pseudocode event-bus.md lines 55-60
pub fn init_event_bus() -> Result<(), EventBusError> {
    // Just ensure the bus is initialized
    let _ = get_or_init_event_bus();
    Ok(())
}

/// Emit an event via the global `EventBus`
///
/// Initializes the `EventBus` on first call if needed.
///
/// # Errors
///
/// Returns `EventBusError::NoSubscribers` when no subscribers are listening.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 65-69
pub fn emit(event: AppEvent) -> Result<(), EventBusError> {
    let bus = get_or_init_event_bus();
    bus.publish(event).map(|_| ())
}

/// Subscribe to events via the global `EventBus`
///
/// Initializes the `EventBus` on first call if needed.
///
/// @plan PLAN-20250125-REFACTOR.P06
/// @pseudocode event-bus.md lines 73-75
#[must_use]
pub fn subscribe() -> broadcast::Receiver<AppEvent> {
    let bus = get_or_init_event_bus();
    bus.subscribe()
}

/// Get a clone of the global `EventBus` for use in Arc
///
/// This is used when you need to share the event bus across threads.
/// The underlying broadcast channel is shared.
#[must_use]
pub fn get_event_bus_clone() -> EventBus {
    // Create a new EventBus that shares the same sender
    // We can't clone the static EventBus, so we subscribe to it
    let bus = get_or_init_event_bus();
    EventBus::from_sender(bus.sender().clone())
}

#[cfg(test)]
mod tests {
    use super::GLOBAL_BUS_CAPACITY;
    use crate::events::bus::EventBus;
    use crate::events::types::{AppEvent, UserEvent};

    /// A subscriber that stops polling while more events are emitted than the
    /// old 16-slot ring could hold still receives every event without lag.
    ///
    /// This pins the global bus capacity arithmetic: at `GLOBAL_BUS_CAPACITY`
    /// slots, a burst of `burst_count` events emitted while the receiver never
    /// polls must be delivered in full when polling resumes, because
    /// `burst_count < GLOBAL_BUS_CAPACITY` means nothing has fallen out of the
    /// ring. With the previous capacity of 16 this burst overflowed the ring
    /// and the first poll returned `RecvError::Lagged`.
    ///
    /// The bus is constructed locally instead of using the global singleton so
    /// the test stays deterministic under the parallel test harness; other
    /// tests emit into the shared process-wide ring concurrently.
    ///
    /// GIVEN: an `EventBus` sized like the global bus
    /// WHEN: more than 16 but fewer than capacity events are emitted while the
    /// subscriber does not poll
    /// THEN: every event is received, in order, with no `Lagged`
    #[tokio::test]
    async fn test_subscriber_survives_burst_beyond_old_ring_without_lag() {
        // Given
        let bus = EventBus::new(GLOBAL_BUS_CAPACITY);
        let mut rx = bus.subscribe();
        let burst_count = GLOBAL_BUS_CAPACITY / 2;
        assert!(
            burst_count > 16,
            "burst must exceed the old 16-slot ring for this test to mean anything"
        );
        assert!(burst_count < GLOBAL_BUS_CAPACITY, "burst must fit the ring");

        // When - emit without polling the receiver
        for i in 0..burst_count {
            let event = AppEvent::User(UserEvent::SendMessage {
                conversation_id: None,
                text: format!("burst-{i}"),
            });
            bus.publish(event)
                .expect("subscriber exists, publish succeeds");
        }

        // Then - every event arrives, in order, with no Lagged
        for i in 0..burst_count {
            let received = rx
                .recv()
                .await
                .expect("event within capacity must not be lagged or lost");
            match received {
                AppEvent::User(UserEvent::SendMessage { text, .. }) => {
                    assert_eq!(text, format!("burst-{i}"), "events arrive in order");
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }
    }
}
