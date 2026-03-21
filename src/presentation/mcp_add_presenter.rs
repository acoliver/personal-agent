//! `McpAddPresenter` - handles MCP server addition UI
//!
//! `McpAddPresenter` subscribes to MCP addition events,
//! coordinates with MCP registry service, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.1

use std::sync::Arc;
use tokio::sync::broadcast;

use super::{Presenter, PresenterError, ViewCommand};
use crate::events::{
    types::{McpEvent, UserEvent},
    AppEvent, EventBus,
};
use crate::services::McpRegistryService;

/// `McpAddPresenter` - handles MCP server addition UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.1
pub struct McpAddPresenter {
    /// Event receiver from `EventBus`
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to MCP registry service
    mcp_registry_service: Arc<dyn McpRegistryService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

struct ManualMcpDraft {
    name: String,
    package: String,
    package_type: crate::mcp::McpPackageType,
    runtime_hint: Option<String>,
    command: String,
    args: Vec<String>,
    url: Option<String>,
}

impl McpAddPresenter {
    /// Create a new `McpAddPresenter`
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
        mcp_registry_service: Arc<dyn McpRegistryService>,
        event_bus: &Arc<EventBus>,
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
            UserEvent::McpAddNext { manual_entry } => {
                Self::on_mcp_add_next(mcp_registry_service, view_tx, manual_entry).await;
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
        tracing::info!(
            "Searching MCP registry source='{}' for: {}",
            source.name,
            query
        );

        match mcp_registry_service
            .search_registry(&query, &source.name)
            .await
        {
            Ok(entries) => {
                tracing::debug!("MCP registry search returned {} results", entries.len());

                let results = entries
                    .into_iter()
                    .map(|entry| super::view_command::McpRegistryResult {
                        id: entry.name.clone(),
                        name: entry.display_name,
                        description: entry.description,
                        source: entry.source,
                        command: entry.command,
                        args: entry.args,
                        env: entry.env,
                        package_type: entry.package_type,
                        runtime_hint: entry.runtime_hint,
                        url: entry.url,
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
                tracing::debug!(
                    "MCP registry entries loaded for selection: {}",
                    entries.len()
                );

                let source_name = source.name;
                let (source_hint, requested_name) = source_name.split_once("::").map_or_else(
                    || ("official".to_string(), source_name.clone()),
                    |(source, name)| (source.to_string(), name.to_string()),
                );

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
                        id: format!("{source_hint}::{package_name}"),
                        name: configure_name,
                        package: package_name,
                        package_type: entry
                            .package_type
                            .unwrap_or(crate::mcp::McpPackageType::Npm),
                        runtime_hint: entry.runtime_hint,
                        env_var_name,
                        command: entry.command,
                        args: entry.args,
                        env: entry.env,
                        url: entry.url,
                    });
                    let _ = view_tx.send(ViewCommand::NavigateTo {
                        view: super::view_command::ViewId::McpConfigure,
                    });
                } else {
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "Selection Failed".to_string(),
                        message: format!("MCP '{requested_name}' not found in registry"),
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

    /// Handle `McpAddNext`: user advanced to next step in MCP add wizard
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-001
    /// @pseudocode component-005-mcp-flow.md lines 015-033
    async fn on_mcp_add_next(
        _mcp_registry_service: &Arc<dyn McpRegistryService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        manual_entry: Option<String>,
    ) {
        tracing::info!("McpAddPresenter: handling McpAddNext");

        let Some(raw_entry) = manual_entry.map(|entry| entry.trim().to_string()) else {
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Manual Entry Required".to_string(),
                message: "Enter an MCP package, docker image, or URL before continuing."
                    .to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        };

        if raw_entry.is_empty() {
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Manual Entry Required".to_string(),
                message: "Enter an MCP package, docker image, or URL before continuing."
                    .to_string(),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }

        match Self::parse_manual_entry(&raw_entry) {
            Ok(draft) => {
                let _ = view_tx.send(ViewCommand::McpConfigureDraftLoaded {
                    id: uuid::Uuid::nil().to_string(),
                    name: draft.name,
                    package: draft.package,
                    package_type: draft.package_type,
                    runtime_hint: draft.runtime_hint,
                    env_var_name: "API_KEY".to_string(),
                    command: draft.command,
                    args: draft.args,
                    env: None,
                    url: draft.url,
                });
                let _ = view_tx.send(ViewCommand::NavigateTo {
                    view: super::view_command::ViewId::McpConfigure,
                });
            }
            Err(message) => {
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Invalid Manual Entry".to_string(),
                    message,
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
        }
    }

    fn parse_manual_entry(raw_entry: &str) -> Result<ManualMcpDraft, String> {
        let trimmed = raw_entry.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            let name = trimmed
                .trim_end_matches('/')
                .split('/')
                .next_back()
                .filter(|segment| !segment.is_empty())
                .unwrap_or("mcp")
                .to_string();
            return Ok(ManualMcpDraft {
                name,
                package: trimmed.to_string(),
                package_type: crate::mcp::McpPackageType::Http,
                runtime_hint: None,
                command: String::new(),
                args: vec![],
                url: Some(trimmed.to_string()),
            });
        }

        if trimmed.starts_with("docker ") {
            let image = trimmed
                .split_whitespace()
                .last()
                .ok_or_else(|| "Invalid docker command".to_string())?
                .to_string();
            let name = image
                .split(':')
                .next()
                .unwrap_or(&image)
                .split('/')
                .next_back()
                .unwrap_or(&image)
                .to_string();
            return Ok(ManualMcpDraft {
                name,
                package: image,
                package_type: crate::mcp::McpPackageType::Docker,
                runtime_hint: Some("docker".to_string()),
                command: "docker".to_string(),
                args: trimmed
                    .split_whitespace()
                    .skip(1)
                    .map(ToString::to_string)
                    .collect(),
                url: None,
            });
        }

        if trimmed.starts_with("npx ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let identifier = parts
                .iter()
                .skip(1)
                .find(|part| !part.starts_with('-'))
                .ok_or_else(|| "Invalid npx command".to_string())?
                .to_string();
            let name = identifier
                .split('/')
                .next_back()
                .unwrap_or(&identifier)
                .to_string();
            return Ok(ManualMcpDraft {
                name,
                package: identifier,
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                command: "npx".to_string(),
                args: parts
                    .iter()
                    .skip(1)
                    .map(|part| (*part).to_string())
                    .collect(),
                url: None,
            });
        }

        if trimmed.starts_with('@') || trimmed.contains('/') {
            let name = trimmed
                .split('/')
                .next_back()
                .unwrap_or(trimmed)
                .to_string();
            return Ok(ManualMcpDraft {
                name,
                package: trimmed.to_string(),
                package_type: crate::mcp::McpPackageType::Npm,
                runtime_hint: Some("npx".to_string()),
                command: "npx".to_string(),
                args: vec![trimmed.to_string()],
                url: None,
            });
        }

        Err("Use a package like @scope/package, an npx command, a docker command, or an http(s) URL.".to_string())
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    async fn handle_mcp_event(_view_tx: &broadcast::Sender<ViewCommand>, event: McpEvent) {
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
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
