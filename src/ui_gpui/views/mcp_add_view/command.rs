//! Command handling for `McpAddView`.

use super::{McpAddView, McpRegistry, McpSearchResult, SearchState};
use crate::presentation::view_command::ViewCommand;

impl McpAddView {
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
