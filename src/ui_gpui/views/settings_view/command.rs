//! Command handling for `SettingsView`.

use super::{McpItem, McpStatus, ProfileItem, SettingsView};
use crate::presentation::view_command::ViewCommand;

impl SettingsView {
    /// Handle `ViewCommand` from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P06
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::ShowSettings {
                profiles,
                selected_profile_id,
            }
            | ViewCommand::ChatProfilesUpdated {
                profiles,
                selected_profile_id,
            } => {
                self.apply_profile_summaries(profiles, selected_profile_id);
            }
            ViewCommand::ShowSettingsTheme {
                options,
                selected_slug,
            } => {
                self.apply_theme_options(options, selected_slug);
            }
            ViewCommand::ProfileCreated { id, name } => {
                self.state.selected_profile_id = Some(id);
                if self.state.profiles.iter().all(|p| p.id != id) {
                    self.state
                        .profiles
                        .push(ProfileItem::new(id, name).with_model("", ""));
                }
            }
            ViewCommand::ProfileUpdated { id, name } => {
                if let Some(profile) = self.state.profiles.iter_mut().find(|p| p.id == id) {
                    profile.name = name;
                }
            }
            ViewCommand::ProfileDeleted { id } => {
                self.state.profiles.retain(|p| p.id != id);
                if self.state.selected_profile_id == Some(id) {
                    self.state.selected_profile_id = self.state.profiles.first().map(|p| p.id);
                }
            }
            ViewCommand::DefaultProfileChanged { profile_id } => {
                self.state.selected_profile_id = profile_id;
                for profile in &mut self.state.profiles {
                    profile.is_default = Some(profile.id) == profile_id;
                }
            }
            ViewCommand::McpStatusChanged { id, status } => {
                self.handle_mcp_status_changed(id, status);
            }
            ViewCommand::McpServerStarted {
                id, name, enabled, ..
            } => {
                self.handle_mcp_server_started(id, name, enabled);
            }
            ViewCommand::McpServerFailed { id, .. } => {
                self.handle_mcp_server_failed(id);
            }
            ViewCommand::McpConfigSaved { id, name } => {
                self.handle_mcp_config_saved(id, name);
            }
            ViewCommand::McpDeleted { id } => {
                self.state.mcps.retain(|m| m.id != id);
                if self.state.selected_mcp_id == Some(id) {
                    self.state.selected_mcp_id = self.state.mcps.first().map(|m| m.id);
                }
            }
            _ => {}
        }
        cx.notify();
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
}
