//! Profile Editor View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P08
//! @requirement REQ-UI-PE

use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, SharedString, Stateful,
};
use std::ops::Range;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::default_api_base_url_for_provider;
use crate::events::types::{ModelProfileAuth, ModelProfileParameters, UserEvent};
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Auth method enum for display
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AuthMethod {
    #[default]
    Keychain,
}

impl AuthMethod {
    #[must_use]
    pub const fn display(&self) -> &'static str {
        match self {
            Self::Keychain => "Keychain",
        }
    }
}

/// API type enum
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ApiType {
    #[default]
    Anthropic,
    OpenAI,
    Custom(String),
}

impl ApiType {
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Anthropic => "Anthropic".to_string(),
            Self::OpenAI => "OpenAI".to_string(),
            Self::Custom(provider) => provider.clone(),
        }
    }

    fn provider_id(&self) -> String {
        match self {
            Self::Anthropic => "anthropic".to_string(),
            Self::OpenAI => "openai".to_string(),
            Self::Custom(provider) => provider.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveField {
    Name,
    Model,
    BaseUrl,
    MaxTokens,
    ContextLimit,
    ThinkingBudget,
    SystemPrompt,
}

/// Profile data for the editor
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, Default)]
pub struct ProfileEditorData {
    pub id: Option<String>,
    pub name: String,
    pub model_id: String,
    pub api_type: ApiType,
    pub base_url: String,
    /// Keychain label for the API key (empty = none selected).
    pub key_label: String,
    /// Available keychain labels populated by `ApiKeysListed`.
    pub available_keys: Vec<String>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub context_limit: u32,
    pub show_thinking: bool,
    pub enable_extended_thinking: bool,
    pub thinking_budget: u32,
    pub system_prompt: String,
}

impl ProfileEditorData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 4096,
            context_limit: 128_000,
            show_thinking: true,
            thinking_budget: 10000,
            system_prompt: crate::models::profile::DEFAULT_SYSTEM_PROMPT.to_string(),
            ..Default::default()
        }
    }

    /// Check if save should be enabled
    #[must_use]
    pub fn can_save(&self) -> bool {
        if self.name.trim().is_empty() {
            return false;
        }
        if self.model_id.trim().is_empty() {
            return false;
        }
        if self.base_url.trim().is_empty() {
            return false;
        }
        !self.key_label.trim().is_empty()
    }
}

/// Profile Editor view state
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Default)]
pub struct ProfileEditorState {
    pub data: ProfileEditorData,
    pub is_new: bool,
    active_field: Option<ActiveField>,
}

impl ProfileEditorState {
    #[must_use]
    pub fn new_profile() -> Self {
        Self {
            data: ProfileEditorData::new(),
            is_new: true,
            active_field: None,
        }
    }

    #[must_use]
    pub const fn edit_profile(data: ProfileEditorData) -> Self {
        Self {
            data,
            is_new: false,
            active_field: None,
        }
    }
}

/// Profile Editor view component
/// @plan PLAN-20250130-GPUIREDUX.P08
pub struct ProfileEditorView {
    state: ProfileEditorState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
    /// Number of bytes inserted by IME marked text (dead key composition).
    /// When composition completes, these bytes are removed before inserting the final text.
    ime_marked_byte_count: usize,
}

