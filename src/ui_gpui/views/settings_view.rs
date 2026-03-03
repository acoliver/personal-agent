//! Settings view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P06
//! @requirement REQ-UI-ST

use gpui::{
    div, prelude::*, px, FocusHandle, FontWeight, MouseButton, SharedString,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ProfileSummary, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Represents a profile in the settings list
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileItem {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub is_default: bool,
}

impl ProfileItem {
    pub fn new(id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            provider: String::new(),
            model: String::new(),
            is_default: false,
        }
    }

    pub fn with_model(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = provider.into();
        self.model = model.into();
        self
    }

    pub fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// Format display string: "name (provider:model)"
    pub fn display_text(&self) -> String {
        if self.provider.is_empty() && self.model.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({}:{})", self.name, self.provider, self.model)
        }
    }
}

/// MCP status indicator
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpStatus {
    Running,
    Stopped,
    Error,
}

/// Represents an MCP in the settings list
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Debug, PartialEq)]
pub struct McpItem {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub status: McpStatus,
}

impl McpItem {
    pub fn new(id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            enabled: false,
            status: McpStatus::Stopped,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.status = if enabled {
            McpStatus::Running
        } else {
            McpStatus::Stopped
        };
        self
    }

    pub fn with_status(mut self, status: McpStatus) -> Self {
        self.status = status;
        self
    }
}

/// Settings view state
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Default)]
pub struct SettingsState {
    pub profiles: Vec<ProfileItem>,
    pub selected_profile_id: Option<Uuid>,
    pub mcps: Vec<McpItem>,
    pub selected_mcp_id: Option<Uuid>,
    pub hotkey: String,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            hotkey: "Cmd+Shift+P".to_string(),
            ..Default::default()
        }
    }
}

/// Settings view component
/// @plan PLAN-20250130-GPUIREDUX.P06
pub struct SettingsView {
    state: SettingsState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
}

