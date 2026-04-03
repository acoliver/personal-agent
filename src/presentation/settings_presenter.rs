//! `SettingsPresenter` - handles settings and profile management UI
//!
//! `SettingsPresenter` subscribes to settings and profile events,
//! coordinates with profile and app settings services, and emits view commands.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.4
//! @pseudocode presenters.md lines 380-444
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
//! @requirement REQ-WIRE-006

use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::view_command::{ProfileSummary, ThemeSummary};
use super::{Presenter, PresenterError, ViewCommand};

use crate::events::{
    emit,
    types::{McpEvent, ProfileEvent, SystemEvent, UserEvent},
    AppEvent, EventBus,
};
use crate::services::{AppSettingsService, ProfileService};
use crate::ui_gpui::theme::{available_theme_options, is_valid_theme_slug, set_active_theme_slug};

/// `SettingsPresenter` - handles settings and profile management UI
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.4
/// @pseudocode presenters.md lines 380-385
pub struct SettingsPresenter {
    /// Event receiver from `EventBus`
    rx: broadcast::Receiver<AppEvent>,

    /// Reference to profile service
    profile_service: Arc<dyn ProfileService>,

    /// Reference to app settings service
    app_settings_service: Arc<dyn AppSettingsService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,

    /// Optional config path override (for testing); `None` → `Config::default_path()`.
    config_path_override: Option<std::path::PathBuf>,
}

