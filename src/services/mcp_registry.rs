// @plan PLAN-20250125-REFACTOR.P07
//! MCP registry service for discovering and managing MCP servers
//!
//! Provides a registry of available MCP servers from the official MCP server registry.

use async_trait::async_trait;

use super::ServiceResult;

/// Information about an MCP server from the registry
#[derive(Debug, Clone)]
pub struct McpRegistryEntry {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub repository: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<Vec<(String, String)>>,
    pub tags: Vec<String>,
}

/// MCP registry service trait
///
/// Implementation: [`super::mcp_registry_impl::McpRegistryServiceImpl`]
#[async_trait]
pub trait McpRegistryService: Send + Sync {
    /// Search for MCP servers in the registry
    ///
    /// # Arguments
    /// * `query` - Search query string
    async fn search(&self, query: &str) -> ServiceResult<Vec<McpRegistryEntry>>;

    /// Get detailed information about a specific MCP server
    ///
    /// # Arguments
    /// * `name` - The server name in the registry
    async fn get_details(&self, name: &str) -> ServiceResult<Option<McpRegistryEntry>>;

    /// List all MCP servers in the registry
    async fn list_all(&self) -> ServiceResult<Vec<McpRegistryEntry>>;

    /// List MCP servers by tag/category
    ///
    /// # Arguments
    /// * `tag` - Tag to filter by (e.g., "database", "filesystem", "productivity")
    async fn list_by_tag(&self, tag: &str) -> ServiceResult<Vec<McpRegistryEntry>>;

    /// Get trending/popular MCP servers
    async fn list_trending(&self) -> ServiceResult<Vec<McpRegistryEntry>>;

    /// Refresh the local MCP registry cache
    async fn refresh(&self) -> ServiceResult<()>;

    /// Get the last refresh timestamp
    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>>;

    /// Install an MCP server from the registry
    ///
    /// # Arguments
    /// * `name` - The server name in the registry
    /// * `config_name` - Custom display name for the installed server
    async fn install(&self, name: &str, config_name: Option<String>) -> ServiceResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_registry_entry_construction() {
        let entry = McpRegistryEntry {
            name: "test-server".to_string(),
            display_name: "Test Server".to_string(),
            description: "A test MCP server".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            license: "MIT".to_string(),
            repository: "https://github.com/test/test-server".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@test/server".to_string()],
            env: None,
            tags: vec!["test".to_string(), "demo".to_string()],
        };

        assert_eq!(entry.name, "test-server");
        assert_eq!(entry.display_name, "Test Server");
        assert_eq!(entry.tags.len(), 2);
    }

    #[test]
    fn test_entry_with_env_vars() {
        let entry = McpRegistryEntry {
            name: "env-server".to_string(),
            display_name: "Env Server".to_string(),
            description: "Server with env vars".to_string(),
            version: "1.0.0".to_string(),
            author: "Test".to_string(),
            license: "MIT".to_string(),
            repository: "https://github.com/test/env-server".to_string(),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            env: Some(vec![("API_KEY".to_string(), "value".to_string())]),
            tags: vec![],
        };

        assert!(entry.env.is_some());
        assert_eq!(entry.env.unwrap().len(), 1);
    }
}
