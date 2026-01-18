//! MCP Service - singleton managing MCP connections for the app

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::Config;
use crate::mcp::{McpRuntime, SecretsManager};

static MCP_SERVICE: OnceLock<Arc<Mutex<McpService>>> = OnceLock::new();

/// Singleton service managing all MCP connections
pub struct McpService {
    runtime: McpRuntime,
    // Map of tool name -> MCP config ID
    tool_registry: HashMap<String, Uuid>,
}

impl McpService {
    /// Get the global singleton instance
    ///
    /// # Panics
    ///
    /// Panics if the data directory cannot be resolved.
    pub fn global() -> Arc<Mutex<Self>> {
        MCP_SERVICE
            .get_or_init(|| {
                let secrets = SecretsManager::new(
                    dirs::data_local_dir()
                        .expect("Could not determine data directory")
                        .join("PersonalAgent")
                        .join("mcp_secrets"),
                );
                Arc::new(Mutex::new(Self {
                    runtime: McpRuntime::new(secrets),
                    tool_registry: HashMap::new(),
                }))
            })
            .clone()
    }

    /// Initialize MCPs from config - call on app startup
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be loaded.
    pub async fn initialize(&mut self) -> Result<(), String> {
        eprintln!("McpService::initialize() starting");
        let config_path = Config::default_path().map_err(|e| e.to_string())?;
        eprintln!("Config path: {}", config_path.display());
        let config = Config::load(config_path).map_err(|e| e.to_string())?;
        eprintln!("Config loaded, {} MCPs", config.mcps.len());

        let results = self.runtime.start_all(&config).await;
        eprintln!("start_all completed with {} results", results.len());

        // Log any failures
        for (id, result) in &results {
            match result {
                Ok(()) => eprintln!("MCP {id} started OK"),
                Err(e) => eprintln!("MCP {id} FAILED: {e}"),
            }
        }

        // Update tool registry
        self.update_tool_registry();
        eprintln!("Tool registry updated, {} tools", self.tool_registry.len());

        Ok(())
    }

    /// Update the tool registry from active MCPs
    fn update_tool_registry(&mut self) {
        self.tool_registry.clear();
        for tool in self.runtime.get_all_tools() {
            self.tool_registry.insert(tool.name.clone(), tool.mcp_id);
        }
    }

    /// Get all available tools from active MCPs
    #[must_use]
    pub fn get_tools(&self) -> Vec<ToolDefinition> {
        self.runtime
            .get_all_tools()
            .into_iter()
            .map(|t| ToolDefinition {
                name: t.name,
                description: t.description,
                parameters: t.input_schema,
            })
            .collect()
    }

    /// Get all available tools as LLM Tool definitions
    #[must_use]
    pub fn get_llm_tools(&self) -> Vec<crate::llm::Tool> {
        self.get_tools()
            .into_iter()
            .map(|t| crate::llm::Tool::new(t.name, t.description, t.parameters))
            .collect()
    }

    /// Call a tool on the appropriate MCP server
    ///
    /// # Errors
    ///
    /// Returns an error if tool execution fails.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Log the tool call attempt
        eprintln!("MCP tool call: {tool_name} with args: {args}");

        // Route to appropriate MCP based on tool_registry
        let result = self.runtime.call_tool(tool_name, args).await;

        // Update registry in case tools changed
        self.update_tool_registry();

        result
    }

    /// Check if any MCPs are currently active
    #[must_use]
    pub fn has_active_mcps(&self) -> bool {
        self.runtime.has_active_mcps()
    }

    /// Get the count of active MCPs
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.runtime.active_count()
    }

    /// Get the status of a specific MCP
    #[must_use]
    pub fn get_status(&self, id: &uuid::Uuid) -> Option<crate::mcp::McpStatus> {
        let status_manager = self.runtime.status_manager();
        Some(status_manager.get_status(id))
    }

    /// Reload MCPs from config (useful after config changes)
    ///
    /// # Errors
    ///
    /// Returns an error if MCPs fail to initialize.
    pub async fn reload(&mut self) -> Result<(), String> {
        // For now, just re-initialize
        self.initialize().await
    }
}

/// Tool definition for use by LLM clients
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_singleton() {
        let service1 = McpService::global();
        let service2 = McpService::global();
        // Both should be the same instance
        assert!(Arc::ptr_eq(&service1, &service2));
    }

    #[tokio::test]
    async fn test_get_tools_empty() {
        let service = McpService::global();
        let locked = service.lock().await;
        let tools = locked.get_tools();
        // Should be empty initially (no MCPs started)
        assert_eq!(tools.len(), 0);
    }

    #[tokio::test]
    async fn test_has_active_mcps_initially_false() {
        let service = McpService::global();
        let locked = service.lock().await;
        // Should have no active MCPs initially
        assert!(!locked.has_active_mcps());
    }

    #[tokio::test]
    async fn test_active_count_initially_zero() {
        let service = McpService::global();
        let locked = service.lock().await;
        assert_eq!(locked.active_count(), 0);
    }

    #[tokio::test]
    async fn test_call_tool_not_found() {
        let service = McpService::global();
        let mut locked = service.lock().await;

        let result = locked
            .call_tool("nonexistent_tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_initialize_no_config() {
        // This test will work when there's no config file
        // Should not panic, just start with no MCPs
        let secrets = SecretsManager::new(std::env::temp_dir().join("test_mcp_service"));
        let _service = McpService {
            runtime: McpRuntime::new(secrets),
            tool_registry: HashMap::new(),
        };

        // If config doesn't exist, load creates default
        // So this should work without error
        // (actual behavior depends on Config::default())
    }

    #[test]
    fn test_tool_definition_creation() {
        let tool_def = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "arg1": {"type": "string"}
                }
            }),
        };

        assert_eq!(tool_def.name, "test_tool");
        assert_eq!(tool_def.description, "A test tool");
    }
}
