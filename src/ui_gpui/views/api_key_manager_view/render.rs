//! Render implementation for `ApiKeyManagerView`.

use super::{ActiveField, ApiKeyManagerView, EditMode};
use crate::presentation::view_command::{ApiKeyInfo, ViewId};
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, SharedString,
};

impl ApiKeyManagerView {
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(44.0))
            .px(px(12.0))
            .bg(Theme::bg_base())
            .border_b_1()
            .border_color(Theme::border())
            .child(
                div()
                    .id("btn-back")
                    .cursor_pointer()
                    .text_size(px(13.0))
                    .text_color(Theme::accent())
                    .hover(|s| s.text_color(Theme::text_primary()))
                    .child("← Back")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(ViewId::ProfileEditor);
                        }),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child("Manage API Keys"),
            )
            .child(
                div()
                    .id("btn-add-key")
                    .cursor_pointer()
                    .text_size(px(13.0))
                    .text_color(Theme::accent())
                    .hover(|s| s.text_color(Theme::text_primary()))
                    .child("+ Add")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.start_adding();
                            cx.notify();
                        }),
                    ),
            )
    }

    fn render_key_row(
        info: &ApiKeyInfo,
        index: usize,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let label = info.label.clone();
        let masked = info.masked_value.clone();
        let used_by = if info.used_by.is_empty() {
            "—".to_string()
        } else {
            info.used_by.join(", ")
        };
        let label_for_edit = label.clone();
        let label_for_delete = label.clone();

        div()
            .id(SharedString::from(format!("key-row-{index}")))
            .flex()
            .items_center()
            .w_full()
            .px(px(12.0))
            .py(px(8.0))
            .border_b_1()
            .border_color(Theme::border())
            .gap(px(8.0))
            // Label column
            .child(
                div()
                    .w(px(120.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(Theme::text_primary())
                    .overflow_hidden()
                    .child(label),
            )
            // Masked value column
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.0))
                    .text_color(Theme::text_muted())
                    .overflow_hidden()
                    .child(masked),
            )
            // Used by column
            .child(
                div()
                    .w(px(120.0))
                    .text_size(px(11.0))
                    .text_color(Theme::text_secondary())
                    .overflow_hidden()
                    .child(used_by),
            )
            // Edit button
            .child(
                div()
                    .id(SharedString::from(format!("btn-edit-{index}")))
                    .cursor_pointer()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_size(px(11.0))
                    .text_color(Theme::accent())
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .child("Edit")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.state.start_editing(&label_for_edit);
                            cx.notify();
                        }),
                    ),
            )
            // Delete button
            .child(
                div()
                    .id(SharedString::from(format!("btn-delete-{index}")))
                    .cursor_pointer()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .text_size(px(11.0))
                    .text_color(gpui::rgb(0x00EF_4444))
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .child("Delete")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, _cx| {
                            this.delete_key(&label_for_delete);
                        }),
                    ),
            )
    }

    fn render_key_list(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let mut list = div().flex().flex_col().w_full();

        // Column headers
        list = list.child(
            div()
                .flex()
                .items_center()
                .w_full()
                .px(px(12.0))
                .py(px(6.0))
                .border_b_1()
                .border_color(Theme::border())
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(120.0))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text_muted())
                        .child("LABEL"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text_muted())
                        .child("KEY"),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text_muted())
                        .child("USED BY"),
                )
                .child(div().w(px(80.0))), // spacer for action buttons
        );

        if self.state.keys.is_empty() {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .py(px(24.0))
                    .text_size(px(13.0))
                    .text_color(Theme::text_muted())
                    .child("No API keys stored. Click + Add to create one."),
            );
        } else {
            for (i, key) in self.state.keys.iter().enumerate() {
                list = list.child(Self::render_key_row(key, i, cx));
            }
        }

        list
    }

    fn render_label_field(
        &self,
        label_editable: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_muted())
                    .child("LABEL"),
            )
            .child(
                div()
                    .id("field-label")
                    .h(px(28.0))
                    .px(px(8.0))
                    .bg(if label_editable {
                        Theme::bg_dark()
                    } else {
                        Theme::bg_darker()
                    })
                    .border_1()
                    .border_color(if self.state.active_field == Some(ActiveField::Label) {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(if label_editable {
                        Theme::text_primary()
                    } else {
                        Theme::text_muted()
                    })
                    .overflow_hidden()
                    .when(label_editable, |d| {
                        d.cursor_text().on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, cx| {
                                this.state.active_field = Some(ActiveField::Label);
                                cx.notify();
                            }),
                        )
                    })
                    .child(if self.state.label_input.is_empty() && label_editable {
                        "e.g. anthropic".to_string()
                    } else {
                        self.state.label_input.clone()
                    }),
            )
    }

    fn render_value_field(
        &self,
        is_adding: bool,
        value_display: String,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_muted())
                            .child(if is_adding { "API KEY" } else { "NEW API KEY" }),
                    )
                    .child(
                        div()
                            .id("checkbox-mask-key")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.mask_value = !this.state.mask_value;
                                    cx.notify();
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
                                    .when(self.state.mask_value, |d| {
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
                    .id("field-value")
                    .h(px(28.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(if self.state.active_field == Some(ActiveField::Value) {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .text_color(if self.state.value_input.is_empty() {
                        Theme::text_muted()
                    } else {
                        Theme::text_primary()
                    })
                    .overflow_hidden()
                    .cursor_text()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.active_field = Some(ActiveField::Value);
                            cx.notify();
                        }),
                    )
                    .child(value_display),
            )
    }

    #[allow(clippy::unused_self)]
    fn render_form_buttons(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .child(
                div()
                    .id("btn-cancel-edit")
                    .cursor_pointer()
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .text_size(px(12.0))
                    .text_color(Theme::text_secondary())
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .child("Cancel")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.cancel_edit();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id("btn-save-key")
                    .cursor_pointer()
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(Theme::accent())
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .hover(|s| s.opacity(0.85))
                    .child("Save")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.save_current();
                            cx.notify();
                        }),
                    ),
            )
    }

    fn render_edit_form(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_adding = self.state.edit_mode == EditMode::Adding;
        let title = if is_adding {
            "Add API Key"
        } else {
            "Update API Key"
        };
        let label_editable = is_adding;
        let value_display = if self.state.value_input.is_empty() {
            "sk-...".to_string()
        } else if self.state.mask_value {
            "•".repeat(self.state.value_input.chars().count().min(64))
        } else {
            self.state.value_input.clone()
        };

        div()
            .id("edit-form")
            .flex()
            .flex_col()
            .w_full()
            .px(px(12.0))
            .py(px(12.0))
            .gap(px(8.0))
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child(title),
            )
            .child(self.render_label_field(label_editable, cx))
            .child(self.render_value_field(is_adding, value_display, cx))
            .when(self.state.error.is_some(), |d| {
                d.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(gpui::rgb(0x00EF_4444))
                        .child(self.state.error.clone().unwrap_or_default()),
                )
            })
            .child(self.render_form_buttons(cx))
    }

    fn render_content(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let showing_form = self.state.edit_mode != EditMode::Idle;

        div()
            .id("api-key-content")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .when(showing_form, |d: gpui::Stateful<gpui::Div>| {
                d.child(self.render_edit_form(cx))
            })
            .child(self.render_key_list(cx))
    }

    pub const fn focus_handle(&self, _cx: &gpui::App) -> &FocusHandle {
        &self.focus_handle
    }
}

