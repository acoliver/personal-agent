//! Settings view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P06
//! @requirement REQ-UI-ST

mod command;
mod render;

use gpui::FocusHandle;
use std::sync::Arc;
use uuid::Uuid;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ProfileSummary, ThemeSummary};
use crate::ui_gpui::bridge::GpuiBridge;

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
    #[must_use]
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

    #[must_use]
    pub fn display_text(&self) -> String {
        if self.provider.is_empty() && self.model.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({}:{})", self.name, self.provider, self.model)
        }
    }
}

/// Status of an MCP server in the settings view
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpStatus {
    Running,
    #[default]
    Stopped,
    Error,
}

/// Represents an MCP server in the settings list
/// @plan PLAN-20250130-GPUIREDUX.P06
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpItem {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub status: McpStatus,
}

impl McpItem {
    #[must_use]
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
    pub fn with_status(mut self, status: McpStatus) -> Self {
        self.status = status;
        if status == McpStatus::Error {
            self.enabled = false;
        }
        self
    }
}

/// A theme option as presented in the settings dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeOption {
    pub name: String,
    pub slug: String,
}

/// Settings view state
/// @plan PLAN-20250130-GPUIREDUX.P06
pub struct SettingsState {
    pub profiles: Vec<ProfileItem>,
    pub mcps: Vec<McpItem>,
    pub selected_profile_id: Option<Uuid>,
    pub selected_mcp_id: Option<Uuid>,
    /// Available themes for the dropdown.
    pub available_themes: Vec<ThemeOption>,
    /// Slug of the currently-selected theme.
    pub selected_theme_slug: String,
}

impl SettingsState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            mcps: Vec::new(),
            selected_profile_id: None,
            selected_mcp_id: None,
            available_themes: Vec::new(),
            selected_theme_slug: "green-screen".to_string(),
        }
    }
}

/// Settings view
/// @plan PLAN-20250130-GPUIREDUX.P06
pub struct SettingsView {
    pub(super) state: SettingsState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
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

    #[must_use]
    pub const fn get_state(&self) -> &SettingsState {
        &self.state
    }

    /// Apply theme options from a `ShowSettingsTheme` command.
    pub(super) fn apply_theme_options(
        &mut self,
        options: Vec<ThemeSummary>,
        selected_slug: String,
    ) {
        self.state.available_themes = options
            .into_iter()
            .map(|t| ThemeOption {
                name: t.name,
                slug: t.slug,
            })
            .collect();

        // Use provided slug if it exists in the list; otherwise keep the
        // first entry or the current selection.
        if self
            .state
            .available_themes
            .iter()
            .any(|t| t.slug == selected_slug)
        {
            self.state.selected_theme_slug = selected_slug;
        } else if let Some(first) = self.state.available_themes.first() {
            self.state.selected_theme_slug = first.slug.clone();
        }
    }

    /// Select a theme by slug and emit the event.
    pub(super) fn select_theme(&mut self, slug: String, cx: &mut gpui::Context<Self>) {
        self.state.selected_theme_slug.clone_from(&slug);
        self.emit(&UserEvent::SelectTheme { slug });
        cx.notify();
    }

    /// Set profiles from presenter
    pub fn set_profiles(&mut self, profiles: Vec<ProfileItem>) {
        self.state.profiles = profiles;
    }

