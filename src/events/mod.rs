//! Event System Module
//!
//! Provides centralized event bus for decoupling components using
//! tokio::sync::broadcast.
//!
//! # Architecture
//!
//! The event system enables loose coupling between components:
//! - Views emit UserEvents to express user intent
//! - Services emit domain events (ChatEvent, McpEvent, etc.) to report state changes
//! - Presenters subscribe to relevant events and coordinate with services
//! - ViewCommands update the UI in response to events
//!
//! @plan PLAN-20250125-REFACTOR.P04
//! @requirement REQ-019.5

pub mod bus;
pub mod error;
pub mod global;
pub mod types;

// Re-export commonly used types
pub use bus::EventBus;
pub use error::EventBusError;
pub use global::{emit, subscribe};
pub use types::AppEvent;