impl ProfileEditorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ProfileEditorState::new_profile(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
        self.request_api_key_refresh();
    }

    fn request_api_key_refresh(&self) {
        self.emit(&UserEvent::RefreshApiKeys);
    }

    /// Set profile data from presenter
    pub fn set_profile(&mut self, data: ProfileEditorData, is_new: bool) {
        self.state.data = data;
        self.state.is_new = is_new;
        self.state.active_field = None;
    }

    fn append_to_active_field(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match self.state.active_field {
            Some(ActiveField::Name) => self.state.data.name.push_str(text),
            Some(ActiveField::Model) => self.state.data.model_id.push_str(text),
            Some(ActiveField::BaseUrl) => self.state.data.base_url.push_str(text),
            Some(ActiveField::MaxTokens) => {
                if text.chars().all(|c| c.is_ascii_digit()) {
                    let mut s = self.state.data.max_tokens.to_string();
                    if s == "0" {
                        s.clear();
                    }
                    s.push_str(text);
                    if let Ok(parsed) = s.parse::<u32>() {
                        self.state.data.max_tokens = parsed;
                    }
                }
            }
            Some(ActiveField::ContextLimit) => {
                if text.chars().all(|c| c.is_ascii_digit()) {
                    let mut s = self.state.data.context_limit.to_string();
                    if s == "0" {
                        s.clear();
                    }
                    s.push_str(text);
                    if let Ok(parsed) = s.parse::<u32>() {
                        self.state.data.context_limit = parsed;
                    }
                }
            }
            Some(ActiveField::ThinkingBudget) => {
                if text.chars().all(|c| c.is_ascii_digit()) {
                    let mut s = self.state.data.thinking_budget.to_string();
                    if s == "0" {
                        s.clear();
                    }
                    s.push_str(text);
                    if let Ok(parsed) = s.parse::<u32>() {
                        self.state.data.thinking_budget = parsed;
                    }
                }
            }
            Some(ActiveField::SystemPrompt) => {
                self.state.data.system_prompt.push_str(text);
            }
            None => {}
        }
    }

    fn backspace_active_field(&mut self) {
        match self.state.active_field {
            Some(ActiveField::Name) => {
                self.state.data.name.pop();
            }
            Some(ActiveField::Model) => {
                self.state.data.model_id.pop();
            }
            Some(ActiveField::BaseUrl) => {
                self.state.data.base_url.pop();
            }
            Some(ActiveField::MaxTokens) => {
                let mut s = self.state.data.max_tokens.to_string();
                s.pop();
                self.state.data.max_tokens = if s.is_empty() {
                    0
                } else {
                    s.parse::<u32>().unwrap_or(self.state.data.max_tokens)
                };
            }
            Some(ActiveField::ContextLimit) => {
                let mut s = self.state.data.context_limit.to_string();
                s.pop();
                self.state.data.context_limit = if s.is_empty() {
                    0
                } else {
                    s.parse::<u32>().unwrap_or(self.state.data.context_limit)
                };
            }
            Some(ActiveField::ThinkingBudget) => {
                let mut s = self.state.data.thinking_budget.to_string();
                s.pop();
                self.state.data.thinking_budget = if s.is_empty() {
                    0
                } else {
                    s.parse::<u32>().unwrap_or(self.state.data.thinking_budget)
                };
            }
            Some(ActiveField::SystemPrompt) => {
                self.state.data.system_prompt.pop();
            }
            None => {}
        }
    }

    /// Cycle to the next editable field on Tab
    fn cycle_active_field(&mut self) {
        let fields = [
            ActiveField::Name,
            ActiveField::Model,
            ActiveField::BaseUrl,
            ActiveField::MaxTokens,
            ActiveField::ContextLimit,
            ActiveField::ThinkingBudget,
            ActiveField::SystemPrompt,
        ];
        let current_idx = self
            .state
            .active_field
            .and_then(|f| fields.iter().position(|&x| x == f));
        let next = current_idx.map_or_else(|| fields[0], |i| fields[(i + 1) % fields.len()]);
        self.state.active_field = Some(next);
    }

    /// Active field text content for `InputHandler`
    fn remove_trailing_bytes_from_active_field(&mut self, byte_count: usize) {
        if byte_count == 0 {
            return;
        }
        match self.state.active_field {
            Some(ActiveField::Name) => {
                let len = self.state.data.name.len();
                self.state
                    .data
                    .name
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::Model) => {
                let len = self.state.data.model_id.len();
                self.state
                    .data
                    .model_id
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::BaseUrl) => {
                let len = self.state.data.base_url.len();
                self.state
                    .data
                    .base_url
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::SystemPrompt) => {
                let len = self.state.data.system_prompt.len();
                self.state
                    .data
                    .system_prompt
                    .truncate(len.saturating_sub(byte_count));
            }
            _ => {}
        }
    }

    fn active_field_text(&self) -> &str {
        match self.state.active_field {
            Some(ActiveField::Name) => &self.state.data.name,
            Some(ActiveField::Model) => &self.state.data.model_id,
            Some(ActiveField::BaseUrl) => &self.state.data.base_url,
            Some(
                ActiveField::MaxTokens | ActiveField::ContextLimit | ActiveField::ThinkingBudget,
            )
            | None => "",
            Some(ActiveField::SystemPrompt) => &self.state.data.system_prompt,
        }
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    fn emit_save_profile(&self) {
        let id = self
            .state
            .data
            .id
            .as_deref()
            .and_then(|raw| Uuid::parse_str(raw).ok())
            .unwrap_or_else(Uuid::new_v4);

        let provider_id = Some(self.state.data.api_type.provider_id());

        let auth = Some(ModelProfileAuth::Keychain {
            label: self.state.data.key_label.clone(),
        });

        let parameters = Some(ModelProfileParameters {
            temperature: Some(f64::from(self.state.data.temperature)),
            max_tokens: Some(self.state.data.max_tokens),
            show_thinking: Some(self.state.data.show_thinking),
            enable_thinking: Some(self.state.data.enable_extended_thinking),
            thinking_budget: if self.state.data.enable_extended_thinking {
                Some(self.state.data.thinking_budget)
            } else {
                None
            },
        });

        self.emit(&UserEvent::SaveProfile {
            profile: crate::events::types::ModelProfile {
                id,
                name: self.state.data.name.clone(),
                provider_id,
                model_id: Some(self.state.data.model_id.clone()),
                base_url: Some(self.state.data.base_url.clone()),
                auth,
                parameters,
                system_prompt: Some(self.state.data.system_prompt.clone()),
            },
        });
    }

    /// Handle `ViewCommand` from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P08
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-002
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::ModelSelected {
                provider_id,
                model_id,
                provider_api_url,
                context_length,
            } => {
                // Prefill profile editor from model selection flow.
                self.state.is_new = true;
                self.state.data.model_id.clone_from(&model_id);
                self.state.data.api_type = match provider_id.as_str() {
                    "anthropic" => ApiType::Anthropic,
                    "openai" => ApiType::OpenAI,
                    _ => ApiType::Custom(provider_id.clone()),
                };
                if self.state.data.name.trim().is_empty() {
                    self.state.data.name = model_id;
                }
                if self.state.data.base_url.trim().is_empty() {
                    self.state.data.base_url = provider_api_url
                        .filter(|url| !url.trim().is_empty())
                        .unwrap_or_else(|| default_api_base_url_for_provider(&provider_id));
                }
                if let Some(limit) = context_length {
                    self.state.data.context_limit = limit;
                }
                self.state.active_field = None;
            }
            ViewCommand::ProfileEditorLoad {
                id,
                name,
                provider_id,
                model_id,
                base_url,
                api_key_label,
                temperature,
                max_tokens,
                context_limit,
                show_thinking,
                enable_thinking,
                thinking_budget,
                system_prompt,
            } => {
                self.state.is_new = false;
                self.state.data.id = Some(id.to_string());
                self.state.data.name = name;
                self.state.data.model_id = model_id;
                self.state.data.base_url = base_url;
                self.state.data.api_type = match provider_id.as_str() {
                    "anthropic" => ApiType::Anthropic,
                    "openai" => ApiType::OpenAI,
                    _ => ApiType::Custom(provider_id.clone()),
                };
                self.state.data.key_label = api_key_label;
                #[allow(clippy::cast_possible_truncation)]
                {
                    self.state.data.temperature = temperature as f32;
                }
                self.state.data.max_tokens = max_tokens;
                if let Some(limit) = context_limit {
                    self.state.data.context_limit = limit;
                }
                self.state.data.show_thinking = show_thinking;
                self.state.data.enable_extended_thinking = enable_thinking;
                self.state.data.thinking_budget = thinking_budget.unwrap_or(10_000);
                self.state.data.system_prompt = system_prompt;
                self.state.active_field = None;
            }

            ViewCommand::ApiKeysListed { keys } => {
                self.state.data.available_keys = keys.iter().map(|k| k.label.clone()).collect();
            }

            _ => {}
        }
        cx.notify();
    }

    /// Render the top bar with cancel, title, and save
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let can_save = self.state.data.can_save();
        let title = if self.state.is_new {
            "New Profile"
        } else {
            "Edit Profile"
        };

        div()
            .id("profile-editor-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            // Left: Cancel button - uses navigation_channel
            .child(
                div()
                    .id("btn-cancel")
                    .w(px(70.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(12.0))
                    .text_color(Theme::text_secondary())
                    .child("Cancel")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("Cancel clicked - navigating to Settings");
                            crate::ui_gpui::navigation_channel().request_navigate(
                                crate::presentation::view_command::ViewId::Settings,
                            );
                        }),
                    ),
            )
            // Center: Title
            .child(
                div().flex_1().flex().justify_center().child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::text_primary())
                        .child(title),
                ),
            )
            // Right: Save button
            .child(
                div()
                    .id("btn-save")
                    .w(px(60.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .flex()
                    .justify_center()
                    .text_size(px(12.0))
                    .when(can_save, |d| {
                        d.cursor_pointer()
                            .bg(Theme::accent())
                            .hover(|s| s.bg(Theme::accent_hover()))
                            .text_color(gpui::white())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    tracing::info!("Save clicked - emitting SaveProfile payload");
                                    this.emit_save_profile();
                                }),
                            )
                    })
                    .when(!can_save, |d| {
                        d.bg(Theme::bg_dark()).text_color(Theme::text_muted())
                    })
                    .child("Save"),
            )
    }

    /// Render a field label
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_label(text: &str) -> impl IntoElement {
        div()
            .text_size(px(11.0))
            .text_color(Theme::text_secondary())
            .mb(px(4.0))
            .child(text.to_string())
    }

    /// Render a text input field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_text_field(
        id: &str,
        value: &str,
        placeholder: &str,
        active: bool,
    ) -> Stateful<gpui::Div> {
        div()
            .id(SharedString::from(id.to_string()))
            .w(px(360.0))
            .h(px(24.0))
            .px(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(if active {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .text_size(px(12.0))
            .child(if value.is_empty() {
                div()
                    .text_color(Theme::text_muted())
                    .child(placeholder.to_string())
            } else {
                div()
                    .text_color(Theme::text_primary())
                    .child(value.to_string())
            })
            .when(active, |d| {
                d.child(
                    div()
                        .ml(px(2.0))
                        .text_color(Theme::text_primary())
                        .child("|"),
                )
            })
    }

    /// Render the name field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_name_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::Name);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("NAME"))
            .child(
                Self::render_text_field(
                    "field-name",
                    &self.state.data.name,
                    "Profile name",
                    active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::Name);
                        cx.notify();
                    }),
                ),
            )
    }

    /// Render the model field (editable) with browse button
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_model_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::Model);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("MODEL"))
            .child(
                div()
                    .w(px(360.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        Self::render_text_field(
                            "field-model-id",
                            &self.state.data.model_id,
                            "e.g. claude-sonnet-4-20250514",
                            active,
                        )
                        .flex_1()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, cx| {
                                this.state.active_field = Some(ActiveField::Model);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        div()
                            .id("btn-browse-model")
                            .w(px(60.0))
                            .h(px(24.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_darker()))
                            .text_size(px(11.0))
                            .text_color(Theme::text_secondary())
                            .child("Browse")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    tracing::info!(
                                        "Browse model clicked - navigating to ModelSelector"
                                    );
                                    let available_keys = this.state.data.available_keys.clone();
                                    this.state = ProfileEditorState::new_profile();
                                    this.state.data.available_keys = available_keys;
                                    this.request_api_key_refresh();
                                    crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::ModelSelector,
                                    );
                                }),
                            ),
                    ),
            )
    }

    /// Render API type dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_api_type_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let api_type = self.state.data.api_type.display();

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("API TYPE"))
            .child(
                div()
                    .id("dropdown-api-type")
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.data.api_type = match this.state.data.api_type {
                                ApiType::Anthropic => ApiType::OpenAI,
                                ApiType::OpenAI | ApiType::Custom(_) => ApiType::Anthropic,
                            };

                            if this.state.data.base_url.trim().is_empty() {
                                this.state.data.base_url = default_api_base_url_for_provider(
                                    &this.state.data.api_type.provider_id(),
                                );
                            }

                            cx.notify();
                        }),
                    )
                    .child(api_type)
                    .child(div().text_color(Theme::text_muted()).child("v")),
            )
    }

    /// Render base URL field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_base_url_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::BaseUrl);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("BASE URL"))
            .child(
                Self::render_text_field(
                    "field-base-url",
                    &self.state.data.base_url,
                    "https://api.example.com/v1",
                    active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::BaseUrl);
                        cx.notify();
                    }),
                ),
            )
    }

    /// Render auth method dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P08
    /// Render API key label dropdown and "Manage Keys" button.
    fn render_key_label_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let current_label = if self.state.data.key_label.is_empty() {
            "Select API Key…".to_string()
        } else {
            self.state.data.key_label.clone()
        };
        let _available = self.state.data.available_keys.clone();

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("API KEY"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Dropdown cycling through available keys
                    .child(
                        div()
                            .id("dropdown-key-label")
                            .flex_1()
                            .h(px(24.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .text_size(px(12.0))
                            .text_color(if self.state.data.key_label.is_empty() {
                                Theme::text_muted()
                            } else {
                                Theme::text_primary()
                            })
                            .overflow_hidden()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    if this.state.data.available_keys.is_empty() {
                                        this.request_api_key_refresh();
                                        cx.notify();
                                        return;
                                    }
                                    let current_idx = this
                                        .state
                                        .data
                                        .available_keys
                                        .iter()
                                        .position(|k| k == &this.state.data.key_label)
                                        .map_or(0, |i| i + 1);
                                    let next_idx =
                                        current_idx % this.state.data.available_keys.len();
                                    this.state.data.key_label =
                                        this.state.data.available_keys[next_idx].clone();
                                    cx.notify();
                                }),
                            )
                            .child(current_label)
                            .child(div().text_color(Theme::text_muted()).child("▾")),
                    )
                    // "Manage Keys" button
                    .child(
                        div()
                            .id("btn-manage-keys")
                            .h(px(24.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_darker()))
                            .text_size(px(11.0))
                            .text_color(Theme::text_secondary())
                            .child("Manage Keys")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, _cx| {
                                    crate::ui_gpui::navigation_channel().request_navigate(
                                        crate::presentation::view_command::ViewId::ApiKeyManager,
                                    );
                                }),
                            ),
                    ),
            )
    }

    /// Render section divider
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_section_divider(title: &str) -> impl IntoElement {
        div()
            .w(px(360.0))
            .flex()
            .flex_col()
            .mt(px(8.0))
            .child(div().h(px(1.0)).w_full().bg(Theme::border()))
            .child(
                div()
                    .mt(px(8.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_secondary())
                    .child(title.to_string()),
            )
    }

    /// Render temperature field with stepper
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_temperature_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let temp = format!("{:.1}", self.state.data.temperature);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("TEMPERATURE"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    // Number field
                    .child(
                        div()
                            .w(px(80.0))
                            .h(px(24.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .text_size(px(12.0))
                            .text_color(Theme::text_primary())
                            .child(temp),
                    )
                    // Stepper
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("stepper-temp-up")
                                    .w(px(20.0))
                                    .h(px(12.0))
                                    .bg(Theme::bg_dark())
                                    .border_1()
                                    .border_color(Theme::border())
                                    .rounded_t(px(2.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_darker()))
                                    .text_size(px(8.0))
                                    .text_color(Theme::text_secondary())
                                    .child("▲")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            this.state.data.temperature =
                                                (this.state.data.temperature + 0.1).min(2.0);
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("stepper-temp-down")
                                    .w(px(20.0))
                                    .h(px(12.0))
                                    .bg(Theme::bg_dark())
                                    .border_1()
                                    .border_color(Theme::border())
                                    .rounded_b(px(2.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_darker()))
                                    .text_size(px(8.0))
                                    .text_color(Theme::text_secondary())
                                    .child("▼")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            this.state.data.temperature =
                                                (this.state.data.temperature - 0.1).max(0.0);
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
    }

    /// Render max tokens field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_max_tokens_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::MaxTokens);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("MAX TOKENS"))
            .child(
                Self::render_text_field(
                    "field-max-tokens",
                    &self.state.data.max_tokens.to_string(),
                    "4096",
                    active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::MaxTokens);
                        cx.notify();
                    }),
                ),
            )
    }

    /// Render context limit field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_context_limit_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::ContextLimit);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("CONTEXT LIMIT"))
            .child(
                Self::render_text_field(
                    "field-context-limit",
                    &self.state.data.context_limit.to_string(),
                    "128000",
                    active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::ContextLimit);
                        cx.notify();
                    }),
                ),
            )
    }

    /// Render show thinking checkbox
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_show_thinking_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let checked = self.state.data.show_thinking;

        div()
            .id("checkbox-show-thinking")
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.data.show_thinking = !this.state.data.show_thinking;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .size(px(14.0))
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(2.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(checked, |d| {
                        d.bg(Theme::accent()).child(
                            div()
                                .text_size(px(10.0))
                                .text_color(gpui::white())
                                .child("v"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child("Show Thinking"),
            )
    }

    /// Render extended thinking checkbox
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_extended_thinking_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let checked = self.state.data.enable_extended_thinking;
        let budget_active = self.state.active_field == Some(ActiveField::ThinkingBudget);

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .id("checkbox-extended-thinking")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.data.enable_extended_thinking =
                                !this.state.data.enable_extended_thinking;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .size(px(14.0))
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(2.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(checked, |d| {
                                d.bg(Theme::accent()).child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(gpui::white())
                                        .child("v"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(Theme::text_primary())
                            .child("Enable Extended Thinking"),
                    ),
            )
            .when(checked, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(Self::render_label("THINKING BUDGET"))
                        .child(
                            Self::render_text_field(
                                "field-thinking-budget",
                                &self.state.data.thinking_budget.to_string(),
                                "10000",
                                budget_active,
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.active_field = Some(ActiveField::ThinkingBudget);
                                    cx.notify();
                                }),
                            ),
                        ),
                )
            })
    }

    /// Render system prompt section
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_system_prompt_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::SystemPrompt);

        div()
            .flex()
            .flex_col()
            .child(Self::render_section_divider("SYSTEM PROMPT"))
            .child(
                div()
                    .id("field-system-prompt")
                    .mt(px(8.0))
                    .w(px(360.0))
                    .h(px(100.0))
                    .px(px(8.0))
                    .py(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(if active {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.active_field = Some(ActiveField::SystemPrompt);
                            cx.notify();
                        }),
                    )
                    .child(if self.state.data.system_prompt.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .child("You are a helpful assistant.")
                    } else {
                        div().child(self.state.data.system_prompt.clone())
                    }),
            )
    }

    /// Render the content area
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_content(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("profile-editor-content")
            .flex_1()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_y_scroll()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Name
            .child(self.render_name_section(cx))
            // Model
            .child(self.render_model_section(cx))
            // API Type
            .child(self.render_api_type_section(cx))
            // Base URL
            .child(self.render_base_url_section(cx))
            // API Key (keychain label dropdown + manage button)
            .child(self.render_key_label_section(cx))
            // Parameters section
            .child(Self::render_section_divider("PARAMETERS"))
            .child(
                div()
                    .mt(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(self.render_temperature_section(cx))
                    .child(self.render_max_tokens_section(cx))
                    .child(self.render_context_limit_section(cx))
                    .child(self.render_show_thinking_section(cx))
                    .child(self.render_extended_thinking_section(cx)),
            )
            // System Prompt
            .child(self.render_system_prompt_section(cx))
    }
}