// -- Key handling --

impl ApiKeyManagerView {
    pub(super) fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();

        let modifiers = &event.keystroke.modifiers;

        if modifiers.platform && key == "v" {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    let sanitized = Self::sanitized_clipboard_text(&text);
                    if !sanitized.is_empty() && self.state.active_field.is_some() {
                        self.push_active_text(&sanitized);
                        cx.notify();
                    }
                }
            }
            return;
        }

        if modifiers.platform || modifiers.control {
            return;
        }

        match key {
            "backspace" => {
                if self.state.active_field.is_some() {
                    if self.ime_marked_byte_count > 0 {
                        let len = self.active_text_len();
                        self.truncate_active_text(len.saturating_sub(self.ime_marked_byte_count));
                        self.ime_marked_byte_count = 0;
                    } else {
                        let text = self.active_text().to_string();
                        let mut chars: Vec<char> = text.chars().collect();
                        chars.pop();
                        let new_text: String = chars.into_iter().collect();
                        self.set_active_text(new_text);
                    }
                    cx.notify();
                }
            }
            "tab" => {
                match (&self.state.edit_mode, self.state.active_field) {
                    (EditMode::Editing { .. }, Some(ActiveField::Value | ActiveField::Label))
                    | (_, Some(ActiveField::Label)) => {
                        self.state.active_field = Some(ActiveField::Value);
                    }
                    (_, Some(ActiveField::Value)) => {
                        self.state.active_field = Some(ActiveField::Label);
                    }
                    (_, None) => {}
                }
                cx.notify();
            }
            "enter" => {
                if self.state.edit_mode != EditMode::Idle {
                    self.save_current();
                    cx.notify();
                }
            }
            "escape" => {
                if self.state.edit_mode == EditMode::Idle {
                    crate::ui_gpui::navigation_channel().request_navigate(ViewId::ProfileEditor);
                } else {
                    self.state.cancel_edit();
                    cx.notify();
                }
            }
            _ => {}
        }
    }
}

// ── Render ────────────────────────────────────────────────────────

impl gpui::Render for ApiKeyManagerView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("api-key-manager-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .track_focus(&self.focus_handle)
            // Invisible canvas for IME InputHandler registration
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
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(Self::render_top_bar(cx))
            .child(self.render_content(cx))
    }
}
