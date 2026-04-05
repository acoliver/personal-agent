//! GPUI-based UI module for `PersonalAgent`
//!
//! @plan PLAN-20250128-GPUI.P03
//! @requirement REQ-GPUI-001

#[cfg(target_os = "macos")]
pub mod app;
pub mod app_assets;
pub mod app_store;
mod app_store_streaming;
pub mod app_store_types;

pub mod bridge;
pub mod components;
pub mod error_log;
pub mod mac_native;
pub mod navigation;
pub mod navigation_channel;
#[cfg(target_os = "macos")]
pub mod popup_window;
pub mod selection_intent_channel;
pub mod theme;
pub mod theme_catalog;
#[cfg(target_os = "macos")]
pub mod tray_bridge;
pub mod views;

pub use app_store::{is_store_managed, GpuiAppSnapshot, GpuiAppStore};

#[cfg(target_os = "macos")]
pub use app::GpuiApp;
pub use bridge::{spawn_user_event_forwarder, GpuiBridge, ViewCommandSink};
pub use navigation::NavigationState;
pub use navigation_channel::{navigation_channel, NavigationChannel};
#[cfg(target_os = "macos")]
pub use popup_window::PopupWindow;
pub use selection_intent_channel::{selection_intent_channel, SelectionIntentChannel};
#[cfg(target_os = "macos")]
pub use tray_bridge::TrayBridge;
