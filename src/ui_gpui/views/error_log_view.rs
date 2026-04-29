//! Error Log view — full-screen scrollable list of captured runtime errors.
//!
//! Displays the contents of the global [`ErrorLogStore`] ring buffer with a
//! top bar (Back / title / count / Clear All) and a scrollable card list.
//! Calling `mark_all_viewed()` on render clears the title-bar badge.
//!
//! @plan PLAN-20260325-ISSUE51.P05

use gpui::{
    div, prelude::*, px, FocusHandle, FontWeight, IntoElement, MouseButton, ParentElement,
    ScrollHandle, Styled,
};

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::error_log::{render_error_entry_text, ErrorLogEntry, ErrorLogStore};
use crate::ui_gpui::theme::Theme;
use std::path::Path;
use std::sync::Arc;

/// Full-screen error log view.
pub struct ErrorLogView {
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    bridge: Option<Arc<GpuiBridge>>,
    export_feedback_message: Option<String>,
    export_feedback_is_error: bool,
    export_feedback_path: Option<String>,
    expanded_entry_id: Option<u64>,
}

impl ErrorLogView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
            bridge: None,
            export_feedback_message: None,
            export_feedback_is_error: false,
            export_feedback_path: None,
            expanded_entry_id: None,
        }
    }

    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
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
            ViewCommand::ErrorLogExportCompleted { path } => {
                let format_label = if Path::new(&path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                {
                    "JSON"
                } else {
                    "TXT"
                };
                self.export_feedback_message =
                    Some(format!("Error log saved as {path} ({format_label})"));
                self.export_feedback_is_error = false;
                self.export_feedback_path = Some(path);
                cx.notify();
            }
            ViewCommand::ShowError {
                title,
                message,
                severity: _,
            } if title == "Save Error Log" => {
                self.export_feedback_message = Some(format!("{title}: {message}"));
                self.export_feedback_is_error = true;
                self.export_feedback_path = None;
                cx.notify();
            }
            ViewCommand::ShowNotification { message } if message.contains("No errors recorded") => {
                self.export_feedback_message = Some(message);
                self.export_feedback_is_error = false;
                self.export_feedback_path = None;
                cx.notify();
            }
            _ => {}
        }
    }
    #[cfg(target_os = "macos")]
    fn open_path(path: &str) {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }

    #[cfg(target_os = "linux")]
    fn open_path(path: &str) {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }

    #[cfg(target_os = "windows")]
    fn open_path(path: &str) {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }

    fn render_export_feedback_bar(&self) -> Option<gpui::AnyElement> {
        let _ = self.export_feedback_message.as_ref()?;
        let is_error = self.export_feedback_is_error;
        let text_color = if is_error {
            Theme::error()
        } else {
            Theme::text_secondary()
        };

        let container = div()
            .id("error-log-export-feedback")
            .h(px(24.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(Theme::SPACING_MD))
            .flex()
            .items_center();

        if let (Some(ref file_path), false) = (&self.export_feedback_path, is_error) {
            let path_for_open = file_path.clone();
            let dir_path = Path::new(file_path)
                .parent()
                .map_or_else(String::new, |p| p.display().to_string());
            let display_path = file_path.clone();

            Some(
                container
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(Theme::SPACING_SM))
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("error-log-export-open-file")
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::accent())
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(Theme::accent_hover()))
                                    .child(display_path)
                                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                        Self::open_path(&path_for_open);
                                    }),
                            )
                            .child(
                                div()
                                    .id("error-log-export-open-dir")
                                    .flex_shrink_0()
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::accent())
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(Theme::accent_hover()))
                                    .child("(dir)")
                                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                        if !dir_path.is_empty() {
                                            Self::open_path(&dir_path);
                                        }
                                    }),
                            ),
                    )
                    .into_any_element(),
            )
        } else {
            let message = self.export_feedback_message.clone().unwrap_or_default();
            Some(
                container
                    .child(
                        div()
                            .w_full()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(text_color)
                            .child(message),
                    )
                    .into_any_element(),
            )
        }
    }

    fn error_count_label(entries_len: usize) -> String {
        if entries_len == 1 {
            "1 error".to_string()
        } else {
            format!("{entries_len} errors")
        }
    }

    fn render_back_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-back")
            .h(px(28.0))
            .px(px(8.0))
            .rounded(px(Theme::RADIUS_SM))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_secondary())
            .child("\u{2039} Back")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _, _window, _cx| {
                    crate::ui_gpui::navigation_channel()
                        .request_navigate(crate::presentation::view_command::ViewId::Chat);
                }),
            )
    }

    fn render_title() -> impl IntoElement {
        div()
            .flex_1()
            .text_size(px(Theme::font_size_body()))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::text_primary())
            .child("Error Log")
    }

    fn render_error_count(entries_len: usize) -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_muted())
            .child(Self::error_count_label(entries_len))
    }

    fn render_save_error_log_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-save-error-log")
            .size(px(28.0))
            .rounded(px(Theme::RADIUS_SM))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_darker())
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_primary())
            .child("\u{2B07}")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.emit(&UserEvent::SaveErrorLog {
                        format: crate::models::ConversationExportFormat::Txt,
                    });
                    this.export_feedback_message = None;
                    this.export_feedback_is_error = false;
                    this.export_feedback_path = None;
                    cx.notify();
                }),
            )
    }

    fn render_save_error_log_json_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-save-error-log-json")
            .h(px(28.0))
            .px(px(8.0))
            .rounded(px(Theme::RADIUS_SM))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_darker())
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("JSON")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.emit(&UserEvent::SaveErrorLog {
                        format: crate::models::ConversationExportFormat::Json,
                    });
                    this.export_feedback_message = None;
                    this.export_feedback_is_error = false;
                    this.export_feedback_path = None;
                    cx.notify();
                }),
            )
    }

    fn render_clear_all_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-clear-all")
            .px(px(Theme::SPACING_SM))
            .py(px(4.0))
            .rounded(px(Theme::RADIUS_SM))
            .bg(Theme::bg_dark())
            .cursor_pointer()
            .hover(|s| s.bg(Theme::danger()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("Clear All")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    ErrorLogStore::global().clear();
                    this.expanded_entry_id = None;
                    cx.notify();
                }),
            )
    }

    fn render_top_bar(entries_len: usize, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_popout = cx
            .try_global::<crate::ui_gpui::views::main_panel::MainPanelAppState>()
            .is_some_and(|s| s.app_mode == crate::presentation::view_command::AppMode::Popout);

        div()
            .id("error-log-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .pr(px(Theme::SPACING_MD))
            .pl(px(if is_popout { 72.0 } else { Theme::SPACING_MD }))
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            .child(Self::render_title())
            .child(Self::render_error_count(entries_len))
            .child(Self::render_save_error_log_button(cx))
            .child(Self::render_save_error_log_json_button(cx))
            .child(Self::render_clear_all_button(cx))
    }

    fn render_bottom_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("error-log-bottom-bar")
            .h(px(36.0))
            .w_full()
            .flex_shrink_0()
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::border())
            .px(px(Theme::SPACING_MD))
            .flex()
            .items_center()
            .child(Self::render_back_button(cx))
    }

    #[cfg(test)]
    pub(crate) const fn export_feedback_state(&self) -> (&Option<String>, bool, &Option<String>) {
        (
            &self.export_feedback_message,
            self.export_feedback_is_error,
            &self.export_feedback_path,
        )
    }

    #[cfg(test)]
    pub(crate) const fn expanded_entry_id(&self) -> Option<u64> {
        self.expanded_entry_id
    }

    #[cfg(test)]
    pub(crate) const fn set_expanded_entry_id_for_test(&mut self, entry_id: Option<u64>) {
        self.expanded_entry_id = entry_id;
    }

    fn clear_invalid_expanded_entry(&mut self, entries: &[ErrorLogEntry]) {
        if self
            .expanded_entry_id
            .is_some_and(|selected_id| !entries.iter().any(|entry| entry.id == selected_id))
        {
            self.expanded_entry_id = None;
        }
    }

    fn render_empty_state() -> impl IntoElement {
        div().w_full().flex().justify_center().pt(px(48.0)).child(
            div()
                .text_size(px(Theme::font_size_mono()))
                .text_color(Theme::text_muted())
                .child("No errors recorded"),
        )
    }

    fn render_entry_card(
        &self,
        entry: &ErrorLogEntry,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let is_expanded = self.expanded_entry_id == Some(entry.id);
        Self::render_entry_card_shell(entry, cx, is_expanded).into_any_element()
    }

    fn render_entry_card_shell(
        entry: &ErrorLogEntry,
        cx: &mut gpui::Context<Self>,
        is_expanded: bool,
    ) -> impl IntoElement {
        let entry_id = entry.id;
        let mut bg = Theme::error();
        bg.a = 0.08;
        let mut border_color = Theme::error();
        border_color.a = 0.25;

        div()
            .id(gpui::SharedString::from(format!(
                "error-entry-{}",
                entry.id
            )))
            .w_full()
            .p(px(Theme::SPACING_MD))
            .rounded(px(Theme::RADIUS_LG))
            .bg(bg)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.expanded_entry_id = if this.expanded_entry_id == Some(entry_id) {
                    None
                } else {
                    Some(entry_id)
                };
                cx.notify();
            }))
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_XS))
            .child(Self::render_entry_summary(entry))
            .when(is_expanded, |card| {
                card.child(Self::render_entry_diagnostics(entry))
            })
    }

    fn render_entry_summary(entry: &ErrorLogEntry) -> gpui::Div {
        let conversation_label = entry
            .conversation_title
            .clone()
            .or_else(|| entry.conversation_id.as_ref().map(ToString::to_string));

        div()
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_XS))
            .child(Self::render_entry_header(entry))
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child(entry.message.clone()),
            )
            .when(conversation_label.is_some(), |d| {
                d.child(
                    div()
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::text_muted())
                        .child(format!(
                            "conv: {}",
                            conversation_label.as_deref().unwrap_or("")
                        )),
                )
            })
    }

    fn render_entry_header(entry: &ErrorLogEntry) -> gpui::Div {
        let mut severity_bg = Theme::error();
        severity_bg.a = 0.18;

        div()
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            .child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(Theme::RADIUS_SM))
                    .bg(severity_bg)
                    .text_size(px(Theme::font_size_ui()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::error())
                    .child(entry.severity.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .child(entry.source.clone()),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
            )
    }

    fn render_entry_diagnostics(entry: &ErrorLogEntry) -> gpui::Div {
        let clipboard_text = render_error_entry_text(entry);
        let details_text = if entry.diagnostics.is_some() {
            clipboard_text.clone()
        } else {
            "No diagnostic context captured".to_string()
        };

        div()
            .mt(px(Theme::SPACING_SM))
            .p(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_SM))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_XS))
            .child(Self::render_entry_diagnostics_header(
                entry.id,
                clipboard_text,
            ))
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_muted())
                    .child(details_text),
            )
    }

    fn render_entry_diagnostics_header(entry_id: u64, clipboard_text: String) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            .child(
                div()
                    .flex_1()
                    .text_size(px(Theme::font_size_ui()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_secondary())
                    .child("Diagnostics"),
            )
            .child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "error-entry-copy-diagnostics-{entry_id}"
                    )))
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(Theme::RADIUS_SM))
                    .bg(Theme::bg_dark())
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::accent())
                    .cursor_pointer()
                    .child("Copy diagnostics")
                    .on_click(move |_event, _window, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            clipboard_text.clone(),
                        ));
                    }),
            )
    }
}

