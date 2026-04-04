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

use super::view_command::{ProfileSummary, ThemeSummary};
use super::{Presenter, PresenterError, ViewCommand};

use crate::events::{types::UserEvent, AppEvent, EventBus};
use crate::services::{AppSettingsService, ProfileService};
use crate::ui_gpui::theme::{
    active_mono_font_family, active_mono_ligatures, available_theme_options, is_valid_theme_slug,
    set_active_font_size, set_active_mono_font_family, set_active_mono_ligatures,
    set_active_theme_slug, set_active_ui_font_family, SETTING_KEY_FONT_SIZE,
    SETTING_KEY_MONO_FONT_FAMILY, SETTING_KEY_MONO_LIGATURES, SETTING_KEY_UI_FONT_FAMILY,
};

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
        Self::emit_font_settings_snapshot(&self.app_settings_service, &self.view_tx).await;
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
            UserEvent::SetFontSize { size } => {
                Self::on_set_font_size(app_settings_service, view_tx, size).await;
            }
            UserEvent::SetUiFontFamily { name } => {
                Self::on_set_ui_font_family(app_settings_service, view_tx, name).await;
            }
            UserEvent::SetMonoFontFamily { name } => {
                Self::on_set_mono_font_family(app_settings_service, view_tx, name).await;
            }
            UserEvent::SetMonoLigatures { enabled } => {
                Self::on_set_mono_ligatures(app_settings_service, view_tx, enabled).await;
            }
            _ => {} // Ignore other user events
        }
    }

    pub(super) async fn emit_profiles_snapshot(
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

    /// Persist and apply a new font size, then emit a font settings snapshot.
    async fn on_set_font_size(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        size: f32,
    ) {
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_FONT_SIZE, size.to_string())
            .await
        {
            tracing::warn!("Failed to persist font_size: {}", e);
        }
        set_active_font_size(size);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Persist and apply a UI font family override, then emit a font settings snapshot.
    async fn on_set_ui_font_family(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        name: Option<String>,
    ) {
        let value = name.clone().unwrap_or_default();
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_UI_FONT_FAMILY, value)
            .await
        {
            tracing::warn!("Failed to persist ui_font_family: {}", e);
        }
        set_active_ui_font_family(name);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Persist and apply a monospace font family, then emit a font settings snapshot.
    async fn on_set_mono_font_family(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        name: String,
    ) {
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_MONO_FONT_FAMILY, name.clone())
            .await
        {
            tracing::warn!("Failed to persist mono_font_family: {}", e);
        }
        set_active_mono_font_family(&name);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Persist and apply the mono-ligatures toggle, then emit a font settings snapshot.
    async fn on_set_mono_ligatures(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        enabled: bool,
    ) {
        if let Err(e) = app_settings_service
            .set_setting(SETTING_KEY_MONO_LIGATURES, enabled.to_string())
            .await
        {
            tracing::warn!("Failed to persist mono_ligatures: {}", e);
        }
        set_active_mono_ligatures(enabled);
        Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
    }

    /// Read all four font settings from persistence and emit `ViewCommand::ShowFontSettings`.
    async fn emit_font_settings_snapshot(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        use crate::ui_gpui::theme::DEFAULT_FONT_SIZE;

        let size = app_settings_service
            .get_setting(SETTING_KEY_FONT_SIZE)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(DEFAULT_FONT_SIZE);

        let ui_family = app_settings_service
            .get_setting(SETTING_KEY_UI_FONT_FAMILY)
            .await
            .ok()
            .flatten()
            .filter(|v| !v.is_empty());

        let mono_family = app_settings_service
            .get_setting(SETTING_KEY_MONO_FONT_FAMILY)
            .await
            .ok()
            .flatten()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(active_mono_font_family);

        let ligatures = app_settings_service
            .get_setting(SETTING_KEY_MONO_LIGATURES)
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or_else(active_mono_ligatures);

        let _ = view_tx.send(ViewCommand::ShowFontSettings {
            size,
            ui_family,
            mono_family,
            ligatures,
        });
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
