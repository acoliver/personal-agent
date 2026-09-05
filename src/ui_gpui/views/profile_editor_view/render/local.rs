//! Local-engine status row of the profile editor, shown for `ApiType::Local`.
//!
//! @plan:PLAN-20260903-LOCALMODEL.P05

use super::ProfileEditorView;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton};

impl ProfileEditorView {
    /// Local-engine section (mockup section 3): engine status dot, the
    /// shared GGUF path with its picker, and the shared-store hint. Rendered
    /// only for `ApiType::Local`; the settings panel stays the full editor
    /// for the engine knobs.
    ///
    /// @plan:PLAN-20260903-LOCALMODEL.P05
    /// @requirement:REQ-LM-006 REQ-LM-007
    pub(super) fn render_local_engine_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        if self.state.data.api_type != super::ApiType::Local {
            return div().flex().flex_col();
        }
        let status = self
            .state
            .local_engine_status
            .clone()
            .unwrap_or(crate::llm::local::engine::EngineStatus::NotLoaded);
        let (phrase, dot) = local_engine_status_presentation(&status);

        div()
            .flex()
            .flex_col()
            .child(Self::render_section_divider("LOCAL ENGINE (shared)"))
            .child(
                div()
                    .mt(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // Live engine state, same tokens as the chat dropdown dot.
                    .child(
                        div().flex().items_center().gap(px(8.0)).child(dot).child(
                            div()
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_secondary())
                                .child(phrase),
                        ),
                    )
                    // Shared GGUF path: displayed read-only; the picker is
                    // the writer, mirroring the settings panel.
                    .child(
                        div()
                            .id("field-local-model-path")
                            .w(px(292.0))
                            .h(px(24.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(if self.state.local_model_path_input.is_empty() {
                                Theme::text_muted()
                            } else {
                                Theme::text_primary()
                            })
                            .child(if self.state.local_model_path_input.is_empty() {
                                "No model file chosen".to_string()
                            } else {
                                self.state.local_model_path_input.clone()
                            }),
                    )
                    .child(
                        div()
                            .id("btn-choose-local-model")
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
                            .child("Choose…")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.choose_local_model_file(cx);
                                }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_muted())
                            .child("One model file serves every local profile"),
                    ),
            )
    }
}

/// Status dot + short phrase for the local engine row, using the same
/// state→token mapping as the chat dropdown's `engine_status_dot` (gray
/// not-resident, amber loading, green resident, red failure).
///
/// @plan:PLAN-20260903-LOCALMODEL.P05
/// @requirement:REQ-LM-006
fn local_engine_status_presentation(
    status: &crate::llm::local::engine::EngineStatus,
) -> (String, gpui::AnyElement) {
    use crate::llm::local::engine::EngineStatus;
    let color = match status {
        EngineStatus::NotLoaded => Theme::text_muted(),
        EngineStatus::Loading => Theme::warning(),
        EngineStatus::Loaded { .. } => Theme::accent(),
        EngineStatus::Error { .. } => Theme::error(),
    };
    let phrase = match status {
        EngineStatus::NotLoaded => "Engine not loaded".to_string(),
        EngineStatus::Loading => "Engine loading…".to_string(),
        EngineStatus::Loaded { .. } => "Engine loaded".to_string(),
        EngineStatus::Error { message } => format!("Engine error: {message}"),
    };
    let dot = div()
        .size(px(8.0))
        .rounded(px(4.0))
        .flex_shrink_0()
        .bg(color)
        .into_any_element();
    (phrase, dot)
}
