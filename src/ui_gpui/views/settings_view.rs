//! Settings view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P06
//! @requirement REQ-UI-ST

use gpui::{div, prelude::*, px, FocusHandle, FontWeight, MouseButton, SharedString};
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ProfileSummary, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Represents a profile in the settings list
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Debug, PartialEq, Eq)]
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

    #[must_use]
    pub fn with_model(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = provider.into();
        self.model = model.into();
        self
    }

    #[must_use]
    pub const fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// Format display string: "name (provider:model)"
    #[must_use]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.status = if enabled {
            McpStatus::Running
        } else {
            McpStatus::Stopped
        };
        self
    }

    #[must_use]
    pub const fn with_status(mut self, status: McpStatus) -> Self {
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
    #[must_use]
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
            if self
                .state
                .profiles
                .iter()
                .all(|profile| profile.id != selected_id)
            {
                self.state.selected_profile_id =
                    self.state.profiles.first().map(|profile| profile.id);
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
        self.state.selected_profile_id.and_then(|id| {
            self.state
                .profiles
                .iter()
                .position(|profile| profile.id == id)
        })
    }

    fn select_profile_by_index(&mut self, index: usize, emit_event: bool) {
        if let Some(profile) = self.state.profiles.get(index) {
            self.state.selected_profile_id = Some(profile.id);
            if emit_event {
                self.emit(&UserEvent::SelectProfile { id: profile.id });
            }
        }
    }

    fn scroll_profiles(&mut self, delta_steps: i32) {
        if self.state.profiles.is_empty() || delta_steps == 0 {
            return;
        }

        let current = self.selected_profile_index().unwrap_or(0);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss
        )]
        let next = {
            let max_index = self.state.profiles.len().saturating_sub(1) as i32;
            (current as i32 + delta_steps).clamp(0, max_index) as usize
        };
        self.select_profile_by_index(next, true);
    }

    fn select_profile(&mut self, profile_id: Uuid, cx: &mut gpui::Context<Self>) {
        self.state.selected_profile_id = Some(profile_id);
        self.emit(&UserEvent::SelectProfile { id: profile_id });
        cx.notify();
    }

    fn delete_selected_profile(&self) {
        if let Some(id) = self.state.selected_profile_id {
            self.emit(&UserEvent::DeleteProfile { id });
        }
    }

    fn edit_selected_profile(&self) {
        if let Some(id) = self.state.selected_profile_id {
            self.emit(&UserEvent::EditProfile { id });
        }
    }

    fn navigate_to_chat() {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::Chat);
    }

    fn navigate_to_profile_editor() {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::ProfileEditor);
    }

    fn toggle_mcp(&self, id: Uuid, enabled: bool) {
        self.emit(&UserEvent::ToggleMcp { id, enabled });
    }

    fn select_mcp(&mut self, mcp_id: Uuid, cx: &mut gpui::Context<Self>) {
        self.state.selected_mcp_id = Some(mcp_id);
        cx.notify();
    }

    fn delete_selected_mcp(&self) {
        if let Some(id) = self.state.selected_mcp_id {
            self.emit(&UserEvent::DeleteMcp { id });
        }
    }

    fn navigate_to_mcp_add() {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::McpAdd);
    }

    fn edit_selected_mcp(&self) {
        if let Some(id) = self.state.selected_mcp_id {
            self.emit(&UserEvent::ConfigureMcp { id });
            crate::ui_gpui::navigation_channel()
                .request_navigate(crate::presentation::view_command::ViewId::McpConfigure);
        }
    }

    fn handle_key_down(&mut self, event: &gpui::KeyDownEvent, cx: &mut gpui::Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if key == "escape" || (modifiers.platform && key == "w") {
            Self::navigate_to_chat();
        } else if key == "=" && modifiers.shift {
            Self::navigate_to_profile_editor();
        } else if key == "e" && !modifiers.platform {
            self.edit_selected_profile();
        } else if key == "m" && !modifiers.platform {
            Self::navigate_to_mcp_add();
        } else if key == "up" && !modifiers.platform {
            self.scroll_profiles(-1);
            cx.notify();
        } else if key == "down" && !modifiers.platform {
            self.scroll_profiles(1);
            cx.notify();
        }
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P06
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Handle `ViewCommand` from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P06
    #[allow(clippy::too_many_lines)]
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
                // Map runtime status to view status.  `enabled` comes
                // from config (set by McpServerStarted/toggle) — only
                // Running/Starting promote it; Stopped/Failed leave it
                // unchanged so the toggle matches config.json truth.
                let (mapped, force_enabled) = match status {
                    crate::presentation::view_command::McpStatus::Running
                    | crate::presentation::view_command::McpStatus::Starting => {
                        (McpStatus::Running, Some(true))
                    }
                    crate::presentation::view_command::McpStatus::Failed
                    | crate::presentation::view_command::McpStatus::Unhealthy => {
                        (McpStatus::Error, None)
                    }
                    crate::presentation::view_command::McpStatus::Stopped => {
                        (McpStatus::Stopped, None)
                    }
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
            ViewCommand::McpServerStarted {
                id, name, enabled, ..
            } => {
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
            ViewCommand::McpServerFailed { id, .. } => {
                if let Some(existing) = self.state.mcps.iter_mut().find(|m| m.id == id) {
                    existing.status = McpStatus::Error;
                    existing.enabled = false;
                } else {
                    self.state
                        .mcps
                        .push(McpItem::new(id, format!("MCP {id}")).with_status(McpStatus::Error));
                }
            }
            ViewCommand::McpConfigSaved { id, name } => {
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
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
                            this.emit(&UserEvent::RefreshModelsRegistry);
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
            .id(SharedString::from(format!("profile-{profile_id}")))
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
                    this.select_profile(profile_id, cx);
                }),
            )
            .into_any_element()
    }

    /// Render the profiles section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    #[allow(clippy::too_many_lines)]
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
                    .w_full()
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
                                .child(format!("{total_profiles} profiles")),
                        )
                    }),
            )
            // Toolbar: [-] [+] [spacer] [Edit]
            .child(
                div()
                    .w_full()
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
                                    }
                                    this.delete_selected_profile();
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
                                    tracing::info!(
                                        "Add profile clicked - navigating to ModelSelector"
                                    );
                                    Self::navigate_to_profile_editor();
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
                                    }
                                    this.edit_selected_profile();
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
            .id(SharedString::from(format!("mcp-{mcp_id}")))
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
                    .id(SharedString::from(format!("toggle-{mcp_id}")))
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
                        cx.listener(move |this, _, _window, _cx| {
                            tracing::info!("MCP toggle clicked: {} -> {}", mcp_id, !enabled);
                            this.toggle_mcp(mcp_id, !enabled);
                        }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("MCP row selected: {}", mcp_id);
                    this.select_mcp(mcp_id, cx);
                }),
            )
            .into_any_element()
    }

    /// Render the MCP tools section
    /// @plan PLAN-20250130-GPUIREDUX.P06
    #[allow(clippy::too_many_lines)]
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
                    .child("MCP TOOLS"),
            )
            // List box
            .child(
                div()
                    .id("mcps-list")
                    .w_full()
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
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("No MCP tools configured"),
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
                                .child(format!("{total_mcps} MCP tools")),
                        )
                    }),
            )
            // Toolbar: [-] [+] [spacer] [Edit]
            .child(
                div()
                    .w_full()
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
                            .text_color(if has_selection {
                                Theme::text_primary()
                            } else {
                                Theme::text_muted()
                            })
                            .child("-")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    if let Some(id) = this.state.selected_mcp_id {
                                        tracing::info!("Delete MCP clicked: {}", id);
                                    }
                                    this.delete_selected_mcp();
                                }),
                            ),
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, _cx| {
                                    tracing::info!("Add MCP clicked - navigating to McpAdd");
                                    Self::navigate_to_mcp_add();
                                }),
                            ),
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
                            .text_color(if has_selection {
                                Theme::text_primary()
                            } else {
                                Theme::text_muted()
                            })
                            .child("Edit")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    if let Some(id) = this.state.selected_mcp_id {
                                        tracing::info!("Edit MCP clicked: {}", id);
                                    }
                                    this.edit_selected_mcp();
                                }),
                            ),
                    ),
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
                    .w_full()
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
                    this.handle_key_down(event, cx);
                }),
            )
            // Top bar (44px)
            .child(Self::render_top_bar(cx))
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

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use crate::presentation::view_command::ViewId;
    use gpui::TestAppContext;

    fn clear_navigation_requests() {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
    }

    fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
    }

    use flume;

    fn profile_summary(
        id: Uuid,
        name: &str,
        provider: &str,
        model: &str,
        is_default: bool,
    ) -> ProfileSummary {
        ProfileSummary {
            id,
            name: name.to_string(),
            provider_id: provider.to_string(),
            model_id: model.to_string(),
            is_default,
        }
    }

    #[gpui::test]
    async fn handle_command_applies_profile_summaries_and_selection_fallbacks(
        cx: &mut gpui::TestAppContext,
    ) {
        let profile_a = Uuid::new_v4();
        let profile_b = Uuid::new_v4();
        let profile_c = Uuid::new_v4();
        let view = cx.new(SettingsView::new);

        view.update(cx, |view: &mut SettingsView, cx| {
            view.handle_command(
                ViewCommand::ShowSettings {
                    profiles: vec![
                        profile_summary(profile_a, "Alpha", "openai", "gpt-4o", false),
                        profile_summary(profile_b, "Beta", "anthropic", "claude", true),
                    ],
                    selected_profile_id: None,
                },
                cx,
            );

            assert_eq!(view.state.selected_profile_id, Some(profile_a));
            assert_eq!(view.state.profiles.len(), 2);
            assert_eq!(
                view.state.profiles[0].display_text(),
                "Alpha (openai:gpt-4o)"
            );
            assert_eq!(
                view.state.profiles[1].display_text(),
                "Beta (anthropic:claude)"
            );
            assert!(view.state.profiles[1].is_default);

            view.handle_command(
                ViewCommand::ChatProfilesUpdated {
                    profiles: vec![
                        profile_summary(profile_b, "Beta", "anthropic", "claude", true),
                        profile_summary(profile_c, "Gamma", "openai", "gpt-4.1", false),
                    ],
                    selected_profile_id: Some(profile_b),
                },
                cx,
            );

            assert_eq!(view.state.selected_profile_id, Some(profile_b));
            assert_eq!(view.state.profiles.len(), 2);

            view.handle_command(
                ViewCommand::ShowSettings {
                    profiles: vec![profile_summary(
                        profile_c, "Gamma", "openai", "gpt-4.1", false,
                    )],
                    selected_profile_id: Some(profile_b),
                },
                cx,
            );

            assert_eq!(view.state.selected_profile_id, Some(profile_c));
            assert_eq!(view.state.profiles.len(), 1);
            assert_eq!(view.state.profiles[0].name, "Gamma");
        });
    }

    #[allow(clippy::too_many_lines)]
    #[gpui::test]
    async fn profile_navigation_and_mcp_commands_update_state_and_emit_events(
        cx: &mut gpui::TestAppContext,
    ) {
        let profile_a = Uuid::new_v4();
        let profile_b = Uuid::new_v4();
        let mcp_existing = Uuid::new_v4();
        let mcp_new = Uuid::new_v4();
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(SettingsView::new);

        view.update(cx, |view: &mut SettingsView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.set_profiles(vec![
                ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
                ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
            ]);
            view.state.selected_profile_id = Some(profile_a);

            view.scroll_profiles(1);
            assert_eq!(view.state.selected_profile_id, Some(profile_b));
            view.scroll_profiles(20);
            assert_eq!(view.state.selected_profile_id, Some(profile_b));
            view.scroll_profiles(-20);
            assert_eq!(view.state.selected_profile_id, Some(profile_a));

            view.set_mcps(vec![
                McpItem::new(mcp_existing, "Existing").with_status(McpStatus::Stopped)
            ]);
            assert_eq!(view.state.selected_mcp_id, Some(mcp_existing));

            view.handle_command(
                ViewCommand::McpStatusChanged {
                    id: mcp_existing,
                    status: crate::presentation::view_command::McpStatus::Running,
                },
                cx,
            );
            let existing = view
                .state
                .mcps
                .iter()
                .find(|mcp| mcp.id == mcp_existing)
                .expect("existing mcp retained");
            assert_eq!(existing.status, McpStatus::Running);
            assert!(existing.enabled);

            view.handle_command(
                ViewCommand::McpServerStarted {
                    id: mcp_new,
                    name: Some("Fetch".to_string()),
                    tool_count: 3,
                    enabled: Some(false),
                },
                cx,
            );
            let inserted = view
                .state
                .mcps
                .iter()
                .find(|mcp| mcp.id == mcp_new)
                .expect("new mcp inserted");
            assert_eq!(inserted.name, "Fetch");
            assert!(!inserted.enabled);
            assert_eq!(inserted.status, McpStatus::Stopped);

            view.handle_command(
                ViewCommand::McpServerFailed {
                    id: mcp_new,
                    error: "boom".to_string(),
                },
                cx,
            );
            let failed = view
                .state
                .mcps
                .iter()
                .find(|mcp| mcp.id == mcp_new)
                .expect("mcp still present after failure");
            assert_eq!(failed.status, McpStatus::Error);
            assert!(!failed.enabled);

            view.handle_command(
                ViewCommand::McpConfigSaved {
                    id: mcp_new,
                    name: Some("Saved MCP".to_string()),
                },
                cx,
            );
            let saved = view
                .state
                .mcps
                .iter()
                .find(|mcp| mcp.id == mcp_new)
                .expect("saved mcp retained");
            assert_eq!(view.state.selected_mcp_id, Some(mcp_new));
            assert_eq!(saved.name, "Saved MCP");
            assert!(saved.enabled);
            assert_eq!(saved.status, McpStatus::Stopped);

            view.handle_command(ViewCommand::McpDeleted { id: mcp_new }, cx);
            assert!(view.state.mcps.iter().all(|mcp| mcp.id != mcp_new));
        });

        assert_eq!(
            user_rx.recv().expect("profile scroll selects beta"),
            UserEvent::SelectProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().expect("profile scroll returns to alpha"),
            UserEvent::SelectProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().expect("profile scroll selects alpha"),
            UserEvent::SelectProfile { id: profile_a }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "settings view test should emit only expected bridge events"
        );
    }

    #[gpui::test]
    async fn profile_and_mcp_setters_enforce_selection_fallbacks_without_bridge(
        cx: &mut gpui::TestAppContext,
    ) {
        let profile_a = Uuid::new_v4();
        let profile_b = Uuid::new_v4();
        let profile_c = Uuid::new_v4();
        let mcp_a = Uuid::new_v4();
        let mcp_b = Uuid::new_v4();
        let mcp_c = Uuid::new_v4();
        let view = cx.new(SettingsView::new);

        view.update(cx, |view: &mut SettingsView, _cx| {
            view.set_profiles(vec![
                ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
                ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
            ]);
            assert_eq!(view.state.selected_profile_id, None);

            view.state.selected_profile_id = Some(profile_a);
            view.set_profiles(vec![
                ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
                ProfileItem::new(profile_c, "Gamma").with_model("openai", "gpt-4.1"),
            ]);
            assert_eq!(view.state.selected_profile_id, Some(profile_a));
            assert_eq!(view.state.profiles.len(), 2);
            assert_eq!(
                view.state.profiles[1].display_text(),
                "Gamma (openai:gpt-4.1)"
            );

            view.state.selected_profile_id = Some(profile_b);
            view.set_profiles(vec![
                ProfileItem::new(profile_c, "Gamma").with_model("openai", "gpt-4.1")
            ]);
            assert_eq!(view.state.selected_profile_id, Some(profile_b));

            view.set_mcps(vec![
                McpItem::new(mcp_a, "Existing").with_status(McpStatus::Stopped),
                McpItem::new(mcp_b, "Runner").with_enabled(true),
            ]);
            assert_eq!(view.state.selected_mcp_id, Some(mcp_a));

            view.state.selected_mcp_id = Some(mcp_b);
            view.set_mcps(vec![
                McpItem::new(mcp_b, "Runner").with_enabled(true),
                McpItem::new(mcp_c, "Fetcher").with_status(McpStatus::Error),
            ]);
            assert_eq!(view.state.selected_mcp_id, Some(mcp_b));
            assert_eq!(view.state.mcps[0].status, McpStatus::Running);
            assert_eq!(view.state.mcps[1].status, McpStatus::Error);

            view.state.selected_mcp_id = Some(mcp_a);
            view.set_mcps(vec![
                McpItem::new(mcp_c, "Fetcher").with_status(McpStatus::Error)
            ]);
            assert_eq!(view.state.selected_mcp_id, Some(mcp_c));
            assert_eq!(view.state.mcps.len(), 1);
            assert!(!view.state.mcps[0].enabled);
        });
    }

    #[gpui::test]
    #[allow(clippy::too_many_lines)]
    async fn helper_actions_and_key_handling_emit_expected_events_and_navigation(
        cx: &mut TestAppContext,
    ) {
        clear_navigation_requests();
        let profile_a = Uuid::new_v4();
        let profile_b = Uuid::new_v4();
        let mcp_a = Uuid::new_v4();
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(SettingsView::new);

        view.update(cx, |view: &mut SettingsView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.set_profiles(vec![
                ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
                ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
            ]);
            view.set_mcps(vec![McpItem::new(mcp_a, "Fetcher").with_enabled(true)]);
            view.state.selected_profile_id = Some(profile_a);
            view.state.selected_mcp_id = Some(mcp_a);

            view.select_profile(profile_b, cx);
            assert_eq!(view.state.selected_profile_id, Some(profile_b));

            view.delete_selected_profile();
            view.edit_selected_profile();

            view.toggle_mcp(mcp_a, false);
            view.select_mcp(mcp_a, cx);
            assert_eq!(view.state.selected_mcp_id, Some(mcp_a));
            view.delete_selected_mcp();
            view.edit_selected_mcp();
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::McpConfigure)
            );

            view.handle_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("up").expect("up keystroke"),
                    is_held: false,
                    prefer_character_input: false,
                },
                cx,
            );
            assert_eq!(view.state.selected_profile_id, Some(profile_a));

            view.handle_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("down").expect("down keystroke"),
                    is_held: false,
                    prefer_character_input: false,
                },
                cx,
            );
            assert_eq!(view.state.selected_profile_id, Some(profile_b));

            view.handle_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("e").expect("e keystroke"),
                    is_held: false,
                    prefer_character_input: false,
                },
                cx,
            );

            view.handle_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("shift-=").expect("plus keystroke"),
                    is_held: false,
                    prefer_character_input: false,
                },
                cx,
            );
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::ProfileEditor)
            );

            view.handle_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("m").expect("m keystroke"),
                    is_held: false,
                    prefer_character_input: false,
                },
                cx,
            );
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::McpAdd)
            );

            view.handle_key_down(
                &gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse("cmd-w").expect("cmd-w keystroke"),
                    is_held: false,
                    prefer_character_input: false,
                },
                cx,
            );
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::Chat)
            );

            SettingsView::navigate_to_profile_editor();
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::ProfileEditor)
            );
            SettingsView::navigate_to_mcp_add();
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::McpAdd)
            );
            SettingsView::navigate_to_chat();
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::Chat)
            );
        });

        assert_eq!(
            user_rx.recv().expect("profile select helper"),
            UserEvent::SelectProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().expect("delete profile helper"),
            UserEvent::DeleteProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().expect("edit profile helper"),
            UserEvent::EditProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().expect("toggle mcp helper"),
            UserEvent::ToggleMcp {
                id: mcp_a,
                enabled: false,
            }
        );
        assert_eq!(
            user_rx.recv().expect("delete mcp helper"),
            UserEvent::DeleteMcp { id: mcp_a }
        );
        assert_eq!(
            user_rx.recv().expect("configure mcp helper"),
            UserEvent::ConfigureMcp { id: mcp_a }
        );
        assert_eq!(
            user_rx.recv().expect("up key selects alpha"),
            UserEvent::SelectProfile { id: profile_a }
        );
        assert_eq!(
            user_rx.recv().expect("down key selects beta"),
            UserEvent::SelectProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().expect("e key edits profile"),
            UserEvent::EditProfile { id: profile_b }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional settings events"
        );
    }
}
