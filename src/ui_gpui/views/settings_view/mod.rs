//! Settings view implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P06
//! @requirement REQ-UI-ST

mod command;
mod render;
mod render_appearance;
mod render_tool_approval;

use gpui::{Bounds, Pixels};

use gpui::{FocusHandle, Window};
use std::ops::Range;
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::McpApprovalMode;
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

/// Which font dropdown is currently open in the Appearance panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontDropdownTarget {
    UiFont,
    MonoFont,
}

/// Categories shown in the settings sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    #[default]
    General,
    Appearance,
    Models,
    Security,
    McpTools,
}

impl SettingsCategory {
    pub const ALL: [Self; 5] = [
        Self::General,
        Self::Appearance,
        Self::Models,
        Self::Security,
        Self::McpTools,
    ];

    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Models => "Models",
            Self::Security => "Security",
            Self::McpTools => "MCP Tools",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(super) enum ActiveField {
    AllowlistInput,
    DenylistInput,
    ExportDirInput,
}

/// Settings view state
/// @plan PLAN-20250130-GPUIREDUX.P06
#[allow(clippy::struct_excessive_bools)]
pub struct SettingsState {
    pub profiles: Vec<ProfileItem>,
    pub mcps: Vec<McpItem>,
    pub selected_profile_id: Option<Uuid>,
    pub selected_mcp_id: Option<Uuid>,
    /// Available themes for the dropdown.
    pub available_themes: Vec<ThemeOption>,
    /// Slug of the currently-selected theme.
    pub selected_theme_slug: String,
    pub selected_category: SettingsCategory,
    pub theme_dropdown_open: bool,
    pub yolo_mode: bool,
    pub auto_approve_reads: bool,
    pub mcp_approval_mode: McpApprovalMode,
    pub persistent_allowlist: Vec<String>,
    pub persistent_denylist: Vec<String>,
    pub allowlist_input: String,
    pub denylist_input: String,
    pub export_dir_input: String,
    pub(super) active_field: Option<ActiveField>,
    pub status_message: Option<String>,
    pub status_is_error: bool,
    // Font settings (Appearance panel)
    pub font_size: f32,
    pub ui_font_family: Option<String>,
    pub mono_font_family: String,
    pub mono_ligatures: bool,
    pub font_dropdown_open_for: Option<FontDropdownTarget>,
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
            selected_category: SettingsCategory::General,
            theme_dropdown_open: false,
            yolo_mode: false,
            auto_approve_reads: false,
            mcp_approval_mode: McpApprovalMode::PerTool,
            persistent_allowlist: Vec::new(),
            persistent_denylist: Vec::new(),
            allowlist_input: String::new(),
            denylist_input: String::new(),
            export_dir_input: String::new(),
            active_field: None,
            status_message: None,
            status_is_error: false,
            font_size: 14.0,
            ui_font_family: None,
            mono_font_family: crate::ui_gpui::theme::DEFAULT_MONO_FONT_FAMILY.to_string(),
            mono_ligatures: true,
            font_dropdown_open_for: None,
        }
    }
}

/// Settings view
/// @plan PLAN-20250130-GPUIREDUX.P06
pub struct SettingsView {
    pub(super) state: SettingsState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) ime_marked_byte_count: usize,
}

