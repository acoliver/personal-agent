//! MCP Add View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P09
//! @requirement REQ-UI-MA

mod ime;
mod render;

use gpui::FocusHandle;
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;

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
    pub package_type: Option<crate::mcp::McpPackageType>,
    pub runtime_hint: Option<String>,
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
            package_type: None,
            runtime_hint: None,
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
    pub fn with_package_metadata(
        mut self,
        package_type: Option<crate::mcp::McpPackageType>,
        runtime_hint: Option<String>,
    ) -> Self {
        self.package_type = package_type;
        self.runtime_hint = runtime_hint;
        self
    }

    #[must_use]
    pub fn with_url(mut self, url: Option<String>) -> Self {
        self.url = url;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActiveField {
    ManualEntry,
    SearchQuery,
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
    pub(super) active_field: Option<ActiveField>,
    pub(super) show_registry_dropdown: bool,
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
    pub(super) state: McpAddState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) ime_marked_byte_count: usize,
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
        self.state.search_state = if self.filtered_results().is_empty() {
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
        self.state.selected_result_id = None;
    }

    pub fn set_manual_entry(&mut self, entry: String) {
        self.state.manual_entry = entry;
        if !self.state.manual_entry.trim().is_empty() {
            self.state.selected_result_id = None;
        }
    }

    /// Get current state for testing/forwarded key handling
    #[must_use]
    pub const fn get_state(&self) -> &McpAddState {
        &self.state
    }

    fn append_to_active_field(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match self.state.active_field {
            Some(ActiveField::ManualEntry) => self.state.manual_entry.push_str(text),
            Some(ActiveField::SearchQuery) => {
                self.state.search_query.push_str(text);
                self.state.selected_result_id = None;
            }
            None => {}
        }
    }

    fn backspace_active_field(&mut self) {
        match self.state.active_field {
            Some(ActiveField::ManualEntry) => {
                self.state.manual_entry.pop();
            }
            Some(ActiveField::SearchQuery) => {
                self.state.search_query.pop();
                self.state.selected_result_id = None;
            }
            None => {}
        }
    }

    fn remove_trailing_bytes_from_active_field(&mut self, byte_count: usize) {
        if byte_count == 0 {
            return;
        }

        match self.state.active_field {
            Some(ActiveField::ManualEntry) => {
                let len = self.state.manual_entry.len();
                self.state
                    .manual_entry
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::SearchQuery) => {
                let len = self.state.search_query.len();
                self.state
                    .search_query
                    .truncate(len.saturating_sub(byte_count));
                self.state.selected_result_id = None;
            }
            None => {}
        }
    }

    fn active_field_text(&self) -> &str {
        match self.state.active_field {
            Some(ActiveField::ManualEntry) => &self.state.manual_entry,
            Some(ActiveField::SearchQuery) => &self.state.search_query,
            None => "",
        }
    }

    fn select_registry(&mut self, registry: McpRegistry) {
        self.state.registry = registry;
        self.state.show_registry_dropdown = false;
        self.state.selected_result_id = None;
        if self.state.search_query.trim().is_empty() {
            self.state.search_state = SearchState::Idle;
            self.state.results.clear();
        } else {
            self.emit_search_registry();
        }
    }

    fn toggle_registry_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.show_registry_dropdown = !self.state.show_registry_dropdown;
        self.state.active_field = None;
        cx.notify();
    }

    fn select_result(&mut self, result_id: String, cx: &mut gpui::Context<Self>) {
        tracing::info!("Result selected: {}", result_id);
        self.state.selected_result_id = Some(result_id);
        self.state.active_field = None;
        self.state.manual_entry.clear();
        cx.notify();
    }

    fn handle_key_down(&mut self, event: &gpui::KeyDownEvent, cx: &mut gpui::Context<Self>) {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        if key == "escape" || (modifiers.platform && key == "w") {
            if self.state.show_registry_dropdown {
                self.state.show_registry_dropdown = false;
                cx.notify();
                return;
            }
            crate::ui_gpui::navigation_channel()
                .request_navigate(crate::presentation::view_command::ViewId::Settings);
            return;
        }

        if modifiers.platform || modifiers.control {
            return;
        }

        if key == "backspace" {
            self.backspace_active_field();
            if self.state.active_field == Some(ActiveField::SearchQuery) {
                self.emit_search_registry();
            }
            cx.notify();
            return;
        }

        if key == "enter" {
            if self.state.show_registry_dropdown {
                self.state.show_registry_dropdown = false;
            } else if self.state.active_field == Some(ActiveField::SearchQuery) {
                self.emit_search_registry();
            }
            cx.notify();
            return;
        }

        if key == "tab" {
            self.state.active_field = Some(match self.state.active_field {
                Some(ActiveField::ManualEntry) => ActiveField::SearchQuery,
                Some(ActiveField::SearchQuery) | None => ActiveField::ManualEntry,
            });
            self.state.show_registry_dropdown = false;
            cx.notify();
        }
    }

    fn filtered_results(&self) -> Vec<McpSearchResult> {
        let query = self.state.search_query.trim().to_lowercase();
        let matches_registry = |result: &McpSearchResult| match self.state.registry {
            McpRegistry::Both => true,
            McpRegistry::Official => result.registry == McpRegistry::Official,
            McpRegistry::Smithery => result.registry == McpRegistry::Smithery,
        };

        if query.is_empty() {
            return self
                .state
                .results
                .iter()
                .filter(|result| matches_registry(result))
                .cloned()
                .collect();
        }

        self.state
            .results
            .iter()
            .filter(|result| matches_registry(result))
            .filter(|result| {
                let haystack = [
                    result.name.as_str(),
                    result.description.as_str(),
                    result.command.as_str(),
                    result.source.as_str(),
                ]
                .join(" ")
                .to_lowercase();
                haystack.contains(&query)
            })
            .cloned()
            .collect()
    }

    fn command_preview(result: &McpSearchResult) -> String {
        if let Some(url) = &result.url {
            if !url.trim().is_empty() {
                return url.clone();
            }
        }

        if result.command.is_empty() {
            return String::new();
        }

        if result.args.is_empty() {
            return result.command.clone();
        }

        format!("{} {}", result.command, result.args.join(" "))
    }

    /// Emit `SearchMcpRegistry` for current search query and selected registry.
    pub fn emit_search_registry(&mut self) {
        let query = self.state.search_query.trim().to_string();
        if query.is_empty() {
            self.state.search_state = SearchState::Idle;
            self.state.results.clear();
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

    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::McpConfigureDraftLoaded {
                id,
                name,
                package,
                package_type,
                runtime_hint,
                env_var_name,
                command,
                args,
                env,
                url,
            } => {
                tracing::info!("MCP draft loaded for configure: {}", name);
                self.state.manual_entry = url.as_ref().map_or_else(
                    || {
                        if command.is_empty() {
                            package.clone()
                        } else if args.is_empty() {
                            command
                        } else {
                            format!("{command} {}", args.join(" ")).trim().to_string()
                        }
                    },
                    Clone::clone,
                );

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
                        .with_package_metadata(Some(package_type), runtime_hint)
                        .with_url(url)];
                self.state.search_state = SearchState::Results;
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
                            .with_package_metadata(r.package_type, r.runtime_hint)
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
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use flume;
    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    use crate::events::types::UserEvent;
    use crate::presentation::view_command::{ErrorSeverity, ViewCommand, ViewId};

    fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
    }

    fn clear_navigation_requests() {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
    }

    #[gpui::test]
    async fn emit_search_registry_trims_query_and_reports_registry_source(cx: &mut TestAppContext) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpAddView::new);

        view.update(cx, |view: &mut McpAddView, _cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.set_search_query("  fetch registry  ".to_string());
            view.state.registry = McpRegistry::Smithery;
            view.emit_search_registry();
        });

        assert_eq!(
            user_rx.recv().expect("search registry event"),
            UserEvent::SearchMcpRegistry {
                query: "fetch registry".to_string(),
                source: crate::events::types::McpRegistrySource {
                    name: "smithery".to_string(),
                },
            }
        );

        view.read_with(cx, |view, _| {
            assert_eq!(view.get_state().search_state, SearchState::Loading);
        });
    }

    #[gpui::test]
    async fn draft_loaded_preserves_transport_metadata_and_requests_configure_navigation(
        cx: &mut TestAppContext,
    ) {
        clear_navigation_requests();
        let view = cx.new(McpAddView::new);

        view.update(cx, |view: &mut McpAddView, cx| {
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: "smithery::fetch".to_string(),
                    name: "Fetch".to_string(),
                    package: "@smithery/fetch".to_string(),
                    package_type: crate::mcp::McpPackageType::Npm,
                    runtime_hint: Some("npx".to_string()),
                    env_var_name: "FETCH_API_KEY".to_string(),
                    command: "npx".to_string(),
                    args: vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-fetch".to_string(),
                    ],
                    env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
                    url: None,
                },
                cx,
            );
            assert_eq!(
                crate::ui_gpui::navigation_channel().take_pending(),
                Some(ViewId::McpConfigure)
            );
        });

        view.read_with(cx, |view, _| {
            let state = view.get_state();
            assert_eq!(
                state.manual_entry,
                "npx -y @modelcontextprotocol/server-fetch"
            );
            assert_eq!(state.selected_result_id.as_deref(), Some("fetch"));
            assert_eq!(state.search_state, SearchState::Results);
            assert_eq!(state.results.len(), 1);
            let result = &state.results[0];
            assert_eq!(result.registry, McpRegistry::Smithery);
            assert_eq!(result.source, "smithery");
            assert_eq!(result.command, "@smithery/fetch");
            assert_eq!(
                result.args,
                vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string()
                ]
            );
            assert_eq!(
                result.env,
                Some(vec![("FETCH_API_KEY".to_string(), String::new())])
            );
            assert_eq!(result.package_type, Some(crate::mcp::McpPackageType::Npm));
            assert_eq!(result.runtime_hint.as_deref(), Some("npx"));
            assert!(state.can_proceed());
        });
    }

    #[gpui::test]
    async fn registry_results_and_errors_update_search_state_without_source_loss(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(McpAddView::new);

        view.update(cx, |view: &mut McpAddView, cx| {
            view.set_search_query("fetch".to_string());
            view.handle_command(
                ViewCommand::McpRegistrySearchResults {
                    results: vec![
                        crate::presentation::view_command::McpRegistryResult {
                            id: "fetch".to_string(),
                            name: "Fetch".to_string(),
                            description: "HTTP fetch server".to_string(),
                            source: "smithery".to_string(),
                            command: "npx".to_string(),
                            args: vec![
                                "-y".to_string(),
                                "@modelcontextprotocol/server-fetch".to_string(),
                            ],
                            env: Some(vec![("FETCH_API_KEY".to_string(), String::new())]),
                            package_type: Some(crate::mcp::McpPackageType::Npm),
                            runtime_hint: Some("npx".to_string()),
                            url: None,
                        },
                        crate::presentation::view_command::McpRegistryResult {
                            id: "exa".to_string(),
                            name: "Exa".to_string(),
                            description: "Remote MCP".to_string(),
                            source: "official".to_string(),
                            command: String::new(),
                            args: vec![],
                            env: None,
                            package_type: Some(crate::mcp::McpPackageType::Http),
                            runtime_hint: None,
                            url: Some("https://exa.example/mcp".to_string()),
                        },
                    ],
                },
                cx,
            );

            assert_eq!(view.get_state().results.len(), 2);
            assert_eq!(view.get_state().search_state, SearchState::Results);
            assert_eq!(view.get_state().results[0].registry, McpRegistry::Smithery);
            assert_eq!(view.get_state().results[0].source, "smithery");
            assert_eq!(
                view.get_state().results[1].url.as_deref(),
                Some("https://exa.example/mcp")
            );

            view.handle_command(
                ViewCommand::ShowError {
                    title: "search failed".to_string(),
                    message: "registry unavailable".to_string(),
                    severity: ErrorSeverity::Warning,
                },
                cx,
            );
            assert_eq!(
                view.get_state().search_state,
                SearchState::Error("registry unavailable".to_string())
            );
        });
    }

    #[gpui::test]
    async fn manual_entry_registry_switch_and_empty_search_follow_real_state_rules(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpAddView::new);

        view.update(cx, |view: &mut McpAddView, _cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.set_results(vec![
                McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                    .with_registry(McpRegistry::Official)
                    .with_command("npx")
                    .with_args(vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-fetch".to_string(),
                    ]),
                McpSearchResult::new("exa", "Exa", "Remote search")
                    .with_registry(McpRegistry::Smithery)
                    .with_url(Some("https://exa.example/mcp".to_string())),
            ]);
            view.state.selected_result_id = Some("fetch".to_string());
            assert!(view.get_state().can_proceed());

            view.set_manual_entry(" custom-mcp ".to_string());
            assert_eq!(view.get_state().manual_entry, " custom-mcp ");
            assert_eq!(view.get_state().selected_result_id, None);
            assert!(view.get_state().can_proceed());

            view.set_search_query("exa".to_string());
            view.state.selected_result_id = Some("exa".to_string());
            view.select_registry(McpRegistry::Official);
            assert_eq!(view.get_state().registry, McpRegistry::Official);
            assert_eq!(view.get_state().selected_result_id, None);
            assert_eq!(view.get_state().search_state, SearchState::Loading);

            view.set_loading(false);
            assert_eq!(view.get_state().search_state, SearchState::Results);

            view.set_search_query("   ".to_string());
            view.select_registry(McpRegistry::Both);
            assert_eq!(view.get_state().registry, McpRegistry::Both);
            assert!(view.get_state().results.is_empty());
            assert_eq!(view.get_state().search_state, SearchState::Idle);
        });

        assert_eq!(
            user_rx.recv().expect("registry switch search"),
            UserEvent::SearchMcpRegistry {
                query: "exa".to_string(),
                source: crate::events::types::McpRegistrySource {
                    name: "official".to_string(),
                },
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional MCP events"
        );
    }

    #[gpui::test]
    async fn set_results_filters_selection_and_command_preview_handles_remote_urls(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(McpAddView::new);

        view.update(cx, |view: &mut McpAddView, _cx| {
            view.set_search_query("exa".to_string());
            view.set_results(vec![
                McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                    .with_registry(McpRegistry::Official)
                    .with_command("npx")
                    .with_args(vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-fetch".to_string(),
                    ]),
                McpSearchResult::new("exa", "Exa", "Remote search")
                    .with_registry(McpRegistry::Smithery)
                    .with_command("npx")
                    .with_args(vec!["-y".to_string(), "exa-mcp".to_string()])
                    .with_url(Some("https://exa.example/mcp".to_string())),
            ]);
            view.state.selected_result_id = Some("exa".to_string());
            view.state.registry = McpRegistry::Smithery;
            assert_eq!(view.filtered_results().len(), 1);
            assert_eq!(view.filtered_results()[0].id, "exa");
            assert_eq!(
                McpAddView::command_preview(&view.filtered_results()[0]),
                "https://exa.example/mcp"
            );

            view.set_results(vec![McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                .with_registry(McpRegistry::Official)
                .with_command("npx")
                .with_args(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string(),
                ])]);
            assert_eq!(view.get_state().selected_result_id, None);
            assert_eq!(view.get_state().search_state, SearchState::Empty);

            view.set_search_query("fetch".to_string());
            view.state.registry = McpRegistry::Official;
            view.set_results(vec![McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                .with_registry(McpRegistry::Official)
                .with_command("npx")
                .with_args(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string(),
                ])]);
            assert_eq!(view.get_state().search_state, SearchState::Results);
            assert_eq!(view.filtered_results().len(), 1);
            assert_eq!(
                McpAddView::command_preview(&view.filtered_results()[0]),
                "npx -y @modelcontextprotocol/server-fetch"
            );
        });
    }

    fn key_event(key: &str) -> gpui::KeyDownEvent {
        gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
            is_held: false,
            prefer_character_input: false,
        }
    }

    fn setup_view_with_results(view: &mut McpAddView, bridge: &Arc<GpuiBridge>) {
        view.set_bridge(Arc::clone(bridge));
        view.set_results(vec![
            McpSearchResult::new("fetch", "Fetch", "HTTP fetch")
                .with_registry(McpRegistry::Official)
                .with_command("npx")
                .with_args(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-fetch".to_string(),
                ]),
            McpSearchResult::new("exa", "Exa", "Remote search")
                .with_registry(McpRegistry::Smithery)
                .with_url(Some("https://exa.example/mcp".to_string())),
        ]);
    }

    #[gpui::test]
    async fn text_input_and_ime_handling_emits_search_events(cx: &mut TestAppContext) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpAddView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpAddView, cx| {
                setup_view_with_results(view, &bridge);

                view.state.active_field = Some(ActiveField::ManualEntry);
                view.handle_key_down(&key_event("tab"), cx);
                assert_eq!(
                    view.get_state().active_field,
                    Some(ActiveField::SearchQuery)
                );

                view.replace_text_in_range(None, "exa", window, cx);
                assert_eq!(view.get_state().search_query, "exa");
                assert_eq!(
                    view.text_for_range(0..2, &mut None, window, cx),
                    Some("ex".to_string())
                );

                view.replace_and_mark_text_in_range(None, "!", None, window, cx);
                assert_eq!(view.get_state().search_query, "exa!");
                assert_eq!(view.marked_text_range(window, cx), Some(3..4));
                view.replace_text_in_range(None, "-mcp", window, cx);
                assert_eq!(view.get_state().search_query, "exa-mcp");
                assert_eq!(view.marked_text_range(window, cx), None);

                let selected = view
                    .selected_text_range(false, window, cx)
                    .expect("selection range");
                let len = "exa-mcp".encode_utf16().count();
                assert_eq!(selected.range, len..len);
            });
        });

        let expected_queries = ["exa", "exa!", "exa-mcp"];
        for query in expected_queries {
            assert_eq!(
                user_rx.recv().expect("search registry event"),
                UserEvent::SearchMcpRegistry {
                    query: query.to_string(),
                    source: crate::events::types::McpRegistrySource {
                        name: "both".to_string(),
                    },
                }
            );
        }
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional search registry events"
        );
    }

    #[gpui::test]
    async fn dropdown_select_and_navigation_keys_behave_correctly(cx: &mut TestAppContext) {
        let (bridge, _user_rx) = make_bridge();
        let view = cx.new(McpAddView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|_window, app| {
            view.update(app, |view: &mut McpAddView, cx| {
                setup_view_with_results(view, &bridge);
                view.state.search_query = "exa-mcp".to_string();

                view.state.show_registry_dropdown = true;
                view.handle_key_down(&key_event("enter"), cx);
                assert!(!view.get_state().show_registry_dropdown);

                view.toggle_registry_dropdown(cx);
                assert!(view.get_state().show_registry_dropdown);
                // flush any navigation requests from concurrent tests before asserting None
                clear_navigation_requests();
                view.handle_key_down(&key_event("escape"), cx);
                assert!(!view.get_state().show_registry_dropdown);
                assert_eq!(crate::ui_gpui::navigation_channel().take_pending(), None);

                view.select_result("exa".to_string(), cx);
                assert_eq!(view.get_state().selected_result_id.as_deref(), Some("exa"));
                assert!(view.get_state().manual_entry.is_empty());

                view.handle_key_down(&key_event("backspace"), cx);
                assert_eq!(view.get_state().search_query, "exa-mcp");
                assert_eq!(view.get_state().selected_result_id, Some("exa".to_string()));

                clear_navigation_requests();
                view.handle_key_down(&key_event("cmd-w"), cx);
                assert_eq!(
                    crate::ui_gpui::navigation_channel().take_pending(),
                    Some(ViewId::Settings)
                );
            });
        });
    }
}
