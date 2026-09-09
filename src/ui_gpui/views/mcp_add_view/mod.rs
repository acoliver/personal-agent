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

/// Payload consumed by [`McpAddView::apply_configure_draft`]: the subset of
/// `ViewCommand::McpConfigureDraftLoaded` this view acts on, grouped so the
/// hand-off is one struct move instead of a positional argument pile.
struct ConfigureDraftPayload {
    id: String,
    name: String,
    package: String,
    package_type: crate::mcp::McpPackageType,
    runtime_hint: Option<String>,
    command: String,
    args: Vec<String>,
    env: Vec<(String, String, bool)>,
    url: Option<String>,
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
                if let Some(popped) = self.state.manual_entry.pop() {
                    // A marked composition tail shrinks with the field;
                    // leaving the counter stale desynchronizes the next
                    // IME replacement from the marked range.
                    self.ime_marked_byte_count =
                        self.ime_marked_byte_count.saturating_sub(popped.len_utf8());
                }
            }
            Some(ActiveField::SearchQuery) => {
                if let Some(popped) = self.state.search_query.pop() {
                    self.ime_marked_byte_count =
                        self.ime_marked_byte_count.saturating_sub(popped.len_utf8());
                }
                self.state.selected_result_id = None;
            }
            None => {}
        }
    }

    fn remove_trailing_bytes_from_active_field(&mut self, byte_count: usize) {
        if byte_count == 0 {
            return;
        }

        // IME APIs can hand back byte counts that split a multi-byte char;
        // walk the cut point back to the nearest char boundary before
        // truncating so arbitrary external input cannot panic here.
        let cut_marked_tail = |text: &mut String| {
            let mut target = text.len().saturating_sub(byte_count);
            while !text.is_char_boundary(target) {
                target -= 1;
            }
            text.truncate(target);
        };

        match self.state.active_field {
            Some(ActiveField::ManualEntry) => cut_marked_tail(&mut self.state.manual_entry),
            Some(ActiveField::SearchQuery) => {
                cut_marked_tail(&mut self.state.search_query);
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
        self.ime_marked_byte_count = 0;
        cx.notify();
    }

    fn select_result(&mut self, result_id: String, cx: &mut gpui::Context<Self>) {
        tracing::info!("Result selected: {}", result_id);
        self.state.selected_result_id = Some(result_id);
        self.state.active_field = None;
        self.ime_marked_byte_count = 0;
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

        if modifiers.platform && key == "v" {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    let sanitized = crate::ui_gpui::sanitize_single_line(&text);
                    if !sanitized.is_empty() && self.state.active_field.is_some() {
                        // A paste during active IME composition replaces the
                        // marked range, mirroring `replace_text_in_range`.
                        self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
                        self.ime_marked_byte_count = 0;
                        self.append_to_active_field(&sanitized);
                        if self.state.active_field == Some(ActiveField::SearchQuery) {
                            self.emit_search_registry();
                        }
                        cx.notify();
                    }
                }
            }
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
            self.ime_marked_byte_count = 0;
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

    /// Apply a configure-screen draft to search state and route to the
    /// configure view.
    fn apply_configure_draft(&mut self, payload: ConfigureDraftPayload) {
        let ConfigureDraftPayload {
            id,
            name,
            package,
            package_type,
            runtime_hint,
            command,
            args,
            env,
            url,
        } = payload;
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

        // The registry result shows plain env metadata; secret flags
        // are a configure-screen concern.
        let env_pairs = if env.is_empty() {
            None
        } else {
            Some(
                env.iter()
                    .map(|(name, value, _)| (name.clone(), value.clone()))
                    .collect(),
            )
        };

        self.state.results = vec![McpSearchResult::new(normalized_id, name, "Selected MCP")
            .with_registry(registry)
            .with_command(package)
            .with_args(args)
            .with_env(env_pairs)
            .with_source(inferred_source)
            .with_package_metadata(Some(package_type), runtime_hint)
            .with_url(url)];
        self.state.search_state = SearchState::Results;
        // Draft switch is a field change: no in-flight IME
        // composition can carry over.
        self.ime_marked_byte_count = 0;
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::McpConfigure);
    }

    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::McpConfigureDraftLoaded {
                id,
                name,
                package,
                package_type,
                runtime_hint,
                env_var_name: _,
                command,
                args,
                auth_type: _,
                oauth_connected: _,
                keyfile_path: _,
                env,
                stored_secret_names: _,
                url,
            } => {
                self.apply_configure_draft(ConfigureDraftPayload {
                    id,
                    name,
                    package,
                    package_type,
                    runtime_hint,
                    command,
                    args,
                    env,
                    url,
                });
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
mod tests;
