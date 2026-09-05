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

use super::{Presenter, PresenterError, ViewCommand};

use crate::events::{types::UserEvent, AppEvent, EventBus};
use crate::services::login_item::{default_login_item_service, LoginItemService};
use crate::services::{AppSettingsService, BackupService, ProfileService, SkillsService};
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

    /// Reference to backup service
    backup_service: Arc<dyn BackupService>,
    /// Reference to skills service
    skills_service: Arc<dyn SkillsService>,

    /// View command sender
    view_tx: broadcast::Sender<ViewCommand>,

    /// Running flag for event loop
    running: Arc<std::sync::atomic::AtomicBool>,

    /// Optional config path override (for testing); `None` → `Config::default_path()`.
    config_path_override: Option<std::path::PathBuf>,

    /// Launch-at-login backend (Issue #177). Defaults to the platform-default
    /// implementation: `SMAppService` on macOS, an `Unsupported` stub
    /// elsewhere. Tests inject a fake via `with_login_item_service`.
    login_item_service: Arc<dyn LoginItemService>,

    /// Generation counter that arbitrates the local-model status poll: each
    /// panel entry bumps it, which both re-arms polling and terminates the
    /// previous poll task.
    local_model_poll_generation: Arc<std::sync::atomic::AtomicU64>,
}

