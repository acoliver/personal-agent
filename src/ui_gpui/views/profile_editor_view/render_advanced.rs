//! Advanced request parameters rendering for `ProfileEditorView`.

use super::{ActiveField, ProfileEditorView};
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, ScrollWheelEvent};

impl ProfileEditorView {
    pub(super) const fn advanced_request_parameters_active_field(&self) -> bool {
        matches!(
            self.state.active_field,
            Some(ActiveField::MaxTokensFieldName | ActiveField::ExtraRequestFields)
        )
    }

    pub(super) fn render_advanced_request_parameters_toggle(
        expanded: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("advanced-request-parameters-toggle")
            .w(px(360.0))
            .px(px(8.0))
            .py(px(8.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_darker()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.advanced_request_parameters_expanded =
                        !this.state.advanced_request_parameters_expanded;
                    if !this.state.advanced_request_parameters_expanded
                        && this.advanced_request_parameters_active_field()
                    {
                        this.state.active_field = None;
                    }
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("Advanced request parameters"),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_secondary())
                    .child(if expanded { "▼" } else { "▶" }),
            )
    }

    pub(super) fn render_max_tokens_field_name_advanced_section(
        &self,
        field_active: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("MAX TOKENS FIELD NAME"))
            .child(
                Self::render_text_field(
                    "field-max-tokens-field-name",
                    &self.state.data.max_tokens_field_name,
                    "max_tokens",
                    field_active,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.state.active_field = Some(ActiveField::MaxTokensFieldName);
                        cx.notify();
                    }),
                ),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child(
                        "Use max_completion_tokens for reasoning-style APIs, or leave the default max_tokens.",
                    ),
            )
    }

    pub(super) fn render_extra_request_fields_editor(
        &self,
        extra_json_active: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("EXTRA JSON FIELDS"))
            .child(
                div()
                    .id("field-extra-request-fields")
                    .w(px(360.0))
                    .h(px(96.0))
                    .px(px(8.0))
                    .py(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(if extra_json_active {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(4.0))
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .overflow_y_scroll()
                    .cursor_text()
                    .block_mouse_except_scroll()
                    .on_scroll_wheel(cx.listener(
                        |_this, _event: &ScrollWheelEvent, _window, cx| {
                            cx.stop_propagation();
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.active_field = Some(ActiveField::ExtraRequestFields);
                            cx.notify();
                        }),
                    )
                    .child(if self.state.data.extra_request_fields.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .whitespace_normal()
                            .child("{}")
                    } else if extra_json_active {
                        div()
                            .w_full()
                            .text_color(Theme::text_primary())
                            .whitespace_normal()
                            .child(format!("{}|", self.state.data.extra_request_fields))
                    } else {
                        div()
                            .w_full()
                            .text_color(Theme::text_primary())
                            .whitespace_normal()
                            .child(self.state.data.extra_request_fields.clone())
                    }),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child(
                        "Provider-specific request fields are merged into the outgoing request JSON.",
                    ),
            )
    }

    pub(super) fn validate_advanced_request_json(&self) {
        let message = match serde_json::from_str::<serde_json::Value>(
            &self.state.data.extra_request_fields,
        ) {
            Ok(serde_json::Value::Object(_)) => "Advanced request JSON is valid.".to_string(),
            Ok(_) => "Advanced request JSON must be a JSON object.".to_string(),
            Err(error) => format!("Advanced request JSON is invalid: {error}"),
        };
        tracing::info!("Advanced request JSON validation: {message}");
    }

    pub(super) fn render_advanced_request_parameter_actions(
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("btn-validate-advanced-json")
                    .px(px(10.0))
                    .py(px(4.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .child("Validate JSON")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            this.validate_advanced_request_json();
                        }),
                    ),
            )
            .child(
                div()
                    .id("btn-reset-advanced")
                    .px(px(10.0))
                    .py(px(4.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("Reset Advanced")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.data.max_tokens_field_name = "max_tokens".to_string();
                            this.state.data.extra_request_fields = "{}".to_string();
                            if this.advanced_request_parameters_active_field() {
                                this.state.active_field = None;
                            }
                            cx.notify();
                        }),
                    ),
            )
    }

    pub(super) fn render_advanced_request_parameters_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let expanded = self.state.advanced_request_parameters_expanded;
        let field_active = self.state.active_field == Some(ActiveField::MaxTokensFieldName);
        let extra_json_active = self.state.active_field == Some(ActiveField::ExtraRequestFields);

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(Self::render_advanced_request_parameters_toggle(
                expanded, cx,
            ))
            .when(expanded, |d| {
                d.child(
                    div()
                        .pl(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(12.0))
                        .child(self.render_max_tokens_field_name_advanced_section(field_active, cx))
                        .child(self.render_extra_request_fields_editor(extra_json_active, cx))
                        .child(Self::render_advanced_request_parameter_actions(cx)),
                )
            })
    }
}
