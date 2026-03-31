//! Render implementation for `ModelSelectorView`.

use super::{ModelInfo, ModelSelectorView};
use crate::ui_gpui::theme::Theme;
use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, SharedString,
};

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
                    .text_size(px(12.0))
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
                        .text_size(px(14.0))
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
                            .text_size(px(12.0))
                            .text_color(Theme::text_muted())
                            .child("Search models...")
                    } else {
                        div()
                            .text_size(px(12.0))
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
                            .text_size(px(11.0))
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
                            .text_size(px(11.0))
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
                                        .text_size(px(10.0))
                                        .text_color(gpui::white())
                                        .child("[OK]"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
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
                                        .text_size(px(10.0))
                                        .text_color(gpui::white())
                                        .child("[OK]"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
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
            .text_size(px(10.0))
            .text_color(Theme::text_primary())
            .child(div().flex_1().child("Model"))
            .child(div().w(px(50.0)).flex().justify_end().child("Context"))
            .child(div().w(px(40.0)).flex().justify_center().child("Caps"))
            .child(div().w(px(50.0)).flex().justify_end().child("In $"))
            .child(div().w(px(50.0)).flex().justify_end().child("Out $"))
    }

    /// Render a single model row
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_model_row(model: &ModelInfo, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
            .text_size(px(11.0))
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

    /// Render a provider section header
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_provider_header(provider_id: &str) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("provider-header-{provider_id}")))
            .h(px(24.0))
            .w_full()
            .bg(Theme::bg_dark())
            .px(px(12.0))
            .flex()
            .items_center()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::text_primary())
            .child(provider_id.to_string())
    }

    /// Render the model list (scrollable unless the provider dropdown is open).
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_model_list(
        filtered: &[&ModelInfo],
        providers: &[&str],
        dropdown_open: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let list = div()
            .id("model-list")
            .flex_1()
            .w_full()
            .bg(Theme::bg_darkest());

        // Disable scroll on the model list while the provider dropdown is open
        // to prevent dual-scroll (both the dropdown and the list scrolling at once).
        let list = if dropdown_open {
            list.overflow_hidden()
        } else {
            list.overflow_y_scroll()
        };

        list.child(
            div().flex().flex_col().children(
                providers
                    .iter()
                    .filter_map(|provider| {
                        let provider_models: Vec<_> = filtered
                            .iter()
                            .filter(|m| m.provider_id == *provider)
                            .collect();

                        if provider_models.is_empty() {
                            return None;
                        }

                        let provider_id = provider.to_string();
                        Some(
                            div()
                                .flex()
                                .flex_col()
                                .child(Self::render_provider_header(&provider_id))
                                .children(
                                    provider_models
                                        .into_iter()
                                        .map(|model| Self::render_model_row(model, cx))
                                        .collect::<Vec<_>>(),
                                ),
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
        )
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
            .text_size(px(11.0))
            .text_color(Theme::text_primary())
            .child(format!(
                "{model_count} models from {provider_count} providers"
            ))
    }

    /// Render the provider dropdown overlay
    fn render_provider_dropdown(
        providers: &[&str],
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("provider-menu-overlay")
            .absolute()
            .top(px(TOP_BAR_H + FILTER_BAR_H))
            .right(px(12.0))
            .min_w(px(180.0))
            .max_w(px(320.0))
            .max_h(px(300.0))
            .overflow_y_scroll()
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
                    .text_size(px(11.0))
                    .text_color(Theme::text_primary())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.clear_provider_filter(cx);
                        }),
                    )
                    .child("All"),
            )
            // Provider options
            .children(providers.iter().map(|p| {
                let provider_id = p.to_string();
                let provider_name = p.to_string();
                div()
                    .id(SharedString::from(format!("provider-{provider_id}")))
                    .px(px(8.0))
                    .py(px(6.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_darker()))
                    .text_size(px(11.0))
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

        // Compute filtered data once per render cycle.
        let filtered = self.state.filtered_models();
        let providers = self.state.all_providers();
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
                    // All other printable chars fall through to EntityInputHandler
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
            // Model list (flex, scrollable — disabled when dropdown is open)
            .child(Self::render_model_list(
                &filtered,
                &providers,
                show_dropdown,
                cx,
            ))
            // Status bar (24px)
            .child(Self::render_status_bar(model_count, provider_count));

        // Dropdown overlay — two *siblings*, NOT parent-child.
        //
        // The backdrop blocks clicks from reaching the content behind it and
        // closes the dropdown on click.  Scroll events pass through the
        // backdrop (via `block_mouse_except_scroll`), but the model list has
        // `overflow_hidden` while the dropdown is open so nothing scrolls.
        //
        // The dropdown menu is a separate element rendered AFTER the backdrop,
        // which gives it higher z-order.  Because it is NOT a child of the
        // backdrop, GPUI's hit-tester finds the dropdown first for any pointer
        // events in its bounds — including scroll — so `overflow_y_scroll` on
        // the dropdown works correctly.
        if show_dropdown {
            root.child(
                div()
                    .id("provider-menu-backdrop")
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .block_mouse_except_scroll()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.show_provider_dropdown = false;
                            cx.notify();
                        }),
                    ),
            )
            .child(Self::render_provider_dropdown(&providers, cx))
        } else {
            root
        }
    }
}
