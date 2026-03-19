//! MCP Add View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P09
//! @requirement REQ-UI-MA

use gpui::{
    canvas, div, prelude::*, px, Bounds, ElementInputHandler, FocusHandle, FontWeight, MouseButton,
    Pixels, SharedString,
};
use std::ops::Range;
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Registry source for MCP search
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum McpRegistry {
    Official,
    Smithery,
    #[default]
    Both,
}

impl McpRegistry {
    #[must_use]
    pub const fn display(&self) -> &'static str {
        match self {
            Self::Official => "Official",
            Self::Smithery => "Smithery",
            Self::Both => "Both",
        }
    }
}

/// MCP search result item
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpSearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub registry: McpRegistry,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<Vec<(String, String)>>,
    pub source: String,
    /// Remote URL for HTTP/SSE transport MCPs (None for stdio-only).
    pub url: Option<String>,
}

impl McpSearchResult {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            registry: McpRegistry::Official,
            command: String::new(),
            args: vec![],
            env: None,
            source: "official".to_string(),
            url: None,
        }
    }

    #[must_use]
    pub const fn with_registry(mut self, registry: McpRegistry) -> Self {
        self.registry = registry;
        self
    }

    #[must_use]
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    #[must_use]
    pub fn with_env(mut self, env: Option<Vec<(String, String)>>) -> Self {
        self.env = env;
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    #[must_use]
    pub fn with_url(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }
}

/// Loading state for search
/// @plan PLAN-20250130-GPUIREDUX.P09
#[derive(Clone, Debug, PartialEq, Eq, Default)]
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if Next should be enabled
    #[must_use]
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
    ime_marked_byte_count: usize,
}

