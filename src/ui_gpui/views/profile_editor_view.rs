//! Profile Editor View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P08
//! @requirement REQ-UI-PE

use gpui::{div, px, prelude::*, SharedString, MouseButton, FocusHandle, FontWeight};
use std::sync::Arc;

use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::events::types::{UserEvent, ViewId};
use crate::presentation::view_command::ViewCommand;

/// Auth method enum for display
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, PartialEq, Default)]
pub enum AuthMethod {
    None,
    #[default]
    ApiKey,
    Keyfile,
}

impl AuthMethod {
    pub fn display(&self) -> &'static str {
        match self {
            AuthMethod::None => "None",
            AuthMethod::ApiKey => "API Key",
            AuthMethod::Keyfile => "Key File",
        }
    }
}

/// API type enum
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ApiType {
    #[default]
    Anthropic,
    OpenAI,
}

impl ApiType {
    pub fn display(&self) -> &'static str {
        match self {
            ApiType::Anthropic => "Anthropic",
            ApiType::OpenAI => "OpenAI",
        }
    }
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
    pub auth_method: AuthMethod,
    pub api_key: String,
    pub keyfile_path: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub context_limit: u32,
    pub show_thinking: bool,
    pub enable_extended_thinking: bool,
    pub thinking_budget: u32,
    pub system_prompt: String,
}

impl ProfileEditorData {
    pub fn new() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 4096,
            context_limit: 128000,
            show_thinking: true,
            thinking_budget: 10000,
            system_prompt: "You are a helpful assistant.".to_string(),
            ..Default::default()
        }
    }

    /// Check if save should be enabled
    pub fn can_save(&self) -> bool {
        if self.name.trim().is_empty() {
            return false;
        }
        if self.base_url.trim().is_empty() {
            return false;
        }
        match self.auth_method {
            AuthMethod::None => true,
            AuthMethod::ApiKey => !self.api_key.trim().is_empty(),
            AuthMethod::Keyfile => !self.keyfile_path.trim().is_empty(),
        }
    }
}

/// Profile Editor view state
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Default)]
pub struct ProfileEditorState {
    pub data: ProfileEditorData,
    pub is_new: bool,
    pub mask_api_key: bool,
}

impl ProfileEditorState {
    pub fn new_profile() -> Self {
        Self {
            data: ProfileEditorData::new(),
            is_new: true,
            mask_api_key: true,
        }
    }

    pub fn edit_profile(data: ProfileEditorData) -> Self {
        Self {
            data,
            is_new: false,
            mask_api_key: true,
        }
    }
}

/// Profile Editor view component
/// @plan PLAN-20250130-GPUIREDUX.P08
pub struct ProfileEditorView {
    state: ProfileEditorState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
}

