//! Render implementation for `ProfileEditorView`.

use super::{ActiveField, ApiType, ProfileEditorView};
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, SharedString, Stateful,
};

mod fields;
mod local;
mod parameters;

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
                        cx.listener(|this, _, _window, _cx| {
                            tracing::info!(
                                "Cancel clicked - resetting editor state and navigating to \
                                 Settings"
                            );
                            // Reset so a subsequent `+` new-profile flow doesn't see
                            // leftover state from the cancelled edit. See issue #182.
                            this.reset_to_new_profile();
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
    pub(super) fn render_label(text: &str) -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_secondary())
            .mb(px(4.0))
            .child(text.to_string())
    }

    /// Render a text input field
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub(super) fn render_text_field(
        id: &str,
        value: &str,
        placeholder: &str,
        active: bool,
    ) -> Stateful<gpui::Div> {
        div()
            .id(SharedString::from(id.to_string()))
            .debug_selector(move || id.to_string())
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
    /// Render the content area
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn render_content(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let capabilities = self.capabilities();
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
            // Local engine (status + shared GGUF path), Local variant only
            .child(self.render_local_engine_section(cx))
            // Parameters section
            .child(Self::render_section_divider("PARAMETERS"))
            .child(
                div()
                    .mt(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .when(capabilities.sampling, |el| {
                        el.child(self.render_temperature_section(cx))
                    })
                    .when(capabilities.max_tokens, |el| {
                        el.child(self.render_max_tokens_section(cx))
                    })
                    .child(self.render_advanced_request_parameters_section(cx))
                    // A local profile's context budget is the engine's
                    // Context size (Settings → Local Model); showing an
                    // editable per-profile value here would let the two
                    // drift. @requirement:REQ-LM-001
                    .when(self.state.data.api_type == ApiType::Local, |el| {
                        el.child(Self::render_context_limit_note())
                    })
                    .when(self.state.data.api_type != ApiType::Local, |el| {
                        el.child(self.render_context_limit_section(cx))
                    })
                    .child(self.render_show_thinking_section(cx))
                    .child(self.render_reasoning_effort_section(cx))
                    .when(capabilities.thinking_budget, |el| {
                        el.child(self.render_extended_thinking_section(cx))
                    }),
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
                        // Match the Cancel button: discard any edits so the next
                        // new-profile flow starts blank. See issue #182.
                        this.reset_to_new_profile();
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
