//! MCP service implementation

use super::{McpService, McpServerStatus, McpTool, ServiceError, ServiceResult};
use serdes_ai_mcp::{McpServerConfig, McpTransportConfig};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use crate::mcp::registry::McpRegistry;

/// File-based implementation of McpService
pub struct McpServiceImpl {
    config_dir: PathBuf,
    configs: Arc<RwLock<Vec<StoredMcpConfig>>>,
    status_map: Arc<RwLock<HashMap<Uuid, McpServerStatus>>>,
    is_refreshing: Arc<AtomicBool>,
}

/// Internal storage format for MCP configs with UUID
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMcpConfig {
    pub name: String,
    pub transport: TransportData,
    pub server_uuid: Uuid,
    pub enabled: bool,
}

/// Serializable transport data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum TransportData {
    #[serde(rename = "stdio")]
    Stdio { command: String, args: Vec<String> },
    #[serde(rename = "http")]
    Http { url: String },
}

impl From<&McpServerConfig> for TransportData {
    fn from(config: &McpServerConfig) -> Self {
        match &config.transport {
            McpTransportConfig::Stdio { command, args } => TransportData::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
            McpTransportConfig::Http { url } => TransportData::Http {
                url: url.clone(),
            },
            McpTransportConfig::Sse { url } => TransportData::Http {
                url: url.clone(),
            },
        }
    }
}

impl From<&TransportData> for McpTransportConfig {
    fn from(data: &TransportData) -> Self {
        match data {
            TransportData::Stdio { command, args } => McpTransportConfig::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
            TransportData::Http { url } => McpTransportConfig::Http {
                url: url.clone(),
            },
        }
    }
}

impl From<&StoredMcpConfig> for McpServerConfig {
    fn from(stored: &StoredMcpConfig) -> Self {
        Self {
            name: stored.name.clone(),
            transport: (&stored.transport).into(),
        }
    }
}

impl McpServiceImpl {
    /// Create a new McpServiceImpl
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created.
    pub fn new(config_dir: PathBuf) -> Result<Self, ServiceError> {
        // Ensure config directory exists
        fs::create_dir_all(&config_dir)
            .map_err(|e| ServiceError::Io(format!("Failed to create MCP config directory: {e}")))?;

        let service = Self {
            config_dir,
            configs: Arc::new(RwLock::new(Vec::new())),
            status_map: Arc::new(RwLock::new(HashMap::new())),
            is_refreshing: Arc::new(AtomicBool::new(false)),
        };

        // Don't load existing configs in new() to avoid blocking_write()
        // The configs will be loaded on-demand or through a separate init method

        Ok(service)
    }

    /// Initialize the service by loading existing configs from disk
    ///
    /// # Errors
    ///
    /// Returns an error if the configs cannot be loaded from disk.
    pub async fn initialize(&self) -> Result<(), ServiceError> {
        let configs = self.load_configs_from_disk()?;
        let mut configs_lock = self.configs.write().await;
        *configs_lock = configs;
        Ok(())
    }

    /// Load all configs from disk
    fn load_configs_from_disk(&self) -> Result<Vec<StoredMcpConfig>, ServiceError> {
        let mut configs = Vec::new();

        if !self.config_dir.exists() {
            return Ok(configs);
        }

        let entries = fs::read_dir(&self.config_dir)
            .map_err(|e| ServiceError::Io(format!("Failed to read MCP config directory: {e}")))?;

        for entry in entries {
            let entry = entry.map_err(|e| ServiceError::Io(format!("Failed to read directory entry: {e}")))?;

            let path = entry.path();

            // Only process JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Read and parse config
            let content = fs::read_to_string(&path)
                .map_err(|e| ServiceError::Io(format!("Failed to read config file {}: {e}", path.display())))?;

            let config: StoredMcpConfig = serde_json::from_str(&content).map_err(|e| {
                ServiceError::Serialization(format!("Failed to parse config file {}: {e}", path.display()))
            })?;

            configs.push(config);
        }

        Ok(configs)
    }

    /// Save a config to disk
    fn save_config_to_disk(&self, config: &StoredMcpConfig) -> Result<(), ServiceError> {
        let filename = format!("{}.json", config.server_uuid);
        let path = self.config_dir.join(filename);

        let content = serde_json::to_string_pretty(config)
            .map_err(|e| ServiceError::Serialization(format!("Failed to serialize config {}: {e}", config.server_uuid)))?;

        fs::write(&path, content)
            .map_err(|e| ServiceError::Io(format!("Failed to write config file {}: {e}", path.display())))?;

        Ok(())
    }

    /// Delete a config from disk
    fn delete_config_from_disk(&self, id: Uuid) -> Result<(), ServiceError> {
        let filename = format!("{}.json", id);
        let path = self.config_dir.join(filename);

        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| ServiceError::Io(format!("Failed to delete config file {}: {e}", path.display())))?;
        }

        Ok(())
    }

    /// Ensure status entry exists for a config
    async fn ensure_status(&self, id: Uuid) {
        let mut status_map = self.status_map.write().await;
        status_map.entry(id).or_insert_with(|| McpServerStatus::Disconnected);
    }
}

