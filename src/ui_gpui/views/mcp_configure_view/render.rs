//! Render implementation for `McpConfigureView`.

use super::{ConfigField, McpAuthMethod, McpConfigureView, OAuthStatus};
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, FocusHandle, FontWeight, MouseButton, SharedString};

impl McpConfigureView {
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
                        cx.listener(|_this, _, _window, _cx| {
                            Self::navigate_to_settings();
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
                            .text_color(Theme::selection_fg())
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
                                                .text_color(Theme::selection_fg())
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
                    .text_color(Theme::selection_fg())
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
                                .text_color(Theme::selection_fg())
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
