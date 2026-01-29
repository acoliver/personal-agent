//! MCP registry service implementation

use super::{McpRegistryEntry, McpRegistryService, ServiceError, ServiceResult};
use crate::mcp::registry::{McpRegistry, McpRegistrySource};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const CACHE_FILENAME: &str = "mcp_registry.json";

/// HTTP cache implementation of McpRegistryService
pub struct McpRegistryServiceImpl {
    cache_dir: PathBuf,
    cached_results: Arc<RwLock<Vec<McpRegistryServerWrapper>>>,
    registry: McpRegistry,
    last_refresh: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
}

/// Wrapper for cache compatibility
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct McpRegistryServerWrapper {
    pub server: crate::mcp::registry::McpRegistryServer,
    #[serde(rename = "_meta")]
    pub meta: serde_json::Value,
}

impl McpRegistryServiceImpl {
    /// Create a new McpRegistryServiceImpl with default cache directory
    ///
    /// # Errors
    ///
    /// Returns an error if the default cache directory cannot be determined.
    pub fn new() -> Result<Self, ServiceError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| ServiceError::Io("Cannot determine cache directory".to_string()))?
            .join("PersonalAgent")
            .join("mcp_registry");

        // Ensure cache directory exists
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| ServiceError::Io(format!("Failed to create cache directory: {e}")))?;

        Ok(Self::with_cache_dir(cache_dir))
    }

    /// Create a new McpRegistryServiceImpl with a specific cache directory
    #[must_use]
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            cached_results: Arc::new(RwLock::new(Vec::new())),
            registry: McpRegistry::new(),
            last_refresh: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the cache file path
    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join(CACHE_FILENAME)
    }

    /// Load cached results from disk
    fn load_from_disk(&self) -> Result<Vec<McpRegistryServerWrapper>, ServiceError> {
        let cache_path = self.cache_path();

        if !cache_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&cache_path)
            .map_err(|e| ServiceError::Io(format!("Failed to read cache file {}: {e}", cache_path.display())))?;

        serde_json::from_str(&content)
            .map_err(|e| ServiceError::Serialization(format!("Failed to parse cache file: {e}")))
    }

    /// Save results to disk
    fn save_to_disk(&self, results: &[McpRegistryServerWrapper]) -> Result<(), ServiceError> {
        let cache_path = self.cache_path();

        let content = serde_json::to_string_pretty(results)
            .map_err(|e| ServiceError::Serialization(format!("Failed to serialize cache: {e}")))?;

        std::fs::write(&cache_path, content)
            .map_err(|e| ServiceError::Io(format!("Failed to write cache file {}: {e}", cache_path.display())))?;

        Ok(())
    }

    /// Convert registry wrapper to entry
    fn wrapper_to_entry(wrapper: &McpRegistryServerWrapper) -> McpRegistryEntry {
        McpRegistryEntry {
            name: wrapper.server.name.clone(),
            display_name: wrapper.server.name.clone(),
            description: wrapper.server.description.clone(),
            version: wrapper.server.version.clone(),
            author: wrapper
                .server
                .repository
                .url
                .as_ref()
                .map(|u| u.clone())
                .unwrap_or_default(),
            license: "Unknown".to_string(),
            repository: wrapper
                .server
                .repository
                .url
                .as_ref()
                .map(|u| u.clone())
                .unwrap_or_default(),
            command: wrapper
                .server
                .packages
                .first()
                .map(|p| p.identifier.clone())
                .unwrap_or_default(),
            args: vec![],
            env: None,
            tags: vec![],
        }
    }
}