#[async_trait::async_trait]
impl McpService for McpServiceImpl {
    /// List all MCP configs
    async fn list(&self) -> ServiceResult<Vec<McpServerConfig>> {
        let configs = self.configs.read().await;
        Ok(configs.iter().map(|c| McpServerConfig::from(c)).collect())
    }

    /// Get a config by ID
    async fn get(&self, id: Uuid) -> ServiceResult<McpServerConfig> {
        let configs = self.configs.read().await;

        configs
            .iter()
            .find(|c| c.server_uuid == id)
            .map(|c| McpServerConfig::from(c))
            .ok_or_else(|| ServiceError::NotFound(format!("MCP server {id} not found")))
    }

    /// Get the current status of an MCP server
    async fn get_status(&self, id: Uuid) -> ServiceResult<McpServerStatus> {
        // Check if config exists
        {
            let configs = self.configs.read().await;
            if !configs.iter().any(|c| c.server_uuid == id) {
                return Err(ServiceError::NotFound(format!("MCP server {id} not found")));
            }
        }

        // Ensure status entry exists
        self.ensure_status(id).await;

        let status_map = self.status_map.read().await;
        Ok(status_map.get(&id).cloned().unwrap_or(McpServerStatus::Disconnected))
    }

    /// Enable or disable a config
    async fn set_enabled(&self, id: Uuid, enabled: bool) -> ServiceResult<()> {
        // Check if config exists and update enabled flag
        {
            let mut configs = self.configs.write().await;
            let config = configs
                .iter_mut()
                .find(|c| c.server_uuid == id)
                .ok_or_else(|| ServiceError::NotFound(format!("MCP server {id} not found")))?;
            config.enabled = enabled;
        }

        // Update status
        let mut status_map = self.status_map.write().await;
        if enabled {
            status_map.insert(id, McpServerStatus::Connecting);
        } else {
            status_map.insert(id, McpServerStatus::Disconnected);
        }

        Ok(())
    }

    /// Get available tools from an MCP server
    async fn get_available_tools(&self, id: Uuid) -> ServiceResult<Vec<McpTool>> {
        // Check if config exists
        {
            let configs = self.configs.read().await;
            if !configs.iter().any(|c| c.server_uuid == id) {
                return Err(ServiceError::NotFound(format!("MCP server {id} not found")));
            }
        }

        // TODO: Actually fetch tools from MCP server
        Ok(vec![])
    }

    /// Add a new MCP server configuration
    async fn add(
        &self,
        name: String,
        command: String,
        args: Vec<String>,
        env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<McpServerConfig> {
        // Note: ignoring env for now as McpTransportConfig::Stdio doesn't support it
        let _ = env;
        let server_uuid = Uuid::new_v4();
        let config = McpServerConfig {
            name: name.clone(),
            transport: McpTransportConfig::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
        };

        let stored = StoredMcpConfig {
            name,
            transport: TransportData::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
            server_uuid,
            enabled: true,
        };

        // Save to disk
        self.save_config_to_disk(&stored)?;

        // Add to in-memory cache
        let mut configs = self.configs.write().await;
        configs.push(stored.clone());

        // Initialize status
        self.ensure_status(stored.server_uuid).await;

        Ok(McpServerConfig::from(&stored))
    }

    /// Update an existing MCP server configuration
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        command: Option<String>,
        args: Option<Vec<String>>,
        env: Option<Vec<(String, String)>>,
    ) -> ServiceResult<McpServerConfig> {
        // Note: ignoring env for now
        let _ = env; // Suppress unused warning

        // Check if config exists
        let mut configs = self.configs.write().await;

        let stored = configs
            .iter_mut()
            .find(|c| c.server_uuid == id)
            .ok_or_else(|| ServiceError::NotFound(format!("MCP server {id} not found")))?;

        // Update fields
        if let Some(new_name) = name {
            stored.name = new_name;
        }
        if let Some(command) = command {
            if let TransportData::Stdio { args: existing_args, .. } = &mut stored.transport {
                *existing_args = args.clone().unwrap_or_default();
                // Update command by recreating the transport
                stored.transport = TransportData::Stdio {
                    command,
                    args: existing_args.clone(),
                };
            }
        }
        if let Some(args) = args {
            if let TransportData::Stdio { command, .. } = &stored.transport {
                stored.transport = TransportData::Stdio {
                    command: command.clone(),
                    args,
                };
            }
        }

        // Clone needed values before dropping lock
        let server_uuid = stored.server_uuid;
        let stored_clone = stored.clone();

        // Save to disk
        drop(configs); // Release lock before I/O
        self.save_config_to_disk(&stored_clone)?;

        // Now get updated config
        let configs = self.configs.read().await;
        let updated_stored = configs.iter().find(|c| c.server_uuid == server_uuid).unwrap();
        Ok(McpServerConfig::from(&*updated_stored))
    }