impl SettingsView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: SettingsState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
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

    pub fn apply_profile_summaries(
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

    fn selected_theme_index(&self) -> Option<usize> {
        self.state
            .available_themes
            .iter()
            .position(|theme| theme.slug == self.state.selected_theme_slug)
    }

    fn append_to_active_field(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match self.state.active_field {
            Some(ActiveField::AllowlistInput) => self.state.allowlist_input.push_str(text),
            Some(ActiveField::DenylistInput) => self.state.denylist_input.push_str(text),
            Some(ActiveField::ExportDirInput) => self.state.export_dir_input.push_str(text),
            None => {}
        }
    }

    fn backspace_active_field(&mut self) {
        match self.state.active_field {
            Some(ActiveField::AllowlistInput) => {
                self.state.allowlist_input.pop();
            }
            Some(ActiveField::DenylistInput) => {
                self.state.denylist_input.pop();
            }
            Some(ActiveField::ExportDirInput) => {
                self.state.export_dir_input.pop();
            }
            None => {}
        }
    }

    fn remove_trailing_bytes_from_active_field(&mut self, byte_count: usize) {
        if byte_count == 0 {
            return;
        }

        match self.state.active_field {
            Some(ActiveField::AllowlistInput) => {
                let len = self.state.allowlist_input.len();
                self.state
                    .allowlist_input
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::DenylistInput) => {
                let len = self.state.denylist_input.len();
                self.state
                    .denylist_input
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::ExportDirInput) => {
                let len = self.state.export_dir_input.len();
                self.state
                    .export_dir_input
                    .truncate(len.saturating_sub(byte_count));
            }
            None => {}
        }
    }

    fn active_field_text(&self) -> &str {
        match self.state.active_field {
            Some(ActiveField::AllowlistInput) => &self.state.allowlist_input,
            Some(ActiveField::DenylistInput) => &self.state.denylist_input,
            Some(ActiveField::ExportDirInput) => &self.state.export_dir_input,
            None => "",
        }
    }

    const fn set_active_field(&mut self, field: Option<ActiveField>) {
        self.state.active_field = field;
        self.ime_marked_byte_count = 0;
    }

    const fn cycle_active_field(&mut self) {
        let next = match self.state.active_field {
            Some(ActiveField::ExportDirInput) => ActiveField::AllowlistInput,
            Some(ActiveField::AllowlistInput) => ActiveField::DenylistInput,
            Some(ActiveField::DenylistInput) | None => ActiveField::ExportDirInput,
        };
        self.set_active_field(Some(next));
    }

    fn emit_set_yolo_mode(&self, enabled: bool) {
        self.emit(&UserEvent::SetToolApprovalYoloMode { enabled });
    }

    fn emit_set_auto_approve_reads(&self, enabled: bool) {
        self.emit(&UserEvent::SetToolApprovalAutoApproveReads { enabled });
    }

    fn emit_set_mcp_approval_mode(&self, mode: McpApprovalMode) {
        self.emit(&UserEvent::SetToolApprovalMcpApprovalMode { mode });
    }

    fn add_allowlist_entry(&mut self) {
        let prefix = self.state.allowlist_input.trim().to_string();
        if prefix.is_empty() {
            return;
        }

        self.emit(&UserEvent::AddToolApprovalAllowlistPrefix { prefix });
    }

    fn remove_allowlist_entry(&self, prefix: String) {
        self.emit(&UserEvent::RemoveToolApprovalAllowlistPrefix { prefix });
    }

    fn add_denylist_entry(&mut self) {
        let prefix = self.state.denylist_input.trim().to_string();
        if prefix.is_empty() {
            return;
        }

        self.emit(&UserEvent::AddToolApprovalDenylistPrefix { prefix });
    }

    fn remove_denylist_entry(&self, prefix: String) {
        self.emit(&UserEvent::RemoveToolApprovalDenylistPrefix { prefix });
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

    fn scroll_themes(&mut self, delta_steps: i32) {
        if self.state.available_themes.is_empty() || delta_steps == 0 {
            return;
        }

        let current = self.selected_theme_index().unwrap_or(0);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss
        )]
        let next = {
            let max_index = self.state.available_themes.len().saturating_sub(1) as i32;
            (current as i32 + delta_steps).clamp(0, max_index) as usize
        };

        if let Some(theme) = self.state.available_themes.get(next) {
            self.state.selected_theme_slug = theme.slug.clone();
        }
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
            return;
        }

        if modifiers.platform && key == "v" {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    self.append_to_active_field(&text);
                    cx.notify();
                }
            }
            return;
        }

        if modifiers.platform || modifiers.control {
            return;
        }

        if key == "backspace" {
            self.backspace_active_field();
            cx.notify();
            return;
        }

        if key == "tab" {
            self.cycle_active_field();
            cx.notify();
            return;
        }

        if key == "enter" {
            self.handle_enter_key(cx);
            return;
        }

        // When a text field is focused, don't intercept regular keys as shortcuts.
        if self.state.active_field.is_some() {
            return;
        }

        if key == "=" && modifiers.shift {
            Self::navigate_to_profile_editor();
            return;
        }

        if key == "e" {
            self.edit_selected_profile();
            return;
        }

        if key == "m" {
            Self::navigate_to_mcp_add();
            return;
        }

        if key == "up" {
            match self.state.selected_category {
                SettingsCategory::Models => self.scroll_profiles(-1),
                SettingsCategory::McpTools => self.scroll_mcps(-1),
                SettingsCategory::Appearance if self.state.theme_dropdown_open => {
                    self.scroll_themes(-1);
                }
                _ => {}
            }
            cx.notify();
            return;
        }

        if key == "down" {
            match self.state.selected_category {
                SettingsCategory::Models => self.scroll_profiles(1),
                SettingsCategory::McpTools => self.scroll_mcps(1),
                SettingsCategory::Appearance if self.state.theme_dropdown_open => {
                    self.scroll_themes(1);
                }
                _ => {}
            }
            cx.notify();
            return;
        }

        if key == "space"
            && self.state.selected_category == SettingsCategory::Appearance
            && self.state.theme_dropdown_open
        {
            self.apply_selected_theme(cx);
        }
    }

    fn handle_enter_key(&mut self, cx: &mut gpui::Context<Self>) {
        match self.state.active_field {
            Some(ActiveField::AllowlistInput) => {
                self.add_allowlist_entry();
                cx.notify();
                return;
            }
            Some(ActiveField::DenylistInput) => {
                self.add_denylist_entry();
                cx.notify();
                return;
            }
            Some(ActiveField::ExportDirInput) => {
                self.save_export_directory();
                cx.notify();
                return;
            }
            None => {}
        }
        if self.state.selected_category == SettingsCategory::Appearance
            && self.state.theme_dropdown_open
        {
            self.apply_selected_theme(cx);
            self.state.theme_dropdown_open = false;
            cx.notify();
        }
    }

    fn save_export_directory(&self) {
        let path = self.state.export_dir_input.trim().to_string();
        self.emit(&UserEvent::SetExportDirectory { path });
    }

    #[allow(clippy::unused_self)] // cx.spawn closure captures the entity handle
    fn browse_export_directory(&mut self, cx: &mut gpui::Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select Export Directory".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.first() {
                    let path_str = path.to_string_lossy().to_string();
                    cx.update(|cx| {
                        this.update(cx, |view, cx| {
                            view.state.export_dir_input = path_str;
                            view.save_export_directory();
                            cx.notify();
                        })
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn apply_selected_theme(&mut self, cx: &mut gpui::Context<Self>) {
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

    pub(super) const fn select_category(&mut self, category: SettingsCategory) {
        self.state.selected_category = category;
        self.state.theme_dropdown_open = false;
        self.state.active_field = None;
        self.ime_marked_byte_count = 0;
    }

    pub(super) const fn toggle_theme_dropdown(&mut self) {
        self.state.theme_dropdown_open = !self.state.theme_dropdown_open;
    }

    pub(super) const fn close_theme_dropdown(&mut self) {
        self.state.theme_dropdown_open = false;
    }

    pub(super) fn select_theme_from_dropdown(
        &mut self,
        slug: String,
        cx: &mut gpui::Context<Self>,
    ) {
        self.select_theme(slug, cx);
        self.state.theme_dropdown_open = false;
    }

    fn scroll_mcps(&mut self, delta_steps: i32) {
        if self.state.mcps.is_empty() || delta_steps == 0 {
            return;
        }

        let current = self
            .state
            .selected_mcp_id
            .and_then(|id| self.state.mcps.iter().position(|m| m.id == id))
            .unwrap_or(0);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss
        )]
        let next = {
            let max_index = self.state.mcps.len().saturating_sub(1) as i32;
            (current as i32 + delta_steps).clamp(0, max_index) as usize
        };
        if let Some(mcp) = self.state.mcps.get(next) {
            self.state.selected_mcp_id = Some(mcp.id);
        }
    }

    pub(super) fn toggle_font_dropdown(&mut self, target: FontDropdownTarget) {
        if self.state.font_dropdown_open_for == Some(target) {
            self.state.font_dropdown_open_for = None;
        } else {
            self.state.font_dropdown_open_for = Some(target);
        }
    }

    pub(super) fn select_ui_font(&mut self, name: Option<String>, cx: &mut gpui::Context<Self>) {
        self.state.ui_font_family.clone_from(&name);
        self.state.font_dropdown_open_for = None;
        self.emit(&UserEvent::SetUiFontFamily { name });
        cx.notify();
    }

    pub(super) fn select_mono_font(&mut self, name: String, cx: &mut gpui::Context<Self>) {
        self.state.mono_font_family.clone_from(&name);
        self.state.font_dropdown_open_for = None;
        self.emit(&UserEvent::SetMonoFontFamily { name });
        cx.notify();
    }

    pub(super) fn set_font_size(&mut self, size: f32, cx: &mut gpui::Context<Self>) {
        let clamped = size.clamp(
            crate::ui_gpui::theme::MIN_FONT_SIZE,
            crate::ui_gpui::theme::MAX_FONT_SIZE,
        );
        self.state.font_size = clamped;
        self.emit(&UserEvent::SetFontSize { size: clamped });
        cx.notify();
    }

    pub(super) fn toggle_mono_ligatures(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.mono_ligatures = !self.state.mono_ligatures;
        self.emit(&UserEvent::SetMonoLigatures {
            enabled: self.state.mono_ligatures,
        });
        cx.notify();
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

impl gpui::EntityInputHandler for SettingsView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = self.active_field_text();
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let start = range.start.min(utf16.len());
        let end = range.end.min(utf16.len());
        String::from_utf16(&utf16[start..end]).ok()
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        let len16 = self.active_field_text().encode_utf16().count();
        Some(gpui::UTF16Selection {
            range: len16..len16,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime_marked_byte_count > 0 {
            let text = self.active_field_text();
            let len16: usize = text.encode_utf16().count();
            let start_utf8 = text.len().saturating_sub(self.ime_marked_byte_count);
            let start_utf16: usize = text[..start_utf8].encode_utf16().count();
            Some(start_utf16..len16)
        } else {
            None
        }
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.ime_marked_byte_count = 0;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
        self.ime_marked_byte_count = 0;
        self.append_to_active_field(text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
        self.ime_marked_byte_count = 0;
        if !new_text.is_empty() {
            self.append_to_active_field(new_text);
            self.ime_marked_byte_count = new_text.len();
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests;
