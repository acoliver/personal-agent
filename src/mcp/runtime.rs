//! MCP Runtime - spawns servers and handles tool calls

use std::collections::HashMap;
use uuid::Uuid;

use crate::config::Config;
use crate::mcp::{McpConfig, McpManager, McpTransport, SecretsManager};

/// Active MCP connection
pub struct McpConnection {
    // This will hold the actual SerdesAI MCP client when spawned
    // For now, placeholder for the connection state
    pub config: McpConfig,
    pub tools: Vec<McpTool>,
}

/// MCP Tool definition  
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub mcp_id: Uuid,
}

/// MCP Runtime manages active connections
pub struct McpRuntime {
    manager: McpManager,
    connections: HashMap<Uuid, McpConnection>,
}

impl McpRuntime {
    pub fn new(secrets: SecretsManager) -> Self {
        Self {
            manager: McpManager::new(secrets),
            connections: HashMap::new(),
        }
    }

    /// Start an MCP server
    pub async fn start_mcp(&mut self, config: &McpConfig) -> Result<(), String> {
        if !config.enabled {
            return Err("MCP is disabled".to_string());
        }

        if self.connections.contains_key(&config.id) {
            return Ok(()); // Already running
        }

        // Build environment
        let _env = self.manager.build_env(config)
            .map_err(|e| e.to_string())?;

        // Build command
        let (cmd, _args) = McpManager::build_command(config);

        if cmd.is_empty() && config.transport == McpTransport::Http {
            // HTTP transport - would connect to URL instead of spawning
            // For now, just register as active
            self.manager.register_active(config.clone());
            self.connections.insert(config.id, McpConnection {
                config: config.clone(),
                tools: Vec::new(),
            });
            return Ok(());
        }

        // For stdio transport, we would spawn using SerdesAI:
        // let transport = StdioTransport::spawn_with_env(&cmd, &args, env).await?;
        // let client = McpClient::new(transport);
        // let tools = client.list_tools().await?;
        
        // For now, register as active (actual spawning requires SerdesAI integration)
        self.manager.register_active(config.clone());
        self.connections.insert(config.id, McpConnection {
            config: config.clone(),
            tools: Vec::new(), // Would be populated from MCP server
        });

        Ok(())
    }

    /// Stop an MCP server
    pub fn stop_mcp(&mut self, id: &Uuid) -> Result<(), String> {
        self.connections.remove(id);
        self.manager.stop(id).map_err(|e| e.to_string())
    }

    /// Start all enabled MCPs from config
    pub async fn start_all(&mut self, config: &Config) -> Vec<(Uuid, Result<(), String>)> {
        let mut results = Vec::new();
        for mcp in config.get_enabled_mcps() {
            let result = self.start_mcp(mcp).await;
            results.push((mcp.id, result));
        }
        results
    }

    /// Get all available tools from active MCPs
    pub fn get_all_tools(&self) -> Vec<McpTool> {
        self.connections.values()
            .flat_map(|c| c.tools.iter().cloned())
            .collect()
    }

    /// Find which MCP provides a tool
    pub fn find_tool_provider(&self, tool_name: &str) -> Option<Uuid> {
        for (id, conn) in &self.connections {
            if conn.tools.iter().any(|t| t.name == tool_name) {
                return Some(*id);
            }
        }
        None
    }

    /// Call a tool on an MCP
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        _arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mcp_id = self.find_tool_provider(tool_name)
            .ok_or_else(|| format!("No MCP provides tool: {}", tool_name))?;

        // Update last used time
        self.manager.touch(&mcp_id);

