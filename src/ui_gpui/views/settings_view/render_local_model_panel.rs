//! Local Model settings panel rendering.
//!
//! Layout follows the mockup (`dev-docs/mockups/local-model-settings.html`
//! section 1): a live status card, the model path field, context/GPU-layers
//! row, idle-unload toggle + timeout, and a Save button.
//!
// @plan:PLAN-20260903-LOCALMODEL.P04
// @plan:PLAN-20260903-LOCALMODEL.P05
// @requirement:REQ-LM-006

use super::{ActiveField, SettingsView};
use crate::llm::local::engine::EngineStatus;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, SharedString};

#[allow(clippy::unused_self)]
impl SettingsView {
    /// Local Model panel: status card, config fields, save.
    pub(super) fn render_local_model_panel(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("local-model-panel-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(16.0))
            .overflow_y_scroll()
            // One-click local profile for installs that predate seeding
            // (REQ-LM-002); hidden once a local profile exists.
            .when(!self.has_local_profile(), |panel| {
                panel.child(self.render_create_local_profile_row(cx))
            })
            .child(self.render_local_model_status_card(cx))
            .child(self.render_local_model_path_field(cx))
            .child(self.render_local_model_numbers_row(cx))
            .child(self.render_local_model_idle_toggle(cx))
            .child(self.render_local_model_idle_minutes_field(cx))
            .child(self.render_local_model_save_row(cx))
    }

    /// "Create 'Granite (local)' profile" row: creates the seeded local
    /// profile via the `ProfileService` and makes it the default, mirroring
    /// fresh-install behavior.
    ///
    /// @plan:PLAN-20260903-LOCALMODEL.P05
    /// @requirement:REQ-LM-002
    fn render_create_local_profile_row(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(12.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .child(
                        "No local profile yet. One click sets up the built-in model and makes \
                         it the default.",
                    ),
            )
            .child(
                div()
                    .id("btn-create-local-profile")
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::accent_hover()))
                    .bg(Theme::selection_bg())
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::selection_fg())
                    .child("Create \u{201c}Granite (local)\u{201d} profile")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.emit_create_local_profile();
                            cx.notify();
                        }),
                    ),
            )
    }

    /// Live engine state card: colored dot, headline, detail line, unload.
    fn render_local_model_status_card(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let (title, detail, dot_color) = local_model_status_presentation(
            &self.state.local_model_status,
            self.state.local_model_error.as_deref(),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(12.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(dot_color)
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child(title),
                    ),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_muted())
                    .child(detail),
            )
            .child(
                div()
                    .id("btn-unload-local-model")
                    .px(px(12.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::danger()))
                    .bg(Theme::error())
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::selection_fg())
                    .child("Unload now")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.emit_unload_local_model();
                            cx.notify();
                        }),
                    ),
            )
    }

    /// Model file (GGUF) path input plus Choose… file picker.
    fn render_local_model_path_field(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(self.render_local_model_label("MODEL FILE (GGUF)"))
            .child(self.render_local_model_text_input(
                "local-model-path-input",
                &self.state.local_model_path_input,
                ActiveField::LocalModelPathInput,
                cx,
            ))
            .child(
                div()
                    .id("btn-choose-local-model")
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("Choose…")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.choose_local_model_file(cx);
                        }),
                    ),
            )
    }

    /// Context size and GPU layers side by side.
    fn render_local_model_numbers_row(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(self.render_local_model_label("CONTEXT SIZE"))
                    .child(self.render_local_model_text_input(
                        "local-model-ctx-input",
                        &self.state.local_model_ctx_input,
                        ActiveField::LocalModelCtxInput,
                        cx,
                    ))
                    .child(self.render_local_model_hint("tokens")),
            )
            .child(
                div()
                    .w(px(140.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(self.render_local_model_label("GPU LAYERS"))
                    .child(self.render_local_model_text_input(
                        "local-model-gpu-input",
                        &self.state.local_model_gpu_layers_input,
                        ActiveField::LocalModelGpuLayersInput,
                        cx,
                    ))
                    .child(self.render_local_model_hint("999 = all layers")),
            )
    }

    /// Unload-when-idle toggle (edited locally; persisted on Save).
    fn render_local_model_idle_toggle(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let enabled = self.state.local_model_idle_unload;

        div()
            .id("local-model-idle-toggle")
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.set_local_model_idle_unload(!enabled);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("Unload when idle"),
            )
            .child(
                div()
                    .w(px(32.0))
                    .h(px(18.0))
                    .rounded(px(9.0))
                    .bg(if enabled {
                        Theme::accent()
                    } else {
                        Theme::bg_dark()
                    })
                    .border_1()
                    .border_color(Theme::border()),
            )
    }

    /// Idle timeout in minutes.
    fn render_local_model_idle_minutes_field(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(self.render_local_model_label("IDLE TIMEOUT (MINUTES)"))
            .child(self.render_local_model_text_input(
                "local-model-idle-input",
                &self.state.local_model_idle_minutes_input,
                ActiveField::LocalModelIdleMinutesInput,
                cx,
            ))
    }

    /// Save row: persists every field via the presenter.
    fn render_local_model_save_row(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().flex().child(
            div()
                .id("btn-save-local-model")
                .px(px(16.0))
                .py(px(8.0))
                .rounded(px(4.0))
                .cursor_pointer()
                .hover(|s| s.bg(Theme::accent()).text_color(Theme::accent_fg()))
                .bg(Theme::selection_bg())
                .text_size(px(Theme::font_size_ui()))
                .text_color(Theme::selection_fg())
                .child("Save")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.save_local_model_edits();
                        cx.notify();
                    }),
                ),
        )
    }

    fn render_local_model_label(&self, label: &str) -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_muted())
            .child(label.to_string())
    }

    fn render_local_model_hint(&self, hint: &str) -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_small()))
            .text_color(Theme::text_muted())
            .child(hint.to_string())
    }

    /// A bordered text input that focuses its [`ActiveField`] on click;
    /// keystrokes land in the buffer via the shared keyboard handler.
    fn render_local_model_text_input(
        &self,
        id: &'static str,
        value: &str,
        field: ActiveField,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.state.active_field == Some(field);
        let value = value.to_string();

        div()
            .id(SharedString::from(id))
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(if is_active {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .overflow_hidden()
            .cursor_text()
            .text_size(px(Theme::font_size_mono()))
            .text_color(if value.is_empty() {
                Theme::text_muted()
            } else {
                Theme::text_primary()
            })
            .child(if value.is_empty() {
                "(empty)".to_string()
            } else {
                value
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.set_active_field(Some(field));
                    cx.notify();
                }),
            )
    }
}