    /// Delete a config
    async fn delete(&self, id: Uuid) -> ServiceResult<()> {
        // Check if config exists
        {
            let configs = self.configs.read().await;
            if !configs.iter().any(|c| c.server_uuid == id) {
                return Err(ServiceError::NotFound(format!("MCP server {id} not found")));
            }
        }

        // Delete from disk
        self.delete_config_from_disk(id)?;

        // Remove from in-memory cache
        let mut configs = self.configs.write().await;
        configs.retain(|c| c.server_uuid != id);

        // Remove status
        let mut status_map = self.status_map.write().await;
        status_map.remove(&id);

        Ok(())
    }

    /// Restart an MCP server connection
    async fn restart(&self, id: Uuid) -> ServiceResult<()> {
        // Check if config exists
        {
            let configs = self.configs.read().await;
            if !configs.iter().any(|c| c.server_uuid == id) {
                return Err(ServiceError::NotFound(format!("MCP server {id} not found")));
            }
        }

        // Update status to connecting
        {
            let mut status_map = self.status_map.write().await;
            status_map.insert(id, McpServerStatus::Connecting);
        }

        // TODO: Actually restart the MCP server process

        // Update status to connected
        {
            let mut status_map = self.status_map.write().await;
            status_map.insert(id, McpServerStatus::Connected);
        }

        Ok(())
    }

    /// Get all enabled MCP servers
    async fn list_enabled(&self) -> ServiceResult<Vec<McpServerConfig>> {
        let configs = self.configs.read().await;
        Ok(configs.iter().filter(|c| c.enabled).map(|c| McpServerConfig::from(c)).collect())
    }

    /// Get all available tools from all enabled servers
    async fn get_all_tools(&self) -> ServiceResult<Vec<(Uuid, McpTool)>> {
        let enabled = self.list_enabled().await?;
        let mut all_tools = Vec::new();

        for config in enabled {
            let configs = self.configs.read().await;
            if let Some(stored) = configs.iter().find(|c| c.name == config.name && c.enabled) {
                let tools = self.get_available_tools(stored.server_uuid).await.unwrap_or_default();
                for tool in tools {
                    all_tools.push((stored.server_uuid, tool));
                }
            }
        }

        Ok(all_tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_list_servers() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        // Add server
        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // List servers
        let servers = service.list().await.unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Test MCP");
    }

    #[tokio::test]
    async fn test_get_server() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // Get the UUID from stored configs
        let stored_configs = service.configs.read().await;
        let server_uuid = stored_configs[0].server_uuid;
        drop(stored_configs);

        let retrieved = service.get(server_uuid).await.unwrap();
        assert_eq!(retrieved.name, "Test MCP");
    }

    #[tokio::test]
    async fn test_update_server() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // Get the UUID from stored configs
        let stored_configs = service.configs.read().await;
        let server_uuid = stored_configs[0].server_uuid;
        drop(stored_configs);

        service
            .update(server_uuid, Some("Updated MCP".to_string()), None, None, None)
            .await
            .unwrap();

        let retrieved = service.get(server_uuid).await.unwrap();
        assert_eq!(retrieved.name, "Updated MCP");
    }

    #[tokio::test]
    async fn test_delete_server() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // Get the UUID from stored configs
        let stored_configs = service.configs.read().await;
        let server_uuid = stored_configs[0].server_uuid;
        drop(stored_configs);

        service.delete(server_uuid).await.unwrap();

        let servers = service.list().await.unwrap();
        assert_eq!(servers.len(), 0);

        assert!(service.get(server_uuid).await.is_err());
    }

    #[tokio::test]
    async fn test_set_enabled() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // Get the UUID from stored configs
        let stored_configs = service.configs.read().await;
        let server_uuid = stored_configs[0].server_uuid;
        drop(stored_configs);

        service.set_enabled(server_uuid, false).await.unwrap();

        let status = service.get_status(server_uuid).await.unwrap();
        assert_eq!(status, McpServerStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_restart() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // Get the UUID from stored configs
        let stored_configs = service.configs.read().await;
        let server_uuid = stored_configs[0].server_uuid;
        drop(stored_configs);

        service.restart(server_uuid).await.unwrap();

        let status = service.get_status(server_uuid).await.unwrap();
        assert_eq!(status, McpServerStatus::Connected);
    }

    #[tokio::test]
    async fn test_get_available_tools() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = McpServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let config = service
            .add(
                "Test MCP".to_string(),
                "npx".to_string(),
                vec!["-y".to_string(), "@modelcontextprotocol/server-test".to_string()],
                None,
            )
            .await
            .unwrap();

        // Get the UUID from stored configs
        let stored_configs = service.configs.read().await;
        let server_uuid = stored_configs[0].server_uuid;
        drop(stored_configs);

        let tools = service.get_available_tools(server_uuid).await.unwrap();
        assert_eq!(tools.len(), 0); // Returns empty vec for now
    }
}
