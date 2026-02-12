//! GPUI-based UI module for PersonalAgent
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-001

pub mod app;
pub mod bridge;
pub mod components;
pub mod navigation;
pub mod navigation_channel;
pub mod popup_window;
pub mod theme;
pub mod tray_bridge;
pub mod views;

pub use app::GpuiApp;
pub use bridge::{GpuiBridge, ViewCommandSink, spawn_user_event_forwarder};
pub use navigation::NavigationState;
pub use navigation_channel::{navigation_channel, NavigationChannel};
pub use popup_window::PopupWindow;
pub use tray_bridge::TrayBridge;