impl SettingsView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: SettingsState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P06
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set profiles from presenter
    pub fn set_profiles(&mut self, profiles: Vec<ProfileItem>) {
        self.state.profiles = profiles;
    }

    fn apply_profile_summaries(
        &mut self,
        profiles: Vec<ProfileSummary>,
        selected_profile_id: Option<Uuid>,
    ) {
        self.state.profiles = profiles
            .into_iter()
            .map(|profile| {
                ProfileItem::new(profile.id, profile.name)
                    .with_model(profile.provider_id, profile.model_id)
                    .with_default(profile.is_default)
            })
            .collect();

        if selected_profile_id.is_some() {
            self.state.selected_profile_id = selected_profile_id;
        }

        if self.state.selected_profile_id.is_none() {
            self.state.selected_profile_id = self.state.profiles.first().map(|profile| profile.id);
        }

        if let Some(selected_id) = self.state.selected_profile_id {
            if self.state.profiles.iter().all(|profile| profile.id != selected_id) {
                self.state.selected_profile_id = self.state.profiles.first().map(|profile| profile.id);
            }
        }
    }

    /// Set MCPs from presenter
    pub fn set_mcps(&mut self, mcps: Vec<McpItem>) {
        self.state.mcps = mcps;

        if self.state.selected_mcp_id.is_none() {
            self.state.selected_mcp_id = self.state.mcps.first().map(|mcp| mcp.id);
        }

        if let Some(selected_id) = self.state.selected_mcp_id {
            if self.state.mcps.iter().all(|mcp| mcp.id != selected_id) {
                self.state.selected_mcp_id = self.state.mcps.first().map(|mcp| mcp.id);
            }
        }
    }

    fn selected_profile_index(&self) -> Option<usize> {
        self.state
            .selected_profile_id
            .and_then(|id| self.state.profiles.iter().position(|profile| profile.id == id))
    }

    fn selected_mcp_index(&self) -> Option<usize> {
        self.state
            .selected_mcp_id
            .and_then(|id| self.state.mcps.iter().position(|mcp| mcp.id == id))
    }

    fn select_profile_by_index(&mut self, index: usize, emit_event: bool) {
        if let Some(profile) = self.state.profiles.get(index) {
            self.state.selected_profile_id = Some(profile.id);
            if emit_event {
                self.emit(UserEvent::SelectProfile { id: profile.id });
            }
        }
    }

    fn select_mcp_by_index(&mut self, index: usize) {
        if let Some(mcp) = self.state.mcps.get(index) {
            self.state.selected_mcp_id = Some(mcp.id);
        }
    }

    fn scroll_profiles(&mut self, delta_steps: i32) {
        if self.state.profiles.is_empty() || delta_steps == 0 {
            return;
        }

        let current = self.selected_profile_index().unwrap_or(0);
        let max_index = self.state.profiles.len().saturating_sub(1) as i32;
        let next = (current as i32 + delta_steps).clamp(0, max_index) as usize;
        self.select_profile_by_index(next, true);
    }

    fn scroll_mcps(&mut self, delta_steps: i32) {
        if self.state.mcps.is_empty() || delta_steps == 0 {
            return;
        }

        let current = self.selected_mcp_index().unwrap_or(0);
        let max_index = self.state.mcps.len().saturating_sub(1) as i32;
        let next = (current as i32 + delta_steps).clamp(0, max_index) as usize;
        self.select_mcp_by_index(next);
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn emit(&self, event: UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Handle ViewCommand from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P06
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack => {
                // Navigation handled by MainPanel
            }
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
                let mapped = match status {
                    crate::presentation::view_command::McpStatus::Running => McpStatus::Running,
                    crate::presentation::view_command::McpStatus::Failed
                    | crate::presentation::view_command::McpStatus::Unhealthy => McpStatus::Error,
                    _ => McpStatus::Stopped,
                };
                if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
                    existing.status = mapped;
                    existing.enabled = matches!(mapped, McpStatus::Running);
                } else {
                    self.state
                        .mcps
                        .push(McpItem::new(id, format!("MCP {}", id)).with_status(mapped));
                }
            }
            ViewCommand::McpServerStarted { id, .. } => {
                if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
                    existing.status = McpStatus::Running;
                    existing.enabled = true;
                } else {
                    self.state
                        .mcps
                        .push(McpItem::new(id, format!("MCP {}", id)).with_enabled(true));
                }
            }
            ViewCommand::McpServerFailed { id, .. } => {
                if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
                    existing.status = McpStatus::Error;
                    existing.enabled = false;
                } else {
                    self.state.mcps.push(
                        McpItem::new(id, format!("MCP {}", id)).with_status(McpStatus::Error),
                    );
                }
            }
            ViewCommand::McpConfigSaved { id, name } => {
                self.state.selected_mcp_id = Some(id);
                if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
                    if let Some(name) = name {
                        existing.name = name;
                    }
                    existing.enabled = true;
                    existing.status = McpStatus::Running;
                } else {
                    self.state.mcps.push(
                        McpItem::new(id, name.unwrap_or_else(|| format!("MCP {}", id)))
                            .with_status(McpStatus::Running)
                            .with_enabled(true),
                    );
                }
            }
            ViewCommand::McpDeleted { id } => {
                self.state.mcps.retain(|m| m.id != id);
                if self.state.selected_mcp_id == Some(id) {
                    self.state.selected_mcp_id = self.state.mcps.first().map(|m| m.id);
                }
            }
            _ => {
                // Other commands handled elsewhere
            }
        }
        cx.notify();
    }

    /// Render the top bar with back button and title
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("settings-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            // Left: back button + title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Back button - uses navigation_channel
                    .child(
                        div()
                            .id("btn-back")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_secondary())
                            .child("<")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, _cx| {
                                    tracing::info!("Back clicked - navigating to Chat");
                                    crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::Chat,
                                    );
                                }),
                            ),
                    )
                    // Title
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("Settings"),
                    ),
            )
            // Right: Refresh Models button
            .child(
                div()
                    .id("btn-refresh-models")
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child("Refresh Models")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            tracing::info!("Refresh Models clicked");
                            this.emit(UserEvent::RefreshModelsRegistry);
                        }),
                    ),
            )
    }

    /// Render a single profile row
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_profile_row(
        &self,
        profile: &ProfileItem,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let profile_id = profile.id;
        let is_selected = self.state.selected_profile_id == Some(profile_id);
        let display_text = profile.display_text();

        div()
            .id(SharedString::from(format!("profile-{}", profile_id)))
            .w_full()
            .h(px(24.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .when(is_selected, |d| {
                d.bg(Theme::accent()).text_color(gpui::white())
            })
            .when(!is_selected, |d| {
                d.hover(|s| s.bg(Theme::bg_dark()))
                    .text_color(Theme::text_primary())
            })
            .text_size(px(12.0))
            .child(display_text)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("Profile selected: {}", profile_id);
                    this.state.selected_profile_id = Some(profile_id);
                    this.emit(UserEvent::SelectProfile { id: profile_id });
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// Render the profiles section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_profiles_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let profiles = &self.state.profiles;
        let has_selection = self.state.selected_profile_id.is_some();
        let total_profiles = profiles.len();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Section header
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .child("PROFILES"),
            )
            // List box
            .child(
                div()
                    .id("profiles-list")
                    .w(px(360.0))
                    .h(px(100.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(profiles.iter().map(|p| self.render_profile_row(p, cx)))
                    .when(profiles.is_empty(), |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("No profiles configured"),
                        )
                    })
                    .when(total_profiles > 0, |d| {
                        d.child(
                            div()
                                .w_full()
                                .px(px(8.0))
                                .pb(px(2.0))
                                .text_size(px(10.0))
                                .text_color(Theme::text_muted())
                                .child(format!("{} profiles", total_profiles)),
                        )
                    }),
            )
            // Toolbar: [-] [+] [spacer] [Edit]
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // [-] Delete button
                    .child(
                        div()
                            .id("btn-delete-profile")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(has_selection, |d| d.hover(|s| s.bg(Theme::danger())))
                            .when(!has_selection, |d| d.text_color(Theme::text_muted()))
                            .text_size(px(14.0))
                            .text_color(if has_selection {
                                Theme::text_primary()
                            } else {
                                Theme::text_muted()
                            })
                            .child("-")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    if let Some(id) = this.state.selected_profile_id {
                                        tracing::info!("Delete profile clicked: {}", id);
                                        this.emit(UserEvent::DeleteProfile { id });
                                    }
                                }),
                            ),
                    )
                    // [+] Add button - uses navigation_channel
                    .child(
                        div()
                            .id("btn-add-profile")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("+")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, _cx| {
                                    println!(">>> Add profile button clicked <<<");
                                    tracing::info!(
                                        "Add profile clicked - navigating to ModelSelector"
                                    );
                                    crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::ModelSelector,
                                    );
                                }),
                            ),
                    )
                    // Spacer
                    .child(div().flex_1())
                    // [Edit] button - emits event (presenter performs prefill + navigation)
                    .child(
                        div()
                            .id("btn-edit-profile")
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(has_selection, |d| d.hover(|s| s.bg(Theme::bg_dark())))
                            .text_size(px(12.0))
                            .text_color(if has_selection {
                                Theme::text_primary()
                            } else {
                                Theme::text_muted()
                            })
                            .child("Edit")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    if let Some(id) = this.state.selected_profile_id {
                                        tracing::info!("Edit profile clicked: {}", id);
                                        this.emit(UserEvent::EditProfile { id });
                                    }
                                }),
                            ),
                    ),
            )
    }

    /// Render a single MCP row with status and toggle
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_mcp_row(&self, mcp: &McpItem, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let mcp_id = mcp.id;
        let is_selected = self.state.selected_mcp_id == Some(mcp_id);
        let name = mcp.name.clone();
        let enabled = mcp.enabled;
        let status = mcp.status;

        // Status color
        let status_color = match status {
            McpStatus::Running => Theme::success(),
            McpStatus::Stopped => Theme::text_muted(),
            McpStatus::Error => Theme::error(),
        };

        div()
            .id(SharedString::from(format!("mcp-{}", mcp_id)))
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .when(is_selected, |d| d.bg(Theme::accent()))
            .when(!is_selected, |d| d.hover(|s| s.bg(Theme::bg_dark())))
            // Status indicator
            .child(
                div()
                    .size(px(8.0))
                    .rounded_full()
                    .bg(status_color)
                    .mr(px(8.0)),
            )
            // Name (left-aligned, truncate from left for long names)
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(if is_selected {
                        gpui::white()
                    } else {
                        Theme::text_primary()
                    })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(name),
            )
            // Toggle switch
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{}", mcp_id)))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(if enabled {
                        Theme::accent()
                    } else {
                        Theme::bg_dark()
                    })
                    .text_size(px(10.0))
                    .text_color(if enabled {
                        gpui::white()
                    } else {
                        Theme::text_muted()
                    })
                    .child(if enabled { "ON" } else { "OFF" })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            tracing::info!("MCP toggle clicked: {} -> {}", mcp_id, !enabled);
                            this.emit(UserEvent::ToggleMcp {
                                id: mcp_id,
                                enabled: !enabled,
                            });
                            // Update local state
                            if let Some(m) = this.state.mcps.iter_mut().find(|m| m.id == mcp_id) {
                                m.enabled = !m.enabled;
                                m.status = if m.enabled {
                                    McpStatus::Running
                                } else {
                                    McpStatus::Stopped
                                };
                            }
                            cx.notify();
                        }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("MCP row selected: {}", mcp_id);
                    this.state.selected_mcp_id = Some(mcp_id);
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// Render the MCP tools section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_mcp_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let mcps = &self.state.mcps;
        let has_selection = self.state.selected_mcp_id.is_some();
        let total_mcps = mcps.len();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Section header
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .child("MCP TOOLS")
            )
            // List box
            .child(
                div()
                    .id("mcps-list")
                    .w(px(360.0))
                    .h(px(100.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .children(mcps.iter().map(|m| self.render_mcp_row(m, cx)))
                    .when(mcps.is_empty(), |d| {
                        d.items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::text_muted())
                                    .child("No MCP tools configured")
                            )
                    })
                    .when(total_mcps > 0, |d| {
                        d.child(
                            div()
                                .w_full()
                                .px(px(8.0))
                                .pb(px(2.0))
                                .text_size(px(10.0))
                                .text_color(Theme::text_muted())
                                .child(format!("{} MCP tools", total_mcps)),
                        )
                    })
            )
            // Toolbar: [-] [+] [spacer] [Edit]
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // [-] Delete button
                    .child(
                        div()
                            .id("btn-delete-mcp")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(has_selection, |d| d.hover(|s| s.bg(Theme::danger())))
                            .text_size(px(14.0))
                            .text_color(if has_selection { Theme::text_primary() } else { Theme::text_muted() })
                            .child("-")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                                if let Some(id) = this.state.selected_mcp_id {
                                    tracing::info!("Delete MCP clicked: {}", id);
                                    this.emit(UserEvent::DeleteMcp { id });
                                }
                            }))
                    )
                    // [+] Add button
                    .child(
                        div()
                            .id("btn-add-mcp")
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(14.0))
                            .text_color(Theme::text_primary())
                            .child("+")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                tracing::info!("Add MCP clicked - navigating to McpAdd");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::McpAdd
                                );
                            }))
                    )
                    // Spacer
                    .child(div().flex_1())
                    // [Edit] button
                    .child(
                        div()
                            .id("btn-edit-mcp")
                            .px(px(12.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(has_selection, |d| d.hover(|s| s.bg(Theme::bg_dark())))
                            .text_size(px(12.0))
                            .text_color(if has_selection { Theme::text_primary() } else { Theme::text_muted() })
                            .child("Edit")
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                                if let Some(id) = this.state.selected_mcp_id {
                                    tracing::info!("Edit MCP clicked: {}", id);
                                    this.emit(UserEvent::ConfigureMcp { id });
                                    crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::McpConfigure
                                    );
                                }
                            }))
                    )
            )
    }

    /// Render the global hotkey section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn render_hotkey_section(&self, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let hotkey = self.state.hotkey.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            // Section header
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .child("GLOBAL HOTKEY"),
            )
            // Hotkey field
            .child(
                div()
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child(hotkey),
            )
    }
}

