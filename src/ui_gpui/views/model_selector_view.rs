//! Model Selector View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P07
//! @requirement REQ-UI-MS

use gpui::{div, px, prelude::*, SharedString, MouseButton, FocusHandle, FontWeight};
use std::sync::Arc;

use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;

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
            context: 128000,
            reasoning: false,
            vision: false,
            cost_input: 0.0,
            cost_output: 0.0,
        }
    }

    pub fn with_context(mut self, context: u64) -> Self {
        self.context = context;
        self
    }

    pub fn with_capabilities(mut self, reasoning: bool, vision: bool) -> Self {
        self.reasoning = reasoning;
        self.vision = vision;
        self
    }

    pub fn with_costs(mut self, input: f64, output: f64) -> Self {
        self.cost_input = input;
        self.cost_output = output;
        self
    }

    /// Format context for display (e.g., "128K", "1M")
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
    pub fn cost_display(cost: f64) -> String {
        if cost == 0.0 {
            "free".to_string()
        } else if cost == cost.floor() {
            format!("${}", cost as i64)
        } else {
            format!("${:.2}", cost)
        }
    }
}

/// Provider information
/// @plan PLAN-20250130-GPUIREDUX.P07
#[derive(Clone, Debug, PartialEq)]
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
}

impl ModelSelectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get filtered models based on current filters
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
                        && !m.provider_id.to_lowercase().contains(&query) {
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
    pub fn all_providers(&self) -> Vec<&str> {
        let mut providers: Vec<&str> = self
            .models
            .iter()
            .map(|m| m.provider_id.as_str())
            .collect();
        providers.sort();
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
}

impl ModelSelectorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ModelSelectorState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
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
    
    /// Get current state for testing
    pub fn get_state(&self) -> &ModelSelectorState {
        &self.state
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn emit(&self, event: UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Handle ViewCommand from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P07
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack => {
                // Navigation handled by MainPanel
            }
            ViewCommand::ModelSearchResults { models } => {
                println!(">>> ModelSelectorView::handle_command received {} models <<<", models.len());
                tracing::info!("ModelSelectorView received {} models", models.len());
                
                // Extract unique providers from the models
                let mut provider_set = std::collections::HashSet::new();
                for m in &models {
                    provider_set.insert(m.provider_id.clone());
                }
                let providers: Vec<ProviderInfo> = provider_set
                    .into_iter()
                    .map(|id| ProviderInfo { id: id.clone(), name: id })
                    .collect();
                    
                println!(">>> Providers extracted: {} <<<", providers.len());
                    
                let local_models: Vec<ModelInfo> = models
                    .into_iter()
                    .map(|m| ModelInfo {
                        id: m.model_id,
                        provider_id: m.provider_id,
                        context: m.context_length.unwrap_or(128000) as u64,
                        reasoning: false,
                        vision: false,
                        cost_input: 0.0,
                        cost_output: 0.0,
                    })
                    .collect();
                
                println!(">>> Setting {} models on view <<<", local_models.len());
                self.set_models(providers, local_models);
                println!(">>> Models set, state.models.len() = {} <<<", self.state.models.len());
            }
            _ => {}
        }
        cx.notify();
    }
    
    /// Request models from presenter on view open
    pub fn request_models(&self) {
        tracing::info!("ModelSelectorView requesting models");
        self.emit(UserEvent::OpenModelSelector);
    }

    /// Render the top bar with cancel button and title
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
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
                    .text_color(Theme::text_secondary())
                    .child("Cancel")
                    .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                        crate::ui_gpui::navigation_channel().request_navigate(
                            crate::presentation::view_command::ViewId::Settings
                        );
                    }))
            )
            // Center: Title
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("Select Model")
                    )
            )
            // Right: spacer for balance
            .child(div().w(px(70.0)))
    }

    /// Render the filter bar with search and provider dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_filter_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let search_query = self.state.search_query.clone();
        let provider_display = self.state.selected_provider.clone().unwrap_or_else(|| "All".to_string());
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
                    .child(
                        if search_query.is_empty() {
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("Search models...")
                        } else {
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_primary())
                                .child(search_query)
                        }
                    )
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
                            .text_color(Theme::text_secondary())
                            .child("Provider:")
                    )
                    .child(
                        div()
                            .id("provider-dropdown")
                            .w(px(100.0))
                            .h(px(28.0))
                            .px(px(8.0))
                            .bg(Theme::bg_dark())
                            .border_1()
                            .border_color(if show_dropdown { Theme::accent() } else { Theme::border() })
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .text_size(px(11.0))
                            .text_color(Theme::text_primary())
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                this.state.show_provider_dropdown = !this.state.show_provider_dropdown;
                                cx.notify();
                            }))
                            .child(provider_display)
                            .child(
                                div()
                                    .text_color(Theme::text_muted())
                                    .child(if show_dropdown { "^" } else { "v" })
                            )
                    )
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
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        this.state.filter_reasoning = !this.state.filter_reasoning;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(14.0))
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(2.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(filter_reasoning, |d| d.bg(Theme::accent()).child(
                                div().text_size(px(10.0)).text_color(gpui::white()).child("[OK]")
                            ))
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_secondary())
                            .child("Reasoning")
                    )
            )
            // Vision checkbox
            .child(
                div()
                    .id("filter-vision")
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        this.state.filter_vision = !this.state.filter_vision;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(14.0))
                            .border_1()
                            .border_color(Theme::border())
                            .rounded(px(2.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(filter_vision, |d| d.bg(Theme::accent()).child(
                                div().text_size(px(10.0)).text_color(gpui::white()).child("[OK]")
                            ))
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::text_secondary())
                            .child("Vision")
                    )
            )
    }

    /// Render column header
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_column_header(&self) -> impl IntoElement {
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
            .text_color(Theme::text_secondary())
            .child(
                div()
                    .flex_1()
                    .child("Model")
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .child("Context")
            )
            .child(
                div()
                    .w(px(40.0))
                    .flex()
                    .justify_center()
                    .child("Caps")
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .child("In $")
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .child("Out $")
            )
    }

    /// Render a single model row
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_model_row(&self, model: &ModelInfo, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let model_id = model.id.clone();
        let provider_id = model.provider_id.clone();
        let context = model.context_display();
        let caps = format!("{}{}",
            if model.reasoning { "R" } else { "" },
            if model.vision { "V" } else { "" }
        );
        let cost_in = ModelInfo::cost_display(model.cost_input);
        let cost_out = ModelInfo::cost_display(model.cost_output);

        div()
            .id(SharedString::from(format!("model-{}-{}", provider_id, model_id)))
            .h(px(28.0))
            .w_full()
            .px(px(12.0))
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(11.0))
            .text_color(Theme::text_primary())
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, _cx| {
                println!(">>> Model selected: {} from {} <<<", model_id, provider_id);
                this.emit(UserEvent::SelectModel {
                    provider_id: provider_id.clone(),
                    model_id: model_id.clone(),
                });
                // Navigate to profile editor
                crate::ui_gpui::navigation_channel().request_navigate(
                    crate::presentation::view_command::ViewId::ProfileEditor
                );
            }))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(model.id.clone())
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .text_color(Theme::text_secondary())
                    .child(context)
            )
            .child(
                div()
                    .w(px(40.0))
                    .flex()
                    .justify_center()
                    .text_color(Theme::text_muted())
                    .child(caps)
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .text_color(Theme::text_secondary())
                    .child(cost_in)
            )
            .child(
                div()
                    .w(px(50.0))
                    .flex()
                    .justify_end()
                    .text_color(Theme::text_secondary())
                    .child(cost_out)
            )
    }

    /// Render a provider section header
    /// @plan PLAN-20250130-GPUIREDUX.P07
    fn render_provider_header(&self, provider_id: &str) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("provider-header-{}", provider_id)))
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
                div()
                    .flex()
                    .flex_col()
                    .children(
                        providers.iter().filter_map(|provider| {
                            let provider_models: Vec<_> = filtered
                                .iter()
                                .filter(|m| &m.provider_id == *provider)
                                .collect();
                            
                            if provider_models.is_empty() {
                                return None;
                            }
                            
                            let provider_id = provider.to_string();
                            Some(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(self.render_provider_header(&provider_id))
                                    .children(
                                        provider_models.into_iter().map(|model| {
                                            self.render_model_row(model, cx)
                                        }).collect::<Vec<_>>()
                                    )
                            )
                        }).collect::<Vec<_>>()
                    )
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
            .text_color(Theme::text_secondary())
            .child(format!("{} models from {} providers", model_count, provider_count))
    }
    
    /// Render the provider dropdown overlay
    fn render_provider_dropdown(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let providers = self.state.all_providers();
        
        div()
            .id("provider-menu-overlay")
            .absolute()
            .top(px(80.0 + 28.0))
            .right(px(12.0))
            .w(px(150.0))
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
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        this.state.selected_provider = None;
                        this.state.show_provider_dropdown = false;
                        cx.notify();
                    }))
                    .child("All")
            )
            // Provider options
            .children(
                providers.into_iter().map(|p| {
                    let provider_id = p.to_string();
                    let provider_name = p.to_string();
                    div()
                        .id(SharedString::from(format!("provider-{}", provider_id)))
                        .px(px(8.0))
                        .py(px(6.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::bg_darker()))
                        .text_size(px(11.0))
                        .text_color(Theme::text_primary())
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, cx| {
                            this.state.selected_provider = Some(provider_id.clone());
                            this.state.show_provider_dropdown = false;
                            cx.notify();
                        }))
                        .child(provider_name)
                }).collect::<Vec<_>>()
            )
    }
}

