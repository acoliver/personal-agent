//! Model Selector View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P07
//! @requirement REQ-UI-MS

use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, ScrollWheelEvent, SharedString,
};
use std::ops::Range;
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Model information for display
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Debug, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub provider_id: String,
    pub context: u64,
    pub reasoning: bool,
    pub vision: bool,
    pub cost_input: f64,
    pub cost_output: f64,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, provider_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            provider_id: provider_id.into(),
            context: 128_000,
            reasoning: false,
            vision: false,
            cost_input: 0.0,
            cost_output: 0.0,
        }
    }

    #[must_use]
    pub const fn with_context(mut self, context: u64) -> Self {
        self.context = context;
        self
    }

    #[must_use]
    pub const fn with_capabilities(mut self, reasoning: bool, vision: bool) -> Self {
        self.reasoning = reasoning;
        self.vision = vision;
        self
    }

    #[must_use]
    pub const fn with_costs(mut self, input: f64, output: f64) -> Self {
        self.cost_input = input;
        self.cost_output = output;
        self
    }

    /// Format context for display (e.g., "128K", "1M")
    #[must_use]
    pub fn context_display(&self) -> String {
        if self.context >= 1_000_000 {
            format!("{}M", self.context / 1_000_000)
        } else if self.context >= 1_000 {
            format!("{}K", self.context / 1_000)
        } else {
            self.context.to_string()
        }
    }

    /// Format cost for display (e.g., "$3", "$0.25", "free")
    #[must_use]
    pub fn cost_display(cost: f64) -> String {
        #[allow(clippy::float_cmp)]
        if cost == 0.0 {
            "free".to_string()
        } else if (cost - cost.floor()).abs() < f64::EPSILON {
            #[allow(clippy::cast_possible_truncation)]
            let whole = cost as i64;
            format!("${whole}")
        } else {
            format!("${cost:.2}")
        }
    }
}

/// Provider information
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
}

impl ProviderInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Model Selector view state
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Default)]
pub struct ModelSelectorState {
    pub providers: Vec<ProviderInfo>,
    pub models: Vec<ModelInfo>,
    pub search_query: String,
    pub selected_provider: Option<String>,
    pub filter_reasoning: bool,
    pub filter_vision: bool,
    pub show_provider_dropdown: bool,

    /// Last search query emitted to presenter, used to avoid redundant events.
    pub last_emitted_search_query: String,
    /// Last provider filter emitted to presenter, used to avoid redundant events.
    pub last_emitted_provider: Option<String>,
}

