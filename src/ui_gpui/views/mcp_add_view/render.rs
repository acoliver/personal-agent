//! Render implementation for `McpAddView`.

use super::{ActiveField, McpAddView, McpRegistry, McpSearchResult, SearchState};
use crate::events::types::UserEvent;
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, ScrollWheelEvent, SharedString,
};

impl McpAddView {
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let can_proceed = self.state.can_proceed();

        div()
            .id("mcp-add-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
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
                        cx.listener(|_this, _, _window, _cx| {
                            tracing::info!("Cancel clicked - navigating to Settings");
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
                        .child("Add MCP"),
                ),
            )
            // Right: Next button
            .child(
                div()
                    .id("btn-next")
                    .w(px(60.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .flex()
                    .justify_center()
                    .text_size(px(Theme::font_size_mono()))
                    .when(can_proceed, |d| {
                        d.cursor_pointer()
                            .bg(Theme::accent())
                            .hover(|s| s.bg(Theme::accent_hover()))
                            .text_color(Theme::selection_fg())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    if let Some(selected_id) = this.state.selected_result_id.clone()
                                    {
                                        tracing::info!(
                                            "Next clicked - selected MCP {}",
                                            selected_id
                                        );
                                        this.emit(&UserEvent::SelectMcpFromRegistry {
                                            source: crate::events::types::McpRegistrySource {
                                                name: selected_id,
                                            },
                                        });
                                    } else {
                                        tracing::info!("Next clicked - proceeding via McpAddNext");
                                        this.emit(&UserEvent::McpAddNext {
                                            manual_entry: Some(this.state.manual_entry.clone()),
                                        });
                                    }
                                }),
                            )
                    })
                    .when(!can_proceed, |d| {
                        d.bg(Theme::bg_dark()).text_color(Theme::text_muted())
                    })
                    .child("Next"),
            )
    }

    /// Render a field label
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_label(text: &str) -> impl IntoElement {
        div()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_secondary())
            .mb(px(4.0))
            .child(text.to_string())
    }

    /// Render the manual entry field
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_manual_entry(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::ManualEntry);

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(Self::render_label("MANUAL ENTRY"))
            .child(
                div()
                    .id("field-manual-entry")
                    .w_full()
                    .min_h(px(30.0))
                    .px(px(8.0))
                    .py(px(6.0))
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
                    .cursor_text()
                    .text_size(px(Theme::font_size_mono()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.state.active_field = Some(ActiveField::ManualEntry);
                            this.state.show_registry_dropdown = false;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        }),
                    )
                    .child(if self.state.manual_entry.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .w_full()
                            .overflow_hidden()
                            .child("npx @scope/package or docker image or URL")
                    } else {
                        div()
                            .text_color(Theme::text_primary())
                            .w_full()
                            .child(self.state.manual_entry.clone())
                    }),
            )
    }

    /// Render the "or search registry" divider
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_divider() -> impl IntoElement {
        div()
            .w_full()
            .my(px(16.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(div().flex_1().h(px(1.0)).bg(Theme::border()))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("or search registry"),
            )
            .child(div().flex_1().h(px(1.0)).bg(Theme::border()))
    }

    /// Render the registry dropdown trigger
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_registry_dropdown(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let registry = self.state.registry.display();

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(Self::render_label("REGISTRY"))
            .child(
                div()
                    .id("dropdown-registry")
                    .w_full()
                    .h(px(30.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(if self.state.show_registry_dropdown {
                        Theme::accent()
                    } else {
                        Theme::border()
                    })
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.toggle_registry_dropdown(cx);
                            window.focus(&this.focus_handle, cx);
                        }),
                    )
                    .child(registry)
                    .child(div().text_color(Theme::text_muted()).child(
                        if self.state.show_registry_dropdown {
                            "▲"
                        } else {
                            "▼"
                        },
                    )),
            )
    }

    /// Render the floating registry dropdown overlay
    fn render_registry_overlay(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("registry-menu-overlay")
            .absolute()
            .top(px(170.0))
            .left(px(12.0))
            .right(px(12.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::accent())
            .rounded(px(4.0))
            .shadow_lg()
            .flex()
            .flex_col()
            .children(
                [
                    (McpRegistry::Official, "Official"),
                    (McpRegistry::Smithery, "Smithery"),
                    (McpRegistry::Both, "Both"),
                ]
                .into_iter()
                .map(|(registry, label)| {
                    div()
                        .id(SharedString::from(format!(
                            "registry-option-{}",
                            label.to_lowercase()
                        )))
                        .px(px(8.0))
                        .py(px(6.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::bg_darker()))
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::text_primary())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.select_registry(registry.clone());
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                            }),
                        )
                        .child(label)
                }),
            )
    }

    /// Render the search field
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_search_field(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let active = self.state.active_field == Some(ActiveField::SearchQuery);

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(Self::render_label("SEARCH"))
            .child(
                div()
                    .id("field-search")
                    .w_full()
                    .h(px(30.0))
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
                    .cursor_text()
                    .text_size(px(Theme::font_size_mono()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.state.active_field = Some(ActiveField::SearchQuery);
                            this.state.show_registry_dropdown = false;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        }),
                    )
                    .child(if self.state.search_query.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .w_full()
                            .overflow_hidden()
                            .child("Search MCP servers...")
                    } else {
                        div()
                            .text_color(Theme::text_primary())
                            .w_full()
                            .child(self.state.search_query.clone())
                    }),
            )
    }

    /// Render a search result row
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_result_row(
        &self,
        result: &McpSearchResult,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let id = result.id.clone();
        let id_for_closure = id.clone();
        let is_selected = self.state.selected_result_id.as_ref() == Some(&id);
        let name = result.name.clone();
        let description = result.description.clone();
        let badge = result.registry.display();
        let source = result.source.clone();
        let command_preview = Self::command_preview(result);

        div()
            .id(SharedString::from(format!("result-{id}")))
            .w_full()
            .min_h(px(72.0))
            .px(px(8.0))
            .py(px(8.0))
            .cursor_pointer()
            .when(is_selected, |d| d.bg(Theme::accent()))
            .when(!is_selected, |d| d.hover(|s| s.bg(Theme::bg_dark())))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.select_result(id_for_closure.clone(), cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(Theme::font_size_mono()))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_selected {
                                Theme::selection_fg()
                            } else {
                                Theme::text_primary()
                            })
                            .child(name),
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(if is_selected {
                                Theme::bg_darker()
                            } else {
                                Theme::bg_dark()
                            })
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_secondary())
                            .child(badge),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(if is_selected {
                        Theme::text_primary()
                    } else {
                        Theme::text_secondary()
                    })
                    .whitespace_normal()
                    .child(description),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(if is_selected {
                        Theme::text_secondary()
                    } else {
                        Theme::text_muted()
                    })
                    .whitespace_normal()
                    .child(format!("{source} · {command_preview}")),
            )
            .into_any_element()
    }

    /// Render the results list
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_results(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(Self::render_label("RESULTS"))
            .child(
                div()
                    .id("results-list")
                    .w_full()
                    .h(px(260.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .when(self.state.search_state == SearchState::Loading, |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(Theme::font_size_mono()))
                                .text_color(Theme::text_secondary())
                                .child("Searching..."),
                        )
                    })
                    .when(self.state.search_state == SearchState::Empty, |d| {
                        d.items_center().justify_center().p(px(16.0)).child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(Theme::font_size_mono()))
                                        .text_color(Theme::text_secondary())
                                        .child(format!(
                                            "No MCPs found matching \"{}\".",
                                            self.state.search_query
                                        )),
                                )
                                .child(
                                    div()
                                        .text_size(px(Theme::font_size_ui()))
                                        .text_color(Theme::text_muted())
                                        .child("Try a different search term."),
                                ),
                        )
                    })
                    .when(self.state.search_state == SearchState::Idle, |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(Theme::font_size_mono()))
                                .text_color(Theme::text_muted())
                                .child("Enter a search term to find MCPs"),
                        )
                    })
                    .when(self.state.search_state == SearchState::Results, |d| {
                        let results: Vec<gpui::AnyElement> = self
                            .filtered_results()
                            .iter()
                            .map(|r| self.render_result_row(r, cx))
                            .collect();
                        d.children(results)
                    })
                    .when(
                        matches!(self.state.search_state, SearchState::Error(_)),
                        |d| {
                            let message = match &self.state.search_state {
                                SearchState::Error(msg) => msg.clone(),
                                _ => String::new(),
                            };
                            d.items_center().justify_center().p(px(16.0)).child(
                                div()
                                    .text_size(px(Theme::font_size_mono()))
                                    .text_color(Theme::danger())
                                    .child(message),
                            )
                        },
                    ),
            )
    }

    /// Render the content area
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_content(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("mcp-add-content")
            .flex_1()
            .w_full()
            .bg(Theme::bg_base())
            .overflow_y_scroll()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(self.render_manual_entry(cx))
            .child(Self::render_divider())
            .child(self.render_registry_dropdown(cx))
            .child(self.render_search_field(cx))
            .child(self.render_results(cx))
    }
}

impl gpui::Focusable for McpAddView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for McpAddView {
    #[allow(clippy::too_many_lines)]
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let root = div()
            .id("mcp-add-view")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .track_focus(&self.focus_handle)
            // Invisible canvas for InputHandler registration (IME/diacritics)
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
                    // All other keys (printable chars) fall through to EntityInputHandler
                }),
            )
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content
            .child(self.render_content(cx));

        if self.state.show_registry_dropdown {
            root.child(
                div()
                    .id("registry-menu-backdrop")
                    .absolute()
                    .top(px(44.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .block_mouse_except_scroll()
                    .on_scroll_wheel(cx.listener(
                        |_this, _event: &ScrollWheelEvent, _window, cx| {
                            cx.stop_propagation();
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.show_registry_dropdown = false;
                            cx.notify();
                        }),
                    )
                    .child(Self::render_registry_overlay(cx)),
            )
        } else {
            root
        }
    }
}
