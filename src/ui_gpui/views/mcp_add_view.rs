//! MCP Add View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P09
//! @requirement REQ-UI-MA

use gpui::{div, px, prelude::*, SharedString, MouseButton, FocusHandle, FontWeight};
use std::sync::Arc;

use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;

/// Registry source for MCP search
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Debug, PartialEq, Default)]
pub enum McpRegistry {
    Official,
    Smithery,
    #[default]
    Both,
}

impl McpRegistry {
    pub fn display(&self) -> &'static str {
        match self {
            McpRegistry::Official => "Official",
            McpRegistry::Smithery => "Smithery",
            McpRegistry::Both => "Both",
        }
    }
}

/// MCP search result item
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Debug, PartialEq)]
pub struct McpSearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub registry: McpRegistry,
    pub command: String,
}

impl McpSearchResult {
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            registry: McpRegistry::Official,
            command: String::new(),
        }
    }

    pub fn with_registry(mut self, registry: McpRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }
}

/// Loading state for search
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Debug, PartialEq, Default)]
pub enum SearchState {
    #[default]
    Idle,
    Loading,
    Results,
    Empty,
    Error(String),
}

/// MCP Add view state
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Default)]
pub struct McpAddState {
    pub manual_entry: String,
    pub registry: McpRegistry,
    pub search_query: String,
    pub search_state: SearchState,
    pub results: Vec<McpSearchResult>,
    pub selected_result_id: Option<String>,
}

impl McpAddState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if Next should be enabled
    pub fn can_proceed(&self) -> bool {
        !self.manual_entry.trim().is_empty() || self.selected_result_id.is_some()
    }
}

/// MCP Add view component
/// @plan PLAN-20250130-GPUIREDUX.P09
pub struct McpAddView {
    state: McpAddState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
}

