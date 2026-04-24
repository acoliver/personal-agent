//! Rendering for the shared `ConversationListView`.
//!
//! Ported from the legacy `chat_view::render_sidebar` so both popout
//! sidebar and popin History panel share a single visual implementation.
//!
//! Width/outer layout is container-controlled — this file never sets a
//! width on the root.
//!
//! @plan PLAN-20260420-ISSUE180.P02
//! @requirement REQ-180-001

use std::collections::HashSet;

use gpui::{
    canvas, div, prelude::*, px, AnyElement, Bounds, ElementInputHandler, FontWeight, MouseButton,
    Pixels, SharedString,
};
use uuid::Uuid;

use super::{ConversationListMode, ConversationListView};
use crate::events::types::UserEvent;
use crate::presentation::view_command::ConversationSummary;
use crate::ui_gpui::theme::Theme;

/// Leading indent (px) for conversation meta and detail rows so they line up
/// beneath the title text, past the delete icon and streaming indicator
/// rendered in the title row.
const SIDEBAR_TITLE_LEADING_INDENT: f32 = 34.0;

/// Returns true if this conversation should display a streaming indicator.
#[must_use]
pub fn conversation_has_streaming_indicator(
    conversation_id: Uuid,
    streaming_ids: &HashSet<Uuid>,
) -> bool {
    streaming_ids.contains(&conversation_id)
}

impl ConversationListView {
    fn render_header(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let query = self.state.sidebar_search_query.clone();
        let is_focused = self.state.sidebar_search_focused;

        div()
            .id("conv-list-header")
            .flex_shrink_0()
            .p(px(10.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .id("conv-list-search")
                    .h(px(30.0))
                    .w_full()
                    .bg(Theme::bg_darkest())
                    .border_1()
                    .border_color(if is_focused || !query.is_empty() {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(Theme::RADIUS_MD))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .cursor_text()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(if query.is_empty() {
                        Theme::text_secondary()
                    } else {
                        Theme::text_primary()
                    })
                    .child(if query.is_empty() && !is_focused {
                        SharedString::from("Search conversations...")
                    } else if query.is_empty() {
                        SharedString::from("|")
                    } else if is_focused {
                        SharedString::from(format!("{query}|"))
                    } else {
                        SharedString::from(query)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.state.sidebar_search_focused = true;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(Theme::text_secondary())
                            .child(self.header_label()),
                    )
                    .child(
                        div()
                            .id("conv-list-new-btn")
                            .size(px(22.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_size(px(Theme::font_size_body()))
                            .text_color(Theme::accent())
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .child("+")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    this.emit(&UserEvent::NewConversation);
                                }),
                            ),
                    ),
            )
    }

    fn header_label(&self) -> SharedString {
        self.state.sidebar_search_results.as_ref().map_or_else(
            || SharedString::from("CONVERSATIONS"),
            |results| SharedString::from(format!("{} results", results.len())),
        )
    }

    fn render_list(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let mut list = div()
            .id("conv-list-scroll")
            .flex_1()
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .px(px(8.0))
            .py(px(4.0))
            .flex()
            .flex_col()
            .gap(px(2.0));

        if let Some(ref results) = self.state.sidebar_search_results {
            let (title_matches, content_matches): (Vec<_>, Vec<_>) =
                results.iter().partition(|r| r.is_title_match);

            if !title_matches.is_empty() {
                list = list.child(Self::render_group_label("TITLE MATCHES"));
                for r in &title_matches {
                    list = list.child(self.render_search_result_item(r, cx));
                }
            }
            if !content_matches.is_empty() {
                list = list.child(Self::render_group_label("CONTENT MATCHES"));
                for r in &content_matches {
                    list = list.child(self.render_search_result_item(r, cx));
                }
            }
        } else {
            for conv in &self.state.conversations {
                list = list.child(self.render_conversation_item(conv, cx));
            }
        }

        list
    }

    fn render_group_label(label: &str) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(4.0))
            .text_size(px(9.0))
            .text_color(Theme::text_secondary())
            .child(SharedString::from(label.to_string()))
    }