impl ProfileEditorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ProfileEditorState::new_profile(),
            bridge: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set profile data from presenter
    pub fn set_profile(&mut self, data: ProfileEditorData, is_new: bool) {
        self.state.data = data;
        self.state.is_new = is_new;
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P08
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
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack => {
                // Navigation handled by MainPanel
            }
            _ => {}
        }
        cx.notify();
    }

    /// Render the top bar with cancel, title, and save
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let can_save = self.state.data.can_save();
        let title = if self.state.is_new { "New Profile" } else { "Edit Profile" };

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
                    .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                        tracing::info!("Cancel clicked - navigating to Settings");
                        crate::ui_gpui::navigation_channel().request_navigate(
                            crate::presentation::view_command::ViewId::Settings
                        );
                    }))
            )
            // Center: Title
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child(title)
                    )
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
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                                tracing::info!("Save clicked - navigating to Settings");
                                this.emit(UserEvent::SaveProfileEditor);
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::Settings
                                );
                            }))
                    })
                    .when(!can_save, |d| {
                        d.bg(Theme::bg_dark())
                            .text_color(Theme::text_muted())
                    })
                    .child("Save")
            )
    }

    /// Render a field label
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_label(&self, text: &str) -> impl IntoElement {
        div()
            .text_size(px(11.0))
            .text_color(Theme::text_secondary())
            .mb(px(4.0))
            .child(text.to_string())
    }

    /// Render a text input field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_text_field(&self, id: &str, value: &str, placeholder: &str) -> impl IntoElement {
        div()
            .id(SharedString::from(id.to_string()))
            .w(px(360.0))
            .h(px(24.0))
            .px(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .text_size(px(12.0))
            .child(
                if value.is_empty() {
                    div().text_color(Theme::text_muted()).child(placeholder.to_string())
                } else {
                    div().text_color(Theme::text_primary()).child(value.to_string())
                }
            )
    }

    /// Render a secure (masked) text field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_secure_field(&self, id: &str, value: &str, masked: bool) -> impl IntoElement {
        let display = if masked && !value.is_empty() {
            "•".repeat(value.len().min(40))
        } else {
            value.to_string()
        };

        div()
            .id(SharedString::from(id.to_string()))
            .w(px(360.0))
            .h(px(24.0))
            .px(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .text_size(px(12.0))
            .child(
                if display.is_empty() {
                    div().text_color(Theme::text_muted()).child("sk-...")
                } else {
                    div().text_color(Theme::text_primary()).child(display)
                }
            )
    }

    /// Render the name field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_name_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("NAME"))
            .child(self.render_text_field("field-name", &self.state.data.name, "Profile name"))
    }

    /// Render the model display with change button
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_model_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let model_id = self.state.data.model_id.clone();

        div()
            .flex()
            .flex_col()
            .child(self.render_label("MODEL"))
            .child(
                div()
                    .w(px(360.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Model ID display
                    .child(
                        div()
                            .flex_1()
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
                            .overflow_hidden()
                            .child(if model_id.is_empty() { "Select a model".to_string() } else { model_id })
                    )
                    // Change button
                    .child(
                        div()
                            .id("btn-change-model")
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
                            .child("Change")
                            .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                                tracing::info!("Change model clicked - navigating to ModelSelector");
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::ModelSelector
                                );
                            }))
                    )
            )
    }

    /// Render API type dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_api_type_section(&self) -> impl IntoElement {
        let api_type = self.state.data.api_type.display();

        div()
            .flex()
            .flex_col()
            .child(self.render_label("API TYPE"))
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
                    .child(api_type)
                    .child(div().text_color(Theme::text_muted()).child("v"))
            )
    }

    /// Render base URL field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_base_url_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("BASE URL"))
            .child(self.render_text_field("field-base-url", &self.state.data.base_url, "https://api.example.com/v1"))
    }

    /// Render auth method dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_auth_method_section(&self) -> impl IntoElement {
        let auth_method = self.state.data.auth_method.display();

        div()
            .flex()
            .flex_col()
            .child(self.render_label("AUTH METHOD"))
            .child(
                div()
                    .id("dropdown-auth-method")
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
                    .child(auth_method)
                    .child(div().text_color(Theme::text_muted()).child("v"))
            )
    }

    /// Render API key field with mask toggle
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_api_key_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let masked = self.state.mask_api_key;

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w(px(360.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_secondary())
                            .child("API KEY")
                    )
                    .child(
                        div()
                            .id("checkbox-mask")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                this.state.mask_api_key = !this.state.mask_api_key;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(12.0))
                                    .border_1()
                                    .border_color(Theme::border())
                                    .rounded(px(2.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(masked, |d| d.bg(Theme::accent()).child(
                                        div().text_size(px(8.0)).text_color(gpui::white()).child("v")
                                    ))
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(Theme::text_muted())
                                    .child("Mask")
                            )
                    )
            )
            .child(
                div()
                    .mt(px(4.0))
                    .child(self.render_secure_field("field-api-key", &self.state.data.api_key, masked))
            )
    }

    /// Render keyfile field with browse button
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_keyfile_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("KEY FILE"))
            .child(
                div()
                    .w(px(360.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Path field
                    .child(
                        div()
                            .flex_1()
                            .h(px(24.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .text_size(px(12.0))
                            .overflow_hidden()
                            .child(
                                if self.state.data.keyfile_path.is_empty() {
                                    div().text_color(Theme::text_muted()).child("/path/to/api_key")
                                } else {
                                    div().text_color(Theme::text_primary()).child(self.state.data.keyfile_path.clone())
                                }
                            )
                    )
                    // Browse button
                    .child(
                        div()
                            .id("btn-browse")
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
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                                tracing::info!("Browse clicked");
                                this.emit(UserEvent::BrowseKeyfile);
                            }))
                    )
            )
    }

    /// Render section divider
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_section_divider(&self, title: &str) -> impl IntoElement {
        div()
            .w(px(360.0))
            .flex()
            .flex_col()
            .mt(px(8.0))
            .child(
                div()
                    .h(px(1.0))
                    .w_full()
                    .bg(Theme::border())
            )
            .child(
                div()
                    .mt(px(8.0))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_secondary())
                    .child(title.to_string())
            )
    }

    /// Render temperature field with stepper
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_temperature_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let temp = format!("{:.1}", self.state.data.temperature);

        div()
            .flex()
            .flex_col()
            .child(self.render_label("TEMPERATURE"))
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
                            .child(temp)
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
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.data.temperature = (this.state.data.temperature + 0.1).min(2.0);
                                        cx.notify();
                                    }))
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
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.data.temperature = (this.state.data.temperature - 0.1).max(0.0);
                                        cx.notify();
                                    }))
                            )
                    )
            )
    }

    /// Render max tokens field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_max_tokens_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("MAX TOKENS"))
            .child(self.render_text_field("field-max-tokens", &self.state.data.max_tokens.to_string(), "4096"))
    }

    /// Render context limit field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_context_limit_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("CONTEXT LIMIT"))
            .child(self.render_text_field("field-context-limit", &self.state.data.context_limit.to_string(), "128000"))
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
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                this.state.data.show_thinking = !this.state.data.show_thinking;
                cx.notify();
            }))
            .child(
                div()
                    .size(px(14.0))
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(2.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(checked, |d| d.bg(Theme::accent()).child(
                        div().text_size(px(10.0)).text_color(gpui::white()).child("v")
                    ))
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child("Show Thinking")
            )
    }

    /// Render extended thinking checkbox
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_extended_thinking_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let checked = self.state.data.enable_extended_thinking;

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
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        this.state.data.enable_extended_thinking = !this.state.data.enable_extended_thinking;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(14.0))
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(2.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(checked, |d| d.bg(Theme::accent()).child(
                                div().text_size(px(10.0)).text_color(gpui::white()).child("v")
                            ))
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(Theme::text_primary())
                            .child("Enable Extended Thinking")
                    )
            )
            .when(checked, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .child(self.render_label("THINKING BUDGET"))
                        .child(self.render_text_field("field-thinking-budget", &self.state.data.thinking_budget.to_string(), "10000"))
                )
            })
    }

    /// Render system prompt section
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_system_prompt_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_section_divider("SYSTEM PROMPT"))
            .child(
                div()
                    .mt(px(8.0))
                    .w(px(360.0))
                    .h(px(100.0))
                    .px(px(8.0))
                    .py(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .overflow_hidden()
                    .child(
                        if self.state.data.system_prompt.is_empty() {
                            div().text_color(Theme::text_muted()).child("You are a helpful assistant.")
                        } else {
                            div().child(self.state.data.system_prompt.clone())
                        }
                    )
            )
    }

    /// Render the content area
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_content(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let auth_method = &self.state.data.auth_method;

        div()
            .id("profile-editor-content")
            .flex_1()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_hidden()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Name
            .child(self.render_name_section())
            // Model
            .child(self.render_model_section(cx))
            // API Type
            .child(self.render_api_type_section())
            // Base URL
            .child(self.render_base_url_section())
            // Auth Method
            .child(self.render_auth_method_section())
            // Conditional auth fields
            .when(*auth_method == AuthMethod::ApiKey, |d| d.child(self.render_api_key_section(cx)))
            .when(*auth_method == AuthMethod::Keyfile, |d| d.child(self.render_keyfile_section(cx)))
            // Parameters section
            .child(self.render_section_divider("PARAMETERS"))
            .child(
                div()
                    .mt(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(self.render_temperature_section(cx))
                    .child(self.render_max_tokens_section())
                    .child(self.render_context_limit_section())
                    .child(self.render_show_thinking_section(cx))
                    .child(self.render_extended_thinking_section(cx))
            )
            // System Prompt
            .child(self.render_system_prompt_section())
    }
}

impl gpui::Focusable for ProfileEditorView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for ProfileEditorView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("profile-editor-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;
                
                // Escape or Cmd+W: Go back to Settings
                if key == "escape" || (modifiers.platform && key == "w") {
                    println!(">>> Escape/Cmd+W pressed - navigating to Settings <<<");
                    crate::ui_gpui::navigation_channel().request_navigate(
                        crate::presentation::view_command::ViewId::Settings
                    );
                }
                // Cmd+S: Save profile
                if modifiers.platform && key == "s" {
                    println!(">>> Cmd+S pressed - saving profile <<<");
                    this.emit(crate::events::types::UserEvent::SaveProfileEditor);
                    crate::ui_gpui::navigation_channel().request_navigate(
                        crate::presentation::view_command::ViewId::Settings
                    );
                }
            }))
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content (scrollable)
            .child(self.render_content(cx))
    }
}
