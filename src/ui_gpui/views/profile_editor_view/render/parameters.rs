//! Parameter sections of the profile editor: sampling, token limits,
//! thinking controls, and the system prompt.

use super::{ActiveField, ProfileEditorView};
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, ScrollWheelEvent};

impl ProfileEditorView {
    /// Render temperature field with stepper
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub(super) fn render_temperature_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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
    pub(super) fn render_max_tokens_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::MaxTokens);

        div()
            .flex()
            .flex_col()
            .child(Self::render_label("MAX TOKENS"))
            .child(
                Self::render_text_field(
                    "field-max-tokens",
                    &self.state.data.max_tokens,
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

    pub(super) fn render_context_limit_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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

    /// Read-only stand-in for the CONTEXT LIMIT field on local profiles.
    pub(super) fn render_context_limit_note() -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("CONTEXT LIMIT"))
            .child(
                div()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_secondary())
                    .child("Set by the engine: Settings → Local Model → Context size"),
            )
    }

    /// Render show thinking checkbox
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub(super) fn render_show_thinking_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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
    pub(super) fn render_extended_thinking_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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
    pub(super) fn render_system_prompt_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
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
                    .block_mouse_except_scroll()
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
}
