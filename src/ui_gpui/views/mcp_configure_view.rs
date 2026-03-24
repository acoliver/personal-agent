//! MCP Configure View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P10
//! @requirement REQ-UI-MC

use gpui::{div, prelude::*, px, FocusHandle, FontWeight, MouseButton, SharedString};
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Auth method for MCP configuration
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum McpAuthMethod {
    None,
    #[default]
    ApiKey,
    Keyfile,
    OAuth,
}

impl McpAuthMethod {
    #[must_use]
    pub const fn display(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::ApiKey => "API Key",
            Self::Keyfile => "Key File",
            Self::OAuth => "OAuth",
        }
    }
}

/// OAuth connection status
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum OAuthStatus {
    #[default]
    NotConnected,
    Connecting,
    Connected {
        username: String,
    },
    Error(String),
}

/// Configuration field types
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Debug)]
pub enum ConfigField {
    String {
        key: String,
        value: String,
        placeholder: String,
    },
    Boolean {
        key: String,
        value: bool,
    },
    Array {
        key: String,
        values: Vec<String>,
    },
}

/// MCP Configure view data
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone)]
pub struct McpConfigureData {
    pub id: Option<String>,
    pub name: String,
    pub package: String,
    pub package_type: crate::mcp::McpPackageType,
    pub runtime_hint: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<Vec<(String, String)>>,
    pub auth_method: McpAuthMethod,
    pub env_var_name: String,
    pub api_key: String,
    pub keyfile_path: String,
    pub oauth_provider: String,
    pub oauth_status: OAuthStatus,
    pub config_fields: Vec<ConfigField>,
    /// Remote URL for HTTP/SSE transport MCPs (None for stdio-only).
    pub url: Option<String>,
}

impl Default for McpConfigureData {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            package: String::new(),
            package_type: crate::mcp::McpPackageType::Npm,
            runtime_hint: Some("npx".to_string()),
            command: String::new(),
            args: vec![],
            env: None,
            auth_method: McpAuthMethod::default(),
            env_var_name: String::new(),
            api_key: String::new(),
            keyfile_path: String::new(),
            oauth_provider: String::new(),
            oauth_status: OAuthStatus::default(),
            config_fields: vec![],
            url: None,
        }
    }
}

impl McpConfigureData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            env_var_name: "API_KEY".to_string(),
            package_type: crate::mcp::McpPackageType::Npm,
            runtime_hint: Some("npx".to_string()),
            command: String::new(),
            args: vec![],
            env: None,
            ..Default::default()
        }
    }

    /// Check if save should be enabled
    #[must_use]
    pub fn can_save(&self) -> bool {
        if self.name.trim().is_empty() {
            return false;
        }

        // Need either a command (stdio) or a URL (remote HTTP/SSE).
        let has_command = !self.command.trim().is_empty();
        let has_url = self.url.as_ref().is_some_and(|u| !u.trim().is_empty());
        if !has_command && !has_url {
            return false;
        }

        match self.auth_method {
            McpAuthMethod::None => true,
            McpAuthMethod::ApiKey => !self.api_key.trim().is_empty(),
            McpAuthMethod::Keyfile => !self.keyfile_path.trim().is_empty(),
            McpAuthMethod::OAuth => matches!(self.oauth_status, OAuthStatus::Connected { .. }),
        }
    }
}

/// MCP Configure view state
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Default)]
pub struct McpConfigureState {
    pub data: McpConfigureData,
    pub is_new: bool,
    pub mask_api_key: bool,
}

impl McpConfigureState {
    #[must_use]
    pub fn new_mcp() -> Self {
        Self {
            data: McpConfigureData::new(),
            is_new: true,
            mask_api_key: true,
        }
    }

    #[must_use]
    pub const fn edit_mcp(data: McpConfigureData) -> Self {
        Self {
            data,
            is_new: false,
            mask_api_key: true,
        }
    }
}

