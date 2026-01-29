//! McpConfigurePresenter - handles MCP server configuration UI
//!
//! McpConfigurePresenter subscribes to MCP configuration events,
//! coordinates with MCP service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::events::{AppEvent, types::{McpEvent, UserEvent}};
use crate::services::McpService;
use super::{Presenter, PresenterError, ViewCommand};

/// McpConfigurePresenter - handles MCP server configuration UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct McpConfigurePresenter {
    /// Event receiver from EventBus
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to MCP service
    mcp_service: Arc<dyn McpService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl McpConfigurePresenter {
    /// Create a new McpConfigurePresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn new(
        mcp_service: Arc<dyn McpService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            mcp_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let mcp_service = self.mcp_service.clone();
        let view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&mcp_service, &view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("McpConfigurePresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("McpConfigurePresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("McpConfigurePresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle incoming events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_event(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(mcp_service, view_tx, user_evt).await;
            }
            AppEvent::Mcp(mcp_evt) => {
                Self::handle_mcp_event(view_tx, mcp_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_user_event(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SaveMcpConfig { id, config } => {
                Self::on_save_config(mcp_service, view_tx, id, config).await;
            }
            UserEvent::StartMcpOAuth { id, provider } => {
                Self::on_start_oauth(mcp_service, view_tx, id, provider).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle save MCP config event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_save_config(
        _mcp_service: &Arc<dyn McpService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        _id: Uuid,
        _config: crate::events::types::McpConfig,
    ) {
        // Placeholder service call - config save not fully implemented
        tracing::info!("Saving MCP config");
    }

    /// Handle start OAuth event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_start_oauth(
        _mcp_service: &Arc<dyn McpService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        _id: Uuid,
        provider: String,
    ) {
        // Placeholder service call - OAuth not fully implemented
        tracing::info!("Starting OAuth flow for provider: {}", provider);
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_mcp_event(
        _view_tx: &broadcast::Sender<ViewCommand>,
        event: McpEvent,
    ) {
        match event {
            McpEvent::ConfigSaved { id: _id } => {
                // Config saved - emit ViewCommand if needed
            }
            _ => {} // Ignore other MCP events
        }
    }

    /// Handle config saved event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_config_saved(
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        let _ = view_tx.send(ViewCommand::McpConfigSaved { id });
        tracing::info!("MCP server configured: {}", id);
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P10
impl Presenter for McpConfigurePresenter {
    fn start(&mut self) -> Result<(), PresenterError> {
        // Note: This is a sync wrapper - in real usage, call async start() directly
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
