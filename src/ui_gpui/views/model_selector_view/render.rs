//! Render implementation for `ModelSelectorView`.

use super::{DisplayRow, ModelInfo, ModelSelectorView};
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, uniform_list, Bounds, ElementInputHandler, FocusHandle,
    FontWeight, MouseButton, Pixels, ScrollWheelEvent, SharedString,
};
use std::ops::Range;

/// Layout height constants — used for both rendering and dropdown positioning.
const TOP_BAR_H: f32 = 44.0;
const FILTER_BAR_H: f32 = 36.0;

impl ModelSelectorView {
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("top-bar")
            .h(px(TOP_BAR_H))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            // Left: Cancel button
            .child(
                div()
                    .id("btn-cancel")
                    .w(px(70.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child("Cancel")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
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
                        .child("Select Model"),
                ),
            )
            // Right: spacer for balance
            .child(div().w(px(70.0)))
    }

    /// Render the filter bar with search and provider dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_filter_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let search_query = self.state.search_query.clone();
        let provider_display = self
            .state
            .selected_provider
            .clone()
            .unwrap_or_else(|| "All".to_string());
        let show_dropdown = self.state.show_provider_dropdown;

        div()
            .id("filter-bar")
            .h(px(FILTER_BAR_H))
            .w_full()
            .bg(Theme::bg_darkest())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            // Search field
            .child(
                div()
                    .id("search-field")
                    .flex_1()
                    .h(px(28.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .cursor_text()
                    .child(if search_query.is_empty() {
                        div()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(Theme::text_muted())
                            .child("Search models...")
                    } else {
                        div()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(Theme::text_primary())
                            .child(search_query)
                    }),
            )
            // Provider dropdown button
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Provider:"),
                    )
                    .child(
                        div()
                            .id("provider-dropdown")
                            .min_w(px(120.0))
                            .max_w(px(220.0))
                            .h(px(28.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(if show_dropdown {
                                Theme::accent()
                            } else {
                                Theme::border()
                            })
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.toggle_provider_dropdown(cx);
                                }),
                            )
                            .child(provider_display)
                            .child(
                                div()
                                    .text_color(Theme::text_muted())
                                    .child(if show_dropdown { "^" } else { "v" }),
                            ),
                    ),
            )
    }

    /// Render capability filter toggles
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_capability_toggles(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let filter_reasoning = self.state.filter_reasoning;
        let filter_vision = self.state.filter_vision;

        div()
            .id("capability-toggles")
            .h(px(28.0))
            .w_full()
            .bg(Theme::bg_darkest())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(16.0))
            // Reasoning checkbox
            .child(
                div()
                    .id("filter-reasoning")
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_reasoning_filter(cx);
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
                            .when(filter_reasoning, |d| {
                                d.bg(Theme::accent()).child(
                                    div()
                                        .text_size(px(Theme::font_size_ui()))
                                        .text_color(Theme::selection_fg())
                                        .child("[OK]"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Reasoning"),
                    ),
            )
            // Vision checkbox
            .child(
                div()
                    .id("filter-vision")
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.toggle_vision_filter(cx);
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
                            .when(filter_vision, |d| {
                                d.bg(Theme::accent()).child(
                                    div()
                                        .text_size(px(Theme::font_size_ui()))
                                        .text_color(Theme::selection_fg())
                                        .child("[OK]"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Vision"),
                    ),
            )
    }

    /// Render column header
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_column_header() -> impl IntoElement {
        div()
            .id("column-header")
            .h(px(20.0))
            .w_full()
            .bg(Theme::bg_darkest())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child(div().flex_1().child("Model"))
            .child(div().w(px(50.0)).flex().justify_end().child("Context"))
            .child(div().w(px(40.0)).flex().justify_center().child("Caps"))
            .child(div().w(px(50.0)).flex().justify_end().child("In $"))
            .child(div().w(px(50.0)).flex().justify_end().child("Out $"))
    }

    /// Render a provider section header for `uniform_list` (28px uniform height).
    fn render_provider_header_uniform(provider_name: &str) -> impl IntoElement {
        div()
            .id(SharedString::from(format!(
                "provider-header-{provider_name}"
            )))
            .h(px(28.0))
            .w_full()
            .bg(Theme::bg_dark())
            .px(px(12.0))
            .flex()
            .items_center()
            .text_size(px(Theme::font_size_mono()))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::text_primary())
            .child(provider_name.to_string())
    }

    /// Render a single model row for `uniform_list` (28px uniform height).
    fn render_model_row_uniform(
        model: &ModelInfo,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let model_id = model.id.clone();
        let provider_id = model.provider_id.clone();
        let context = model.context_display();
        let caps = format!(
            "{}{}",
            if model.reasoning { "R" } else { "" },
            if model.vision { "V" } else { "" }
        );
        let cost_in = ModelInfo::cost_display(model.cost_input);
        let cost_out = ModelInfo::cost_display(model.cost_output);

        div()
            .id(SharedString::from(format!(
                "model-{provider_id}-{model_id}"
            )))
            .h(px(28.0))
            .w_full()
            .px(px(12.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, _cx| {
                    this.select_model(provider_id.clone(), model_id.clone());
                }),
            )
            .child(div().flex_1().overflow_hidden().child(model.id.clone()))
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .text_color(Theme::text_primary())
                    .child(context),
            )
            .child(
                div()
                    .w(px(40.0))
                    .flex()
                    .justify_center()
                    .text_color(Theme::text_secondary())
                    .child(caps),
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .text_color(Theme::text_primary())
                    .child(cost_in),
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .text_color(Theme::text_primary())
                    .child(cost_out),
            )
    }

    /// Render the model list using virtual scrolling via `uniform_list`.
    ///
    /// Returns `AnyElement` because `uniform_list` and `div` are different concrete
    /// types, requiring type erasure at the branch point (empty vs populated).
    fn render_model_list(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let row_count = self.state.cached_display_rows.len();

        if row_count == 0 {
            return div()
                .id("model-list-empty")
                .flex_1()
                .w_full()
                .bg(Theme::bg_darkest())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_muted())
                        .child("No matching models"),
                )
                .into_any_element();
        }

        // `cx.processor()` bridges `Context<Self>` into the `Fn(E, &mut Window, &mut App)`
        // callback that `uniform_list` requires.  Inside the closure, `this` is
        // `&mut ModelSelectorView` and `list_cx` is `&mut Context<Self>`, so
        // `list_cx.listener()` works for click handlers.
        //
        // `list_cx` is intentionally named differently from the outer `cx` to avoid
        // shadowing confusion — both are valid `Context<Self>` references, but they
        // come from different call sites.
        uniform_list(
            "model-list",
            row_count,
            cx.processor(|this: &mut Self, range: Range<usize>, _window, list_cx| {
                range
                    .filter_map(|ix| {
                        let row = this.state.cached_display_rows.get(ix)?;
                        match row {
                            DisplayRow::ProviderHeader(name) => {
                                Some(Self::render_provider_header_uniform(name).into_any_element())
                            }
                            DisplayRow::Model(idx) => {
                                // Stale-index guard: skip if models Vec was replaced
                                // between item_count capture and callback execution.
                                let model = this.state.models.get(*idx)?;
                                Some(
                                    Self::render_model_row_uniform(model, list_cx)
                                        .into_any_element(),
                                )
                            }
                        }
                    })
                    .collect()
            }),
        )
        .track_scroll(&self.scroll_handle)
        .flex_1()
        .w_full()
        .into_any_element()
    }

    /// Render the status bar
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_status_bar(model_count: usize, provider_count: usize) -> impl IntoElement {
        div()
            .id("status-bar")
            .h(px(24.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_t_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child(format!(
                "{model_count} models from {provider_count} providers"
            ))
    }

    /// Render the click-dismiss backdrop for the dropdown overlay.
    ///
    /// Starts at `TOP_BAR_H` so the top bar (Cancel) remains accessible.
    fn render_dropdown_backdrop(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("provider-menu-backdrop")
            .absolute()
            .top(px(TOP_BAR_H))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .block_mouse_except_scroll()
            .on_scroll_wheel(
                cx.listener(|_this, _event: &ScrollWheelEvent, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.show_provider_dropdown = false;
                    cx.notify();
                }),
            )
    }

    /// Render the provider dropdown overlay.
    /// Reads from `cached_providers` — no parameters needed.
    fn render_provider_dropdown(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("provider-menu-overlay")
            .absolute()
            .top(px(TOP_BAR_H + FILTER_BAR_H))
            .right(px(12.0))
            .min_w(px(180.0))
            .max_w(px(320.0))
            .max_h(px(300.0))
            .overflow_y_scroll()
            .on_scroll_wheel(
                cx.listener(|_this, _event: &ScrollWheelEvent, _window, cx| {
                    cx.stop_propagation();
                }),
            )
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(Theme::accent())
            .rounded(px(4.0))
            .shadow_lg()
            // "All" option
            .child(
                div()
                    .id("provider-all")
                    .px(px(8.0))
                    .py(px(6.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.clear_provider_filter(cx);
                        }),
                    )
                    .child("All"),
            )
            // Provider options from cached state
            .children(self.state.cached_providers.iter().map(|p| {
                let provider_id = p.clone();
                let provider_name = p.clone();
                div()
                    .id(SharedString::from(format!("provider-{provider_id}")))
                    .px(px(8.0))
                    .py(px(6.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.select_provider_filter(provider_id.clone(), cx);
                        }),
                    )
                    .child(provider_name)
            }))
    }
}

