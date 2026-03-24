//! API Key Manager View — CRUD screen for OS keychain-stored API keys.
//!
//! Displays a list of stored keys with masked values and "Used by" cross-refs.
//! Allows adding new keys, editing existing ones, and deleting keys.

use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, SharedString,
};
use std::ops::Range;
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ApiKeyInfo, ViewCommand, ViewId};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Editing mode for the add/edit form.
#[derive(Debug, Clone, PartialEq)]
enum EditMode {
    /// Not editing — just viewing the list.
    Idle,
    /// Adding a new key.
    Adding,
    /// Editing an existing key (label is fixed).
    Editing { label: String },
}

/// Active text field in the form.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ActiveField {
    Label,
    Value,
}

pub struct ApiKeyManagerState {
    /// All known API key entries from the presenter.
    pub keys: Vec<ApiKeyInfo>,
    /// Current editing mode.
    edit_mode: EditMode,
    /// Label text field content.
    label_input: String,
    /// Value (secret) text field content.
    value_input: String,
    /// Whether the value field is visually masked.
    mask_value: bool,
    /// Which field is active for text input.
    active_field: Option<ActiveField>,
    /// Error message to display (e.g. validation).
    error: Option<String>,
}

impl ApiKeyManagerState {
    const fn new() -> Self {
        Self {
            keys: Vec::new(),
            edit_mode: EditMode::Idle,
            label_input: String::new(),
            value_input: String::new(),
            mask_value: true,
            active_field: None,
            error: None,
        }
    }

    fn start_adding(&mut self) {
        self.edit_mode = EditMode::Adding;
        self.label_input.clear();
        self.value_input.clear();
        self.mask_value = true;
        self.active_field = Some(ActiveField::Label);
        self.error = None;
    }

    fn start_editing(&mut self, label: &str) {
        self.edit_mode = EditMode::Editing {
            label: label.to_string(),
        };
        self.label_input = label.to_string();
        self.value_input.clear();
        self.mask_value = true;
        self.active_field = Some(ActiveField::Value);
        self.error = None;
    }

    fn cancel_edit(&mut self) {
        self.edit_mode = EditMode::Idle;
        self.label_input.clear();
        self.value_input.clear();
        self.mask_value = true;
        self.active_field = None;
        self.error = None;
    }
}

pub struct ApiKeyManagerView {
    state: ApiKeyManagerState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
    ime_marked_byte_count: usize,
}

impl ApiKeyManagerView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ApiKeyManagerState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
        }
    }

    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
        self.emit(&UserEvent::RefreshApiKeys);
    }

    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::ApiKeysListed { keys } => {
                self.state.keys = keys;
                cx.notify();
            }
            ViewCommand::ApiKeyStored { .. } | ViewCommand::ApiKeyDeleted { .. } => {
                self.state.cancel_edit();
                cx.notify();
            }
            _ => {}
        }
    }

    // ── form actions ────────────────────────────────────────────────

    fn save_current(&mut self) {
        let label = self.state.label_input.trim().to_string();
        let value = self.state.value_input.trim().to_string();

        if label.is_empty() {
            self.state.error = Some("Label cannot be empty".to_string());
            return;
        }
        if value.is_empty() {
            self.state.error = Some("API key value cannot be empty".to_string());
            return;
        }

        self.emit(&UserEvent::StoreApiKey { label, value });
    }

    fn delete_key(&mut self, label: &str) {
        self.emit(&UserEvent::DeleteApiKey {
            label: label.to_string(),
        });
    }

    fn active_text(&self) -> &str {
        match self.state.active_field {
            Some(ActiveField::Label) => &self.state.label_input,
            Some(ActiveField::Value) => &self.state.value_input,
            None => "",
        }
    }

    fn set_active_text(&mut self, text: String) {
        match self.state.active_field {
            Some(ActiveField::Label) => self.state.label_input = text,
            Some(ActiveField::Value) => self.state.value_input = text,
            None => {}
        }
    }

    fn push_active_text(&mut self, s: &str) {
        match self.state.active_field {
            Some(ActiveField::Label) => self.state.label_input.push_str(s),
            Some(ActiveField::Value) => self.state.value_input.push_str(s),
            None => {}
        }
    }

    fn truncate_active_text(&mut self, at: usize) {
        match self.state.active_field {
            Some(ActiveField::Label) => self.state.label_input.truncate(at),
            Some(ActiveField::Value) => self.state.value_input.truncate(at),
            None => {}
        }
    }

    fn active_text_len(&self) -> usize {
        self.active_text().len()
    }

    fn sanitized_clipboard_text(text: &str) -> String {
        text.trim_matches(|c| c == '\r' || c == '\n').to_string()
    }

    // ── render helpers ──────────────────────────────────────────────

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

    #[allow(clippy::too_many_lines)]
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
            // Title
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child(title),
            )
            // Label field
            .child(
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
                    ),
            )
            // Value field
            .child(
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
                    ),
            )
            // Error message
            .when(self.state.error.is_some(), |d| {
                d.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(gpui::rgb(0x00EF_4444))
                        .child(self.state.error.clone().unwrap_or_default()),
                )
            })
            // Buttons
            .child(
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
                    ),
            )
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