    pub(super) fn apply_profile_summaries(
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

    pub(super) fn select_profile(&mut self, profile_id: Uuid, cx: &mut gpui::Context<Self>) {
        self.state.selected_profile_id = Some(profile_id);
        self.emit(&UserEvent::SelectProfile { id: profile_id });
        cx.notify();
    }

    pub(super) fn delete_selected_profile(&self) {
        if let Some(id) = self.state.selected_profile_id {
            self.emit(&UserEvent::DeleteProfile { id });
        }
    }

    pub(super) fn edit_selected_profile(&self) {
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

    pub(super) fn toggle_mcp(&self, id: Uuid, enabled: bool) {
        self.emit(&UserEvent::ToggleMcp { id, enabled });
    }

    pub(super) fn select_mcp(&mut self, mcp_id: Uuid, cx: &mut gpui::Context<Self>) {
        self.state.selected_mcp_id = Some(mcp_id);
        cx.notify();
    }

    pub(super) fn delete_selected_mcp(&self) {
        if let Some(id) = self.state.selected_mcp_id {
            self.emit(&UserEvent::DeleteMcp { id });
        }
    }

    fn navigate_to_mcp_add() {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::McpAdd);
    }

    pub(super) fn edit_selected_mcp(&self) {
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
        } else if (key == "enter" || key == "space") && !modifiers.platform {
            if self.state.available_themes.is_empty() {
                return;
            }

            let selected_slug = self
                .state
                .available_themes
                .iter()
                .find(|theme| theme.slug == self.state.selected_theme_slug)
                .map(|theme| theme.slug.clone())
                .or_else(|| {
                    self.state
                        .available_themes
                        .first()
                        .map(|theme| theme.slug.clone())
                });

            if let Some(slug) = selected_slug {
                self.select_theme(slug, cx);
            }
        }
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P06
    pub(super) fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use crate::presentation::view_command::{ViewCommand, ViewId};
    use gpui::AppContext;
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

    #[gpui::test]
    async fn scroll_profiles_clamps_and_emits_selection_events(cx: &mut gpui::TestAppContext) {
        let profile_a = Uuid::new_v4();
        let profile_b = Uuid::new_v4();
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(SettingsView::new);

        view.update(cx, |view: &mut SettingsView, _cx| {
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
    async fn mcp_commands_update_status_lifecycle_and_selection(cx: &mut gpui::TestAppContext) {
        let mcp_existing = Uuid::new_v4();
        let mcp_new = Uuid::new_v4();
        let view = cx.new(SettingsView::new);

        view.update(cx, |view: &mut SettingsView, cx| {
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
                .find(|m| m.id == mcp_existing)
                .unwrap();
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
            let inserted = view.state.mcps.iter().find(|m| m.id == mcp_new).unwrap();
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
            let failed = view.state.mcps.iter().find(|m| m.id == mcp_new).unwrap();
            assert_eq!(failed.status, McpStatus::Error);
            assert!(!failed.enabled);

            view.handle_command(
                ViewCommand::McpConfigSaved {
                    id: mcp_new,
                    name: Some("Saved MCP".to_string()),
                },
                cx,
            );
            let saved = view.state.mcps.iter().find(|m| m.id == mcp_new).unwrap();
            assert_eq!(view.state.selected_mcp_id, Some(mcp_new));
            assert_eq!(saved.name, "Saved MCP");
            assert!(saved.enabled);
            assert_eq!(saved.status, McpStatus::Stopped);

            view.handle_command(ViewCommand::McpDeleted { id: mcp_new }, cx);
            assert!(view.state.mcps.iter().all(|m| m.id != mcp_new));
        });
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

    fn settings_key_event(key: &str) -> gpui::KeyDownEvent {
        gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
            is_held: false,
            prefer_character_input: false,
        }
    }

    #[gpui::test]
    async fn helper_actions_emit_expected_bridge_events(cx: &mut TestAppContext) {
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
        });

        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::SelectProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::DeleteProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::EditProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::ToggleMcp {
                id: mcp_a,
                enabled: false
            }
        );
        assert_eq!(user_rx.recv().unwrap(), UserEvent::DeleteMcp { id: mcp_a });
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::ConfigureMcp { id: mcp_a }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional settings events"
        );
    }

    #[gpui::test]
    async fn key_handling_navigates_and_emits_profile_events(cx: &mut TestAppContext) {
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
            view.state.selected_profile_id = Some(profile_b);

            view.handle_key_down(&settings_key_event("up"), cx);
            assert_eq!(view.state.selected_profile_id, Some(profile_a));

            view.handle_key_down(&settings_key_event("down"), cx);
            assert_eq!(view.state.selected_profile_id, Some(profile_b));

            view.handle_key_down(&settings_key_event("e"), cx);

            view.handle_key_down(&settings_key_event("shift-="), cx);
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::ProfileEditor)
            );

            view.handle_key_down(&settings_key_event("m"), cx);
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::McpAdd)
            );

            view.state.available_themes = vec![ThemeOption {
                name: "Green Screen".to_string(),
                slug: "green-screen".to_string(),
            }];
            view.state.selected_theme_slug = "green-screen".to_string();
            view.handle_key_down(&settings_key_event("enter"), cx);

            view.handle_key_down(&settings_key_event("cmd-w"), cx);
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
            user_rx.recv().unwrap(),
            UserEvent::SelectProfile { id: profile_a }
        );
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::SelectProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::EditProfile { id: profile_b }
        );
        assert_eq!(
            user_rx.recv().unwrap(),
            UserEvent::SelectTheme {
                slug: "green-screen".to_string()
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional settings events"
        );
    }
}