impl McpAddView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: McpAddState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
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
        if let Some(selected_id) = self.state.selected_result_id.clone() {
            let still_present = self.state.results.iter().any(|r| r.id == selected_id);
            if !still_present {
                self.state.selected_result_id = None;
            }
        }
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

    /// Set search query programmatically (for keyboard forwarding/tests)
    pub fn set_search_query(&mut self, query: String) {
        self.state.search_query = query;
    }

    /// Get current state for testing/forwarded key handling
    #[must_use]
    pub const fn get_state(&self) -> &McpAddState {
        &self.state
    }

    /// Emit `SearchMcpRegistry` for current search query and selected registry.
    pub fn emit_search_registry(&mut self) {
        let query = self.state.search_query.trim().to_string();
        if query.is_empty() {
            self.state.search_state = SearchState::Idle;
            return;
        }

        self.state.search_state = SearchState::Loading;

        let source_name = match self.state.registry {
            McpRegistry::Official => "official",
            McpRegistry::Smithery => "smithery",
            McpRegistry::Both => "both",
        }
        .to_string();

        self.emit(&UserEvent::SearchMcpRegistry {
            query,
            source: crate::events::types::McpRegistrySource { name: source_name },
        });
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P09
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
    /// @plan PLAN-20250130-GPUIREDUX.P09
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::McpConfigureDraftLoaded {
                id,
                name,
                package,
                env_var_name,
                command,
                args,
                env,
                url,
            } => {
                tracing::info!("MCP draft loaded for configure: {}", name);
                self.state.manual_entry =
                    format!("{} {}", command, args.join(" ")).trim().to_string();

                let (source_hint, normalized_id) = id.split_once("::").map_or_else(
                    || (None, id.clone()),
                    |(source, raw_id)| (Some(source.to_string()), raw_id.to_string()),
                );
                self.state.selected_result_id = Some(normalized_id.clone());

                let registry = match source_hint.as_deref() {
                    Some("smithery") => McpRegistry::Smithery,
                    Some("official") => McpRegistry::Official,
                    Some("both") => McpRegistry::Both,
                    _ => self.state.registry.clone(),
                };
                let inferred_source = source_hint.unwrap_or_else(|| match registry {
                    McpRegistry::Official => "official".to_string(),
                    McpRegistry::Smithery => "smithery".to_string(),
                    McpRegistry::Both => "both".to_string(),
                });

                self.state.results =
                    vec![McpSearchResult::new(normalized_id, name, "Selected MCP")
                        .with_registry(registry)
                        .with_command(package)
                        .with_args(args)
                        .with_env(env)
                        .with_source(inferred_source)
                        .with_url(url)];
                let _ = env_var_name;
                crate::ui_gpui::navigation_channel()
                    .request_navigate(crate::presentation::view_command::ViewId::McpConfigure);
            }
            ViewCommand::McpRegistrySearchResults { results } => {
                let mapped = results
                    .into_iter()
                    .map(|r| {
                        let registry = match r.source.as_str() {
                            "smithery" => McpRegistry::Smithery,
                            "both" => McpRegistry::Both,
                            _ => McpRegistry::Official,
                        };
                        McpSearchResult::new(r.id, r.name, r.description)
                            .with_registry(registry)
                            .with_command(r.command)
                            .with_args(r.args)
                            .with_env(r.env)
                            .with_source(r.source)
                            .with_url(r.url)
                    })
                    .collect();
                self.set_results(mapped);
            }
            ViewCommand::ShowError { message, .. } => {
                self.state.search_state = SearchState::Error(message);
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
                        .text_size(px(14.0))
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
                    .text_size(px(12.0))
                    .when(can_proceed, |d| {
                        d.cursor_pointer()
                            .bg(Theme::accent())
                            .hover(|s| s.bg(Theme::accent_hover()))
                            .text_color(gpui::white())
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
                                        this.emit(&UserEvent::McpAddNext);
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
            .child(Self::render_label("MANUAL ENTRY"))
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
                    .child(if self.state.manual_entry.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .child("npx @scope/package or docker image or URL")
                    } else {
                        div()
                            .text_color(Theme::text_primary())
                            .child(self.state.manual_entry.clone())
                    }),
            )
    }

    /// Render the "or search registry" divider
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_divider() -> impl IntoElement {
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
                    .child("or search registry"),
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
            .child(Self::render_label("REGISTRY"))
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
                    .child(div().text_color(Theme::text_muted()).child("v")),
            )
    }

    /// Render the search field
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_search_field(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("SEARCH"))
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
                    .child(if self.state.search_query.is_empty() {
                        div()
                            .text_color(Theme::text_muted())
                            .child("Search MCP servers...")
                    } else {
                        div()
                            .text_color(Theme::text_primary())
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

        div()
            .id(SharedString::from(format!("result-{id}")))
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    tracing::info!("Result selected: {}", id_for_closure);
                    this.state.selected_result_id = Some(id_for_closure.clone());
                    cx.notify();
                }),
            )
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
                            .text_color(if is_selected {
                                gpui::white()
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
                            .text_size(px(9.0))
                            .text_color(Theme::text_secondary())
                            .child(badge),
                    ),
            )
            // Second row: description
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(if is_selected {
                        Theme::text_primary()
                    } else {
                        Theme::text_secondary()
                    })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(description),
            )
            // Third row: source + command preview
            .child(
                div()
                    .text_size(px(9.0))
                    .text_color(if is_selected {
                        Theme::text_secondary()
                    } else {
                        Theme::text_muted()
                    })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(format!("{} · {}", source, result.command)),
            )
            .into_any_element()
    }

    /// Render the results list
    /// @plan PLAN-20250130-GPUIREDUX.P09
    fn render_results(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(Self::render_label("RESULTS"))
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
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_secondary())
                                .child("Searching..."),
                        )
                    })
                    // Empty state
                    .when(self.state.search_state == SearchState::Empty, |d| {
                        d.items_center().justify_center().p(px(16.0)).child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::text_secondary())
                                        .child(format!(
                                            "No MCPs found matching \"{}\".",
                                            self.state.search_query
                                        )),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(Theme::text_muted())
                                        .child("Try a different search term."),
                                ),
                        )
                    })
                    // Idle state (no search yet)
                    .when(self.state.search_state == SearchState::Idle, |d| {
                        d.items_center().justify_center().child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::text_muted())
                                .child("Enter a search term to find MCPs"),
                        )
                    })
                    // Results state
                    .when(self.state.search_state == SearchState::Results, |d| {
                        let results: Vec<gpui::AnyElement> = self
                            .state
                            .results
                            .iter()
                            .map(|r| self.render_result_row(r, cx))
                            .collect();
                        d.children(results)
                    })
                    // Error state
                    .when(
                        matches!(self.state.search_state, SearchState::Error(_)),
                        |d| {
                            let message = match &self.state.search_state {
                                SearchState::Error(msg) => msg.clone(),
                                _ => String::new(),
                            };
                            d.items_center().justify_center().p(px(16.0)).child(
                                div()
                                    .text_size(px(12.0))
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
            .overflow_hidden()
            .p(px(12.0))
            .flex()
            .flex_col()
            // Manual entry
            .child(self.render_manual_entry())
            // Divider
            .child(Self::render_divider())
            // Registry dropdown
            .child(self.render_registry_dropdown())
            // Search field
            .child(div().mt(px(12.0)).child(self.render_search_field()))
            // Results
            .child(div().mt(px(12.0)).child(self.render_results(cx)))
    }
}

impl gpui::Focusable for McpAddView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::EntityInputHandler for McpAddView {
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
        self.emit_search_registry();
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
        self.emit_search_registry();
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

impl gpui::Render for McpAddView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("mcp-add-view")
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
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;

                    // Escape or Cmd+W: Go back to Settings
                    if key == "escape" || (modifiers.platform && key == "w") {
                        crate::ui_gpui::navigation_channel()
                            .request_navigate(crate::presentation::view_command::ViewId::Settings);
                        return;
                    }

                    if modifiers.platform || modifiers.control {
                        return;
                    }

                    if key == "backspace" {
                        let mut q = this.state.search_query.clone();
                        q.pop(); // char-boundary safe unlike byte-level truncation
                        this.set_search_query(q);
                        this.emit_search_registry();
                        cx.notify();
                        return;
                    }

                    if key == "enter" {
                        this.emit_search_registry();
                        cx.notify();
                    }

                    // All other keys (printable chars) fall through to EntityInputHandler
                }),
            )
            // Top bar (44px)
            .child(self.render_top_bar(cx))
            // Content
            .child(self.render_content(cx))
    }
}