        // Would actually call the tool via SerdesAI client
        // For now, return placeholder
        Ok(serde_json::json!({
            "error": "MCP tool calling not yet implemented"
        }))
    }

    /// Check if any MCPs are active
    pub fn has_active_mcps(&self) -> bool {
        !self.connections.is_empty()
    }

    /// Get active MCP count
    pub fn active_count(&self) -> usize {
        self.connections.len()
    }

    /// Cleanup idle MCPs
    pub fn cleanup_idle(&mut self) {
        self.manager.cleanup_idle();
        // Remove connections for MCPs that were cleaned up
        let active_ids: Vec<Uuid> = self.connections.keys().copied().collect();
        for id in active_ids {
            if !self.manager.is_active(&id) {
                self.connections.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::mcp::{McpSource, McpPackage, McpPackageType, McpAuthType};

    fn create_test_runtime() -> McpRuntime {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        McpRuntime::new(secrets)
    }

    fn create_test_mcp_config(enabled: bool) -> McpConfig {
        McpConfig {
            id: Uuid::new_v4(),
            name: "Test MCP".to_string(),
            enabled,
            source: McpSource::Manual { url: "test".to_string() },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@test/mcp".to_string(),
                runtime_hint: Some("npx".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::None,
            env_vars: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn test_new_runtime() {
        let runtime = create_test_runtime();
        assert_eq!(runtime.active_count(), 0);
        assert!(!runtime.has_active_mcps());
    }

    #[tokio::test]
    async fn test_start_mcp_enabled() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);
        let id = config.id;

        let result = runtime.start_mcp(&config).await;
        assert!(result.is_ok());
        assert_eq!(runtime.active_count(), 1);
        assert!(runtime.has_active_mcps());
        assert!(runtime.connections.contains_key(&id));
    }

    #[tokio::test]
    async fn test_start_mcp_disabled() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(false);

        let result = runtime.start_mcp(&config).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "MCP is disabled");
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_start_mcp_already_running() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);

        runtime.start_mcp(&config).await.unwrap();
        let result = runtime.start_mcp(&config).await;
        assert!(result.is_ok());
        assert_eq!(runtime.active_count(), 1); // Still just 1
    }

    #[tokio::test]
    async fn test_start_mcp_http_transport() {
        let mut runtime = create_test_runtime();
        let mut config = create_test_mcp_config(true);
        config.transport = McpTransport::Http;
        config.package.package_type = McpPackageType::Http;
        let id = config.id;

        let result = runtime.start_mcp(&config).await;
        assert!(result.is_ok());
        assert_eq!(runtime.active_count(), 1);
        assert!(runtime.connections.contains_key(&id));
    }

    #[tokio::test]
    async fn test_stop_mcp() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);
        let id = config.id;

        runtime.start_mcp(&config).await.unwrap();
        assert!(runtime.has_active_mcps());

        let result = runtime.stop_mcp(&id);
        assert!(result.is_ok());
        assert_eq!(runtime.active_count(), 0);
        assert!(!runtime.has_active_mcps());
    }

    #[tokio::test]
    async fn test_stop_mcp_not_running() {
        let mut runtime = create_test_runtime();
        let id = Uuid::new_v4();

        let result = runtime.stop_mcp(&id);
        assert!(result.is_ok()); // Stopping non-existent is OK
    }

    #[tokio::test]
    async fn test_get_all_tools_empty() {
        let runtime = create_test_runtime();
        let tools = runtime.get_all_tools();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_tools() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);
        let id = config.id;

        runtime.start_mcp(&config).await.unwrap();
        
        // Manually add some tools for testing
        if let Some(conn) = runtime.connections.get_mut(&id) {
            conn.tools.push(McpTool {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({}),
                mcp_id: id,
            });
        }

        let tools = runtime.get_all_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "test_tool");
    }

    #[tokio::test]
    async fn test_find_tool_provider() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);
        let id = config.id;

        runtime.start_mcp(&config).await.unwrap();
        
        // Add a tool
        if let Some(conn) = runtime.connections.get_mut(&id) {
            conn.tools.push(McpTool {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({}),
                mcp_id: id,
            });
        }

        let provider = runtime.find_tool_provider("test_tool");
        assert_eq!(provider, Some(id));

        let no_provider = runtime.find_tool_provider("nonexistent");
        assert_eq!(no_provider, None);
    }

    #[tokio::test]
    async fn test_call_tool() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);
        let id = config.id;

        runtime.start_mcp(&config).await.unwrap();
        
        // Add a tool
        if let Some(conn) = runtime.connections.get_mut(&id) {
            conn.tools.push(McpTool {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({}),
                mcp_id: id,
            });
        }

        let result = runtime.call_tool("test_tool", serde_json::json!({})).await;
        assert!(result.is_ok());
        // Currently returns placeholder error
        let value = result.unwrap();
        assert!(value.get("error").is_some());
    }

    #[tokio::test]
    async fn test_call_tool_not_found() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);

        runtime.start_mcp(&config).await.unwrap();

        let result = runtime.call_tool("nonexistent", serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No MCP provides tool"));
    }

    #[tokio::test]
    async fn test_has_active_mcps() {
        let mut runtime = create_test_runtime();
        assert!(!runtime.has_active_mcps());

        let config = create_test_mcp_config(true);
        runtime.start_mcp(&config).await.unwrap();
        assert!(runtime.has_active_mcps());

        runtime.stop_mcp(&config.id).unwrap();
        assert!(!runtime.has_active_mcps());
    }

    #[tokio::test]
    async fn test_active_count() {
        let mut runtime = create_test_runtime();
        assert_eq!(runtime.active_count(), 0);

        let config1 = create_test_mcp_config(true);
        let config2 = create_test_mcp_config(true);

        runtime.start_mcp(&config1).await.unwrap();
        assert_eq!(runtime.active_count(), 1);

        runtime.start_mcp(&config2).await.unwrap();
        assert_eq!(runtime.active_count(), 2);

        runtime.stop_mcp(&config1.id).unwrap();
        assert_eq!(runtime.active_count(), 1);
    }

    #[tokio::test]
    async fn test_cleanup_idle() {
        let mut runtime = create_test_runtime();
        let config1 = create_test_mcp_config(true);
        let config2 = create_test_mcp_config(true);

        runtime.start_mcp(&config1).await.unwrap();
        runtime.start_mcp(&config2).await.unwrap();
        assert_eq!(runtime.active_count(), 2);

        // Cleanup shouldn't remove anything yet (no idle timeout)
        runtime.cleanup_idle();
        assert_eq!(runtime.active_count(), 2);
    }
}
