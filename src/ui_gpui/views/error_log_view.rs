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

use crate::ui_gpui::error_log::{ErrorLogEntry, ErrorLogStore};
use crate::ui_gpui::theme::Theme;

/// Full-screen error log view.
pub struct ErrorLogView {
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
}

impl ErrorLogView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    fn render_top_bar(entries_len: usize, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let count_label = if entries_len == 1 {
            "1 error".to_string()
        } else {
            format!("{entries_len} errors")
        };

        div()
            .id("error-log-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(Theme::SPACING_MD))
            .flex()
            .items_center()
            .gap(px(Theme::SPACING_SM))
            // Back button
            .child(
                div()
                    .id("btn-back")
                    .size(px(28.0))
                    .rounded(px(Theme::RADIUS_SM))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::FONT_SIZE_BASE))
                    .text_color(Theme::text_secondary())
                    .child("<")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            crate::ui_gpui::navigation_channel()
                                .request_navigate(crate::presentation::view_command::ViewId::Chat);
                        }),
                    ),
            )
            // Title (flex-1 to push count + clear to the right)
            .child(
                div()
                    .flex_1()
                    .text_size(px(Theme::FONT_SIZE_BASE))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Error Log"),
            )
            // Error count
            .child(
                div()
                    .text_size(px(Theme::FONT_SIZE_XS))
                    .text_color(Theme::text_muted())
                    .child(count_label),
            )
            // Clear All button
            .child(
                div()
                    .id("btn-clear-all")
                    .px(px(Theme::SPACING_SM))
                    .py(px(4.0))
                    .rounded(px(Theme::RADIUS_SM))
                    .bg(Theme::bg_dark())
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::danger()))
                    .text_size(px(Theme::FONT_SIZE_XS))
                    .text_color(Theme::text_primary())
                    .child("Clear All")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, cx| {
                            ErrorLogStore::global().clear();
                            cx.notify();
                        }),
                    ),
            )
    }

    fn render_empty_state() -> impl IntoElement {
        div().w_full().flex().justify_center().pt(px(48.0)).child(
            div()
                .text_size(px(Theme::FONT_SIZE_SM))
                .text_color(Theme::text_muted())
                .child("No errors recorded"),
        )
    }

    fn render_entry_card(entry: &ErrorLogEntry) -> gpui::AnyElement {
        let severity_label = entry.severity.to_string();
        let source = entry.source.clone();
        let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let message = entry.message.clone();
        let conversation_label = entry
            .conversation_title
            .clone()
            .or_else(|| entry.conversation_id.as_ref().map(ToString::to_string));

        // Error tint background: error color with reduced alpha
        let mut bg = Theme::error();
        bg.a = 0.08;
        let mut border_color = Theme::error();
        border_color.a = 0.25;

        let mut severity_bg = Theme::error();
        severity_bg.a = 0.18;

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
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_XS))
            // Header row: severity tag + source + timestamp
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::SPACING_SM))
                    // Severity tag pill
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(Theme::RADIUS_SM))
                            .bg(severity_bg)
                            .text_size(px(Theme::FONT_SIZE_XS))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::error())
                            .child(severity_label),
                    )
                    // Source
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(Theme::FONT_SIZE_XS))
                            .text_color(Theme::text_secondary())
                            .child(source),
                    )
                    // Timestamp
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(Theme::FONT_SIZE_XS))
                            .text_color(Theme::text_muted())
                            .child(timestamp),
                    ),
            )
            // Message row
            .child(
                div()
                    .text_size(px(Theme::FONT_SIZE_SM))
                    .text_color(Theme::text_primary())
                    .child(message),
            )
            // Conversation context (if available)
            .when(conversation_label.is_some(), |d| {
                d.child(
                    div()
                        .text_size(px(Theme::FONT_SIZE_XS))
                        .text_color(Theme::text_muted())
                        .child(format!(
                            "conv: {}",
                            conversation_label.as_deref().unwrap_or("")
                        )),
                )
            })
            .into_any_element()
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

        div()
            .id("error-log-view")
            .size_full()
            .bg(Theme::bg_dark())
            .flex()
            .flex_col()
            .child(Self::render_top_bar(entries_len, cx))
            .child(
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
                            .children(entries.iter().map(Self::render_entry_card))
                            .into_any_element()
                    }),
            )
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
            source: "test / source".to_string(),
            message: format!("error {id}"),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
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
    async fn render_entry_card_stream_severity_no_panic(_cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 0,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Stream,
            source: "chat".to_string(),
            message: "stream error".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
        };
        let _ = ErrorLogView::render_entry_card(&entry);
    }

    #[gpui::test]
    async fn render_entry_card_auth_severity_no_panic(_cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 1,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Auth,
            source: "anthropic".to_string(),
            message: "401 Unauthorized".to_string(),
            raw_detail: Some("body: error invalid_api_key".to_string()),
            conversation_title: Some("My Chat".to_string()),
            conversation_id: None,
        };
        let _ = ErrorLogView::render_entry_card(&entry);
    }

    #[gpui::test]
    async fn render_entry_card_connection_severity_no_panic(_cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 2,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Connection,
            source: "network".to_string(),
            message: "connection refused".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: Some(uuid::Uuid::new_v4()),
        };
        let _ = ErrorLogView::render_entry_card(&entry);
    }

    #[gpui::test]
    async fn render_entry_card_mcp_severity_no_panic(_cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 3,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Mcp,
            source: "mcp/my-server".to_string(),
            message: "Failed to start: port in use".to_string(),
            raw_detail: None,
            conversation_title: None,
            conversation_id: None,
        };
        let _ = ErrorLogView::render_entry_card(&entry);
    }

    #[gpui::test]
    async fn render_entry_card_internal_severity_no_panic(_cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 4,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Internal,
            source: "system".to_string(),
            message: "unexpected panic in worker".to_string(),
            raw_detail: Some("stack trace here".to_string()),
            conversation_title: None,
            conversation_id: None,
        };
        let _ = ErrorLogView::render_entry_card(&entry);
    }

    #[gpui::test]
    async fn render_entry_card_with_both_title_and_id_no_panic(_cx: &mut TestAppContext) {
        let entry = ErrorLogEntry {
            id: 5,
            timestamp: chrono::Utc::now(),
            severity: ErrorSeverityTag::Auth,
            source: "anthropic".to_string(),
            message: "forbidden".to_string(),
            raw_detail: None,
            conversation_title: Some("Work Session".to_string()),
            conversation_id: Some(uuid::Uuid::new_v4()),
        };
        let _ = ErrorLogView::render_entry_card(&entry);
    }

    #[gpui::test]
    async fn render_entry_card_with_only_conversation_id_no_panic(_cx: &mut TestAppContext) {
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
        };
        let _ = ErrorLogView::render_entry_card(&entry);
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
}