#[async_trait::async_trait]
impl McpRegistryService for McpRegistryServiceImpl {
    /// Search for MCP servers in the registry
    async fn search(&self, query: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        // Try to load from cache first
        let cached = self.load_from_disk()?;
        if !cached.is_empty() {
            *self.cached_results.write().await = cached.clone();
        }

        // Perform search using the McpRegistry client
        let search_result = self.registry.search(query).await.map_err(|e| {
            ServiceError::Network(format!("Failed to search registry: {e}"))
        })?;

        // Convert to wrapper format
        let wrappers: Vec<McpRegistryServerWrapper> = search_result
            .entries
            .into_iter()
            .map(|entry| McpRegistryServerWrapper {
                server: entry.server,
                meta: entry.meta,
            })
            .collect();

        // Save to cache
        self.save_to_disk(&wrappers)?;

        // Update in-memory cache
        *self.cached_results.write().await = wrappers.clone();

        // Convert to output format
        let results: Vec<McpRegistryEntry> = wrappers
            .iter()
            .map(Self::wrapper_to_entry)
            .collect();

        Ok(results)
    }

    /// Get detailed information about a specific MCP server
    async fn get_details(&self, name: &str) -> ServiceResult<Option<McpRegistryEntry>> {
        // Ensure we have cached data
        if self.cached_results.read().await.is_empty() {
            let cached = self.load_from_disk()?;
            *self.cached_results.write().await = cached;
        }

        let results = self.cached_results.read().await;

        let wrapper = results
            .iter()
            .find(|w| w.server.name == name);

        Ok(wrapper.map(Self::wrapper_to_entry))
    }

    /// List all MCP servers in the registry
    async fn list_all(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        // Ensure we have cached data
        if self.cached_results.read().await.is_empty() {
            let cached = self.load_from_disk()?;
            *self.cached_results.write().await = cached;
        }

        let results = self.cached_results.read().await;

        Ok(results
            .iter()
            .map(Self::wrapper_to_entry)
            .collect())
    }

    /// List MCP servers by tag/category
    async fn list_by_tag(&self, tag: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        // Ensure we have cached data
        if self.cached_results.read().await.is_empty() {
            let cached = self.load_from_disk()?;
            *self.cached_results.write().await = cached;
        }

        let results = self.cached_results.read().await;

        // Filter by tag in metadata
        let tag_lower = tag.to_lowercase();
        Ok(results
            .iter()
            .filter(|w| {
                w.meta.get("tags")
                    .and_then(|t| t.as_array())
                    .is_some_and(|tags| tags.iter().any(|t| {
                        t.as_str()
                            .is_some_and(|s| s.to_lowercase().contains(&tag_lower))
                    }))
            })
            .map(Self::wrapper_to_entry)
            .collect())
    }

    /// Get trending/popular MCP servers
    async fn list_trending(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        // For now, just return all servers
        // Could be enhanced with actual trending data from registry
        self.list_all().await
    }

    /// Refresh the local MCP registry cache
    async fn refresh(&self) -> ServiceResult<()> {
        // Fetch all servers from registry
        let all_servers = self.registry.fetch_official().await.map_err(|e| {
            ServiceError::Network(format!("Failed to fetch registry: {e}"))
        })?;

        // Convert to wrapper format
        let wrappers: Vec<McpRegistryServerWrapper> = all_servers
            .into_iter()
            .map(|entry| McpRegistryServerWrapper {
                server: entry.server,
                meta: entry.meta,
            })
            .collect();

        // Save to cache
        self.save_to_disk(&wrappers)?;

        // Update in-memory cache
        *self.cached_results.write().await = wrappers.clone();

        // Update last refresh time
        *self.last_refresh.write().await = Some(chrono::Utc::now());

        Ok(())
    }

    /// Get the last refresh timestamp
    async fn get_last_refresh(&self) -> ServiceResult<Option<chrono::DateTime<chrono::Utc>>> {
        Ok(*self.last_refresh.read().await)
    }

