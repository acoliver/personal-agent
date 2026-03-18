//! `McpConfigurePresenter` - handles MCP server configuration UI
//!
//! `McpConfigurePresenter` subscribes to MCP configuration events,
//! coordinates with MCP service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::{Presenter, PresenterError, ViewCommand};
use crate::events::{
    types::{McpEvent, UserEvent},
    AppEvent, EventBus,
};
use crate::services::McpService;

/// `McpConfigurePresenter` - handles MCP server configuration UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct McpConfigurePresenter {
    /// Event receiver from `EventBus`
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to MCP service
    mcp_service: Arc<dyn McpService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl McpConfigurePresenter {
    /// Create a new `McpConfigurePresenter`
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

    /// Stub constructor using unified global `EventBus` (REQ-WIRE-006 unification path).
    ///
    /// This constructor accepts Arc<EventBus> directly, subscribing to the global event
    /// bus rather than a caller-supplied `broadcast::Sender`. Full wiring deferred to
    /// later implementation phases.
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
    /// @requirement REQ-WIRE-006
    /// @pseudocode component-001-event-pipeline.md lines 090-136
    #[allow(dead_code)]
    pub fn new_with_event_bus(
        mcp_service: Arc<dyn McpService>,
        event_bus: &Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.sender().subscribe();
        Self {
            rx,
            mcp_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

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
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter shutdown becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    #[must_use]
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
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    async fn handle_user_event(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::ConfigureMcp { id } => {
                Self::on_configure_mcp(mcp_service, view_tx, id).await;
            }
            UserEvent::SaveMcpConfig { id, config } => {
                Self::on_save_config(mcp_service, view_tx, id, config).await;
            }
            UserEvent::StartMcpOAuth { id, provider } => {
                Self::on_start_oauth(mcp_service, view_tx, id, provider).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle save MCP config event (full config payload)
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// Handle configure MCP event
    ///
    /// Loads persisted MCP data and projects it into MCP configure draft payload.
    async fn on_configure_mcp(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        tracing::info!("Loading MCP config for id: {}", id);

        match mcp_service.get(id).await {
            Ok(cfg) => {
                let (command, args) = match cfg.transport {
                    serdes_ai_mcp::McpTransportConfig::Stdio { command, args } => (command, args),
                    serdes_ai_mcp::McpTransportConfig::Http { url }
                    | serdes_ai_mcp::McpTransportConfig::Sse { url } => (url, vec![]),
                };

                let _ = view_tx.send(ViewCommand::McpConfigureDraftLoaded {
                    id: id.to_string(),
                    name: cfg.name,
                    package: String::new(),
                    env_var_name: "API_KEY".to_string(),
                    command,
                    args,
                    env: None,
                });

                let _ = view_tx.send(ViewCommand::NavigateTo {
                    view: super::view_command::ViewId::McpConfigure,
                });
            }
            Err(e) => {
                tracing::error!("Failed to load MCP config {}: {}", id, e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Load Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    async fn on_save_config(
        mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
        config: crate::events::types::McpConfig,
    ) {
        tracing::info!("Saving MCP config for id: {}", id);

        let crate::events::types::McpConfig {
            id: _,
            name,
            command,
            args,
            env,
        } = config;

        let fallback_name = name.clone();
        let result = if id.is_nil() {
            mcp_service
                .add(name, command, args, env)
                .await
                .map(|cfg| cfg.name)
        } else {
            mcp_service
                .update(id, Some(name), Some(command), Some(args), env)
                .await
                .map(|cfg| cfg.name)
        };

        match result {
            Ok(saved_name_from_service) => {
                let resolved_name = if saved_name_from_service.is_empty() {
                    fallback_name
                } else {
                    saved_name_from_service
                };

                let saved_id = if id.is_nil() {
                    match mcp_service.resolve_id_by_name(&resolved_name).await {
                        Ok(Some(found)) => found,
                        Ok(None) => {
                            tracing::warn!(
                                "MCP save created '{}', but service could not resolve id by name; using nil id fallback",
                                resolved_name
                            );
                            uuid::Uuid::nil()
                        }
                        Err(e) => {
                            tracing::warn!(
                                "MCP save created '{}', but id resolution failed: {}; using nil id fallback",
                                resolved_name,
                                e
                            );
                            uuid::Uuid::nil()
                        }
                    }
                } else {
                    id
                };

                let _ = view_tx.send(ViewCommand::McpConfigSaved {
                    id: saved_id,
                    name: Some(resolved_name),
                });
                let _ = view_tx.send(ViewCommand::NavigateBack);
            }
            Err(e) => {
                tracing::error!("MCP config save failed: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Save Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle start OAuth event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_start_oauth(
        _mcp_service: &Arc<dyn McpService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        _id: Uuid,
        provider: String,
    ) {
        tracing::info!("Starting OAuth flow for provider: {}", provider);
        let _ = view_tx.send(ViewCommand::ShowNotification {
            message: format!("Starting OAuth for {provider}"),
        });
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_mcp_event(view_tx: &broadcast::Sender<ViewCommand>, event: McpEvent) {
        if let McpEvent::ConfigSaved { id } = event {
            let _ = view_tx.send(ViewCommand::McpConfigSaved { id, name: None });
        }
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
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