/// MCP Configure view component
/// @plan PLAN-20250130-GPUIREDUX.P10
pub struct McpConfigureView {
    state: McpConfigureState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
}

impl McpConfigureView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: McpConfigureState::new_mcp(),
            bridge: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P10
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set MCP data from presenter
    pub fn set_mcp(&mut self, data: McpConfigureData, is_new: bool) {
        self.state.data = data;
        self.state.is_new = is_new;
    }

    fn navigate_to_settings(&self) {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::Settings);
    }

    fn save_current(&self) {
        self.emit_save_mcp_config();
    }

    fn toggle_mask_api_key(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.mask_api_key = !self.state.mask_api_key;
        cx.notify();
    }

    fn start_oauth(&self) {
        let parsed_id = self
            .state
            .data
            .id
            .as_ref()
            .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
            .unwrap_or_else(uuid::Uuid::nil);
        self.emit(&UserEvent::StartMcpOAuth {
            id: parsed_id,
            provider: self.state.data.oauth_provider.clone(),
        });
    }

    fn handle_key_down(&self, event: &gpui::KeyDownEvent) {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if key == "escape" || (modifiers.platform && key == "w") {
            self.navigate_to_settings();
        }
        if modifiers.platform && key == "s" {
            self.save_current();
        }
    }

    fn emit_save_mcp_config(&self) {
        let id = self
            .state
            .data
            .id
            .clone()
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .unwrap_or_else(uuid::Uuid::new_v4);

        let d = &self.state.data;
        let has_url = d.url.as_ref().is_some_and(|u| !u.trim().is_empty());
        let package_type = if has_url {
            crate::mcp::McpPackageType::Http
        } else {
            d.package_type.clone()
        };

        let transport = match package_type {
            crate::mcp::McpPackageType::Http => crate::mcp::McpTransport::Http,
            crate::mcp::McpPackageType::Npm | crate::mcp::McpPackageType::Docker => {
                crate::mcp::McpTransport::Stdio
            }
        };

        let source_url = match package_type {
            crate::mcp::McpPackageType::Http => d.url.clone().unwrap_or_default(),
            crate::mcp::McpPackageType::Docker => format!("docker run {}", d.package),
            crate::mcp::McpPackageType::Npm => {
                let runtime = d.runtime_hint.as_deref().unwrap_or("npx");
                format!("{runtime} {}", d.package)
            }
        };

        let source = crate::mcp::McpSource::Manual { url: source_url };

        let package = crate::mcp::McpPackage {
            package_type: package_type.clone(),
            identifier: d.package.clone(),
            runtime_hint: match package_type {
                crate::mcp::McpPackageType::Npm => d.runtime_hint.clone(),
                crate::mcp::McpPackageType::Docker => Some("docker".to_string()),
                crate::mcp::McpPackageType::Http => None,
            },
        };

        let config = crate::mcp::McpConfig {
            id,
            name: d.name.clone(),
            enabled: true,
            source,
            package,
            transport,
            auth_type: crate::mcp::McpAuthType::None,
            env_vars: d.env.as_ref().map_or_else(Vec::new, |pairs| {
                pairs
                    .iter()
                    .map(|(k, _)| crate::mcp::EnvVarConfig {
                        name: k.clone(),
                        required: true,
                    })
                    .collect()
            }),
            package_args: vec![],
            keyfile_path: None,
            config: serde_json::Value::Null,
            oauth_token: None,
        };

        self.emit(&UserEvent::SaveMcpConfig {
            id,
            config: Box::new(config),
        });
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P10
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
    /// @plan PLAN-20250130-GPUIREDUX.P10
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::McpConfigureDraftLoaded {
                id,
                name,
                package,
                package_type,
                runtime_hint,
                env_var_name,
                command,
                args,
                env,
                url,
            } => {
                // Infer auth method from env vars: if no env vars exist the
                // server needs no credentials (e.g. Exa remote HTTP).
                let has_env = env
                    .as_ref()
                    .is_some_and(|v| v.iter().any(|(k, _)| !k.is_empty()));
                self.state.data.auth_method = if has_env {
                    McpAuthMethod::ApiKey
                } else {
                    McpAuthMethod::None
                };

                self.state.data.id = Some(id);
                self.state.data.name = name;
                self.state.data.package = package;
                self.state.data.package_type = package_type;
                self.state.data.runtime_hint = runtime_hint;
                self.state.data.env_var_name = env_var_name;
                self.state.data.command = command;
                self.state.data.args = args;
                self.state.data.env = env;
                self.state.data.url = url;
                self.state.is_new = self
                    .state
                    .data
                    .id
                    .as_ref()
                    .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
                    .is_none_or(|parsed| parsed.is_nil());
            }
            ViewCommand::ShowNotification { message } => {
                self.state.data.oauth_status = OAuthStatus::Connected { username: message };
            }
            ViewCommand::ShowError { message, .. } => {
                self.state.data.oauth_status = OAuthStatus::Error(message);
            }
            ViewCommand::McpConfigSaved { id, name } => {
                self.state.data.id = Some(id.to_string());
                if let Some(saved_name) = name {
                    self.state.data.name = saved_name;
                }
                self.state.is_new = id.is_nil();
                self.state.data.oauth_status = OAuthStatus::Connected {
                    username: "Saved".to_string(),
                };
            }
            _ => {}
        }
        cx.notify();
    }

    /// Render the top bar with cancel, title, and save
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let can_save = self.state.data.can_save();
        let title = if self.state.is_new {
            "Configure MCP"
        } else {
            "Edit MCP"
        };

        div()
            .id("mcp-configure-top-bar")
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
                        cx.listener(|this, _, _window, _cx| {
                            tracing::info!("Cancel clicked - navigating to Settings");
                            this.navigate_to_settings();
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
                                    tracing::info!("Save clicked - emitting SaveMcpConfig");
                                    this.save_current();
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
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_label(text: &str) -> impl IntoElement {
        div()
            .text_size(px(11.0))
            .text_color(Theme::text_secondary())
            .mb(px(4.0))
            .child(text.to_string())
    }

    /// Render the name field
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_name_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("NAME"))
            .child(
                div()
                    .id("field-name")
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
                    .child(if self.state.data.name.is_empty() {
                        div().text_color(Theme::text_muted()).child("MCP name")
                    } else {
                        div()
                            .text_color(Theme::text_primary())
                            .child(self.state.data.name.clone())
                    }),
            )
    }

    /// Render the package field (read-only)
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_package_section(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("PACKAGE"))
            .child(
                div()
                    .id("field-package")
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_darkest())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(Theme::text_secondary())
                    .overflow_hidden()
                    .child(if self.state.data.package.is_empty() {
                        "npx @scope/package".to_string()
                    } else {
                        self.state.data.package.clone()
                    }),
            )
    }

    /// Render section divider
    /// @plan PLAN-20250130-GPUIREDUX.P10
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

    /// Render auth method dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_auth_method_section(&self) -> impl IntoElement {
        let auth_method = self.state.data.auth_method.display();

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("AUTH METHOD"))
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
                    .child(div().text_color(Theme::text_muted()).child("v")),
            )
    }

    /// Render API key field with mask toggle
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_api_key_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let masked = self.state.mask_api_key;
        let env_var_name = self.state.data.env_var_name.clone();
        let display = if masked && !self.state.data.api_key.is_empty() {
            "•".repeat(self.state.data.api_key.len().min(40))
        } else {
            self.state.data.api_key.clone()
        };

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
                            .child(env_var_name),
                    )
                    .child(
                        div()
                            .id("checkbox-mask")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.toggle_mask_api_key(cx);
                                }),
                            )
                            .child(
                                div()
                                    .size(px(12.0))
                                    .border_1()
                                    .border_color(Theme::border())
                                    .rounded(px(2.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(masked, |d| {
                                        d.bg(Theme::accent()).child(
                                            div()
                                                .text_size(px(8.0))
                                                .text_color(gpui::white())
                                                .child("v"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(Theme::text_muted())
                                    .child("Mask"),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(4.0))
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
                    .child(if display.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .child("Enter API key...")
                    } else {
                        div().text_color(Theme::text_primary()).child(display)
                    }),
            )
    }

    /// Render keyfile field with browse button
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_keyfile_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("KEY FILE"))
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
                            .child(if self.state.data.keyfile_path.is_empty() {
                                div().text_color(Theme::text_muted()).child("/path/to/key")
                            } else {
                                div()
                                    .text_color(Theme::text_primary())
                                    .child(self.state.data.keyfile_path.clone())
                            }),
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_this, _, _window, _cx| {
                                    tracing::info!("Browse keyfile is not supported — use the API Key Manager instead");
                                }),
                            ),
                    ),
            )
    }

    /// Render OAuth section
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_oauth_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let provider = if self.state.data.oauth_provider.is_empty() {
            "Provider"
        } else {
            &self.state.data.oauth_provider
        };

        let status_text = match &self.state.data.oauth_status {
            OAuthStatus::NotConnected => ("Not connected".to_string(), Theme::text_secondary()),
            OAuthStatus::Connecting => ("Connecting...".to_string(), Theme::text_secondary()),
            OAuthStatus::Connected { username } => {
                (format!("Connected as @{username}"), Theme::success())
            }
            OAuthStatus::Error(msg) => (msg.clone(), Theme::error()),
        };

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Authorize button
            .child(
                div()
                    .id("btn-oauth")
                    .w(px(360.0))
                    .h(px(36.0))
                    .bg(Theme::accent())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::accent_hover()))
                    .text_size(px(12.0))
                    .text_color(gpui::white())
                    .child(format!("Authorize with {provider}"))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            tracing::info!("OAuth authorize clicked");
                            this.start_oauth();
                        }),
                    ),
            )
            // Status
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(status_text.1)
                    .child(format!("Status: {}", status_text.0)),
            )
    }

    /// Render no-auth message
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_no_auth_section() -> impl IntoElement {
        div()
            .text_size(px(11.0))
            .text_color(Theme::text_secondary())
            .child("No authentication required for this MCP.")
    }

    /// Render a string config field
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_string_field(key: &str, value: &str, placeholder: &str) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label(key))
            .child(
                div()
                    .id(SharedString::from(format!("config-{key}")))
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
                    .child(if value.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .child(placeholder.to_string())
                    } else {
                        div()
                            .text_color(Theme::text_primary())
                            .child(value.to_string())
                    }),
            )
    }

    /// Render a boolean config field
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_boolean_field(
        key: &str,
        value: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let key_clone = key.to_string();

        div()
            .id(SharedString::from(format!("config-{key}")))
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_this, _, _window, _cx| {
                    tracing::info!("Toggle config field: {}", key_clone);
                    // Config field toggle would be handled by presenter
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
                    .when(value, |d| {
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
                    .child(key.to_string()),
            )
    }

    /// Render an array config field
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_array_field(key: &str, values: &[String]) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label(key))
            .child(
                div()
                    .id(SharedString::from(format!("config-{key}")))
                    .w(px(360.0))
                    .min_h(px(48.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .p(px(4.0))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .children(values.iter().map(|v| {
                        div()
                            .h(px(24.0))
                            .px(px(8.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::text_primary())
                                    .child(v.clone()),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .text_size(px(12.0))
                                    .text_color(Theme::text_muted())
                                    .hover(|s| s.text_color(Theme::danger()))
                                    .child("[-]"),
                            )
                            .into_any_element()
                    }))
                    .child(
                        div()
                            .h(px(24.0))
                            .px(px(8.0))
                            .cursor_pointer()
                            .text_size(px(12.0))
                            .text_color(Theme::text_secondary())
                            .hover(|s| s.text_color(Theme::text_primary()))
                            .child("[+ Add]"),
                    ),
            )
    }

    /// Render configuration fields from schema
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_config_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let fields = &self.state.data.config_fields;

        if fields.is_empty() {
            return div().into_any_element();
        }

        div()
            .flex()
            .flex_col()
            .child(Self::render_section_divider("CONFIGURATION"))
            .child(
                div().mt(px(8.0)).flex().flex_col().gap(px(12.0)).children(
                    fields
                        .iter()
                        .map(|f| match f {
                            ConfigField::String {
                                key,
                                value,
                                placeholder,
                            } => Self::render_string_field(key, value, placeholder)
                                .into_any_element(),
                            ConfigField::Boolean { key, value } => {
                                Self::render_boolean_field(key, *value, cx).into_any_element()
                            }
                            ConfigField::Array { key, values } => {
                                Self::render_array_field(key, values).into_any_element()
                            }
                        })
                        .collect::<Vec<_>>(),
                ),
            )
            .into_any_element()
    }

    /// Render the content area
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn render_content(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let auth_method = &self.state.data.auth_method;

        div()
            .id("mcp-configure-content")
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
            // Package (read-only)
            .child(self.render_package_section())
            // Authentication section
            .child(Self::render_section_divider("AUTHENTICATION"))
            .child(
                div()
                    .mt(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(self.render_auth_method_section())
                    .when(*auth_method == McpAuthMethod::ApiKey, |d| {
                        d.child(self.render_api_key_section(cx))
                    })
                    .when(*auth_method == McpAuthMethod::Keyfile, |d| {
                        d.child(self.render_keyfile_section(cx))
                    })
                    .when(*auth_method == McpAuthMethod::OAuth, |d| {
                        d.child(self.render_oauth_section(cx))
                    })
                    .when(*auth_method == McpAuthMethod::None, |d| {
                        d.child(Self::render_no_auth_section())
                    }),
            )
            // Configuration fields (if any)
            .child(self.render_config_section(cx))
    }
}

impl gpui::Focusable for McpConfigureView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for McpConfigureView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("mcp-configure-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .track_focus(&self.focus_handle)
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                    this.handle_key_down(event);
                }),
            )
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content (scrollable)
            .child(self.render_content(cx))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use flume;
    use gpui::{AppContext, TestAppContext};
    use uuid::Uuid;

    use crate::events::types::UserEvent;
    use crate::presentation::view_command::{ErrorSeverity, ViewCommand};

    fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
    }

    fn clear_navigation_requests() {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
    }

    #[gpui::test]
    async fn draft_loaded_sets_auth_transport_and_save_payload_for_remote_http(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: Uuid::nil().to_string(),
                    name: "Exa Remote".to_string(),
                    package: "exa-remote".to_string(),
                    package_type: crate::mcp::McpPackageType::Http,
                    runtime_hint: None,
                    env_var_name: String::new(),
                    command: String::new(),
                    args: vec![],
                    env: None,
                    url: Some("https://exa.example/mcp".to_string()),
                },
                cx,
            );
            assert!(view.state.is_new);
            assert_eq!(view.state.data.auth_method, McpAuthMethod::None);
            assert_eq!(
                view.state.data.url.as_deref(),
                Some("https://exa.example/mcp")
            );
            assert!(view.state.data.can_save());
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save mcp config event") {
            UserEvent::SaveMcpConfig { id, config } => {
                assert_eq!(id, Uuid::nil());
                assert_eq!(config.name, "Exa Remote");
                assert_eq!(
                    config.package.package_type,
                    crate::mcp::McpPackageType::Http
                );
                assert_eq!(config.transport, crate::mcp::McpTransport::Http);
                assert_eq!(
                    config.source,
                    crate::mcp::McpSource::Manual {
                        url: "https://exa.example/mcp".to_string()
                    }
                );
                assert!(config.env_vars.is_empty());
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn draft_loaded_with_env_requires_api_key_and_status_commands_update_oauth_state(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: saved_id.to_string(),
                    name: "Filesystem".to_string(),
                    package: "@modelcontextprotocol/server-filesystem".to_string(),
                    package_type: crate::mcp::McpPackageType::Npm,
                    runtime_hint: Some("npx".to_string()),
                    env_var_name: "FILESYSTEM_TOKEN".to_string(),
                    command: "npx".to_string(),
                    args: vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-filesystem".to_string(),
                    ],
                    env: Some(vec![("FILESYSTEM_TOKEN".to_string(), String::new())]),
                    url: None,
                },
                cx,
            );
            assert!(!view.state.is_new);
            assert_eq!(view.state.data.auth_method, McpAuthMethod::ApiKey);
            assert_eq!(view.state.data.env_var_name, "FILESYSTEM_TOKEN");
            assert_eq!(view.state.data.runtime_hint.as_deref(), Some("npx"));
            assert!(!view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowNotification {
                    message: "alice".to_string(),
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Connected {
                    username: "alice".to_string()
                }
            );

            view.handle_command(
                ViewCommand::ShowError {
                    title: "oauth failed".to_string(),
                    message: "denied".to_string(),
                    severity: ErrorSeverity::Error,
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Error("denied".to_string())
            );

            view.handle_command(
                ViewCommand::McpConfigSaved {
                    id: saved_id,
                    name: Some("Filesystem Saved".to_string()),
                },
                cx,
            );
            assert_eq!(
                view.state.data.id.as_deref(),
                Some(saved_id.to_string().as_str())
            );
            assert_eq!(view.state.data.name, "Filesystem Saved");
            assert!(!view.state.is_new);
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Connected {
                    username: "Saved".to_string()
                }
            );
        });
    }

    #[gpui::test]
    async fn set_mcp_with_keyfile_auth_and_docker_package_emits_stdio_save_payload(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, _cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.id = Some(saved_id.to_string());
            data.name = "Docker Filesystem".to_string();
            data.package = "ghcr.io/example/filesystem-mcp:latest".to_string();
            data.package_type = crate::mcp::McpPackageType::Docker;
            data.command = "docker".to_string();
            data.args = vec!["run".to_string(), "--rm".to_string()];
            data.env = Some(vec![
                ("FILESYSTEM_TOKEN".to_string(), String::new()),
                ("ROOT".to_string(), String::new()),
            ]);
            data.auth_method = McpAuthMethod::Keyfile;
            data.keyfile_path = "/tmp/filesystem-key.json".to_string();

            view.set_mcp(data, false);
            assert!(!view.state.is_new);
            assert!(view.state.data.can_save());

            view.state.data.keyfile_path.clear();
            assert!(!view.state.data.can_save());
            view.state.data.keyfile_path = "/tmp/filesystem-key.json".to_string();
            assert!(view.state.data.can_save());

            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save docker mcp event") {
            UserEvent::SaveMcpConfig { id, config } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.name, "Docker Filesystem");
                assert_eq!(
                    config.package.package_type,
                    crate::mcp::McpPackageType::Docker
                );
                assert_eq!(
                    config.package.identifier,
                    "ghcr.io/example/filesystem-mcp:latest"
                );
                assert_eq!(config.package.runtime_hint.as_deref(), Some("docker"));
                assert_eq!(config.transport, crate::mcp::McpTransport::Stdio);
                assert_eq!(
                    config.source,
                    crate::mcp::McpSource::Manual {
                        url: "docker run ghcr.io/example/filesystem-mcp:latest".to_string()
                    }
                );
                assert_eq!(
                    config.env_vars,
                    vec![
                        crate::mcp::EnvVarConfig {
                            name: "FILESYSTEM_TOKEN".to_string(),
                            required: true,
                        },
                        crate::mcp::EnvVarConfig {
                            name: "ROOT".to_string(),
                            required: true,
                        },
                    ]
                );
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn set_mcp_with_oauth_only_saves_when_connected_and_emits_npm_payload(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.name = "OAuth MCP".to_string();
            data.package = "@example/oauth-mcp".to_string();
            data.package_type = crate::mcp::McpPackageType::Npm;
            data.runtime_hint = Some("npx".to_string());
            data.command = "npx".to_string();
            data.auth_method = McpAuthMethod::OAuth;
            data.oauth_status = OAuthStatus::NotConnected;

            view.set_mcp(data, true);
            assert!(view.state.is_new);
            assert!(!view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowNotification {
                    message: "carol".to_string(),
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Connected {
                    username: "carol".to_string()
                }
            );
            assert!(view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowError {
                    title: "oauth failed".to_string(),
                    message: "expired".to_string(),
                    severity: ErrorSeverity::Error,
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Error("expired".to_string())
            );
            assert!(!view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowNotification {
                    message: "carol".to_string(),
                },
                cx,
            );
            assert!(view.state.data.can_save());
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save npm mcp event") {
            UserEvent::SaveMcpConfig { id, config } => {
                assert_ne!(id, Uuid::nil());
                assert_eq!(config.name, "OAuth MCP");
                assert_eq!(config.package.package_type, crate::mcp::McpPackageType::Npm);
                assert_eq!(config.package.identifier, "@example/oauth-mcp");
                assert_eq!(config.package.runtime_hint.as_deref(), Some("npx"));
                assert_eq!(config.transport, crate::mcp::McpTransport::Stdio);
                assert_eq!(
                    config.source,
                    crate::mcp::McpSource::Manual {
                        url: "npx @example/oauth-mcp".to_string()
                    }
                );
                assert!(config.env_vars.is_empty());
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn helper_actions_and_key_shortcuts_emit_oauth_save_and_navigation_events(
        cx: &mut TestAppContext,
    ) {
        clear_navigation_requests();
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.id = Some(saved_id.to_string());
            data.name = "Weather MCP".to_string();
            data.package = "@example/weather-mcp".to_string();
            data.package_type = crate::mcp::McpPackageType::Npm;
            data.runtime_hint = Some("npx".to_string());
            data.command = "npx".to_string();
            data.auth_method = McpAuthMethod::ApiKey;
            data.env_var_name = "WEATHER_API_KEY".to_string();
            data.api_key = "secret-token".to_string();
            data.oauth_provider = "ExampleAuth".to_string();
            view.set_mcp(data, false);

            assert!(view.state.mask_api_key);
            view.toggle_mask_api_key(cx);
            assert!(!view.state.mask_api_key);
            view.toggle_mask_api_key(cx);
            assert!(view.state.mask_api_key);

            view.start_oauth();
            view.save_current();

            view.handle_key_down(&gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("cmd-s").expect("cmd-s keystroke"),
                is_held: false,
                prefer_character_input: false,
            });

            view.handle_key_down(&gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("escape").expect("escape keystroke"),
                is_held: false,
                prefer_character_input: false,
            });
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(crate::presentation::view_command::ViewId::Settings)
            );

            view.navigate_to_settings();
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(crate::presentation::view_command::ViewId::Settings)
            );
        });

        assert_eq!(
            user_rx.recv().expect("oauth start event"),
            UserEvent::StartMcpOAuth {
                id: saved_id,
                provider: "ExampleAuth".to_string(),
            }
        );

        match user_rx.recv().expect("explicit save event") {
            UserEvent::SaveMcpConfig { id, config } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.name, "Weather MCP");
                assert_eq!(config.package.identifier, "@example/weather-mcp");
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }

        match user_rx.recv().expect("cmd-s save event") {
            UserEvent::SaveMcpConfig { id, config } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.name, "Weather MCP");
                assert_eq!(config.package.identifier, "@example/weather-mcp");
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }

        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional mcp configure events"
        );
    }
}
