//! Registry search helpers for MCP add view.

use std::sync::Mutex;

use objc2::rc::Retained;
use objc2_app_kit::NSPopUpButton;
use objc2_foundation::{NSNotificationCenter, NSString};

use personal_agent::mcp::registry::{McpRegistry, McpRegistryServerWrapper, McpRegistrySource};

use crate::ui::mcp_add_helpers::log_to_file;

pub static SEARCH_RESULTS: Mutex<Option<Vec<McpRegistryServerWrapper>>> = Mutex::new(None);

pub struct SearchContext {
    pub registry_source: McpRegistrySource,
}

impl SearchContext {
    pub fn from_popup(popup: &std::cell::RefCell<Option<Retained<NSPopUpButton>>>) -> Self {
        let selected_index = popup
            .borrow()
            .as_ref()
            .map(|popup| popup.indexOfSelectedItem())
            .unwrap_or(0);

        let registry_source = match selected_index {
            2 => McpRegistrySource::Smithery,
            _ => McpRegistrySource::Official,
        };

        Self { registry_source }
    }

    pub fn load_smithery_key(&self) -> Option<String> {
        if self.registry_source != McpRegistrySource::Smithery {
            return None;
        }

        let config_path = match personal_agent::config::Config::default_path() {
            Ok(path) => path,
            Err(e) => {
                log_to_file(&format!("ERROR: Failed to get config path: {e}"));
                return None;
            }
        };

        match personal_agent::config::Config::load(&config_path) {
            Ok(config) => config.smithery_auth.clone(),
            Err(e) => {
                log_to_file(&format!("ERROR: Failed to load config: {e}"));
                None
            }
        }
    }

    pub fn spawn_search(&self, query: String, smithery_key: Option<String>) {
        let registry_source = self.registry_source;
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to create runtime: {e}");
                    return;
                }
            };

            let registry = McpRegistry::new();
            let results = runtime.block_on(async {
                registry
                    .search_registry(&query, registry_source, smithery_key.as_deref())
                    .await
            });

            match results {
                Ok(search_results) => {
                    log_to_file(&format!(
                        "Search found {} results",
                        search_results.entries.len()
                    ));

                    if let Ok(mut guard) = SEARCH_RESULTS.lock() {
                        *guard = Some(search_results.entries.clone());
                    }

                    SearchResults::notify_complete();
                }
                Err(e) => {
                    eprintln!("Search failed: {e}");
                    SearchResults::notify_error();
                }
            }
        });
    }
}

pub struct SearchResults;

impl SearchResults {
    pub fn clear() {
        if let Ok(mut guard) = SEARCH_RESULTS.lock() {
            *guard = None;
        }
    }

    pub fn take() -> Option<Vec<McpRegistryServerWrapper>> {
        SEARCH_RESULTS.lock().ok().and_then(|guard| guard.clone())
    }

    fn notify(name: &str) {
        let name = name.to_string();
        dispatch::Queue::main().exec_async(move || {
            let center = NSNotificationCenter::defaultCenter();
            let name = NSString::from_str(&name);
            unsafe {
                center.postNotificationName_object(&name, None);
            }
        });
    }

    fn notify_complete() {
        Self::notify("PersonalAgentMcpSearchComplete");
    }

    fn notify_error() {
        Self::notify("PersonalAgentMcpSearchError");
    }
}