impl SettingsPresenter {
    /// Create a new `SettingsPresenter`
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
            config_path_override: None,
        }
    }

    /// Override the config file path (for testing).
    #[must_use]
    pub fn with_config_path(mut self, path: std::path::PathBuf) -> Self {
        self.config_path_override = Some(path);
        self
    }

    /// Stub constructor using unified global `EventBus` (REQ-WIRE-006 unification path).
    ///
    /// This constructor accepts Arc<EventBus> directly, subscribing to the global event
    /// bus rather than a caller-supplied `broadcast::Sender`. This resolves the split
    /// intake channel problem identified in the remediation plan. Full wiring of all
    /// callers deferred to later implementation phases.
    ///
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P03
    /// @requirement REQ-WIRE-006
    /// @pseudocode component-001-event-pipeline.md lines 090-136
    #[allow(dead_code)]
    pub fn new_with_event_bus(
        profile_service: Arc<dyn ProfileService>,
        app_settings_service: Arc<dyn AppSettingsService>,
        event_bus: &Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.sender().subscribe();
        Self {
            rx,
            profile_service,
            app_settings_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            config_path_override: None,
        }
    }

    /// Start the presenter event loop
    ///
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter startup becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub async fn start(&mut self) -> Result<(), PresenterError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }

        self.running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Self::emit_profiles_snapshot(
            &self.profile_service,
            &self.app_settings_service,
            &self.view_tx,
        )
        .await;

        Self::emit_theme_snapshot(&self.app_settings_service, &self.view_tx, None).await;
        Self::emit_tool_approval_policy_snapshot(&self.app_settings_service, &self.view_tx).await;

        let mut rx = self.rx.resubscribe();
        let running = self.running.clone();
        let profile_service = self.profile_service.clone();
        let app_settings_service = self.app_settings_service.clone();
        let view_tx = self.view_tx.clone();
        let config_path = self.config_path_override.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(
                            &profile_service,
                            &app_settings_service,
                            &view_tx,
                            event,
                            config_path.as_deref(),
                        )
                        .await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("SettingsPresenter lagged: {} events missed", n);
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
    /// # Errors
    ///
    /// Returns `PresenterError` if presenter shutdown becomes fallible in the future.
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub async fn stop(&mut self) -> Result<(), PresenterError> {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Check if presenter is running
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Handle events from `EventBus`
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn handle_event(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
        config_path: Option<&std::path::Path>,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(
                    profile_service,
                    app_settings_service,
                    view_tx,
                    user_evt,
                    config_path,
                )
                .await;
            }
            AppEvent::Profile(profile_evt) => {
                Self::handle_profile_event(
                    profile_service,
                    app_settings_service,
                    view_tx,
                    profile_evt,
                )
                .await;
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
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
        config_path: Option<&std::path::Path>,
    ) {
        match event {
            UserEvent::SelectProfile { id } | UserEvent::SelectChatProfile { id } => {
                Self::on_select_profile(profile_service, app_settings_service, view_tx, id).await;
            }
            UserEvent::DeleteProfile { id } | UserEvent::ConfirmDeleteProfile { id } => {
                Self::on_delete_profile(profile_service, app_settings_service, view_tx, id).await;
            }
            UserEvent::EditProfile { id } => {
                Self::on_edit_profile(profile_service, view_tx, id).await;
            }
            UserEvent::RefreshProfiles => {
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
                Self::emit_theme_snapshot(app_settings_service, view_tx, None).await;
                Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
            }
            UserEvent::RefreshToolApprovalPolicy => {
                Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
            }
            UserEvent::SetToolApprovalYoloMode { enabled } => {
                Self::on_set_tool_approval_yolo_mode(app_settings_service, view_tx, enabled).await;
            }
            UserEvent::SetToolApprovalAutoApproveReads { enabled } => {
                Self::on_set_tool_approval_auto_approve_reads(
                    app_settings_service,
                    view_tx,
                    enabled,
                )
                .await;
            }
            UserEvent::SetToolApprovalMcpApprovalMode { mode } => {
                Self::on_set_tool_approval_mcp_mode(app_settings_service, view_tx, mode).await;
            }
            UserEvent::AddToolApprovalAllowlistPrefix { prefix } => {
                Self::on_add_tool_approval_allowlist_prefix(app_settings_service, view_tx, prefix)
                    .await;
            }
            UserEvent::RemoveToolApprovalAllowlistPrefix { prefix } => {
                Self::on_remove_tool_approval_allowlist_prefix(
                    app_settings_service,
                    view_tx,
                    prefix,
                )
                .await;
            }
            UserEvent::AddToolApprovalDenylistPrefix { prefix } => {
                Self::on_add_tool_approval_denylist_prefix(app_settings_service, view_tx, prefix)
                    .await;
            }
            UserEvent::RemoveToolApprovalDenylistPrefix { prefix } => {
                Self::on_remove_tool_approval_denylist_prefix(
                    app_settings_service,
                    view_tx,
                    prefix,
                )
                .await;
            }
            UserEvent::ToggleMcp { id, enabled } => {
                Self::on_toggle_mcp(view_tx, id, enabled, config_path).await;
            }
            UserEvent::DeleteMcp { id } | UserEvent::ConfirmDeleteMcp { id } => {
                Self::on_delete_mcp(view_tx, id, config_path).await;
            }
            UserEvent::SelectTheme { slug } => {
                Self::on_select_theme(app_settings_service, view_tx, slug).await;
            }
            _ => {} // Ignore other user events
        }
    }

    /// Handle profile domain events
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn handle_profile_event(
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
                if let Some(id) = profile_id {
                    let _ = app_settings_service.set_default_profile_id(id).await;
                }
                let _ = view_tx.send(ViewCommand::DefaultProfileChanged { profile_id });
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
            }
            _ => {} // Ignore other profile events
        }
    }

    /// Handle MCP domain events
    ///
    /// @plan PLAN-20250128-PRESENTERS.P03
    /// @requirement REQ-025.4
    async fn handle_mcp_event(view_tx: &broadcast::Sender<ViewCommand>, event: McpEvent) {
        match event {
            McpEvent::Starting { id, name: _ } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Starting,
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
                    status: super::view_command::McpStatus::Running,
                });
            }
            McpEvent::StartFailed { id, name: _, error } => {
                let _ = view_tx.send(ViewCommand::McpServerFailed { id, error });
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Failed,
                });
            }
            McpEvent::Stopped { id, name: _ } => {
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
                    message: format!("{name}: {error}"),
                    severity: super::view_command::ErrorSeverity::Warning,
                });
            }
            McpEvent::Recovered { id, name } => {
                let _ = view_tx.send(ViewCommand::McpStatusChanged {
                    id,
                    status: super::view_command::McpStatus::Running,
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
    async fn handle_system_event(view_tx: &broadcast::Sender<ViewCommand>, event: SystemEvent) {
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
    async fn on_select_profile(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        match profile_service.set_default(id).await {
            Ok(()) => {
                if let Err(e) = app_settings_service.set_default_profile_id(id).await {
                    tracing::warn!("Failed to persist default profile in app settings: {}", e);
                }
                let _ = view_tx.send(ViewCommand::DefaultProfileChanged {
                    profile_id: Some(id),
                });
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
            }
            Err(e) => {
                tracing::error!("Failed to select profile: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Error".to_string(),
                    message: format!("Failed to select profile: {e}"),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle `EditProfile` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn on_edit_profile(
        profile_service: &Arc<dyn ProfileService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        id: Uuid,
    ) {
        match profile_service.get(id).await {
            Ok(profile) => {
                let api_key_label = match &profile.auth {
                    crate::models::AuthConfig::Keychain { label } => label.clone(),
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
                    context_limit: None,
                    show_thinking: profile.parameters.show_thinking,
                    enable_thinking: profile.parameters.enable_thinking,
                    thinking_budget: profile.parameters.thinking_budget,
                    system_prompt: profile.system_prompt,
                });
                let _ = view_tx.send(ViewCommand::NavigateTo {
                    view: super::view_command::ViewId::ProfileEditor,
                });
            }
            Err(e) => {
                tracing::error!("Failed to load profile for edit: {}", e);
                let _ = view_tx.send(ViewCommand::ShowError {
                    title: "Edit Failed".to_string(),
                    message: format!("Failed to load profile: {e}"),
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Handle `DeleteProfile` user event
    ///
    /// @plan PLAN-20250125-REFACTOR.P12
    /// @requirement REQ-025.4
    async fn on_delete_profile(
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
                    severity: super::view_command::ErrorSeverity::Error,
                });
            }
        }
    }

    /// Toggle an MCP's enabled state in config.json, reload the global MCP runtime,
    /// and emit the updated status.
    async fn on_toggle_mcp(
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
                    return;
                }
            },
        };
        let mut config = match crate::config::Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Cannot load config for MCP toggle: {e}");
                return;
            }
        };
        if let Some(mcp) = config.mcps.iter_mut().find(|m| m.id == id) {
            mcp.enabled = enabled;
        } else {
            tracing::warn!("MCP {id} not found in config for toggle");
            return;
        }
        if let Err(e) = config.save(&config_path) {
            tracing::error!("Failed to save config after MCP toggle: {e}");
            return;
        }

        // Reload global MCP runtime so the change takes effect immediately.
        let global = crate::mcp::McpService::global();
        tokio::spawn(async move {
            let mut svc = global.lock().await;
            if let Err(e) = svc.reload().await {
                tracing::error!("MCP global reload after toggle failed: {e}");
            } else {
                tracing::info!("MCP global runtime reloaded after toggle");
            }
        });

        let status = if enabled {
            super::view_command::McpStatus::Starting
        } else {
            super::view_command::McpStatus::Stopped
        };
        let _ = view_tx.send(ViewCommand::McpStatusChanged { id, status });
    }

    /// Delete an MCP from config.json, reload the global MCP runtime, and emit the result.
    async fn on_delete_mcp(
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
                    return;
                }
            },
        };
        let mut config = match crate::config::Config::load(&config_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Cannot load config for MCP delete: {e}");
                return;
            }
        };
        if let Err(e) = config.remove_mcp(&id) {
            tracing::error!("Failed to remove MCP {id}: {e}");
            return;
        }
        if let Err(e) = config.save(&config_path) {
            tracing::error!("Failed to save config after MCP delete: {e}");
            return;
        }

        // Reload global MCP runtime so the deleted server is stopped immediately.
        let global = crate::mcp::McpService::global();
        tokio::spawn(async move {
            let mut svc = global.lock().await;
            if let Err(e) = svc.reload().await {
                tracing::error!("MCP global reload after delete failed: {e}");
            } else {
                tracing::info!("MCP global runtime reloaded after delete");
            }
        });

        let _ = view_tx.send(ViewCommand::McpDeleted { id });
    }

    async fn emit_profiles_snapshot(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let profiles = match profile_service.list().await {
            Ok(profiles) => profiles,
            Err(e) => {
                tracing::warn!("Failed to list profiles for settings snapshot: {}", e);
                return;
            }
        };

        let selected_profile_id =
            if let Ok(Some(id)) = app_settings_service.get_default_profile_id().await {
                Some(id)
            } else {
                profile_service
                    .get_default()
                    .await
                    .ok()
                    .flatten()
                    .map(|p| p.id)
            };

        let summaries = profiles
            .into_iter()
            .map(|profile| ProfileSummary {
                id: profile.id,
                name: profile.name,
                provider_id: profile.provider_id,
                model_id: profile.model_id,
                is_default: Some(profile.id) == selected_profile_id,
            })
            .collect::<Vec<_>>();

        tracing::info!(
            "SettingsPresenter::emit_profiles_snapshot: sending {} profiles, default={:?}",
            summaries.len(),
            selected_profile_id
        );
        match view_tx.send(ViewCommand::ShowSettings {
            profiles: summaries.clone(),
            selected_profile_id,
        }) {
            Ok(n) => tracing::info!("SettingsPresenter: ShowSettings sent to {} receivers", n),
            Err(e) => tracing::error!("SettingsPresenter: ShowSettings send failed: {}", e),
        }
        match view_tx.send(ViewCommand::ChatProfilesUpdated {
            profiles: summaries,
            selected_profile_id,
        }) {
            Ok(n) => tracing::info!(
                "SettingsPresenter: ChatProfilesUpdated sent to {} receivers",
                n
            ),
            Err(e) => tracing::error!("SettingsPresenter: ChatProfilesUpdated send failed: {}", e),
        }
    }

    /// Emit the list of available themes and the currently-active slug to the
    /// settings view.  Called on startup and after a successful theme switch.
    async fn emit_theme_snapshot(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        selected_override: Option<String>,
    ) {
        let options: Vec<ThemeSummary> = available_theme_options()
            .into_iter()
            .map(|opt| ThemeSummary {
                name: opt.name,
                slug: opt.slug,
            })
            .collect();

        let persisted_slug = app_settings_service
            .get_theme()
            .await
            .ok()
            .flatten()
            .filter(|slug| is_valid_theme_slug(slug));

        let selected_slug = selected_override
            .filter(|slug| is_valid_theme_slug(slug))
            .or(persisted_slug)
            .unwrap_or_else(|| "green-screen".to_string());

        let _ = view_tx.send(ViewCommand::ShowSettingsTheme {
            options,
            selected_slug,
        });
    }

    /// Persist the selected theme slug and apply it to the runtime.
    async fn on_select_theme(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        slug: String,
    ) {
        if !is_valid_theme_slug(&slug) {
            tracing::warn!(
                "Rejected invalid theme selection '{}'; emitting persisted snapshot",
                slug
            );
            Self::emit_theme_snapshot(app_settings_service, view_tx, None).await;
            return;
        }

        if let Err(e) = app_settings_service.set_theme(slug.clone()).await {
            tracing::warn!("Failed to persist theme selection '{}': {}", slug, e);
            Self::emit_theme_snapshot(app_settings_service, view_tx, None).await;
            return;
        }

        let applied_slug = app_settings_service
            .get_theme()
            .await
            .ok()
            .flatten()
            .filter(|persisted| is_valid_theme_slug(persisted))
            .unwrap_or_else(|| "green-screen".to_string());

        set_active_theme_slug(&applied_slug);

        Self::emit_theme_snapshot(app_settings_service, view_tx, Some(applied_slug)).await;
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
                Some(crate::mcp::McpStatus::Running) => super::view_command::McpStatus::Running,
                Some(crate::mcp::McpStatus::Starting | crate::mcp::McpStatus::Restarting) => {
                    super::view_command::McpStatus::Starting
                }
                Some(crate::mcp::McpStatus::Error(_)) => super::view_command::McpStatus::Failed,
                Some(crate::mcp::McpStatus::Stopped | crate::mcp::McpStatus::Disabled) => {
                    super::view_command::McpStatus::Stopped
                }
                None if mcp.enabled => super::view_command::McpStatus::Starting,
                None => super::view_command::McpStatus::Stopped,
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
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