/// Map an engine status to (headline, detail line, dot color).
fn local_model_status_presentation(
    status: &EngineStatus,
    error: Option<&str>,
) -> (String, String, gpui::AnyElement) {
    match status {
        EngineStatus::NotLoaded => (
            "Not loaded".to_string(),
            "Model loads on first request.".to_string(),
            text_muted_dot(),
        ),
        EngineStatus::Loading => (
            "Loading…".to_string(),
            "Reading GGUF into memory.".to_string(),
            text_secondary_dot(),
        ),
        EngineStatus::Loaded {
            layers,
            total_layers,
            n_ctx,
            last_tok_s,
        } => (
            "Loaded".to_string(),
            format!(
                "Metal: {} · ctx {n_ctx} · last gen {last_tok_s:.1} tok/s",
                layers_phrase(*layers, *total_layers),
            ),
            accent_dot(),
        ),
        EngineStatus::Error { message } => (
            "Load error".to_string(),
            error.unwrap_or(message).to_string(),
            error_dot(),
        ),
    }
}

/// "N/M layers" for offloaded-over-total; plain "N layers" when the total is
/// unknown (older snapshots deserialize `total_layers` as 0).
fn layers_phrase(layers: u32, total_layers: u32) -> String {
    if total_layers == 0 {
        format!("{layers} layers")
    } else {
        format!("{layers}/{total_layers} layers")
    }
}

// The Theme accessors return gpui color values whose type is opaque here, so
// each dot variant is its own tiny helper instead of a color enum.
fn accent_dot() -> gpui::AnyElement {
    div()
        .size(px(10.0))
        .rounded(px(5.0))
        .bg(Theme::accent())
        .into_any_element()
}

fn text_muted_dot() -> gpui::AnyElement {
    div()
        .size(px(10.0))
        .rounded(px(5.0))
        .bg(Theme::text_muted())
        .into_any_element()
}

fn text_secondary_dot() -> gpui::AnyElement {
    div()
        .size(px(10.0))
        .rounded(px(5.0))
        .bg(Theme::text_secondary())
        .into_any_element()
}

fn error_dot() -> gpui::AnyElement {
    div()
        .size(px(10.0))
        .rounded(px(5.0))
        .bg(Theme::error())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_detail_shows_offloaded_over_total_layers() {
        let (_, detail, _) = local_model_status_presentation(
            &EngineStatus::Loaded {
                layers: 10,
                total_layers: 41,
                n_ctx: 8192,
                last_tok_s: 68.4,
            },
            None,
        );
        assert_eq!(
            detail,
            "Metal: 10/41 layers · ctx 8192 · last gen 68.4 tok/s"
        );
    }

    #[test]
    fn loaded_detail_falls_back_to_plain_layers_when_total_unknown() {
        let (_, detail, _) = local_model_status_presentation(
            &EngineStatus::Loaded {
                layers: 41,
                total_layers: 0,
                n_ctx: 4096,
                last_tok_s: 0.0,
            },
            None,
        );
        assert_eq!(detail, "Metal: 41 layers · ctx 4096 · last gen 0.0 tok/s");
    }
}
