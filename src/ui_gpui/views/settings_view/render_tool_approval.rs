//! Tool approval section rendering for `SettingsView`.

use std::sync::Arc;

use super::{ActiveField, SettingsView};
use crate::agent::McpApprovalMode;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, SharedString};

impl SettingsView {
    pub(super) fn render_toggle(
        id: &str,
        label: &str,
        checked: bool,
        cx: &mut gpui::Context<Self>,
        on_toggle: impl Fn(&mut Self, &mut gpui::Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(id.to_string()))
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    on_toggle(this, cx);
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
                                .text_size(px(Theme::font_size_small()))
                                .text_color(Theme::selection_fg())
                                .child("v"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child(label.to_string()),
            )
    }

    #[allow(clippy::unused_self)]
    fn render_prefix_list(
        &self,
        id_prefix: &str,
        entries: &[String],
        on_remove: impl Fn(&mut Self, String) + 'static,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let on_remove = Arc::new(on_remove);
        div()
            .id(SharedString::from(format!("{id_prefix}-list")))
            .w_full()
            .min_h(px(32.0))
            .max_h(px(80.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .when(entries.is_empty(), |d| {
                d.items_center().justify_center().child(
                    div()
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::text_muted())
                        .child("(none)"),
                )
            })
            .children(entries.iter().enumerate().map(|(i, entry)| {
                let entry_clone = entry.clone();
                let remove = Arc::clone(&on_remove);
                div()
                    .id(SharedString::from(format!("{id_prefix}-entry-{i}")))
                    .w_full()
                    .h(px(24.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(Theme::text_primary())
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(entry.clone()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("{id_prefix}-rm-{i}")))
                            .size(px(18.0))
                            .rounded(px(2.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::danger()))
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .child("x")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, _cx| {
                                    remove(this, entry_clone.clone());
                                }),
                            ),
                    )
                    .into_any_element()
            }))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_input_row(
        id: &str,
        value: &str,
        placeholder: &str,
        is_active: bool,
        on_focus: impl Fn(&mut Self, &mut gpui::Context<Self>) + 'static,
        on_add: impl Fn(&mut Self, &mut gpui::Context<Self>) + 'static,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-field")))
                    .flex_1()
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(if is_active {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(Theme::font_size_mono()))
                    .overflow_hidden()
                    .cursor_text()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            window.focus(&this.focus_handle, cx);
                            on_focus(this, cx);
                        }),
                    )
                    .child(if value.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .child(placeholder.to_string())
                    } else {
                        div()
                            .text_color(Theme::text_primary())
                            .child(value.to_string())
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{id}-add-btn")))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::accent()).text_color(Theme::accent_fg()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("Add")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            on_add(this, cx);
                        }),
                    ),
            )
    }

    fn render_mcp_mode_selector(
        mcp_mode: McpApprovalMode,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let mode_button =
            |id: &str, label: &str, target: McpApprovalMode, cx: &mut gpui::Context<Self>| {
                let is_selected = mcp_mode == target;
                div()
                    .id(SharedString::from(id.to_string()))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .border_1()
                    .border_color(if is_selected {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .when(is_selected, |d| {
                        d.bg(Theme::selection_bg())
                            .text_color(Theme::selection_fg())
                    })
                    .when(!is_selected, |d| {
                        d.bg(Theme::bg_dark())
                            .text_color(Theme::text_primary())
                            .hover(|s| s.bg(Theme::bg_darker()))
                    })
                    .text_size(px(Theme::font_size_ui()))
                    .child(label.to_string())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, _cx| {
                            this.emit_set_mcp_approval_mode(target);
                        }),
                    )
            };

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child("MCP approval:"),
            )
            .child(mode_button(
                "mcp-mode-per-tool",
                "Per Tool",
                McpApprovalMode::PerTool,
                cx,
            ))
            .child(mode_button(
                "mcp-mode-per-server",
                "Per Server",
                McpApprovalMode::PerServer,
                cx,
            ))
    }

    #[allow(clippy::too_many_arguments)]
    fn render_editable_list_section(
        &self,
        label: &str,
        id_prefix: &str,
        entries: &[String],
        input: &str,
        placeholder: &str,
        field: ActiveField,
        on_remove: impl Fn(&mut Self, String) + 'static,
        on_add: impl Fn(&mut Self, &mut gpui::Context<Self>) + 'static,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.state.active_field == Some(field);
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(label.to_string()),
            )
            .child(self.render_prefix_list(id_prefix, entries, on_remove, cx))
            .child(Self::render_input_row(
                id_prefix,
                input,
                placeholder,
                is_active,
                move |this, cx| {
                    this.set_active_field(Some(field));
                    cx.notify();
                },
                on_add,
                cx,
            ))
    }

    pub(super) fn render_tool_approval_section(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let yolo = self.state.yolo_mode;
        let reads = self.state.auto_approve_reads;

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("TOOL APPROVAL"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(Self::render_toggle(
                        "toggle-yolo",
                        "YOLO mode",
                        yolo,
                        cx,
                        |this, _cx| this.emit_set_yolo_mode(!this.state.yolo_mode),
                    ))
                    .when(yolo, |d| {
                        d.child(
                            div()
                                .px(px(22.0))
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::warning())
                                .child("Auto-approves all tool calls except denylisted commands."),
                        )
                    }),
            )
            .child(Self::render_toggle(
                "toggle-reads",
                "Auto-approve read-only tools",
                reads,
                cx,
                |this, _cx| this.emit_set_auto_approve_reads(!this.state.auto_approve_reads),
            ))
            .child(Self::render_mcp_mode_selector(
                self.state.mcp_approval_mode,
                cx,
            ))
            .child(self.render_editable_list_section(
                "ALLOWLIST",
                "allowlist",
                &self.state.persistent_allowlist,
                &self.state.allowlist_input,
                "e.g. git, ls, cat",
                ActiveField::AllowlistInput,
                |this, prefix| this.remove_allowlist_entry(prefix),
                |this, cx| {
                    this.add_allowlist_entry();
                    cx.notify();
                },
                cx,
            ))
            .child(self.render_editable_list_section(
                "DENYLIST",
                "denylist",
                &self.state.persistent_denylist,
                &self.state.denylist_input,
                "e.g. rm, sudo, shutdown",
                ActiveField::DenylistInput,
                |this, prefix| this.remove_denylist_entry(prefix),
                |this, cx| {
                    this.add_denylist_entry();
                    cx.notify();
                },
                cx,
            ))
            .when_some(self.state.status_message.clone(), |d, msg| {
                d.child(
                    div()
                        .w_full()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(if self.state.status_is_error {
                            Theme::error()
                        } else {
                            Theme::bg_dark()
                        })
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(if self.state.status_is_error {
                            Theme::selection_fg()
                        } else {
                            Theme::text_primary()
                        })
                        .child(msg),
                )
            })
    }
}