    /// Install an MCP server from the registry
    ///
    /// This uses the MCP registry module to convert the server to a config and adds it to the app.
    async fn install(&self, name: &str, config_name: Option<String>) -> ServiceResult<()> {
        // Get the raw registry data to build proper McpConfig
        let cache = self.cached_results.read().await;
        let wrapper = cache.iter()
            .find(|w| w.server.name.eq_ignore_ascii_case(name) || 
                      w.server.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| ServiceError::NotFound(format!("MCP server '{name}' not found in registry")))?
            .clone();
        drop(cache);

        // Convert to the registry module's wrapper type and use entry_to_config
        let registry_wrapper = crate::mcp::registry::McpRegistryServerWrapper {
            server: wrapper.server.clone(),
            meta: wrapper.meta.clone(),
        };
        let mut mcp_config = McpRegistry::entry_to_config(&registry_wrapper)
            .map_err(|e| ServiceError::Internal(format!("Failed to convert server to config: {}", e)))?;

        // Override name if provided
        if let Some(display_name) = config_name {
            mcp_config.name = display_name;
        }

        // Load current app config
        let config_path = crate::config::Config::default_path()
            .map_err(|e| ServiceError::Internal(format!("Failed to get config path: {}", e)))?;
        
        let mut config = crate::config::Config::load(&config_path)
            .map_err(|e| ServiceError::Internal(format!("Failed to load config: {}", e)))?;

        // Check if MCP with same name already exists
        if config.mcps.iter().any(|m| m.name == mcp_config.name) {
            return Err(ServiceError::Validation(format!(
                "MCP '{}' already exists in config",
                mcp_config.name
            )));
        }

        // Add MCP config
        config.mcps.push(mcp_config);

        // Save config
        config.save(&config_path)
            .map_err(|e| ServiceError::Internal(format!("Failed to save config: {}", e)))?;

        // Reload MCP service to pick up new config
        let mcp_service = crate::mcp::McpService::global();
        let mut mcp = mcp_service.lock().await;
        mcp.reload().await
            .map_err(|e| ServiceError::Internal(format!("Failed to reload MCP service: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::registry::McpRegistryRepository;
    use tempfile::TempDir;

    fn create_test_wrapper(name: &str) -> McpRegistryServerWrapper {
        McpRegistryServerWrapper {
            server: crate::mcp::registry::McpRegistryServer {
                name: name.to_string(),
                description: format!("Test description for {name}"),
                version: "1.0.0".to_string(),
                repository: McpRegistryRepository {
                    url: Some(format!("https://github.com/example/{}", name)),
                    source: None,
                },
                packages: vec![],
                remotes: vec![],
            },
            meta: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn test_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();

        let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());

        // Create test wrappers and save to cache
        let wrappers = vec![
            create_test_wrapper("test-server-1"),
            create_test_wrapper("test-server-2"),
        ];
        service.save_to_disk(&wrappers).unwrap();

        // Load from cache
        let loaded = service.load_from_disk().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].server.name, "test-server-1");
        assert_eq!(loaded[1].server.name, "test-server-2");
    }

    #[tokio::test]
    async fn test_get_details() {
        let temp_dir = TempDir::new().unwrap();

        let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());

        // Create test wrappers and save to cache
        let wrappers = vec![create_test_wrapper("test-server")];
        service.save_to_disk(&wrappers).unwrap();

        // Get details
        let details = service.get_details("test-server").await.unwrap();
        assert!(details.is_some());
        assert_eq!(details.unwrap().name, "test-server");
    }

    #[tokio::test]
    async fn test_not_found() {
        let temp_dir = TempDir::new().unwrap();

        let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());

        // Create test wrappers and save to cache
        let wrappers = vec![create_test_wrapper("test-server")];
        service.save_to_disk(&wrappers).unwrap();

        // Try to get details for non-existent server
        let result = service.get_details("non-existent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_all() {
        let temp_dir = TempDir::new().unwrap();

        let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());

        // Create test wrappers and save to cache
        let wrappers = vec![
            create_test_wrapper("test-server-1"),
            create_test_wrapper("test-server-2"),
        ];
        service.save_to_disk(&wrappers).unwrap();

        // List all
        let all = service.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_wrapper_to_entry() {
        let wrapper = create_test_wrapper("test-server");

        let entry = McpRegistryServiceImpl::wrapper_to_entry(&wrapper);

        assert_eq!(entry.name, "test-server");
        assert_eq!(entry.display_name, "test-server");
        assert!(entry.description.contains("test-server"));
    }
}
