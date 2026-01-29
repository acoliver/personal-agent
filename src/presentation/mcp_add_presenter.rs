//! McpAddPresenter - handles MCP server addition UI
//!
//! McpAddPresenter subscribes to MCP addition events,
//! coordinates with MCP registry service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::events::{AppEvent, types::{McpEvent, UserEvent}};
use crate::services::McpRegistryService;
use super::{Presenter, PresenterError, ViewCommand};

/// McpAddPresenter - handles MCP server addition UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct McpAddPresenter {
    /// Event receiver from EventBus
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to MCP registry service
    mcp_registry_service: Arc<dyn McpRegistryService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl McpAddPresenter {
    /// Create a new McpAddPresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub fn new(
        mcp_registry_service: Arc<dyn McpRegistryService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            mcp_registry_service,
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
        let mcp_registry_service = self.mcp_registry_service.clone();
        let view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&mcp_registry_service, &view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("McpAddPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("McpAddPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("McpAddPresenter event loop ended");
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
        mcp_registry_service: &Arc<dyn McpRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(mcp_registry_service, view_tx, user_evt).await;
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
        mcp_registry_service: &Arc<dyn McpRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SearchMcpRegistry { query, source } => {
                Self::on_search_registry(mcp_registry_service, view_tx, query, source).await;
            }
            UserEvent::SelectMcpFromRegistry { source } => {
                Self::on_select_from_registry(mcp_registry_service, view_tx, source).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle search MCP registry event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_search_registry(
        _mcp_registry_service: &Arc<dyn McpRegistryService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        query: String,
        _source: crate::events::types::McpRegistrySource,
    ) {
        // Placeholder service call - search not fully implemented
        tracing::info!("Searching MCP registry for: {}", query);
    }

    /// Handle select MCP from registry event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_select_from_registry(
        _mcp_registry_service: &Arc<dyn McpRegistryService>,
        _view_tx: &broadcast::Sender<ViewCommand>,
        source: crate::events::types::McpRegistrySource,
    ) {
        // Placeholder service call
        tracing::info!("Loading MCP from registry: {:?}", source);
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_mcp_event(
        _view_tx: &broadcast::Sender<ViewCommand>,
        event: McpEvent,
    ) {
        match event {
            _ => {
                // MCP events handled elsewhere
                tracing::debug!("MCP event in McpAddPresenter: {:?}", event);
            }
        }
    }
}

// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P10
impl Presenter for McpAddPresenter {
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
