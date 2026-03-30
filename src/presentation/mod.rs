//! Presentation layer - Presenters coordinate between views and services
//!
//! This module contains presenter implementations following the MVP pattern.
//! Presenters subscribe to events, coordinate service calls, and emit view commands.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                          Views (UI)                              │
//! │  • Pure rendering, no business logic                            │
//! │  • Emit UserEvents on user actions                              │
//! └──────────────────────────────┬──────────────────────────────────┘
//!                                │ UserEvent::*
//!                                ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         EventBus                                 │
//! │  • tokio::sync::broadcast<AppEvent>                             │
//! └──────────────────────────────┬──────────────────────────────────┘
//!                                │ AppEvent::*
//!                                ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        Presenters                                │
//! │  • Subscribe to relevant events                                 │
//! │  • Call services to perform operations                          │
//! │  • Emit ViewCommands to update UI                               │
//! └──────────────────────────────┬──────────────────────────────────┘
//!                                │ method calls
//!                                ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         Services                                 │
//! │  • Pure business logic                                          │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

// Presenter modules
pub mod api_key_manager_presenter;
pub mod chat_presenter;
mod chat_presenter_export;
mod chat_presenter_handlers;
mod conversation_export;
pub mod error_presenter;
pub mod history_presenter;
pub mod mcp_add_presenter;
pub mod mcp_configure_presenter;
pub mod model_selector_presenter;
pub mod profile_editor_presenter;
pub mod settings_presenter;
pub mod view_command;

/// Presenter error type
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, thiserror::Error)]
pub enum PresenterError {
    #[error("Event stream closed")]
    EventStreamClosed,

    #[error("Service call failed: {0}")]
    ServiceCallFailed(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("View channel closed")]
    ViewChannelClosed,
}

/// Base trait for all presenters
///
/// @plan PLAN-20250125-REFACTOR.P10
pub trait Presenter: Send + Sync {
    /// Start the presenter (subscribe to events, initialize state)
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup fails.
    fn start(&mut self) -> Result<(), PresenterError>;

    /// Stop the presenter (unsubscribe from events)
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter shutdown fails.
    fn stop(&mut self) -> Result<(), PresenterError>;

    /// Check if presenter is running
    fn is_running(&self) -> bool;
}

pub use api_key_manager_presenter::ApiKeyManagerPresenter;
pub use chat_presenter::ChatPresenter;
pub use error_presenter::ErrorPresenter;
pub use history_presenter::HistoryPresenter;
pub use mcp_add_presenter::McpAddPresenter;
pub use mcp_configure_presenter::McpConfigurePresenter;
pub use model_selector_presenter::ModelSelectorPresenter;
pub use profile_editor_presenter::ProfileEditorPresenter;
pub use settings_presenter::SettingsPresenter;
/// Re-exports
pub use view_command::ViewCommand;