impl gpui::Focusable for ProfileEditorView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::EntityInputHandler for ProfileEditorView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
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
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        let text = self.active_field_text();
        let len16 = text.encode_utf16().count();
        Some(gpui::UTF16Selection {
            range: len16..len16,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime_marked_byte_count > 0 {
            let text = self.active_field_text();
            let len16: usize = text.encode_utf16().count();
            let marked_bytes = self.ime_marked_byte_count;
            let marked_start_utf8 = text.len().saturating_sub(marked_bytes);
            let marked_start_utf16: usize = text[..marked_start_utf8].encode_utf16().count();
            Some(marked_start_utf16..len16)
        } else {
            None
        }
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) {
        self.ime_marked_byte_count = 0;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        // Remove any pending IME marked text before inserting the composed result
        if self.ime_marked_byte_count > 0 {
            self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
            self.ime_marked_byte_count = 0;
        }
        if !text.is_empty() {
            self.append_to_active_field(text);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        // Remove previously marked text before inserting updated composition
        if self.ime_marked_byte_count > 0 {
            self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
            self.ime_marked_byte_count = 0;
        }
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
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl gpui::Render for ProfileEditorView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("profile-editor-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .track_focus(&self.focus_handle)
            // Invisible canvas to register InputHandler for IME/diacritics
            .child(
                canvas(
                    |bounds, _window: &mut gpui::Window, _cx: &mut gpui::App| bounds,
                    {
                        let entity = cx.entity();
                        let focus = self.focus_handle.clone();
                        move |bounds: Bounds<Pixels>,
                              _,
                              window: &mut gpui::Window,
                              cx: &mut gpui::App| {
                            window.handle_input(
                                &focus,
                                ElementInputHandler::new(bounds, entity),
                                cx,
                            );
                        }
                    },
                )
                .size_0(),
            )
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;

                    if key == "escape" || (modifiers.platform && key == "w") {
                        crate::ui_gpui::navigation_channel()
                            .request_navigate(crate::presentation::view_command::ViewId::Settings);
                        return;
                    }

                    if modifiers.platform && key == "s" {
                        this.emit_save_profile();
                        return;
                    }

                    if modifiers.platform && key == "v" {
                        if let Some(item) = cx.read_from_clipboard() {
                            if let Some(text) = item.text() {
                                this.append_to_active_field(&text);
                                cx.notify();
                            }
                        }
                        return;
                    }

                    if modifiers.platform || modifiers.control {
                        return;
                    }

                    if key == "backspace" {
                        this.backspace_active_field();
                        cx.notify();
                        return;
                    }

                    if key == "enter" {
                        if this.state.active_field == Some(ActiveField::SystemPrompt) {
                            this.append_to_active_field(
                                "
",
                            );
                            cx.notify();
                        }
                        return;
                    }

                    if key == "tab" {
                        this.cycle_active_field();
                        cx.notify();
                    }

                    // All other keys (printable chars) fall through to EntityInputHandler
                }),
            )
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content (scrollable)
            .child(self.render_content(cx))
    }
}
