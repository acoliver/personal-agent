//! Render implementation for `ProfileEditorView`.

use super::{ActiveField, ApiType, ProfileEditorState, ProfileEditorView};
use crate::config::default_api_base_url_for_provider;
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, ScrollWheelEvent, SharedString, Stateful,
};

impl ProfileEditorView {
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let can_save = self.state.data.can_save();
        let title = if self.state.is_new {
            "New Profile"
        } else {
            "Edit Profile"
        };

        let is_popout = cx
            .try_global::<crate::ui_gpui::views::main_panel::MainPanelAppState>()
            .is_some_and(|s| s.app_mode == crate::presentation::view_command::AppMode::Popout);

        div()
            .id("profile-editor-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .pr(px(12.0))
            .pl(px(if is_popout { 72.0 } else { 12.0 }))
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
                    .text_size(px(Theme::font_size_mono()))
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
                        .text_size(px(Theme::font_size_body()))
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
                    .text_size(px(Theme::font_size_mono()))
                    .when(can_save, |d| {
                        d.cursor_pointer()
                            .bg(Theme::accent())
                            .hover(|s| s.bg(Theme::accent_hover()))
                            .text_color(Theme::selection_fg())
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
            .text_size(px(Theme::font_size_ui()))
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
            .text_size(px(Theme::font_size_mono()))
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
                            .text_size(px(Theme::font_size_ui()))
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
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.data.api_type = match this.state.data.api_type {
                                ApiType::Anthropic => ApiType::OpenAI,
                                ApiType::OpenAI => ApiType::Local,
                                ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
                            };

                            // Clear key_label when switching to Local (no key needed)
                            if matches!(this.state.data.api_type, ApiType::Local) {
                                this.state.data.key_label.clear();
                            }

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
        // For Local provider, show "No API key required" message instead of key dropdown
        if !self.state.data.api_type.requires_api_key() {
            return div()
                .flex()
                .flex_col()
                .child(Self::render_label("API KEY"))
                .child(
                    div()
                        .w(px(360.0))
                        .h(px(24.0))
                        .px(px(8.0))
                        .bg(Theme::bg_dark())
                        .border_1()
                        .border_color(Theme::border())
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_muted())
                        .child("No API key required"),
                )
                .into_any_element();
        }

        let current_label = if self.state.data.key_label.is_empty() {
            "Select API Key…".to_string()
        } else {
            self.state.data.key_label.clone()
        };

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("API KEY"))
            .child(Self::render_key_dropdown_and_manage_button(
                current_label,
                cx,
            ))
            .into_any_element()
    }

    /// Render the key dropdown and manage button for providers that require API keys.
    fn render_key_dropdown_and_manage_button(
        current_label: String,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(if current_label == "Select API Key…" {
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
                            let next_idx = current_idx % this.state.data.available_keys.len();
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
                    .text_size(px(Theme::font_size_ui()))
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
                    .text_size(px(Theme::font_size_ui()))
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
                            .text_size(px(Theme::font_size_mono()))
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
                                    .text_size(px(Theme::font_size_small()))
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
                                    .text_size(px(Theme::font_size_small()))
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
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::selection_fg())
                                .child("v"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
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
                                        .text_size(px(Theme::font_size_ui()))
                                        .text_color(Theme::selection_fg())
                                        .child("v"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_mono()))
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
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .overflow_y_scroll()
                    .cursor_text()
                    // Stop scroll events from propagating to parent
                    .on_scroll_wheel(cx.listener(
                        |_this, _event: &ScrollWheelEvent, _window, cx| {
                            cx.stop_propagation();
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.active_field = Some(ActiveField::SystemPrompt);
                            cx.notify();
                        }),
                    )
                    .child(Self::render_system_prompt_content(
                        active,
                        &self.state.data.system_prompt,
                    )),
            )
    }

    /// Render system prompt content with cursor visibility when active
    /// Shows placeholder when empty, cursor when focused, and scrollable content.
    fn render_system_prompt_content(active: bool, system_prompt: &str) -> impl IntoElement {
        if system_prompt.is_empty() {
            // Show placeholder when empty
            div()
                .text_color(Theme::text_muted())
                .child("You are a helpful assistant.")
        } else if active {
            // Show cursor at end when field is active
            let text_content = format!("{system_prompt}|");

            div()
                .w_full()
                .text_color(Theme::text_primary())
                .whitespace_normal()
                .child(text_content)
        } else {
            // Show plain text when not active
            div()
                .w_full()
                .text_color(Theme::text_primary())
                .whitespace_normal()
                .child(system_prompt.to_string())
        }
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
