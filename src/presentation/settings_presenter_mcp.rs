//! MCP/system/profile domain handlers for `SettingsPresenter`.

use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use super::settings_presenter::SettingsPresenter;
use super::view_command::{self, ViewCommand};
use crate::events::{emit, types::McpEvent, types::ProfileEvent, types::SystemEvent, AppEvent};
use crate::services::{AppSettingsService, ProfileService};

impl SettingsPresenter {
    /// Handle profile domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    pub(super) async fn handle_profile_event(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: ProfileEvent,
    ) {
        match event {
            ProfileEvent::Created { id, name } => {
                let _ = view_tx.send(ViewCommand::ProfileCreated { id, name });
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
            }
            ProfileEvent::Updated { id, name } => {
                let _ = view_tx.send(ViewCommand::ProfileUpdated { id, name });
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
            }
            ProfileEvent::Deleted { id, .. } => {
                let _ = view_tx.send(ViewCommand::ProfileDeleted { id });
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
            }
            ProfileEvent::DefaultChanged { profile_id } => {
                let persist_result = match profile_id {
                    Some(id) => app_settings_service.set_default_profile_id(id).await,
                    None => app_settings_service.clear_default_profile_id().await,
                };

                if let Err(error) = persist_result {
                    tracing::warn!(
                        "Failed to persist default profile update from ProfileEvent::DefaultChanged: {}",
                        error
                    );
                    let live_default = profile_service
                        .get_default()
                        .await
                        .ok()
                        .flatten()
                        .map(|profile| profile.id);
                    let _ = view_tx.send(ViewCommand::DefaultProfileChanged {
                        profile_id: live_default,
                    });
                    Self::emit_profiles_snapshot_with_default(
                        profile_service,
                        live_default,
                        view_tx,
                    )
                    .await;
                    return;
                }

                let _ = view_tx.send(ViewCommand::DefaultProfileChanged { profile_id });
                Self::emit_profiles_snapshot_with_default(profile_service, profile_id, view_tx)
                    .await;
            }
            _ => {} // Ignore other profile events
        }
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P03
    /// @requirement REQ-025.4
    pub(super) async fn handle_mcp_event(
        view_tx: &broadcast::Sender<ViewCommand>,
        event: McpEvent,
    ) {
        match event {
            McpEvent::Starting { id, name: _ } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: view_command::McpStatus::Starting,
                });
            }
            McpEvent::Started {
                id,
                name,
                tools: _,
                tool_count,
            } => {
                let _ = view_tx.send(ViewCommand::McpServerStarted {
                    id,
                    name: Some(name),
                    tool_count,
                    enabled: None,
                });
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: view_command::McpStatus::Running,
                });
            }
            McpEvent::StartFailed { id, name: _, error } => {
                let _ = view_tx.send(ViewCommand::McpServerFailed { id, error });
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: view_command::McpStatus::Failed,
                });
            }
            McpEvent::Stopped { id, name: _ } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: view_command::McpStatus::Stopped,
                });
            }
            McpEvent::Unhealthy { id, name, error } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: view_command::McpStatus::Unhealthy,
                });
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "MCP Server Unhealthy".to_string(),
                    message: format!("{name}: {error}"),
                    severity: view_command::ErrorSeverity::Warning,
                });
            }
            McpEvent::Recovered { id, name } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: view_command::McpStatus::Running,
                });
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: format!("{name} recovered"),
                });
            }
            McpEvent::ConfigSaved { id } => {
                let _ = view_tx.send(ViewCommand::McpConfigSaved { id, name: None });
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
    pub(super) async fn handle_system_event(
        view_tx: &broadcast::Sender<ViewCommand>,
        event: SystemEvent,
    ) {
        match event {
            SystemEvent::Error {
                source,
                error,
                context,
            } => {
                let message = context.map_or_else(
                    || format!("{source}: {error}"),
                    |ctx| format!("{source}: {error} (context: {ctx})"),
                );
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "System Error".to_string(),
                    message,
                    severity: view_command::ErrorSeverity::Error,
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
            SystemEvent::ModelsRegistryRefreshed {
                provider_count,
                model_count,
            } => {
                let _ = view_tx.send(ViewCommand::ShowNotification {
                    message: format!(
                        "Models refreshed: {provider_count} providers, {model_count} models"
                    ),
                });
            }
            _ => {} // Ignore other system events
        }
    }

    /// Handle `SelectProfile` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    pub(super) async fn on_select_profile(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        match profile_service.set_default(id).await {
            Ok(()) => {
                if let Err(e) = app_settings_service.set_default_profile_id(id).await {
                    tracing::warn!("Failed to persist default profile in app settings: {}", e);
                    let live_default = profile_service
                        .get_default()
                        .await
                        .ok()
                        .flatten()
                        .map(|profile| profile.id);
                    let _ = view_tx.send(ViewCommand::DefaultProfileChanged {
                        profile_id: live_default,
                    });
                    Self::emit_profiles_snapshot_with_default(
                        profile_service,
                        live_default,
                        view_tx,
                    )
                    .await;
                    return;
                }
                let _ = view_tx.send(ViewCommand::DefaultProfileChanged {
                    profile_id: Some(id),
                });
                Self::emit_profiles_snapshot_with_default(profile_service, Some(id), view_tx).await;
            }
            Err(e) => {
                tracing::error!("Failed to select profile: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: format!("Failed to select profile: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle `EditProfile` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    pub(super) async fn on_edit_profile(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        match profile_service.get(id).await {
            Ok(profile) => {
                let api_key_label = match &profile.auth {
                    crate::models::AuthConfig::Keychain { label } => label.clone(),
                    crate::models::AuthConfig::None => String::new(),
                };

                let _ = view_tx.send(ViewCommand::ProfileEditorLoad {
                    id: profile.id,
                    name: profile.name,
                    provider_id: profile.provider_id,
                    model_id: profile.model_id,
                    base_url: profile.base_url,
                    api_key_label,
                    temperature: profile.parameters.temperature,
                    max_tokens: profile.parameters.max_tokens,
                    max_tokens_field_name: profile
                        .parameters
                        .max_tokens_field_name
                        .clone()
                        .unwrap_or_else(|| "max_tokens".to_string()),
                    extra_request_fields: profile
                        .parameters
                        .extra_request_fields
                        .clone()
                        .map_or_else(|| "{}".to_string(), |value| value.to_string()),
                    context_limit: None,
                    show_thinking: profile.parameters.show_thinking,
                    enable_thinking: profile.parameters.enable_thinking,
                    thinking_budget: profile.parameters.thinking_budget,
                    system_prompt: profile.system_prompt,
                });
                let _ = view_tx.send(ViewCommand::NavigateTo {
                    view: view_command::ViewId::ProfileEditor,
                });
            }
            Err(e) => {
                tracing::error!("Failed to load profile for edit: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Edit Failed".to_string(),
                    message: format!("Failed to load profile: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle `DeleteProfile` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    pub(super) async fn on_delete_profile(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        let profile_name = profile_service
            .get(id)
            .await
            .ok()
            .map_or_else(|| "Profile".to_string(), |profile| profile.name);

        match profile_service.delete(id).await {
            Ok(()) => {
                let _ = emit(AppEvent::Profile(ProfileEvent::Deleted {
                    id,
                    name: profile_name,
                }));
                let _ = view_tx.send(ViewCommand::ProfileDeleted { id });
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
            }
            Err(e) => {
                tracing::error!("Failed to delete profile: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Delete Failed".to_string(),
                    message: format!("Failed to delete profile: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Toggle an MCP's enabled state in config.json, reload the global MCP runtime,
    /// and emit the updated status.
    pub(super) async fn on_toggle_mcp(
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
        enabled: bool,
        config_path_override: Option<&std::path::Path>,
    ) {
        tracing::info!("Toggling MCP {id} enabled={enabled}");
        let config_path = match config_path_override {
            Some(p) => p.to_path_buf(),
            None => match crate::config::Config::default_path() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Cannot resolve config path for MCP toggle: {e}");
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "MCP Toggle Failed".to_string(),
                        message: format!("Failed to resolve config path: {e}"),
                        severity: view_command::ErrorSeverity::Error,
                    });
                    return;
                }
            },
        };
        let mut config = match crate::config::Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Cannot load config for MCP toggle: {e}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "MCP Toggle Failed".to_string(),
                    message: format!("Failed to load config: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
                return;
            }
        };
        if let Some(mcp) = config.mcps.iter_mut().find(|m| m.id == id) {
            mcp.enabled = enabled;
        } else {
            tracing::warn!("MCP {id} not found in config for toggle");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "MCP Toggle Failed".to_string(),
                message: format!("MCP {id} not found in config"),
                severity: view_command::ErrorSeverity::Error,
            });
            return;
        }
        if let Err(e) = config.save(&config_path) {
            tracing::error!("Failed to save config after MCP toggle: {e}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "MCP Toggle Failed".to_string(),
                message: format!("Failed to save config: {e}"),
                severity: view_command::ErrorSeverity::Error,
            });
            return;
        }

        let status = if enabled {
            view_command::McpStatus::Starting
        } else {
            view_command::McpStatus::Stopped
        };
        let _ = view_tx.send(ViewCommand::McpStatusChanged { id, status });

        // Reload global MCP runtime so the change takes effect immediately.
        let global = crate::mcp::McpService::global();
        let reload_view_tx = view_tx.clone();
        let reload_config_path = config_path;
        tokio::spawn(async move {
            let mut svc = global.lock().await;
            if let Err(e) = svc
                .reload_with_path(Some(reload_config_path.as_path()))
                .await
            {
                tracing::error!("MCP global reload after toggle failed: {e}");
                let _ = reload_view_tx.send(ViewCommand::ShowError {
                    title: "MCP Toggle Failed".to_string(),
                    message: format!("Config updated, but MCP runtime reload failed: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
                Self::emit_mcp_snapshot(&reload_view_tx);
            } else {
                tracing::info!("MCP global runtime reloaded after toggle");
            }
        });
    }

    /// Delete an MCP from config.json, reload the global MCP runtime, and emit the result.
    pub(super) async fn on_delete_mcp(
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
        config_path_override: Option<&std::path::Path>,
    ) {
        tracing::info!("Deleting MCP {id}");
        let config_path = match config_path_override {
            Some(p) => p.to_path_buf(),
            None => match crate::config::Config::default_path() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Cannot resolve config path for MCP delete: {e}");
                    let _ = view_tx.send(ViewCommand::ShowError {
                        title: "MCP Delete Failed".to_string(),
                        message: format!("Failed to resolve config path: {e}"),
                        severity: view_command::ErrorSeverity::Error,
                    });
                    return;
                }
            },
        };
        let mut config = match crate::config::Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Cannot load config for MCP delete: {e}");
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "MCP Delete Failed".to_string(),
                    message: format!("Failed to load config: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
                return;
            }
        };
        if let Err(e) = config.remove_mcp(&id) {
            tracing::error!("Failed to remove MCP {id}: {e}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "MCP Delete Failed".to_string(),
                message: format!("Failed to remove MCP {id}: {e}"),
                severity: view_command::ErrorSeverity::Error,
            });
            return;
        }
        if let Err(e) = config.save(&config_path) {
            tracing::error!("Failed to save config after MCP delete: {e}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "MCP Delete Failed".to_string(),
                message: format!("Failed to save config: {e}"),
                severity: view_command::ErrorSeverity::Error,
            });
            return;
        }

        // Reload global MCP runtime so the deleted server is stopped immediately.
        let global = crate::mcp::McpService::global();
        let reload_view_tx = view_tx.clone();
        let reload_config_path = config_path;
        tokio::spawn(async move {
            let mut svc = global.lock().await;
            if let Err(e) = svc
                .reload_with_path(Some(reload_config_path.as_path()))
                .await
            {
                tracing::error!("MCP global reload after delete failed: {e}");
                let _ = reload_view_tx.send(ViewCommand::ShowError {
                    title: "MCP Delete Failed".to_string(),
                    message: format!("Config updated, but MCP runtime reload failed: {e}"),
                    severity: view_command::ErrorSeverity::Error,
                });
                Self::emit_mcp_snapshot(&reload_view_tx);
            } else {
                tracing::info!("MCP global runtime reloaded after delete");
            }
        });

        let _ = view_tx.send(ViewCommand::McpDeleted { id });
    }

    /// Emit the current MCP list from config.json so the settings view
    /// shows all configured MCPs (with real runtime status when available).
    pub fn emit_mcp_snapshot(view_tx: &broadcast::Sender<ViewCommand>) {
        let config_path = match crate::config::Config::default_path() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Cannot resolve config path for MCP snapshot: {e}");
                return;
            }
        };
        let config = match crate::config::Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Cannot load config for MCP snapshot: {e}");
                return;
            }
        };

        let global_mcp = crate::mcp::McpService::global();

        for mcp in &config.mcps {
            let runtime_status = global_mcp
                .try_lock()
                .ok()
                .and_then(|svc| svc.get_status(&mcp.id));

            // Map runtime status, falling back to config.enabled
            let status = match runtime_status {
                Some(crate::mcp::McpStatus::Running) => view_command::McpStatus::Running,
                Some(crate::mcp::McpStatus::Starting | crate::mcp::McpStatus::Restarting) => {
                    view_command::McpStatus::Starting
                }
                Some(crate::mcp::McpStatus::Error(_)) => view_command::McpStatus::Failed,
                Some(crate::mcp::McpStatus::Stopped | crate::mcp::McpStatus::Disabled) => {
                    view_command::McpStatus::Stopped
                }
                None if mcp.enabled => view_command::McpStatus::Starting,
                None => view_command::McpStatus::Stopped,
            };

            let _ = view_tx.send(ViewCommand::McpServerStarted {
                id: mcp.id,
                name: Some(mcp.name.clone()),
                tool_count: 0,
                enabled: Some(mcp.enabled),
            });
            let _ = view_tx.send(ViewCommand::McpStatusChanged { id: mcp.id, status });
        }

        tracing::info!(
            "SettingsPresenter::emit_mcp_snapshot: sent {} MCP entries",
            config.mcps.len()
        );
    }
}
