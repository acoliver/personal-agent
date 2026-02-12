//! UserEvent forwarder - tokio task that bridges flume to EventBus
//!
//! @plan PLAN-20250128-GPUI.P05
//! @requirement REQ-GPUI-006.1

use std::sync::Arc;
use flume::Receiver;
use tokio::task::JoinHandle;
use crate::events::{AppEvent, EventBus};
use crate::events::types::UserEvent;

/// Spawn a tokio task that forwards UserEvents from flume to EventBus
///
/// This task runs on the tokio runtime and:
/// 1. Receives UserEvents from the flume channel (from GPUI)
/// 2. Wraps them in AppEvent::User
/// 3. Publishes to the EventBus
///
/// The task exits when the sender is dropped (GPUI closed).
///
/// @plan PLAN-20250128-GPUI.P05
/// @requirement REQ-GPUI-006.1
pub fn spawn_user_event_forwarder(
    event_bus: Arc<EventBus>,
    user_rx: Receiver<UserEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("UserEvent forwarder started");
        
        while let Ok(event) = user_rx.recv_async().await {
            tracing::debug!("Forwarding UserEvent: {:?}", event);
            
            // Publish to EventBus (may fail if no subscribers, that's OK)
            if let Err(e) = event_bus.publish(AppEvent::User(event)) {
                tracing::debug!("EventBus publish failed (no subscribers?): {:?}", e);
            }
        }
        
        tracing::info!("UserEvent forwarder shutting down (sender dropped)");
    })
}