impl SettingsPresenter {
    /// Create a new `SettingsPresenter`
    ///
    /// @plan PLAN-20250125-REFACTOR.P10
    /// @requirement REQ-025.4
    pub fn new(
        profile_service: Arc<dyn ProfileService>,
        app_settings_service: Arc<dyn AppSettingsService>,
        backup_service: Arc<dyn BackupService>,
        skills_service: Arc<dyn SkillsService>,
        event_bus: &broadcast::Sender<AppEvent>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.subscribe();
        Self {
            rx,
            profile_service,
            app_settings_service,
            backup_service,
            skills_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            config_path_override: None,
            login_item_service: Arc::from(default_login_item_service()),
            local_model_poll_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Override the config file path (for testing).
    #[must_use]
    pub fn with_config_path(mut self, path: std::path::PathBuf) -> Self {
        self.config_path_override = Some(path);
        self
    }

    /// Inject a custom launch-at-login backend (used by tests so they don't
    /// touch the real `SMAppService` API).
    #[must_use]
    pub fn with_login_item_service(mut self, service: Arc<dyn LoginItemService>) -> Self {
        self.login_item_service = service;
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
        backup_service: Arc<dyn BackupService>,
        skills_service: Arc<dyn SkillsService>,
        event_bus: &Arc<EventBus>,
        view_tx: broadcast::Sender<ViewCommand>,
    ) -> Self {
        let rx = event_bus.sender().subscribe();
        Self {
            rx,
            profile_service,
            app_settings_service,
            backup_service,
            skills_service,
            view_tx,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            config_path_override: None,
            login_item_service: Arc::from(default_login_item_service()),
            local_model_poll_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
        Self::emit_skills_snapshot(&self.skills_service, &self.view_tx).await;
        Self::emit_launch_at_login_snapshot(
            &self.app_settings_service,
            &self.login_item_service,
            &self.view_tx,
        )
        .await;

        let mut rx = self.rx.resubscribe();
        Self::emit_backup_settings_snapshot(&self.backup_service, &self.view_tx).await;

        let running = self.running.clone();
        let profile_service = self.profile_service.clone();
        let app_settings_service = self.app_settings_service.clone();
        let skills_service = self.skills_service.clone();
        let view_tx = self.view_tx.clone();
        let backup_service = self.backup_service.clone();
        let login_item_service = self.login_item_service.clone();
        let local_model_poll_generation = self.local_model_poll_generation.clone();

        let config_path = self.config_path_override.clone();

        tokio::spawn(async move {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(event) => {
                        Self::handle_event(
                            &profile_service,
                            &app_settings_service,
                            &backup_service,
                            &skills_service,
                            &login_item_service,
                            &local_model_poll_generation,
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
    // Signature fixed by the shared call convention, not by the body.
    #[allow(clippy::unused_async_trait_impl)]
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

    #[allow(clippy::too_many_arguments)]
    async fn handle_event(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        backup_service: &Arc<dyn BackupService>,
        skills_service: &Arc<dyn SkillsService>,
        login_item_service: &Arc<dyn LoginItemService>,
        local_model_poll_generation: &Arc<std::sync::atomic::AtomicU64>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: AppEvent,
        config_path: Option<&std::path::Path>,
    ) {
        match event {
            AppEvent::User(user_evt) => {
                Self::handle_user_event(
                    profile_service,
                    app_settings_service,
                    backup_service,
                    skills_service,
                    login_item_service,
                    local_model_poll_generation,
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

    #[allow(clippy::too_many_arguments)]
    async fn handle_user_event(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        backup_service: &Arc<dyn BackupService>,
        skills_service: &Arc<dyn SkillsService>,
        login_item_service: &Arc<dyn LoginItemService>,
        local_model_poll_generation: &Arc<std::sync::atomic::AtomicU64>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: UserEvent,
        config_path: Option<&std::path::Path>,
    ) {
        if Self::handle_launch_at_login_user_event(
            app_settings_service,
            login_item_service,
            view_tx,
            &event,
        )
        .await
        {
            return;
        }

        if Self::handle_profile_user_event(profile_service, app_settings_service, view_tx, &event)
            .await
        {
            return;
        }

        if Self::handle_refresh_user_event(
            profile_service,
            app_settings_service,
            skills_service,
            view_tx,
            &event,
        )
        .await
        {
            return;
        }

        if Self::handle_tool_approval_user_event(app_settings_service, view_tx, &event).await {
            return;
        }

        if Self::handle_skills_user_event(skills_service, view_tx, &event).await {
            return;
        }

        if Self::handle_mcp_user_event(view_tx, config_path, &event).await {
            return;
        }

        if Self::handle_backup_user_event_wrapper(backup_service, view_tx, &event).await {
            return;
        }

        if Self::handle_local_model_user_event(
            app_settings_service,
            view_tx,
            &event,
            local_model_poll_generation,
        )
        .await
        {
            return;
        }

        Self::handle_appearance_user_event(app_settings_service, view_tx, &event).await;
    }

    async fn handle_profile_user_event(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::SelectProfile { id } | UserEvent::SelectChatProfile { id } => {
                Self::on_select_profile(profile_service, app_settings_service, view_tx, *id).await;
                true
            }
            UserEvent::DeleteProfile { id } | UserEvent::ConfirmDeleteProfile { id } => {
                Self::on_delete_profile(profile_service, app_settings_service, view_tx, *id).await;
                true
            }
            UserEvent::EditProfile { id } => {
                Self::on_edit_profile(profile_service, view_tx, *id).await;
                true
            }
            UserEvent::CreateLocalProfile => {
                Self::on_create_local_profile(profile_service, app_settings_service, view_tx).await;
                true
            }
            _ => false,
        }
    }

    /// One-click local profile for existing installs (REQ-LM-002): create
    /// the seed profile through the shared helper, make it the default, and
    /// refresh the profile snapshots so the create button disappears.
    ///
    /// @plan:PLAN-20260903-LOCALMODEL.P05
    /// @requirement:REQ-LM-002 REQ-LM-006
    async fn on_create_local_profile(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        if let Err(error) =
            crate::services::profile_impl::ensure_local_seed_profile(profile_service.as_ref()).await
        {
            tracing::warn!("Failed to create the local profile: {error}");
            let _ = view_tx.send(ViewCommand::ShowError {
                title: "Local Model".to_string(),
                message: format!("Failed to create the local profile: {error}"),
                severity: super::view_command::ErrorSeverity::Warning,
            });
            return;
        }
        Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
    }

    async fn handle_refresh_user_event(
        profile_service: &Arc<dyn ProfileService>,
        app_settings_service: &Arc<dyn AppSettingsService>,
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::RefreshProfiles => {
                Self::emit_profiles_snapshot(profile_service, app_settings_service, view_tx).await;
                Self::emit_theme_snapshot(app_settings_service, view_tx, None).await;
                Self::emit_font_settings_snapshot(app_settings_service, view_tx).await;
                Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
                Self::emit_skills_snapshot(skills_service, view_tx).await;
                true
            }
            UserEvent::RefreshToolApprovalPolicy => {
                Self::emit_tool_approval_policy_snapshot(app_settings_service, view_tx).await;
                true
            }
            UserEvent::RefreshSkills => {
                Self::emit_skills_snapshot(skills_service, view_tx).await;
                true
            }
            _ => false,
        }
    }

    async fn handle_tool_approval_user_event(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::SetToolApprovalYoloMode { enabled } => {
                Self::on_set_tool_approval_yolo_mode(app_settings_service, view_tx, *enabled).await;
                true
            }
            UserEvent::SetToolApprovalAutoApproveReads { enabled } => {
                Self::on_set_tool_approval_auto_approve_reads(
                    app_settings_service,
                    view_tx,
                    *enabled,
                )
                .await;
                true
            }
            UserEvent::SetToolApprovalSkillsAutoApprove { enabled } => {
                Self::on_set_tool_approval_skills_auto_approve(
                    app_settings_service,
                    view_tx,
                    *enabled,
                )
                .await;
                true
            }
            UserEvent::SetToolApprovalMcpApprovalMode { mode } => {
                Self::on_set_tool_approval_mcp_mode(app_settings_service, view_tx, *mode).await;
                true
            }
            UserEvent::AddToolApprovalAllowlistPrefix { prefix } => {
                Self::on_add_tool_approval_allowlist_prefix(
                    app_settings_service,
                    view_tx,
                    prefix.clone(),
                )
                .await;
                true
            }
            UserEvent::RemoveToolApprovalAllowlistPrefix { prefix } => {
                Self::on_remove_tool_approval_allowlist_prefix(
                    app_settings_service,
                    view_tx,
                    prefix.clone(),
                )
                .await;
                true
            }
            UserEvent::AddToolApprovalDenylistPrefix { prefix } => {
                Self::on_add_tool_approval_denylist_prefix(
                    app_settings_service,
                    view_tx,
                    prefix.clone(),
                )
                .await;
                true
            }
            UserEvent::RemoveToolApprovalDenylistPrefix { prefix } => {
                Self::on_remove_tool_approval_denylist_prefix(
                    app_settings_service,
                    view_tx,
                    prefix.clone(),
                )
                .await;
                true
            }
            _ => false,
        }
    }

    async fn handle_skills_user_event(
        skills_service: &Arc<dyn SkillsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::SetSkillEnabled { name, enabled } => {
                Self::on_set_skill_enabled(skills_service, view_tx, name.clone(), *enabled).await;
                true
            }
            UserEvent::AddSkillsDirectory { path } => {
                Self::on_add_skills_directory(skills_service, view_tx, path.clone()).await;
                true
            }
            UserEvent::RemoveSkillsDirectory { path } => {
                Self::on_remove_skills_directory(skills_service, view_tx, path.clone()).await;
                true
            }
            UserEvent::InstallSkillFromUrl { url } => {
                Self::on_install_skill_from_url(skills_service, view_tx, url.clone()).await;
                true
            }
            _ => false,
        }
    }

    async fn handle_mcp_user_event(
        view_tx: &broadcast::Sender<ViewCommand>,
        config_path: Option<&std::path::Path>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::ToggleMcp { id, enabled } => {
                Self::on_toggle_mcp(view_tx, *id, *enabled, config_path).await;
                true
            }
            UserEvent::DeleteMcp { id } | UserEvent::ConfirmDeleteMcp { id } => {
                Self::on_delete_mcp(view_tx, *id, config_path).await;
                true
            }
            _ => false,
        }
    }

    async fn handle_backup_user_event_wrapper(
        backup_service: &Arc<dyn BackupService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::TriggerBackupNow
            | UserEvent::SetBackupDirectory { .. }
            | UserEvent::RestoreBackup { .. }
            | UserEvent::RefreshBackupList
            | UserEvent::SetBackupEnabled { .. }
            | UserEvent::SetBackupIntervalHours { .. }
            | UserEvent::SetBackupMaxCopies { .. } => {
                Self::handle_backup_user_event(backup_service, view_tx, event.clone()).await;
                true
            }
            _ => false,
        }
    }

    async fn handle_appearance_user_event(
        app_settings_service: &Arc<dyn AppSettingsService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) {
        match event {
            UserEvent::SelectTheme { slug } => {
                Self::on_select_theme(app_settings_service, view_tx, slug.clone()).await;
            }
            UserEvent::SetFontSize { size } => {
                Self::on_set_font_size(app_settings_service, view_tx, *size).await;
            }
            UserEvent::SetUiFontFamily { name } => {
                Self::on_set_ui_font_family(app_settings_service, view_tx, name.clone()).await;
            }
            UserEvent::SetMonoFontFamily { name } => {
                Self::on_set_mono_font_family(app_settings_service, view_tx, name.clone()).await;
            }
            UserEvent::SetMonoLigatures { enabled } => {
                Self::on_set_mono_ligatures(app_settings_service, view_tx, *enabled).await;
            }
            _ => {}
        }
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