// ── EntityInputHandler for keyboard text entry ───────────────────

impl gpui::EntityInputHandler for ApiKeyManagerView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = self.active_text();
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let start = range.start.min(utf16.len());
        let end = range.end.min(utf16.len());
        String::from_utf16(&utf16[start..end]).ok()
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        let len = self.active_text().encode_utf16().count();
        Some(gpui::UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime_marked_byte_count > 0 {
            let text = self.active_text();
            let len16: usize = text.encode_utf16().count();
            let start_utf8 = text.len().saturating_sub(self.ime_marked_byte_count);
            let start_utf16: usize = text[..start_utf8].encode_utf16().count();
            Some(start_utf16..len16)
        } else {
            None
        }
    }

    fn unmark_text(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) {
        self.ime_marked_byte_count = 0;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.active_field.is_none() {
            return;
        }

        // Remove marked (composing) portion first
        if self.ime_marked_byte_count > 0 {
            let len = self.active_text_len();
            self.truncate_active_text(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }

        if !text.is_empty() {
            self.push_active_text(text);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.state.active_field.is_none() {
            return;
        }

        if self.ime_marked_byte_count > 0 {
            let len = self.active_text_len();
            self.truncate_active_text(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }

        if !new_text.is_empty() {
            self.push_active_text(new_text);
            self.ime_marked_byte_count = new_text.len();
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<usize> {
        None
    }
}

// ── Key handling ──────────────────────────────────────────────────

impl ApiKeyManagerView {
    fn handle_key_down(
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

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    fn key_info(label: &str, masked_value: &str, used_by: &[&str]) -> ApiKeyInfo {
        ApiKeyInfo {
            label: label.to_string(),
            masked_value: masked_value.to_string(),
            used_by: used_by.iter().map(|value| (*value).to_string()).collect(),
        }
    }

    #[gpui::test]
    async fn handle_command_updates_key_list_and_resets_edit_state(cx: &mut TestAppContext) {
        let view = cx.new(ApiKeyManagerView::new);

        view.update(cx, |view: &mut ApiKeyManagerView, cx| {
            view.state.start_adding();
            view.state.label_input = "anthropic".to_string();
            view.state.value_input = "sk-secret".to_string();
            view.state.error = Some("boom".to_string());

            view.handle_command(
                ViewCommand::ApiKeysListed {
                    keys: vec![
                        key_info("anthropic", "••••1234", &["Claude"]),
                        key_info("openai", "••••5678", &[]),
                    ],
                },
                cx,
            );

            assert_eq!(view.state.keys.len(), 2);
            assert_eq!(view.state.keys[0].label, "anthropic");
            assert_eq!(view.state.edit_mode, EditMode::Adding);

            view.handle_command(
                ViewCommand::ApiKeyStored {
                    label: "anthropic".to_string(),
                },
                cx,
            );

            assert_eq!(view.state.edit_mode, EditMode::Idle);
            assert!(view.state.label_input.is_empty());
            assert!(view.state.value_input.is_empty());
            assert!(view.state.error.is_none());

            view.state.start_editing("openai");
            view.state.value_input = "replacement".to_string();
            view.handle_command(
                ViewCommand::ApiKeyDeleted {
                    label: "openai".to_string(),
                },
                cx,
            );
            assert_eq!(view.state.edit_mode, EditMode::Idle);
            assert!(view.state.active_field.is_none());
        });
    }

    #[gpui::test]
    async fn save_current_validates_and_emits_store_event(cx: &mut TestAppContext) {
        let (user_tx, user_rx) = flume::bounded(8);
        let (_view_tx, view_rx) = flume::bounded(8);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ApiKeyManagerView::new);

        view.update(cx, |view: &mut ApiKeyManagerView, _cx| {
            view.set_bridge(Arc::clone(&bridge));
        });

        assert_eq!(
            user_rx.recv().expect("refresh event"),
            UserEvent::RefreshApiKeys
        );

        view.update(cx, |view: &mut ApiKeyManagerView, _cx| {
            view.state.start_adding();
            view.state.label_input.clear();
            view.state.value_input = "secret".to_string();
            view.save_current();
            assert_eq!(view.state.error.as_deref(), Some("Label cannot be empty"));

            view.state.label_input = "anthropic".to_string();
            view.state.value_input.clear();
            view.save_current();
            assert_eq!(
                view.state.error.as_deref(),
                Some("API key value cannot be empty")
            );

            view.state.value_input = "  sk-live  ".to_string();
            view.save_current();
        });

        assert_eq!(
            user_rx.recv().expect("store event"),
            UserEvent::StoreApiKey {
                label: "anthropic".to_string(),
                value: "sk-live".to_string(),
            }
        );
    }

    #[test]
    fn text_entry_and_key_handling_follow_active_field_rules() {
        let mut view = ApiKeyManagerState::new();

        view.start_adding();
        assert_eq!(view.active_field, Some(ActiveField::Label));

        let mut manager = ApiKeyManagerState::new();
        manager.start_adding();
        assert_eq!(manager.edit_mode, EditMode::Adding);
        assert_eq!(manager.active_field, Some(ActiveField::Label));

        let mut key_manager = ApiKeyManagerState::new();
        key_manager.start_adding();

        let mut wrapper = ApiKeyManagerState::new();
        wrapper.start_adding();

        let mut state = ApiKeyManagerState::new();
        state.start_adding();
        assert_eq!(state.edit_mode, EditMode::Adding);
        assert_eq!(state.active_field, Some(ActiveField::Label));

        let mut view = ApiKeyManagerState::new();
        view.start_adding();
        assert_eq!(view.edit_mode, EditMode::Adding);

        let mut manager = ApiKeyManagerState::new();
        manager.start_adding();
        manager.label_input = "anthropic".to_string();
        assert_eq!(manager.label_input, "anthropic");

        manager.active_field = Some(ActiveField::Value);
        manager.value_input = "sk-".to_string();
        assert_eq!(manager.value_input, "sk-");

        manager.value_input.push_str("live");
        assert_eq!(manager.value_input, "sk-live");

        manager.value_input.pop();
        assert_eq!(manager.value_input, "sk-liv");

        manager.active_field = Some(ActiveField::Label);
        assert_eq!(manager.active_field, Some(ActiveField::Label));

        manager.start_editing("anthropic");
        assert_eq!(manager.active_field, Some(ActiveField::Value));
        assert_eq!(manager.label_input, "anthropic");

        manager.cancel_edit();
        assert_eq!(manager.edit_mode, EditMode::Idle);
        assert!(manager.active_field.is_none());
    }

    #[test]
    fn sanitized_clipboard_text_trims_only_newlines() {
        assert_eq!(
            ApiKeyManagerView::sanitized_clipboard_text("\nsecret\r\n"),
            "secret"
        );
        assert_eq!(
            ApiKeyManagerView::sanitized_clipboard_text("  secret  "),
            "  secret  "
        );
    }


    fn clear_navigation_requests() {
        while crate::ui_gpui::navigation_channel().take_pending().is_some() {}
    }

    #[gpui::test]
    async fn delete_key_and_escape_navigation_emit_expected_user_and_navigation_actions(
        cx: &mut TestAppContext,
    ) {
        clear_navigation_requests();
        let (user_tx, user_rx) = flume::bounded(8);
        let (_view_tx, view_rx) = flume::bounded(8);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ApiKeyManagerView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ApiKeyManagerView, cx| {
                view.set_bridge(Arc::clone(&bridge));
                view.handle_command(
                    ViewCommand::ApiKeysListed {
                        keys: vec![key_info("openai", "••••5678", &["Default"])],
                    },
                    cx,
                );
                view.state.start_editing("openai");
                view.state.value_input = "replacement".to_string();
                view.delete_key("openai");
                view.handle_command(
                    ViewCommand::ApiKeyDeleted {
                        label: "openai".to_string(),
                    },
                    cx,
                );
                assert_eq!(view.state.edit_mode, EditMode::Idle);
                assert!(view.state.active_field.is_none());

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("escape")
                            .expect("escape keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );
            });
        });

        assert_eq!(
            user_rx.recv().expect("refresh event"),
            UserEvent::RefreshApiKeys
        );
        assert_eq!(
            user_rx.recv().expect("delete event"),
            UserEvent::DeleteApiKey {
                label: "openai".to_string()
            }
        );
        assert!(user_rx.try_recv().is_err(), "unexpected additional user events");
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(ViewId::ProfileEditor)
        );
    }

    #[gpui::test]
    async fn input_handler_tracks_marked_text_replacement_and_cursor_position(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(ApiKeyManagerView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ApiKeyManagerView, cx| {
                view.state.start_adding();
                view.replace_text_in_range(None, "anth", window, cx);
                assert_eq!(view.state.label_input, "anth");
                assert_eq!(
                    view.text_for_range(0..2, &mut None, window, cx),
                    Some("an".to_string())
                );

                view.replace_and_mark_text_in_range(None, "ro", None, window, cx);
                assert_eq!(view.state.label_input, "anthro");
                assert_eq!(view.marked_text_range(window, cx), Some(4..6));

                view.replace_text_in_range(None, "pic", window, cx);
                assert_eq!(view.state.label_input, "anthpic");
                assert_eq!(view.marked_text_range(window, cx), None);

                let selection = view
                    .selected_text_range(false, window, cx)
                    .expect("selection range");
                let len = "anthpic".encode_utf16().count();
                assert_eq!(selection.range, len..len);

                view.unmark_text(window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
    }

}
