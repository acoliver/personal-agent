//! McpAddPresenter - handles MCP server addition UI
//!
//! McpAddPresenter subscribes to MCP addition events,
//! coordinates with MCP registry service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::events::{AppEvent, EventBus, types::{McpEvent, UserEvent}};
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

    /// Stub constructor using unified global EventBus (REQ-WIRE-006 unification path).
    ///
    /// This constructor accepts Arc<EventBus> directly, subscribing to the global event
    /// bus rather than a caller-supplied broadcast::Sender. Full wiring deferred to
    /// later implementation phases.
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
    /// @requirement REQ-WIRE-006
    /// @pseudocode component-001-event-pipeline.md lines 090-136
    #[allow(dead_code)]
    pub fn new_with_event_bus(
        mcp_registry_service: Arc<dyn McpRegistryService>,
        event_bus: Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.sender().subscribe();
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
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
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
            UserEvent::McpAddNext => {
                Self::on_mcp_add_next(mcp_registry_service, view_tx).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle search MCP registry event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_search_registry(
        mcp_registry_service: &Arc<dyn McpRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        query: String,
        source: crate::events::types::McpRegistrySource,
    ) {
        tracing::info!("Searching MCP registry source='{}' for: {}", source.name, query);

        // Current registry service is source-agnostic; preserve caller source in projected payload
        // so the UI can keep source-specific context while backend support evolves.
        let source_name = source.name;
        match mcp_registry_service.search(&query).await {
            Ok(entries) => {
                tracing::debug!("MCP registry search returned {} results", entries.len());

                let results = entries
                    .into_iter()
                    .map(|entry| super::view_command::McpRegistryResult {
                        id: entry.name.clone(),
                        name: entry.display_name,
                        description: entry.description,
                        source: source_name.clone(),
                        command: entry.command,
                        args: entry.args,
                        env: entry.env,
                    })
                    .collect();

                let _ = view_tx.send(ViewCommand::McpRegistrySearchResults { results });
            }
            Err(e) => {
                tracing::error!("MCP registry search failed: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Search Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }

    /// Handle select MCP from registry event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn on_select_from_registry(
        mcp_registry_service: &Arc<dyn McpRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        source: crate::events::types::McpRegistrySource,
    ) {
        tracing::info!("Loading MCP from registry: {:?}", source);
        match mcp_registry_service.list_all().await {
            Ok(entries) => {
                tracing::debug!("MCP registry entries loaded for selection: {}", entries.len());

                let source_name = source.name;
                let (source_hint, requested_name) = source_name
                    .split_once("::")
                    .map_or(("official".to_string(), source_name.clone()), |(source, name)| {
                        (source.to_string(), name.to_string())
                    });

                let selected = entries.into_iter().find(|e| e.name == requested_name);
                if let Some(entry) = selected {
                    let env_var_name = entry
                        .env
                        .as_ref()
                        .and_then(|vars| vars.first().map(|(k, _)| k.clone()))
                        .unwrap_or_else(|| "API_KEY".to_string());

                    let configure_name = entry.display_name;
                    let package_name = entry.name;
                    let _ = view_tx.send(ViewCommand::McpConfigureDraftLoaded {
                        id: format!("{}::{}", source_hint, package_name),
                        name: configure_name,
                        package: package_name,
                        env_var_name,
                        command: entry.command,
                        args: entry.args,
                        env: entry.env,
                    });
                    let _ = view_tx.send(ViewCommand::NavigateTo {
                        view: super::view_command::ViewId::McpConfigure,
                    });
                } else {
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Selection Failed".to_string(),
                        message: format!("MCP '{}' not found in registry", requested_name),
                        severity: super::view_command::ErrorSeverity::Warning,
                    });
                }
            }
            Err(e) => {
                tracing::error!("MCP registry load failed: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Load Failed".to_string(),
                    message: e.to_string(),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }

    /// Handle McpAddNext: user advanced to next step in MCP add wizard
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    /// @pseudocode component-005-mcp-flow.md lines 015-033
    async fn on_mcp_add_next(
        _mcp_registry_service: &Arc<dyn McpRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        tracing::info!("McpAddPresenter: handling McpAddNext");
        // Advance to MCP configure view for the selected server
        let _ = view_tx.send(ViewCommand::NavigateTo {
            view: super::view_command::ViewId::McpConfigure,
        });
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_mcp_event(
        _view_tx: &broadcast::Sender<ViewCommand>,
        event: McpEvent,
    ) {
        tracing::debug!("MCP event in McpAddPresenter: {:?}", event);
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