impl gpui::Focusable for ModelSelectorView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for ModelSelectorView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let show_dropdown = self.state.show_provider_dropdown;
        let model_count = self.state.cached_filtered_model_count();
        let provider_count = self.state.cached_visible_provider_count();

        let root = div()
            .id("model-selector-view")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
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
                }),
            )
            // Top bar
            .child(Self::render_top_bar(cx))
            // Filter bar
            .child(self.render_filter_bar(cx))
            // Capability toggles (28px)
            .child(self.render_capability_toggles(cx))
            // Column header (20px)
            .child(Self::render_column_header())
            // Model list — hidden when dropdown open to prevent scroll bleed.
            // uniform_list hardcodes overflow.y=Scroll, so the only way to stop
            // scroll events from reaching it is to not render it at all.
            .child(if show_dropdown {
                div()
                    .id("model-list-hidden")
                    .flex_1()
                    .w_full()
                    .bg(Theme::bg_darkest())
                    .into_any_element()
            } else {
                self.render_model_list(cx)
            })
            // Status bar (24px)
            .child(Self::render_status_bar(model_count, provider_count));

        // Dropdown overlay — backdrop and menu are *siblings*, NOT parent-child.
        // Later sibling renders on top, so the dropdown receives pointer events
        // in its bounds while the backdrop catches everything else.
        if show_dropdown {
            root.child(Self::render_dropdown_backdrop(cx))
                .child(self.render_provider_dropdown(cx))
        } else {
            root
        }
    }
}