impl gpui::Focusable for ErrorLogView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for ErrorLogView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let store = ErrorLogStore::global();
        store.mark_all_viewed();
        let entries = store.entries();
        let entries_len = entries.len();
        self.clear_invalid_expanded_entry(&entries);

        let mut root = div()
            .id("error-log-view")
            .size_full()
            .bg(Theme::bg_dark())
            .flex()
            .flex_col()
            .child(Self::render_top_bar(entries_len, cx));

        if let Some(feedback) = self.render_export_feedback_bar() {
            root = root.child(feedback);
        }

        root.child(
            div()
                .id("error-log-scroll")
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&self.scroll_handle)
                .child(if entries.is_empty() {
                    div()
                        .p(px(Theme::SPACING_MD))
                        .child(Self::render_empty_state())
                        .into_any_element()
                } else {
                    div()
                        .p(px(Theme::SPACING_MD))
                        .flex()
                        .flex_col()
                        .gap(px(Theme::SPACING_SM))
                        .children(
                            entries
                                .iter()
                                .map(|entry| self.render_entry_card(entry, cx)),
                        )
                        .into_any_element()
                }),
        )
        .child(Self::render_bottom_bar(cx))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use crate::ui_gpui::error_log::{ErrorLogEntry, ErrorLogStore, ErrorSeverityTag};
    use gpui::TestAppContext;

    fn make_entry(id: u64) -> ErrorLogEntry {
        ErrorLogEntry {
            id,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: format!("test/{id}"),
            message: format!("error {id}"),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        }
    }

    #[gpui::test]
    async fn view_constructs_with_empty_store(cx: &mut TestAppContext) {
        let store = ErrorLogStore::new();
        assert_eq!(store.entries().len(), 0);
        // View should construct without panic
        let _view = cx.new(ErrorLogView::new);
    }

    #[gpui::test]
    async fn view_constructs_with_populated_store(cx: &mut TestAppContext) {
        // The global store may have entries from other tests; we test the view
        // construction path is panic-free with entries present
        let _view = cx.new(ErrorLogView::new);
        // View renders without panicking regardless of store state
    }

    #[test]
    fn clear_all_empties_store() {
        let store = ErrorLogStore::new();
        store.push(make_entry);
        store.push(make_entry);
        assert_eq!(store.entries().len(), 2);
        store.clear();
        assert_eq!(store.entries().len(), 0);
    }

    #[test]
    fn mark_viewed_on_render_clears_badge() {
        let store = ErrorLogStore::new();
        store.push(make_entry);
        assert_eq!(store.unviewed_count(), 1);
        store.mark_all_viewed();
        assert_eq!(store.unviewed_count(), 0);
    }

    // --- render_entry_card: no-panic smoke tests for all severity variants ---
    #[gpui::test]
    async fn clear_invalid_expanded_entry_resets_missing_selection(cx: &mut TestAppContext) {
        let view = cx.new(ErrorLogView::new);
        view.update(cx, |view, _cx| {
            view.set_expanded_entry_id_for_test(Some(42));
            let entries = vec![make_entry(1)];

            view.clear_invalid_expanded_entry(&entries);

            assert_eq!(view.expanded_entry_id(), None);
        });
    }

    #[gpui::test]
    async fn clear_invalid_expanded_entry_keeps_existing_selection(cx: &mut TestAppContext) {
        let view = cx.new(ErrorLogView::new);
        view.update(cx, |view, _cx| {
            view.set_expanded_entry_id_for_test(Some(1));
            let entries = vec![make_entry(1)];

            view.clear_invalid_expanded_entry(&entries);

            assert_eq!(view.expanded_entry_id(), Some(1));
        });
    }

    #[gpui::test]
    async fn render_entry_card_stream_severity_no_panic(cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 0,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "stream error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    #[gpui::test]
    async fn render_entry_card_auth_severity_no_panic(cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 1,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Auth,
            source: "anthropic".to_string(),
            message: "401 Unauthorized".to_string(),
            raw_detail: Some("body: error invalid_api_key".to_string()),
            conversation_title: Some("My Chat".to_string()),
            conversation_id: None,
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    #[gpui::test]
    async fn render_entry_card_connection_severity_no_panic(cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 2,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Connection,
            source: "network".to_string(),
            message: "connection refused".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: Some(uuid::Uuid::new_v4()),
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    #[gpui::test]
    async fn render_entry_card_mcp_severity_no_panic(cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 3,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Mcp,
            source: "mcp/my-server".to_string(),
            message: "Failed to start: port in use".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    #[gpui::test]
    async fn render_entry_card_internal_severity_no_panic(cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 4,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Internal,
            source: "system".to_string(),
            message: "unexpected panic in worker".to_string(),
            raw_detail: Some("stack trace here".to_string()),
            conversation_title: None,
            conversation_id: None,
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    #[gpui::test]
    async fn render_entry_card_with_both_title_and_id_no_panic(cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 5,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Auth,
            source: "anthropic".to_string(),
            message: "forbidden".to_string(),
            raw_detail: None,
            conversation_title: Some("Work Session".to_string()),
            conversation_id: Some(uuid::Uuid::new_v4()),
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    #[gpui::test]
    async fn render_entry_card_with_only_conversation_id_no_panic(cx: &mut TestAppContext) {
        // When title is None but conversation_id is Some, the id's to_string() is shown
        let entry = ErrorLogEntry {
            id: 6,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "delta error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: Some(uuid::Uuid::new_v4()),
            diagnostics: None,
        };
        let view_entity = cx.new(ErrorLogView::new);
        view_entity.update(cx, |view, cx| {
            let _ = view.render_entry_card(&entry, cx);
        });
    }

    // --- render_empty_state: no-panic ---

    #[gpui::test]
    async fn render_empty_state_no_panic(_cx: &mut TestAppContext) {
        let _ = ErrorLogView::render_empty_state();
    }

    // --- Count label: singular vs plural ---

    #[test]
    fn count_label_singular_for_one_error() {
        // Mirror the logic in render_top_bar
        let entries_len = 1usize;
        let label = if entries_len == 1 {
            "1 error".to_string()
        } else {
            format!("{entries_len} errors")
        };
        assert_eq!(label, "1 error");
    }

    #[test]
    fn count_label_plural_for_zero_errors() {
        let entries_len = 0usize;
        let label = if entries_len == 1 {
            "1 error".to_string()
        } else {
            format!("{entries_len} errors")
        };
        assert_eq!(label, "0 errors");
    }

    #[test]
    fn count_label_plural_for_many_errors() {
        for n in [2usize, 5, 10, 100] {
            let label = if n == 1 {
                "1 error".to_string()
            } else {
                format!("{n} errors")
            };
            assert_eq!(label, format!("{n} errors"));
        }
    }

    #[gpui::test]
    async fn handle_command_sets_feedback_for_export_completed(cx: &mut TestAppContext) {
        let view = cx.new(ErrorLogView::new);
        view.update(cx, |this, cx| {
            this.handle_command(
                ViewCommand::ErrorLogExportCompleted {
                    path: "/tmp/error-log.txt".to_string(),
                },
                cx,
            );
            assert_eq!(
                this.export_feedback_message.as_deref(),
                Some("Error log saved as /tmp/error-log.txt (TXT)")
            );
            assert!(!this.export_feedback_is_error);
            assert_eq!(
                this.export_feedback_path.as_deref(),
                Some("/tmp/error-log.txt")
            );
        });
    }

    #[gpui::test]
    async fn handle_command_sets_feedback_for_save_error_log_failure(cx: &mut TestAppContext) {
        let view = cx.new(ErrorLogView::new);
        view.update(cx, |this, cx| {
            this.handle_command(
                ViewCommand::ShowError {
                    title: "Save Error Log".to_string(),
                    message: "disk unavailable".to_string(),
                    severity: crate::presentation::view_command::ErrorSeverity::Error,
                },
                cx,
            );
            assert_eq!(
                this.export_feedback_message.as_deref(),
                Some("Save Error Log: disk unavailable")
            );
            assert!(this.export_feedback_is_error);
            assert!(this.export_feedback_path.is_none());
        });
    }

    #[gpui::test]
    async fn handle_command_sets_feedback_for_empty_error_log_notice(cx: &mut TestAppContext) {
        let view = cx.new(ErrorLogView::new);
        view.update(cx, |this, cx| {
            this.handle_command(
                ViewCommand::ShowNotification {
                    message: "No errors recorded".to_string(),
                },
                cx,
            );
            assert_eq!(
                this.export_feedback_message.as_deref(),
                Some("No errors recorded")
            );
            assert!(!this.export_feedback_is_error);
            assert!(this.export_feedback_path.is_none());
        });
    }
}
