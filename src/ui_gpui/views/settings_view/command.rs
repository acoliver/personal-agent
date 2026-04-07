//! Command handling for `SettingsView`.

use super::{McpItem, McpStatus, ProfileItem, SettingsView};
use crate::presentation::view_command::ViewCommand;

impl SettingsView {
    /// Handle `ViewCommand` from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P06
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        let should_notify = self.apply_command(command, cx);
        if should_notify {
            cx.notify();
        }
    }

    fn apply_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) -> bool {
        if self.apply_profile_command(&command) {
            return true;
        }
        if self.apply_mcp_command(&command) {
            return true;
        }
        if self.apply_policy_command(&command) {
            return true;
        }
        if self.apply_backup_command(&command) {
            return true;
        }
        self.apply_misc_command(command, cx)
    }

    fn apply_profile_command(&mut self, command: &ViewCommand) -> bool {
        match command {
            ViewCommand::ShowSettings {
                profiles,
                selected_profile_id,
            }
            | ViewCommand::ChatProfilesUpdated {
                profiles,
                selected_profile_id,
            } => {
                self.apply_profile_summaries(profiles.clone(), *selected_profile_id);
                true
            }
            ViewCommand::ProfileCreated { id, name } => {
                self.handle_profile_created(*id, name.clone());
                true
            }
            ViewCommand::ProfileUpdated { id, name } => {
                self.handle_profile_updated(*id, name.clone());
                true
            }
            ViewCommand::ProfileDeleted { id } => {
                self.handle_profile_deleted(*id);
                true
            }
            ViewCommand::DefaultProfileChanged { profile_id } => {
                self.handle_default_profile_changed(*profile_id);
                true
            }
            _ => false,
        }
    }

    fn apply_mcp_command(&mut self, command: &ViewCommand) -> bool {
        match command {
            ViewCommand::McpStatusChanged { id, status } => {
                self.handle_mcp_status_changed(*id, *status);
                true
            }
            ViewCommand::McpServerStarted {
                id, name, enabled, ..
            } => {
                self.handle_mcp_server_started(*id, name.clone(), *enabled);
                true
            }
            ViewCommand::McpServerFailed { id, .. } => {
                self.handle_mcp_server_failed(*id);
                true
            }
            ViewCommand::McpConfigSaved { id, name } => {
                self.handle_mcp_config_saved(*id, name.clone());
                true
            }
            ViewCommand::McpDeleted { id } => {
                self.handle_mcp_deleted(*id);
                true
            }
            _ => false,
        }
    }

    fn apply_policy_command(&mut self, command: &ViewCommand) -> bool {
        match command {
            ViewCommand::ToolApprovalPolicyUpdated {
                yolo_mode,
                auto_approve_reads,
                skills_auto_approve,
                mcp_approval_mode,
                persistent_allowlist,
                persistent_denylist,
            } => {
                self.state.yolo_mode = *yolo_mode;
                self.state.auto_approve_reads = *auto_approve_reads;
                self.state.skills_auto_approve = *skills_auto_approve;
                self.state.mcp_approval_mode = *mcp_approval_mode;
                self.state
                    .persistent_allowlist
                    .clone_from(persistent_allowlist);
                self.state
                    .persistent_denylist
                    .clone_from(persistent_denylist);
                self.state.allowlist_input.clear();
                self.state.denylist_input.clear();
                true
            }
            ViewCommand::SkillsLoaded {
                skills,
                watched_directories,
                default_directory,
            } => {
                self.set_skill_items(skills.iter().cloned().map(Into::into).collect());
                self.state
                    .watched_skill_directories
                    .clone_from(watched_directories);
                self.state
                    .default_skill_directory
                    .clone_from(default_directory);
                true
            }
            ViewCommand::YoloModeChanged { active } => {
                self.state.yolo_mode = *active;
                true
            }
            _ => false,
        }
    }

    fn apply_misc_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) -> bool {
        match command {
            ViewCommand::ShowSettingsTheme {
                options,
                selected_slug,
            } => {
                self.apply_theme_options(options, selected_slug);
                true
            }
            ViewCommand::ShowFontSettings {
                size,
                ui_family,
                mono_family,
                ligatures,
            } => self.apply_font_settings(size, ui_family, mono_family, ligatures, cx),
            ViewCommand::ExportDirectoryLoaded { path } => {
                self.state.export_dir_input = path;
                true
            }
            ViewCommand::SetEmojiFilterVisibility { enabled } => {
                self.state.filter_emoji = enabled;
                true
            }
            ViewCommand::ShowNotification { message } => {
                self.state.status_message = Some(message);
                self.state.status_is_error = false;
                true
            }
            ViewCommand::ShowError { title, message, .. } => {
                self.state.status_message = Some(format!("{title}: {message}"));
                self.state.status_is_error = true;
                true
            }
            _ => false,
        }
    }

    fn apply_font_settings(
        &mut self,
        size: f32,
        ui_family: Option<String>,
        mono_family: String,
        ligatures: bool,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        self.state.font_size = size;
        self.state.ui_font_family = ui_family;
        self.state.mono_font_family = mono_family;
        self.state.mono_ligatures = ligatures;
        cx.notify();
        false
    }

    fn handle_profile_created(&mut self, id: uuid::Uuid, name: String) {
        self.state.selected_profile_id = Some(id);
        if let Some(existing) = self.state.profiles.iter_mut().find(|p| p.id == id) {
            existing.name = name;
        } else {
            self.state
                .profiles
                .push(ProfileItem::new(id, name).with_model("", ""));
        }
    }

    fn handle_profile_updated(&mut self, id: uuid::Uuid, name: String) {
        if let Some(profile) = self.state.profiles.iter_mut().find(|p| p.id == id) {
            profile.name = name;
        }
    }

    fn handle_profile_deleted(&mut self, id: uuid::Uuid) {
        self.state.profiles.retain(|p| p.id != id);
        if self.state.selected_profile_id == Some(id) {
            self.state.selected_profile_id = self.state.profiles.first().map(|p| p.id);
        }
    }

    fn handle_default_profile_changed(&mut self, profile_id: Option<uuid::Uuid>) {
        let resolved = profile_id.filter(|id| self.state.profiles.iter().any(|p| p.id == *id));
        self.state.selected_profile_id = resolved;
        for profile in &mut self.state.profiles {
            profile.is_default = Some(profile.id) == resolved;
        }
    }

    fn handle_mcp_deleted(&mut self, id: uuid::Uuid) {
        self.state.mcps.retain(|m| m.id != id);
        if self.state.selected_mcp_id == Some(id) {
            self.state.selected_mcp_id = self.state.mcps.first().map(|m| m.id);
        }
    }

    fn handle_mcp_status_changed(
        &mut self,
        id: uuid::Uuid,
        status: crate::presentation::view_command::McpStatus,
    ) {
        let (mapped, force_enabled) = match status {
            crate::presentation::view_command::McpStatus::Running
            | crate::presentation::view_command::McpStatus::Starting => {
                (McpStatus::Running, Some(true))
            }
            crate::presentation::view_command::McpStatus::Failed
            | crate::presentation::view_command::McpStatus::Unhealthy => (McpStatus::Error, None),
            crate::presentation::view_command::McpStatus::Stopped => (McpStatus::Stopped, None),
        };
        if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
            existing.status = mapped;
            if let Some(en) = force_enabled {
                existing.enabled = en;
            }
        } else {
            self.state
                .mcps
                .push(McpItem::new(id, format!("MCP {id}")).with_status(mapped));
        }
    }

    fn handle_mcp_server_started(
        &mut self,
        id: uuid::Uuid,
        name: Option<String>,
        enabled: Option<bool>,
    ) {
        let is_enabled = enabled.unwrap_or(true);
        if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
            if let Some(n) = name {
                existing.name = n;
            }
            existing.enabled = is_enabled;
            if is_enabled {
                existing.status = McpStatus::Running;
            }
        } else {
            let display = name.unwrap_or_else(|| format!("MCP {id}"));
            self.state
                .mcps
                .push(McpItem::new(id, display).with_enabled(is_enabled));
        }
    }

    fn handle_mcp_server_failed(&mut self, id: uuid::Uuid) {
        if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
            existing.status = McpStatus::Error;
            existing.enabled = false;
        } else {
            self.state
                .mcps
                .push(McpItem::new(id, format!("MCP {id}")).with_status(McpStatus::Error));
        }
    }

    fn handle_mcp_config_saved(&mut self, id: uuid::Uuid, name: Option<String>) {
        self.state.selected_mcp_id = Some(id);
        if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
            if let Some(name) = name {
                existing.name = name;
            }
            existing.enabled = true;
            existing.status = McpStatus::Stopped;
        } else {
            self.state.mcps.push(
                McpItem::new(id, name.unwrap_or_else(|| format!("MCP {id}")))
                    .with_enabled(true)
                    .with_status(McpStatus::Stopped),
            );
        }
    }

    /// Handle backup-related commands
    fn apply_backup_command(&mut self, command: &ViewCommand) -> bool {
        match command {
            ViewCommand::BackupSettingsLoaded {
                settings,
                backups,
                last_backup_time,
            } => {
                tracing::info!(
                    "SettingsView: BackupSettingsLoaded received - {} backups",
                    backups.len()
                );
                self.state.backup_settings = Some(settings.clone());
                self.state.backups.clone_from(backups);
                self.state.last_backup_time = *last_backup_time;
                self.state.backup_in_progress = false;
                true
            }
            ViewCommand::BackupCompleted { result } => {
                tracing::info!("SettingsView: BackupCompleted received - {:?}", result);
                self.state.backup_in_progress = false;
                self.state.backup_status = Some(result.message());
                if result.is_success() {
                    self.state.last_backup_time = Some(chrono::Utc::now());
                }
                true
            }
            ViewCommand::BackupListRefreshed { backups } => {
                tracing::info!(
                    "SettingsView: BackupListRefreshed received - {} backups",
                    backups.len()
                );
                self.state.backups.clone_from(backups);
                true
            }
            ViewCommand::RestoreCompleted { result } => {
                tracing::info!("SettingsView: RestoreCompleted received - {:?}", result);
                self.state.backup_status = Some(result.message());
                self.state.backup_in_progress = false;
                // Clear selection so user can select a different backup
                self.state.selected_backup_id = None;
                true
            }
            _ => false,
        }
    }
}