impl McpAddView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: McpAddState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P09
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set search results from presenter
    pub fn set_results(&mut self, results: Vec<McpSearchResult>) {
        self.state.results = results;
        self.state.search_state = if self.state.results.is_empty() {
            SearchState::Empty
        } else {
            SearchState::Results
        };
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.state.search_state = if loading {
            SearchState::Loading
        } else if self.state.results.is_empty() {
            SearchState::Idle
        } else {
            SearchState::Results
        };
    }

    /// Emit a UserEvent through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P09
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
    /// @plan PLAN-20250130-GPUIREDUX.P09
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::NavigateTo { .. } | ViewCommand::NavigateBack => {
                // Navigation handled by MainPanel
            }
            _ => {}
        }
        cx.notify();
    }

    /// Render the top bar with cancel, title, and next
    /// @plan PLAN-20250130-GPUIREDUX.P09
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
                    .text_size(px(12.0))
                    .text_color(Theme::text_secondary())
                    .child("Cancel")
                    .on_mouse_down(MouseButton::Left, cx.listener(|_this, _, _window, _cx| {
                        tracing::info!("Cancel clicked - navigating to Settings");
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
                            .child("Add MCP")
                    )
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
                    .text_size(px(12.0))
                    .when(can_proceed, |d| {
                        d.cursor_pointer()
                            .bg(Theme::accent())
                            .hover(|s| s.bg(Theme::accent_hover()))
                            .text_color(gpui::white())
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                                tracing::info!("Next clicked - navigating to McpConfigure");
                                this.emit(UserEvent::McpAddNext);
                                crate::ui_gpui::navigation_channel().request_navigate(
                                    crate::presentation::view_command::ViewId::McpConfigure
                                );
                            }))
                    })
                    .when(!can_proceed, |d| {
                        d.bg(Theme::bg_dark())
                            .text_color(Theme::text_muted())
                    })
                    .child("Next")
            )
    }

    /// Render a field label
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_label(&self, text: &str) -> impl IntoElement {
        div()
            .text_size(px(11.0))
            .text_color(Theme::text_secondary())
            .mb(px(4.0))
            .child(text.to_string())
    }

    /// Render the manual entry field
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_manual_entry(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("MANUAL ENTRY"))
            .child(
                div()
                    .id("field-manual-entry")
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .child(
                        if self.state.manual_entry.is_empty() {
                            div()
                                .text_color(Theme::text_muted())
                                .child("npx @scope/package or docker image or URL")
                        } else {
                            div()
                                .text_color(Theme::text_primary())
                                .child(self.state.manual_entry.clone())
                        }
                    )
            )
    }

    /// Render the "or search registry" divider
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_divider(&self) -> impl IntoElement {
        div()
            .w(px(360.0))
            .my(px(16.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(div().flex_1().h(px(1.0)).bg(Theme::border()))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::text_muted())
                    .child("or search registry")
            )
            .child(div().flex_1().h(px(1.0)).bg(Theme::border()))
    }

    /// Render the registry dropdown
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_registry_dropdown(&self) -> impl IntoElement {
        let registry = self.state.registry.display();

        div()
            .flex()
            .flex_col()
            .child(self.render_label("REGISTRY"))
            .child(
                div()
                    .id("dropdown-registry")
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_size(px(12.0))
                    .text_color(Theme::text_primary())
                    .child(registry)
                    .child(div().text_color(Theme::text_muted()).child("v"))
            )
    }

    /// Render the search field
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_search_field(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("SEARCH"))
            .child(
                div()
                    .id("field-search")
                    .w(px(360.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .child(
                        if self.state.search_query.is_empty() {
                            div()
                                .text_color(Theme::text_muted())
                                .child("Search MCP servers...")
                        } else {
                            div()
                                .text_color(Theme::text_primary())
                                .child(self.state.search_query.clone())
                        }
                    )
            )
    }

    /// Render a search result row
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_result_row(&self, result: &McpSearchResult, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let id = result.id.clone();
        let id_for_closure = id.clone();
        let is_selected = self.state.selected_result_id.as_ref() == Some(&id);
        let name = result.name.clone();
        let description = result.description.clone();
        let badge = result.registry.display();

        div()
            .id(SharedString::from(format!("result-{}", id)))
            .w_full()
            .h(px(48.0))
            .px(px(8.0))
            .py(px(4.0))
            .cursor_pointer()
            .when(is_selected, |d| d.bg(Theme::accent()))
            .when(!is_selected, |d| d.hover(|s| s.bg(Theme::bg_dark())))
            .flex()
            .flex_col()
            .justify_center()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _window, cx| {
                tracing::info!("Result selected: {}", id_for_closure);
                this.state.selected_result_id = Some(id_for_closure.clone());
                cx.notify();
            }))
            // First row: name + badge
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_selected { gpui::white() } else { Theme::text_primary() })
                            .child(name)
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(if is_selected { Theme::bg_darker() } else { Theme::bg_dark() })
                            .text_size(px(9.0))
                            .text_color(Theme::text_secondary())
                            .child(badge)
                    )
            )
            // Second row: description
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(if is_selected { Theme::text_primary() } else { Theme::text_secondary() })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(description)
            )
            .into_any_element()
    }

    /// Render the results list
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_results(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(self.render_label("RESULTS"))
            .child(
                div()
                    .id("results-list")
                    .w(px(360.0))
                    .h(px(200.0))
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Loading state
                    .when(self.state.search_state == SearchState::Loading, |d| {
                        d.items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::text_secondary())
                                    .child("Searching...")
                            )
                    })
                    // Empty state
                    .when(self.state.search_state == SearchState::Empty, |d| {
                        d.items_center()
                            .justify_center()
                            .p(px(16.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(Theme::text_secondary())
                                            .child(format!("No MCPs found matching \"{}\".", self.state.search_query))
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(Theme::text_muted())
                                            .child("Try a different search term.")
                                    )
                            )
                    })
                    // Idle state (no search yet)
                    .when(self.state.search_state == SearchState::Idle, |d| {
                        d.items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::text_muted())
                                    .child("Enter a search term to find MCPs")
                            )
                    })
                    // Results state
                    .when(self.state.search_state == SearchState::Results, |d| {
                        let results: Vec<gpui::AnyElement> = self.state.results
                            .iter()
                            .map(|r| self.render_result_row(r, cx))
                            .collect();
                        d.children(results)
                    })
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
            .overflow_hidden()
            .p(px(12.0))
            .flex()
            .flex_col()
            // Manual entry
            .child(self.render_manual_entry())
            // Divider
            .child(self.render_divider())
            // Registry dropdown
            .child(self.render_registry_dropdown())
            // Search field
            .child(
                div()
                    .mt(px(12.0))
                    .child(self.render_search_field())
            )
            // Results
            .child(
                div()
                    .mt(px(12.0))
                    .child(self.render_results(cx))
            )
    }
}

impl gpui::Focusable for McpAddView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for McpAddView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("mcp-add-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_base())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|_this, event: &gpui::KeyDownEvent, _window, _cx| {
                let key = &event.keystroke.key;
                let modifiers = &event.keystroke.modifiers;
                
                // Escape or Cmd+W: Go back to Settings
                if key == "escape" || (modifiers.platform && key == "w") {
                    println!(">>> Escape/Cmd+W pressed - navigating to Settings <<<");
                    crate::ui_gpui::navigation_channel().request_navigate(
                        crate::presentation::view_command::ViewId::Settings
                    );
                }
            }))
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content
            .child(self.render_content(cx))
    }
}