impl gpui::Focusable for SettingsView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for SettingsView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("settings-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;

                    println!(
                        ">>> SettingsView key: {} platform={} <<<",
                        key, modifiers.platform
                    );

                    // Escape or Cmd+W: Go back to Chat
                    if key == "escape" || (modifiers.platform && key == "w") {
                        println!(">>> Escape/Cmd+W pressed - navigating to Chat <<<");
                        crate::ui_gpui::navigation_channel()
                            .request_navigate(crate::presentation::view_command::ViewId::Chat);
                    }
                    // "+" key: Add new profile (navigate to ModelSelector)
                    else if key == "=" && modifiers.shift {
                        // Shift+= is "+"
                        println!(">>> + pressed - Add Profile <<<");
                        crate::ui_gpui::navigation_channel().request_navigate(
                            crate::presentation::view_command::ViewId::ModelSelector,
                        );
                    }
                    // "e" key: Edit selected profile
                    else if key == "e" && !modifiers.platform {
                        if let Some(id) = this.state.selected_profile_id {
                            println!(">>> e pressed - Edit Profile {:?} <<<", id);
                            this.emit(UserEvent::EditProfile { id });
                        }
                    }
                    // "m" key: Add MCP
                    else if key == "m" && !modifiers.platform {
                        println!(">>> m pressed - Add MCP <<<");
                        crate::ui_gpui::navigation_channel()
                            .request_navigate(crate::presentation::view_command::ViewId::McpAdd);
                    }
                    // Arrow key navigation for profile list
                    else if key == "up" && !modifiers.platform {
                        this.scroll_profiles(-1);
                        cx.notify();
                    } else if key == "down" && !modifiers.platform {
                        this.scroll_profiles(1);
                        cx.notify();
                    }
                }),
            )
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content scroll area
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .overflow_hidden()
                    // Profiles section
                    .child(self.render_profiles_section(cx))
                    // MCP Tools section
                    .child(self.render_mcp_section(cx))
                    // Hotkey section
                    .child(self.render_hotkey_section(cx)),
            )
    }
}
