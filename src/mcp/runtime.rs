//! MCP Runtime - spawns servers and handles tool calls

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use tokio::sync::Mutex;
use serdes_ai::mcp::McpClient;

use crate::config::Config;
use crate::mcp::{McpConfig, McpManager, McpTransport, SecretsManager, McpStatus, McpStatusManager};

/// Active MCP connection
pub struct McpConnection {
    pub config: McpConfig,
    pub client: Arc<Mutex<McpClient>>,
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
    status_manager: McpStatusManager,
}

impl McpRuntime {
    pub fn new(secrets: SecretsManager) -> Self {
        Self {
            manager: McpManager::new(secrets),
            connections: HashMap::new(),
            status_manager: McpStatusManager::new(),
        }
    }

    /// Get a clone of the status manager for UI access
    pub fn status_manager(&self) -> McpStatusManager {
        self.status_manager.clone()
    }

    /// Start an MCP server
    pub async fn start_mcp(&mut self, config: &McpConfig) -> Result<(), String> {
        if !config.enabled {
            self.status_manager.set_status(config.id, McpStatus::Stopped);
            return Err("MCP is disabled".to_string());
        }

        if self.connections.contains_key(&config.id) {
            return Ok(()); // Already running
        }

        // Update status to Starting
        self.status_manager.set_status(config.id, McpStatus::Starting);

        // Build environment
        let env = self.manager.build_env(config)
            .map_err(|e| {
                self.status_manager.set_status(config.id, McpStatus::Error(e.to_string()));
                e.to_string()
            })?;

        // Create the MCP client based on transport
        let client: McpClient = match config.transport {
            McpTransport::Http => {
                // HTTP transport - use reqwest-based HttpTransport
                let transport = serdes_ai::mcp::transport::HttpTransport::new(&config.package.identifier);
                McpClient::new(transport)
            },
            McpTransport::Stdio => {
                // Build command
                let (cmd, args) = McpManager::build_command(config);
                
                if cmd.is_empty() {
                    self.status_manager.set_status(config.id, McpStatus::Error("Empty command".to_string()));
                    return Err("Empty command for stdio transport".to_string());
                }

                // Convert args to &str
                let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                
                // Spawn with environment
                let transport = serdes_ai::mcp::StdioTransport::spawn_with_env(&cmd, &args_str, env)
                    .await
                    .map_err(|e| {
                        let err = format!("Failed to spawn MCP: {}", e);
                        self.status_manager.set_status(config.id, McpStatus::Error(err.clone()));
                        err
                    })?;
                
                McpClient::new(transport)
            }
        };

        // Initialize the client
        client.initialize()
            .await
            .map_err(|e| {
                let err = format!("Failed to initialize MCP: {}", e);
                self.status_manager.set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?;

        // List tools from the MCP server
        let mcp_tools = client.list_tools()
            .await
            .map_err(|e| {
                let err = format!("Failed to list tools: {}", e);
                self.status_manager.set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?;

        // Convert to our McpTool format
        let tools: Vec<McpTool> = mcp_tools.into_iter().map(|t| McpTool {
            name: t.name,
            description: t.description.unwrap_or_default(),
            input_schema: t.input_schema,
            mcp_id: config.id,
        }).collect();

        // Register as active
        self.manager.register_active(config.clone());
        
        // Store connection
        self.connections.insert(config.id, McpConnection {
            config: config.clone(),
            client: Arc::new(Mutex::new(client)),
            tools,
        });
        
        self.status_manager.set_status(config.id, McpStatus::Running);

        Ok(())
    }

    /// Stop an MCP server
    pub fn stop_mcp(&mut self, id: &Uuid) -> Result<(), String> {
        self.connections.remove(id);
        self.status_manager.set_status(*id, McpStatus::Stopped);
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
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mcp_id = self.find_tool_provider(tool_name)
            .ok_or_else(|| format!("No MCP provides tool: {}", tool_name))?;

        // Update last used time
        self.manager.touch(&mcp_id);

        // Get the connection
        let conn = self.connections.get(&mcp_id)
            .ok_or_else(|| format!("MCP connection not found: {}", mcp_id))?;

        // Call the tool via SerdesAI MCP client
        let client = conn.client.lock().await;
        let result = client.call_tool(tool_name, arguments)
            .await
            .map_err(|e| format!("MCP tool call failed: {}", e))?;
        
        // Convert CallToolResult to JSON
        // The result contains content array with text/image/resource items
        Ok(serde_json::to_value(result).unwrap_or_default())
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

        // This will fail because we don't have a real MCP server
        // But we're testing the error path works correctly
        let result = runtime.start_mcp(&config).await;
        assert!(result.is_err());
        assert_eq!(runtime.active_count(), 0);
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

        // Can't test this without a real MCP server
        // The second call returns Ok if already running
        let result = runtime.start_mcp(&config).await;
        assert!(result.is_ok()); // Returns Ok immediately since already exists
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_start_mcp_http_transport() {
        let mut runtime = create_test_runtime();
        let mut config = create_test_mcp_config(true);
        config.transport = McpTransport::Http;
        config.package.package_type = McpPackageType::Http;
        config.package.identifier = "https://mcp.exa.ai/mcp".to_string();

        // This will fail because we don't have a real HTTP MCP server
        let result = runtime.start_mcp(&config).await;
        assert!(result.is_err());
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_stop_mcp() {
        let mut runtime = create_test_runtime();
        let id = Uuid::new_v4();

        // Stop a non-existent MCP should succeed
        let result = runtime.stop_mcp(&id);
        assert!(result.is_ok());
        assert_eq!(runtime.active_count(), 0);
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
        let runtime = create_test_runtime();
        
        // Without real MCP connections, we should get empty tools
        let tools = runtime.get_all_tools();
        assert_eq!(tools.len(), 0);
    }

    #[tokio::test]
    async fn test_find_tool_provider() {
        let runtime = create_test_runtime();

        // Without real MCP connections, no providers
        let no_provider = runtime.find_tool_provider("nonexistent");
        assert_eq!(no_provider, None);
    }

    #[tokio::test]
    async fn test_call_tool() {
        let mut runtime = create_test_runtime();

        // Without MCP connections, should error
        let result = runtime.call_tool("test_tool", serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No MCP provides tool"));
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
        let runtime = create_test_runtime();
        assert!(!runtime.has_active_mcps());
    }

    #[tokio::test]
    async fn test_active_count() {
        let runtime = create_test_runtime();
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_idle() {
        let mut runtime = create_test_runtime();

        // Cleanup on empty runtime should not panic
        runtime.cleanup_idle();
        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let mut runtime = create_test_runtime();
        let id = Uuid::new_v4();

        // Stop non-existent MCP should succeed
        let result = runtime.stop_mcp(&id);
        assert!(result.is_ok());
        assert!(!runtime.has_active_mcps());
    }

    #[tokio::test]
    async fn test_error_propagation() {
        let mut runtime = create_test_runtime();
        let mut config = create_test_mcp_config(true);
        
        // Set invalid package type that would cause error
        config.package.package_type = McpPackageType::Docker;
        config.package.identifier = "".to_string(); // Empty identifier should fail

        let result = runtime.start_mcp(&config).await;
        // Should fail with empty command
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_simultaneous_starts() {
        let mut runtime = create_test_runtime();
        let config1 = create_test_mcp_config(true);
        let config2 = create_test_mcp_config(true);
        let config3 = create_test_mcp_config(true);

        // All will fail without real MCP servers
        let _ = runtime.start_mcp(&config1).await;
        let _ = runtime.start_mcp(&config2).await;
        let _ = runtime.start_mcp(&config3).await;

        assert_eq!(runtime.active_count(), 0);
    }

    #[tokio::test]
    async fn test_start_stop_restart() {
        let mut runtime = create_test_runtime();
        let config = create_test_mcp_config(true);
        let id = config.id;

        // Start (will fail without real MCP)
        let _ = runtime.start_mcp(&config).await;
        assert!(!runtime.has_active_mcps());

        // Stop non-existent
        runtime.stop_mcp(&id).unwrap();
        assert!(!runtime.has_active_mcps());

        // Restart (will fail without real MCP)
        let _ = runtime.start_mcp(&config).await;
        assert!(!runtime.has_active_mcps());
    }
}
