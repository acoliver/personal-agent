//! Runtime bridge between GPUI (smol) and tokio
//!
//! This module provides the infrastructure for communication between
//! the GPUI UI (smol-based) and the presenter/service layer (tokio-based).
//!
//! ## Architecture
//!
//! ```text
//!     (GPUI/smol)                    (tokio)
//!   ┌─────────────┐              ┌─────────────┐
//!   │ GpuiBridge  │──UserEvent──►│ Forwarder   │──►EventBus
//!   │             │              │             │
//!   │             │◄─ViewCmd────│ViewCmdSink  │◄──Presenter
//!   └─────────────┘   +notify   └─────────────┘
//! ```
//!
//! @plan PLAN-20250128-GPUI.P05
//! @requirement REQ-GPUI-006

pub mod gpui_bridge;
pub mod view_command_sink;
pub mod user_event_forwarder;

pub use gpui_bridge::GpuiBridge;
pub use view_command_sink::{GpuiNotifier, ViewCommandSink};
pub use user_event_forwarder::spawn_user_event_forwarder;