impl gpui::Focusable for ModelSelectorView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for ModelSelectorView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let show_dropdown = self.state.show_provider_dropdown;
        
        div()
            .id("model-selector-view")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;
                
                // Escape: close dropdown or go back
                if key == "escape" {
                    if this.state.show_provider_dropdown {
                        this.state.show_provider_dropdown = false;
                        cx.notify();
                    } else {
                        crate::ui_gpui::navigation_channel().request_navigate(
                            crate::presentation::view_command::ViewId::Settings
                        );
                    }
                    return;
                }
                
                // Cmd+W: Go back to Settings
                if modifiers.platform && key == "w" {
                    crate::ui_gpui::navigation_channel().request_navigate(
                        crate::presentation::view_command::ViewId::Settings
                    );
                    return;
                }
                
                // Handle typing for search - any alphanumeric key updates search
                if !modifiers.platform && !modifiers.control {
                    if key == "backspace" {
                        this.state.search_query.pop();
                        cx.notify();
                    } else if key.len() == 1 {
                        this.state.search_query.push_str(key);
                        cx.notify();
                    }
                }
            }))
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Filter bar (36px)
            .child(self.render_filter_bar(cx))
            // Capability toggles (28px)
            .child(self.render_capability_toggles(cx))
            // Column header (20px)
            .child(self.render_column_header())
            // Model list (flex, scrollable)
            .child(self.render_model_list(cx))
            // Status bar (24px)
            .child(self.render_status_bar())
            // Provider dropdown overlay - rendered last so it's on top
            .when(show_dropdown, |d| {
                d.child(self.render_provider_dropdown(cx))
            })
    }
}
