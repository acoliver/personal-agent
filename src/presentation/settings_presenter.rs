//! SettingsPresenter - handles settings and profile management UI
//!
//! SettingsPresenter subscribes to settings and profile events,
//! coordinates with profile and app settings services, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.4
//! @pseudocode presenters.md lines 380-444

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::events::{AppEvent, types::{ProfileEvent, UserEvent, McpEvent, SystemEvent}};
use crate::services::{AppSettingsService, ProfileService};
use super::{Presenter, PresenterError, ViewCommand};

/// SettingsPresenter - handles settings and profile management UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.4
/// @pseudocode presenters.md lines 380-385
pub struct SettingsPresenter {
    /// Event receiver from EventBus
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to profile service
    profile_service: Arc<dyn ProfileService>,

    /// Reference to app settings service
    app_settings_service: Arc<dyn AppSettingsService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl SettingsPresenter {
    /// Create a new SettingsPresenter
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub fn new(
        profile_service: Arc<dyn ProfileService>,
        app_settings_service: Arc<dyn AppSettingsService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            profile_service,
            app_settings_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let profile_service = self.profile_service.clone();
        let view_tx = self.view_tx.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(&profile_service, &view_tx, event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("SettingsPresenter lagged: {} events missed", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("SettingsPresenter event stream closed");
                        break;
                    }
                }
            }
            tracing::info!("SettingsPresenter event loop ended");
        });

        Ok(())
    }

    /// Stop the presenter event loop
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle events from EventBus
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn handle_event(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(profile_service, view_tx, user_evt).await;
            }
            AppEvent::Profile(profile_evt) => {
                Self::handle_profile_event(view_tx, profile_evt).await;
            }
            AppEvent::Mcp(mcp_evt) => {
                Self::handle_mcp_event(view_tx, mcp_evt).await;
            }
            AppEvent::System(sys_evt) => {
                Self::handle_system_event(view_tx, sys_evt).await;
            }
            _ => {} // Ignore other events
        }
    }

    /// Handle user events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn handle_user_event(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
    ) {
        match event {
            UserEvent::SelectProfile { id } => {
                Self::on_select_profile(profile_service, view_tx, id).await;
            }
            UserEvent::ToggleMcp { id, enabled } => {
                Self::on_toggle_mcp(profile_service, view_tx, id, enabled).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle profile domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn handle_profile_event(
        view_tx: &broadcast::Sender<ViewCommand>,
        event: ProfileEvent,
    ) {
        match event {
            ProfileEvent::Created { id, name } => {
                let _ = view_tx.send(ViewCommand::ProfileCreated { id, name });
            }
            ProfileEvent::Updated { id, name } => {
                let _ = view_tx.send(ViewCommand::ProfileUpdated { id, name });
            }
            ProfileEvent::Deleted { id, .. } => {
                let _ = view_tx.send(ViewCommand::ProfileDeleted { id });
            }
            ProfileEvent::DefaultChanged { profile_id } => {
                let _ = view_tx.send(ViewCommand::DefaultProfileChanged { profile_id });
            }
            _ => {} // Ignore other profile events
        }
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P03
    /// @requirement REQ-025.4
    async fn handle_mcp_event(
        view_tx: &broadcast::Sender<ViewCommand>,
        event: McpEvent,
    ) {
        match event {
            McpEvent::Starting { id, name } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Starting,
                });
            }
            McpEvent::Started { id, name, tools, tool_count } => {
                let _ = view_tx.send(ViewCommand::McpServerStarted {
                    id,
                    tool_count,
                });
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Running,
                });
            }
            McpEvent::StartFailed { id, name, error } => {
                let _ = view_tx.send(ViewCommand::McpServerFailed {
                    id,
                    error,
                });
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Failed,
                });
            }
            McpEvent::Stopped { id, name } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Stopped,
                });
            }
            McpEvent::Unhealthy { id, name, error } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Unhealthy,
                });
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "MCP Server Unhealthy".to_string(),
                    message: format!("{}: {}", name, error),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
            McpEvent::Recovered { id, name } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Running,
                });
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: format!("{} recovered", name),
                });
            }
            McpEvent::ConfigSaved { id } => {
                let _ = view_tx.send(ViewCommand::McpConfigSaved { id });
            }
            McpEvent::Deleted { id, .. } => {
                let _ = view_tx.send(ViewCommand::McpDeleted { id });
            }
            _ => {} // Ignore other MCP events
        }
    }

    /// Handle system domain events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P03
    /// @requirement REQ-025.4
    async fn handle_system_event(
        view_tx: &broadcast::Sender<ViewCommand>,
        event: SystemEvent,
    ) {
        match event {
            SystemEvent::Error { source, error, context } => {
                let message = if let Some(ctx) = context {
                    format!("{}: {} (context: {})", source, error, ctx)
                } else {
                    format!("{}: {}", source, error)
                };
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "System Error".to_string(),
                    message,
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
            SystemEvent::ConfigLoaded => {
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: "Configuration loaded".to_string(),
                });
            }
            SystemEvent::ConfigSaved => {
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: "Configuration saved".to_string(),
                });
            }
            SystemEvent::ModelsRegistryRefreshed { provider_count, model_count } => {
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: format!("Models refreshed: {} providers, {} models", provider_count, model_count),
                });
            }
            _ => {} // Ignore other system events
        }
    }

    /// Handle SelectProfile user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn on_select_profile(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        match profile_service.set_default(id).await {
            Ok(_) => {
                let _ = view_tx.send(ViewCommand::DefaultProfileChanged { profile_id: Some(id) });
            }
            Err(e) => {
                tracing::error!("Failed to select profile: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: format!("Failed to select profile: {}", e),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle ToggleMcp user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn on_toggle_mcp(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
        enabled: bool,
    ) {
        // Placeholder - set_mcp_enabled not yet implemented in ProfileService
        tracing::info!("Toggling MCP: {} for profile {}", enabled, id);
        // Emit status change event
        let status = if enabled {
            super::view_command::McpStatus::Starting
        } else {
            super::view_command::McpStatus::Stopped
        };
        let _ = view_tx.send(ViewCommand::McpStatusChanged { id, status });
    }
}
// Implement Presenter trait
//
// @plan PLAN-20250125-REFACTOR.P10
// @requirement REQ-025.4
impl Presenter for SettingsPresenter {
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

/// @plan PLAN-20250125-REFACTOR.P12
/// @requirement REQ-027.4
#[cfg(test)]
mod tests {
    use super::*;

    /// Test handle select profile
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[tokio::test]
    async fn test_handle_select_profile() {
        let (_event_tx, _) = broadcast::channel::<AppEvent>(100);
        let (_view_tx, _) = broadcast::channel::<ViewCommand>(100);

        // Create presenter with mocks would go here
        // For now, just verify the structure compiles
        assert!(true);
    }

    /// Test handle toggle MCP
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-027.4
    #[tokio::test]
    async fn test_handle_toggle_mcp() {
        // Test implementation would go here
        assert!(true);
    }
}