impl ModelSelectorState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get filtered models based on current filters
    #[must_use]
    pub fn filtered_models(&self) -> Vec<&ModelInfo> {
        self.models
            .iter()
            .filter(|m| {
                // Provider filter
                if let Some(ref provider) = self.selected_provider {
                    if &m.provider_id != provider {
                        return false;
                    }
                }
                // Search filter
                if !self.search_query.is_empty() {
                    let query = self.search_query.to_lowercase();
                    if !m.id.to_lowercase().contains(&query)
                        && !m.provider_id.to_lowercase().contains(&query)
                    {
                        return false;
                    }
                }
                // Capability filters
                if self.filter_reasoning && !m.reasoning {
                    return false;
                }
                if self.filter_vision && !m.vision {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Get unique providers from all models (not just filtered)
    #[must_use]
    pub fn all_providers(&self) -> Vec<&str> {
        let mut providers: Vec<&str> = self.models.iter().map(|m| m.provider_id.as_str()).collect();
        providers.sort_unstable();
        providers.dedup();
        providers
    }
}

/// Model Selector view component
/// @plan PLAN-20250130-GPUIREDUX.P07
pub struct ModelSelectorView {
    state: ModelSelectorState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
    ime_marked_byte_count: usize,
}

impl ModelSelectorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ModelSelectorState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P07
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set models from presenter
    pub fn set_models(&mut self, providers: Vec<ProviderInfo>, models: Vec<ModelInfo>) {
        self.state.providers = providers;
        self.state.models = models;
    }

    /// Set search query programmatically (for testing)
    pub fn set_search_query(&mut self, query: String) {
        self.state.search_query = query;
    }

    /// Set selected provider programmatically (for testing)
    pub fn set_selected_provider(&mut self, provider: Option<String>) {
        self.state.selected_provider = provider;
    }

    /// Emit `SearchModels` event for current query.
    pub fn emit_search_models(&self) {
        self.emit(&UserEvent::SearchModels {
            query: self.state.search_query.clone(),
        });
    }

    /// Get current state for testing
    #[must_use]
    pub const fn get_state(&self) -> &ModelSelectorState {
        &self.state
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Handle `ViewCommand` from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P07
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        if let ViewCommand::ModelSearchResults { models } = command {
            println!(
                ">>> ModelSelectorView::handle_command received {} models <<<",
                models.len()
            );
            tracing::info!("ModelSelectorView received {} models", models.len());

            // Extract unique providers from the models
            let mut provider_set = std::collections::HashSet::new();
            for m in &models {
                provider_set.insert(m.provider_id.clone());
            }
            let providers: Vec<ProviderInfo> = provider_set
                .into_iter()
                .map(|id| ProviderInfo {
                    id: id.clone(),
                    name: id,
                })
                .collect();

            println!(">>> Providers extracted: {} <<<", providers.len());

            let local_models: Vec<ModelInfo> = models
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.model_id,
                    provider_id: m.provider_id,
                    context: u64::from(m.context_length.unwrap_or(128_000)),
                    reasoning: false,
                    vision: false,
                    cost_input: 0.0,
                    cost_output: 0.0,
                })
                .collect();

            println!(">>> Setting {} models on view <<<", local_models.len());
            self.set_models(providers, local_models);
            println!(
                ">>> Models set, state.models.len() = {} <<<",
                self.state.models.len()
            );
        }
        cx.notify();
    }

    /// Request models from presenter on view open
    pub fn request_models(&self) {
        tracing::info!("ModelSelectorView requesting models");
        self.emit(&UserEvent::OpenModelSelector);
    }

    /// Emit search/filter events when local filter state changes.
    fn emit_filter_events_if_changed(&mut self) {
        if self.state.search_query != self.state.last_emitted_search_query {
            self.emit(&UserEvent::SearchModels {
                query: self.state.search_query.clone(),
            });
            self.state.last_emitted_search_query = self.state.search_query.clone();
        }

        if self.state.selected_provider != self.state.last_emitted_provider {
            self.emit(&UserEvent::FilterModelsByProvider {
                provider_id: self.state.selected_provider.clone(),
            });
            self.state.last_emitted_provider = self.state.selected_provider.clone();
        }
    }


    fn toggle_provider_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.show_provider_dropdown = !self.state.show_provider_dropdown;
        cx.notify();
    }

    fn toggle_reasoning_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.filter_reasoning = !self.state.filter_reasoning;
        cx.notify();
    }

    fn toggle_vision_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.filter_vision = !self.state.filter_vision;
        cx.notify();
    }

    fn clear_provider_filter(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.selected_provider = None;
        self.state.show_provider_dropdown = false;
        self.emit_filter_events_if_changed();
        cx.notify();
    }

    fn select_provider_filter(&mut self, provider_id: String, cx: &mut gpui::Context<Self>) {
        self.state.selected_provider = Some(provider_id);
        self.state.show_provider_dropdown = false;
        self.emit_filter_events_if_changed();
        cx.notify();
    }

    fn select_model(&mut self, provider_id: String, model_id: String) {
        println!(">>> Model selected: {model_id} from {provider_id} <<<");
        self.emit(&UserEvent::SelectModel {
            provider_id,
            model_id,
        });
        self.state.show_provider_dropdown = false;
    }

    fn handle_key_down(&mut self, event: &gpui::KeyDownEvent, cx: &mut gpui::Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if key == "escape" {
            if self.state.show_provider_dropdown {
                self.state.show_provider_dropdown = false;
                cx.notify();
            } else {
                crate::ui_gpui::navigation_channel()
                    .request_navigate(crate::presentation::view_command::ViewId::Settings);
            }
            return;
        }

        if modifiers.platform && key == "w" {
            crate::ui_gpui::navigation_channel()
                .request_navigate(crate::presentation::view_command::ViewId::Settings);
            return;
        }

        if !modifiers.platform && !modifiers.control && key == "backspace" {
            self.state.search_query.pop();
            self.emit_filter_events_if_changed();
            cx.notify();
        }
    }

    /// Render the top bar with cancel button and title
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_top_bar(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("top-bar")
            .h(px(44.0))
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
            .h(px(36.0))
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

    /// Render the model list (scrollable)
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_model_list(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let filtered = self.state.filtered_models();
        let providers = self.state.all_providers();

        div()
            .id("model-list")
            .flex_1()
            .w_full()
            .bg(Theme::bg_darkest())
            .overflow_y_scroll()
            .child(
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
    fn render_status_bar(&self) -> impl IntoElement {
        let filtered = self.state.filtered_models();
        let providers = self.state.all_providers();
        let model_count = filtered.len();
        let provider_count = providers.len();

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
    fn render_provider_dropdown(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let providers = self.state.all_providers();

        div()
            .id("provider-menu-overlay")
            .absolute()
            .top(px(80.0 + 28.0))
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
            .children(providers.into_iter().map(|p| {
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

impl gpui::EntityInputHandler for ModelSelectorView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<String> {
        let text = &self.state.search_query;
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
        let len16 = self.state.search_query.encode_utf16().count();
        Some(gpui::UTF16Selection {
            range: len16..len16,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<Range<usize>> {
        if self.ime_marked_byte_count > 0 {
            let q = &self.state.search_query;
            let len16: usize = q.encode_utf16().count();
            let start_utf8 = q.len().saturating_sub(self.ime_marked_byte_count);
            let start_utf16: usize = q[..start_utf8].encode_utf16().count();
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
        if self.ime_marked_byte_count > 0 {
            let len = self.state.search_query.len();
            self.state
                .search_query
                .truncate(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }
        if !text.is_empty() {
            self.state.search_query.push_str(text);
        }
        self.emit_filter_events_if_changed();
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
        if self.ime_marked_byte_count > 0 {
            let len = self.state.search_query.len();
            self.state
                .search_query
                .truncate(len.saturating_sub(self.ime_marked_byte_count));
            self.ime_marked_byte_count = 0;
        }
        if !new_text.is_empty() {
            self.state.search_query.push_str(new_text);
            self.ime_marked_byte_count = new_text.len();
        }
        self.emit_filter_events_if_changed();
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

impl gpui::Render for ModelSelectorView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let show_dropdown = self.state.show_provider_dropdown;

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
            // Top bar (44px)
            .child(Self::render_top_bar(cx))
            // Filter bar (36px)
            .child(self.render_filter_bar(cx))
            // Capability toggles (28px)
            .child(self.render_capability_toggles(cx))
            // Column header (20px)
            .child(Self::render_column_header())
            // Model list (flex, scrollable)
            .child(self.render_model_list(cx))
            // Status bar (24px)
            .child(self.render_status_bar());

        // Dropdown overlay isolation: when open, capture background clicks and close only.
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
                    .on_scroll_wheel(cx.listener(
                        |_this, _event: &ScrollWheelEvent, _window, cx| {
                            cx.stop_propagation();
                        },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.show_provider_dropdown = false;
                            cx.notify();
                        }),
                    )
                    .child(self.render_provider_dropdown(cx)),
            )
        } else {
            root
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use flume;
    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    fn remote_model(
        provider_id: &str,
        model_id: &str,
        context_length: Option<u32>,
    ) -> crate::presentation::view_command::ModelInfo {
        crate::presentation::view_command::ModelInfo {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            name: model_id.to_string(),
            context_length,
        }
    }

    #[test]
    fn model_info_formatting_and_state_filters_work() {
        let free = ModelInfo::new("claude", "anthropic")
            .with_context(200_000)
            .with_capabilities(true, false)
            .with_costs(0.0, 3.5);
        assert_eq!(free.context_display(), "200K");
        assert_eq!(ModelInfo::cost_display(0.0), "free");
        assert_eq!(ModelInfo::cost_display(3.0), "$3");
        assert_eq!(ModelInfo::cost_display(0.25), "$0.25");

        let vision = ModelInfo::new("gpt-4o", "openai")
            .with_context(1_000_000)
            .with_capabilities(false, true);
        assert_eq!(vision.context_display(), "1M");

        let mut state = ModelSelectorState::new();
        state.models = vec![free, vision];
        assert_eq!(state.filtered_models().len(), 2);

        state.selected_provider = Some("anthropic".to_string());
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider_id, "anthropic");

        state.selected_provider = None;
        state.search_query = "4o".to_string();
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "gpt-4o");

        state.search_query.clear();
        state.filter_reasoning = true;
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].reasoning);

        state.filter_reasoning = false;
        state.filter_vision = true;
        let filtered = state.filtered_models();
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].vision);

        assert_eq!(state.all_providers(), vec!["anthropic", "openai"]);
    }

    #[gpui::test]
    async fn handle_command_maps_models_and_filter_events_emit_only_on_changes(
        cx: &mut TestAppContext,
    ) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ModelSelectorView::new);

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.handle_command(
                ViewCommand::ModelSearchResults {
                    models: vec![
                        remote_model("anthropic", "claude-3-5-sonnet", Some(200_000)),
                        remote_model("openai", "gpt-4o", Some(128_000)),
                        remote_model("anthropic", "claude-haiku", None),
                    ],
                },
                cx,
            );

            assert_eq!(view.state.models.len(), 3);
            assert_eq!(view.state.providers.len(), 2);
            assert_eq!(view.state.models[0].id, "claude-3-5-sonnet");
            assert_eq!(view.state.models[0].context, 200_000);
            assert_eq!(view.state.models[2].context, 128_000);

            view.state.search_query = "claude".to_string();
            view.emit_filter_events_if_changed();
            view.emit_filter_events_if_changed();

            view.state.selected_provider = Some("anthropic".to_string());
            view.emit_filter_events_if_changed();
            view.emit_filter_events_if_changed();

            view.request_models();
            view.emit_search_models();
        });

        assert_eq!(
            user_rx.recv().expect("search event"),
            UserEvent::SearchModels {
                query: "claude".to_string(),
            }
        );
        assert_eq!(
            user_rx.recv().expect("provider filter event"),
            UserEvent::FilterModelsByProvider {
                provider_id: Some("anthropic".to_string()),
            }
        );
        assert_eq!(
            user_rx.recv().expect("open selector event"),
            UserEvent::OpenModelSelector
        );
        assert_eq!(
            user_rx.recv().expect("manual search emit"),
            UserEvent::SearchModels {
                query: "claude".to_string(),
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "duplicate filter events should not be emitted"
        );
    }

    #[gpui::test]
    async fn input_handler_mutates_search_query_and_marks_composition(cx: &mut TestAppContext) {
        let view = cx.new(ModelSelectorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.replace_text_in_range(None, "cla", window, cx);
                assert_eq!(view.state.search_query, "cla");
                assert_eq!(
                    view.text_for_range(0..2, &mut None, window, cx),
                    Some("cl".to_string())
                );

                view.replace_and_mark_text_in_range(None, "u", None, window, cx);
                assert_eq!(view.state.search_query, "clau");
                assert!(view.marked_text_range(window, cx).is_some());

                view.replace_text_in_range(None, "de", window, cx);
                assert_eq!(view.state.search_query, "clade");
                assert_eq!(view.marked_text_range(window, cx), None);

                let selected = view
                    .selected_text_range(false, window, cx)
                    .expect("selection range");
                let len = "clade".encode_utf16().count();
                assert_eq!(selected.range, len..len);

                view.unmark_text(window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
    }

    #[gpui::test]
    async fn provider_dropdown_selection_and_model_emission_follow_real_filter_rules(
        cx: &mut TestAppContext,
    ) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ModelSelectorView::new);

        view.update(cx, |view: &mut ModelSelectorView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.set_models(
                vec![
                    ProviderInfo::new("anthropic", "anthropic"),
                    ProviderInfo::new("openai", "openai"),
                ],
                vec![
                    ModelInfo::new("claude-3-7-sonnet", "anthropic")
                        .with_context(200_000)
                        .with_capabilities(true, false),
                    ModelInfo::new("gpt-4o", "openai")
                        .with_context(128_000)
                        .with_capabilities(false, true),
                ],
            );

            view.toggle_provider_dropdown(cx);
            assert!(view.get_state().show_provider_dropdown);

            view.select_provider_filter("anthropic".to_string(), cx);
            assert_eq!(
                view.get_state().selected_provider.as_deref(),
                Some("anthropic")
            );
            assert!(!view.get_state().show_provider_dropdown);
            assert_eq!(view.get_state().filtered_models().len(), 1);
            assert_eq!(view.get_state().filtered_models()[0].id, "claude-3-7-sonnet");

            view.toggle_reasoning_filter(cx);
            assert!(view.get_state().filter_reasoning);
            assert_eq!(view.get_state().filtered_models().len(), 1);

            view.toggle_vision_filter(cx);
            assert!(view.get_state().filter_vision);
            assert!(view.get_state().filtered_models().is_empty());

            view.clear_provider_filter(cx);
            assert_eq!(view.get_state().selected_provider, None);
            assert_eq!(view.get_state().filtered_models().len(), 0);

            view.toggle_vision_filter(cx);
            assert!(!view.get_state().filter_vision);
            assert_eq!(view.get_state().filtered_models().len(), 1);
            assert_eq!(view.get_state().filtered_models()[0].id, "claude-3-7-sonnet");

            view.select_model("anthropic".to_string(), "claude-3-7-sonnet".to_string());
            assert!(!view.get_state().show_provider_dropdown);
        });

        assert_eq!(
            user_rx.recv().expect("provider filter event"),
            UserEvent::FilterModelsByProvider {
                provider_id: Some("anthropic".to_string()),
            }
        );
        assert_eq!(
            user_rx.recv().expect("clear provider filter event"),
            UserEvent::FilterModelsByProvider { provider_id: None }
        );
        assert_eq!(
            user_rx.recv().expect("select model event"),
            UserEvent::SelectModel {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-7-sonnet".to_string(),
            }
        );
        assert!(user_rx.try_recv().is_err(), "unexpected additional selector events");
    }

    #[gpui::test]
    async fn key_handling_closes_dropdown_navigates_and_backspaces_search_once(
        cx: &mut TestAppContext,
    ) {
        while crate::ui_gpui::navigation_channel().take_pending().is_some() {}
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ModelSelectorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ModelSelectorView, cx| {
                view.set_bridge(Arc::clone(&bridge));
                view.state.show_provider_dropdown = true;
                view.state.search_query = "claude".to_string();
                view.state.last_emitted_search_query = "claude".to_string();

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("escape").expect("escape keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert!(!view.state.show_provider_dropdown);
                assert_eq!(crate::ui_gpui::navigation_channel().take_pending(), None);

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("backspace")
                            .expect("backspace keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert_eq!(view.state.search_query, "claud");

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("cmd-w").expect("cmd-w keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    cx,
                );
                assert_eq!(
                    crate::ui_gpui::navigation_channel().take_pending(),
                    Some(crate::presentation::view_command::ViewId::Settings)
                );

                view.replace_and_mark_text_in_range(None, "e", None, window, cx);
                assert_eq!(view.state.search_query, "claude");
                assert_eq!(view.marked_text_range(window, cx), Some(5..6));
                view.replace_text_in_range(None, "e-3", window, cx);
                assert_eq!(view.state.search_query, "claude-3");
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });

        assert_eq!(
            user_rx.recv().expect("backspace search event"),
            UserEvent::SearchModels {
                query: "claud".to_string(),
            }
        );
        assert_eq!(
            user_rx.recv().expect("composition search event"),
            UserEvent::SearchModels {
                query: "claude".to_string(),
            }
        );
        assert_eq!(
            user_rx.recv().expect("composition replace search event"),
            UserEvent::SearchModels {
                query: "claude-3".to_string(),
            }
        );
        assert!(user_rx.try_recv().is_err(), "unexpected additional key-handling events");
    }

}
