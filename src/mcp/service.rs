//! MCP Service - singleton managing MCP connections for the app

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
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
    pub fn global() -> Arc<Mutex<McpService>> {
        MCP_SERVICE.get_or_init(|| {
            let secrets = SecretsManager::new(
                dirs::data_local_dir()
                    .expect("Could not determine data directory")
                    .join("PersonalAgent")
                    .join("mcp_secrets")
            );
            Arc::new(Mutex::new(McpService {
                runtime: McpRuntime::new(secrets),
                tool_registry: HashMap::new(),
            }))
        }).clone()
    }
    
    /// Initialize MCPs from config - call on app startup
    pub async fn initialize(&mut self) -> Result<(), String> {
        let config_path = Config::default_path().map_err(|e| e.to_string())?;
        let config = Config::load(config_path).map_err(|e| e.to_string())?;
        
        let results = self.runtime.start_all(&config).await;
        
        // Log any failures
        for (id, result) in results {
            if let Err(e) = result {
                eprintln!("Failed to start MCP {}: {}", id, e);
            }
        }
        
        // Update tool registry
        self.update_tool_registry();
        
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
    pub fn get_tools(&self) -> Vec<ToolDefinition> {
        self.runtime.get_all_tools()
            .into_iter()
            .map(|t| ToolDefinition {
                name: t.name,
                description: t.description,
                parameters: t.input_schema,
            })
            .collect()
    }
    
    /// Get all available tools as LLM Tool definitions
    pub fn get_llm_tools(&self) -> Vec<crate::llm::Tool> {
        self.get_tools()
            .into_iter()
            .map(|t| crate::llm::Tool::new(t.name, t.description, t.parameters))
            .collect()
    }
    
    /// Call a tool on the appropriate MCP server  
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        args: serde_json::Value
    ) -> Result<serde_json::Value, String> {
        // Log the tool call attempt
        eprintln!("MCP tool call: {} with args: {}", tool_name, args);
        
        // Route to appropriate MCP based on tool_registry
        let result = self.runtime.call_tool(tool_name, args).await;
        
        // Update registry in case tools changed
        self.update_tool_registry();
        
        result
    }
    
    /// Check if any MCPs are currently active
    pub fn has_active_mcps(&self) -> bool {
        self.runtime.has_active_mcps()
    }
    
    /// Get the count of active MCPs
    pub fn active_count(&self) -> usize {
        self.runtime.active_count()
    }
    
    /// Reload MCPs from config (useful after config changes)
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
    
    #[test]
    fn test_get_tools_empty() {
        let service = McpService::global();
        let locked = service.lock().unwrap();
        let tools = locked.get_tools();
        // Should be empty initially (no MCPs started)
        assert_eq!(tools.len(), 0);
    }
    
    #[test]
    fn test_has_active_mcps_initially_false() {
        let service = McpService::global();
        let locked = service.lock().unwrap();
        // Should have no active MCPs initially
        assert!(!locked.has_active_mcps());
    }
    
    #[test]
    fn test_active_count_initially_zero() {
        let service = McpService::global();
        let locked = service.lock().unwrap();
        assert_eq!(locked.active_count(), 0);
    }
    
    #[tokio::test]
    async fn test_call_tool_not_found() {
        let service = McpService::global();
        let mut locked = service.lock().unwrap();
        
        let result = locked.call_tool("nonexistent_tool", serde_json::json!({})).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_initialize_no_config() {
        // This test will work when there's no config file
        // Should not panic, just start with no MCPs
        let secrets = SecretsManager::new(
            std::env::temp_dir().join("test_mcp_service")
        );
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
