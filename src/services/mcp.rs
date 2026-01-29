// @plan PLAN-20250125-REFACTOR.P07
//! MCP (Model Context Protocol) service for managing MCP servers
//!
//! Handles the lifecycle of MCP server connections including status tracking,
//! tool discovery, and configuration management.

use async_trait::async_trait;
use uuid::Uuid;

use serdes_ai_mcp::McpServerConfig;

use super::{ServiceError, ServiceResult};

/// Status of an MCP server connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Tool available from an MCP server
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP service trait for managing MCP servers
///
/// Implementation: [`super::mcp_impl::McpServiceImpl`]
#[async_trait]
pub trait McpService: Send + Sync {
    /// List all configured MCP servers
    async fn list(&self) -> ServiceResult<Vec<McpServerConfig>>;

    /// Get a specific MCP server configuration
    async fn get(&self, id: Uuid) -> ServiceResult<McpServerConfig>;

    /// Get the current status of an MCP server
    async fn get_status(&self, id: Uuid) -> ServiceResult<McpServerStatus>;

    /// Enable or disable an MCP server
    async fn set_enabled(&self, id: Uuid, enabled: bool) -> ServiceResult<()>;

    /// Get available tools from an MCP server
    async fn get_available_tools(&self, id: Uuid) -> ServiceResult<Vec<McpTool>>;

    /// Add a new MCP server configuration
    ///
    /// # Arguments
    /// * `name` - Display name for the server
    /// * `command` - Command to start the server
    /// * `args` - Arguments for the command
    /// * `env` - Optional environment variables
    async fn add(
        &self,
        name: String,
        command: String,
        args: Vec<String>,
        env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<McpServerConfig>;

    /// Update an existing MCP server configuration
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        command: Option<String>,
        args: Option<Vec<String>>,
        env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<McpServerConfig>;

    /// Delete an MCP server configuration
    async fn delete(&self, id: Uuid) -> ServiceResult<()>;

    /// Restart an MCP server connection
    async fn restart(&self, id: Uuid) -> ServiceResult<()>;

    /// Get all enabled MCP servers
    async fn list_enabled(&self) -> ServiceResult<Vec<McpServerConfig>>;

    /// Get all available tools from all enabled servers
    async fn get_all_tools(&self) -> ServiceResult<Vec<(Uuid, McpTool)>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_status_variants() {
        let disconnected = McpServerStatus::Disconnected;
        let connecting = McpServerStatus::Connecting;
        let connected = McpServerStatus::Connected;
        let error = McpServerStatus::Error("Test error".to_string());

        assert_eq!(disconnected, McpServerStatus::Disconnected);
        assert_eq!(connecting, McpServerStatus::Connecting);
        assert_eq!(connected, McpServerStatus::Connected);
        assert_eq!(error, McpServerStatus::Error("Test error".to_string()));
    }

    #[test]
    fn test_mcp_tool_construction() {
        let tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        };

        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "A test tool");
    }
}