    fn render_conversation_item(
        &self,
        conv: &ConversationSummary,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let is_selected = self.state.active_conversation_id == Some(conv.id);
        let conv_id = conv.id;

        if self.state.delete_confirming_id == Some(conv.id) {
            return self.render_delete_confirmation(conv, cx).into_any_element();
        }

        let is_renaming = is_selected && self.state.conversation_title_editing;
        let title = if is_renaming {
            let input = &self.state.conversation_title_input;
            if input.is_empty() {
                conv.title.clone()
            } else {
                input.clone()
            }
        } else if conv.title.trim().is_empty() {
            "Untitled Conversation".to_string()
        } else {
            conv.title.clone()
        };
        let (title_color, meta_color) = selection_colors(is_selected);
        let updated = format_relative_time(conv.updated_at);
        let msg_count = conv.message_count;
        let preview = conv.preview.clone().unwrap_or_default();

        let has_streaming_indicator =
            conversation_has_streaming_indicator(conv_id, &self.state.streaming_conversation_ids);

        Self::render_item_body(conv_id, is_selected)
            .child(render_title_row(
                self.render_delete_x(conv_id, cx),
                title,
                title_color,
                is_renaming,
                has_streaming_indicator,
            ))
            .child(render_meta_row(&updated, msg_count, meta_color))
            .when(!preview.is_empty(), |d| {
                d.child(render_detail_row(preview, meta_color))
            })
            .on_mouse_down(MouseButton::Left, {
                cx.listener(move |this, _, _window, cx| {
                    this.state.delete_confirming_id = None;
                    this.state.sidebar_search_focused = false;
                    this.handle_row_click(conv_id);
                    cx.notify();
                })
            })
            .into_any_element()
    }

    fn render_search_result_item(
        &self,
        result: &crate::presentation::view_command::ConversationSearchResult,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        let conv_id = result.id;

        if self.state.delete_confirming_id == Some(conv_id) {
            let summary = ConversationSummary {
                id: result.id,
                title: result.title.clone(),
                updated_at: result.updated_at,
                message_count: result.message_count,
                preview: None,
            };
            return self
                .render_delete_confirmation(&summary, cx)
                .into_any_element();
        }

        let is_selected = self.state.active_conversation_id == Some(conv_id);
        let title = if result.title.trim().is_empty() {
            "Untitled Conversation".to_string()
        } else {
            result.title.clone()
        };
        let updated = format_relative_time(result.updated_at);
        let msg_count = result.message_count;
        let context = result.match_context.clone();
        let (title_color, meta_color) = selection_colors(is_selected);
        let context_color = if is_selected {
            Theme::selection_fg()
        } else {
            Theme::accent()
        };

        let has_streaming_indicator =
            conversation_has_streaming_indicator(conv_id, &self.state.streaming_conversation_ids);

        Self::render_item_body(conv_id, is_selected)
            .child(render_title_row(
                self.render_delete_x(conv_id, cx),
                title,
                title_color,
                false,
                has_streaming_indicator,
            ))
            .child(render_meta_row(&updated, msg_count, meta_color))
            .when(!context.is_empty(), |d| {
                d.child(render_detail_row(context, context_color))
            })
            .on_mouse_down(MouseButton::Left, {
                cx.listener(move |this, _, _window, cx| {
                    this.state.delete_confirming_id = None;
                    this.state.sidebar_search_focused = false;
                    this.handle_row_click(conv_id);
                    cx.notify();
                })
            })
            .into_any_element()
    }

    fn render_item_body(conv_id: Uuid, is_selected: bool) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("conv-{conv_id}")))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .when(is_selected, |d| d.bg(Theme::selection_bg()))
            .when(!is_selected, |d| d.hover(|s| s.bg(Theme::bg_dark())))
            .flex()
            .flex_col()
            .gap(px(2.0))
    }

    #[allow(clippy::unused_self)]
    fn render_delete_x(&self, conv_id: Uuid, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("del-{conv_id}")))
            .size(px(16.0))
            .rounded(px(3.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(px(11.0))
            .text_color(Theme::error())
            .hover(|s| s.bg(Theme::bg_dark()))
            .child("x")
            .on_mouse_down(MouseButton::Left, {
                cx.listener(move |this, _, _window, cx| {
                    cx.stop_propagation();
                    this.state.delete_confirming_id = Some(conv_id);
                    cx.notify();
                })
            })
    }

    #[allow(clippy::unused_self)]
    fn render_delete_confirmation(
        &self,
        conv: &ConversationSummary,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let conv_id = conv.id;
        let title = if conv.title.trim().is_empty() {
            "Untitled Conversation".to_string()
        } else {
            conv.title.clone()
        };

        div()
            .id(SharedString::from(format!("confirm-del-{conv_id}")))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::error())
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::error())
                    .child(SharedString::from(format!("Delete \"{title}\"?"))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id(SharedString::from(format!("confirm-yes-{conv_id}")))
                            .px(px(14.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_dark())
                            .text_size(px(10.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::error())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_darkest()))
                            .child("Delete")
                            .on_mouse_down(MouseButton::Left, {
                                cx.listener(move |this, _, _window, cx| {
                                    this.state.delete_confirming_id = None;
                                    this.emit(&UserEvent::DeleteConversation { id: conv_id });
                                    cx.notify();
                                })
                            }),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("confirm-no-{conv_id}")))
                            .px(px(14.0))
                            .py(px(3.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(Theme::border())
                            .text_size(px(10.0))
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_darkest()))
                            .child("Cancel")
                            .on_mouse_down(MouseButton::Left, {
                                cx.listener(move |this, _, _window, cx| {
                                    this.state.delete_confirming_id = None;
                                    cx.notify();
                                })
                            }),
                    ),
            )
    }
}

fn selection_colors(is_selected: bool) -> (gpui::Hsla, gpui::Hsla) {
    if is_selected {
        (Theme::selection_fg(), Theme::selection_fg())
    } else {
        (Theme::text_primary(), Theme::text_secondary())
    }
}

fn render_streaming_indicator(show: bool) -> impl IntoElement {
    div()
        .size(px(6.0))
        .rounded(px(3.0))
        .flex_shrink_0()
        .when(show, |d| d.bg(Theme::accent()))
        .when(!show, |d| d.bg(gpui::transparent_black()))
}

fn render_title_row(
    delete_x: impl IntoElement,
    title: String,
    title_color: gpui::Hsla,
    is_renaming: bool,
    has_streaming_indicator: bool,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(delete_x)
        .child(render_streaming_indicator(has_streaming_indicator))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(title_color)
                .child(SharedString::from(title))
                .when(is_renaming, |d| {
                    d.border_b_1().border_color(Theme::accent())
                }),
        )
}

fn render_meta_row(updated: &str, msg_count: usize, color: gpui::Hsla) -> impl IntoElement {
    div()
        .pl(px(SIDEBAR_TITLE_LEADING_INDENT))
        .text_size(px(10.0))
        .text_color(color)
        .child(SharedString::from(format!(
            "{updated} \u{00B7} {msg_count} messages"
        )))
}

fn render_detail_row(text: String, color: gpui::Hsla) -> impl IntoElement {
    div()
        .pl(px(SIDEBAR_TITLE_LEADING_INDENT))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(10.0))
        .text_color(color)
        .child(SharedString::from(text))
}

fn format_relative_time(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);

    if diff.num_minutes() < 1 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 7 {
        format!("{}d ago", diff.num_days())
    } else {
        dt.format("%b %d").to_string()
    }
}

impl gpui::Render for ConversationListView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        // Container-controlled width: never set width here. Fill the parent.
        let bg = match self.mode {
            ConversationListMode::Inline => Theme::bg_darker(),
            ConversationListMode::FullPanel => Theme::bg_dark(),
        };
        div()
            .id("conversation-list")
            .size_full()
            .bg(bg)
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
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
                    this.handle_key_down(event, cx);
                }),
            )
            .child(self.render_header(cx))
            .child(self.render_list(cx))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use uuid::Uuid;

    use super::conversation_has_streaming_indicator;

    #[test]
    fn streaming_indicator_true_when_in_set() {
        let id = Uuid::new_v4();
        let mut ids = HashSet::new();
        ids.insert(id);
        assert!(conversation_has_streaming_indicator(id, &ids));
    }

    #[test]
    fn streaming_indicator_false_when_not_in_set() {
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut ids = HashSet::new();
        ids.insert(other);
        assert!(!conversation_has_streaming_indicator(id, &ids));
    }

    #[test]
    fn streaming_indicator_false_for_empty_set() {
        let id = Uuid::new_v4();
        let ids = HashSet::new();
        assert!(!conversation_has_streaming_indicator(id, &ids));
    }
}
