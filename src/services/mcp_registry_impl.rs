//! MCP registry service implementation

use super::{McpRegistryEntry, McpRegistryService, ServiceError, ServiceResult};
use crate::mcp::registry::McpRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

const CACHE_FILENAME: &str = "mcp_registry.json";

/// HTTP cache implementation of `McpRegistryService`
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
    /// Create a new `McpRegistryServiceImpl` with default cache directory
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

    /// Create a new `McpRegistryServiceImpl` with a specific cache directory
    #[must_use]
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self::with_registry(cache_dir, McpRegistry::new())
    }

    /// Create a new `McpRegistryServiceImpl` with a pre-configured registry (useful for testing)
    #[must_use]
    pub fn with_registry(cache_dir: PathBuf, registry: McpRegistry) -> Self {
        Self {
            cache_dir,
            cached_results: Arc::new(RwLock::new(Vec::new())),
            registry,
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

        let content = std::fs::read_to_string(&cache_path).map_err(|e| {
            ServiceError::Io(format!(
                "Failed to read cache file {}: {e}",
                cache_path.display()
            ))
        })?;

        serde_json::from_str(&content)
            .map_err(|e| ServiceError::Serialization(format!("Failed to parse cache file: {e}")))
    }

    /// Save results to disk
    fn save_to_disk(&self, results: &[McpRegistryServerWrapper]) -> Result<(), ServiceError> {
        let cache_path = self.cache_path();

        let content = serde_json::to_string_pretty(results)
            .map_err(|e| ServiceError::Serialization(format!("Failed to serialize cache: {e}")))?;

        std::fs::write(&cache_path, content).map_err(|e| {
            ServiceError::Io(format!(
                "Failed to write cache file {}: {e}",
                cache_path.display()
            ))
        })?;

        Ok(())
    }

    async fn prime_cache_from_disk(&self) -> Result<(), ServiceError> {
        let cached = self.load_from_disk()?;
        if !cached.is_empty() {
            self.cached_results.write().await.clone_from(&cached);
        }
        Ok(())
    }

    fn load_smithery_key() -> Result<String, ServiceError> {
        let config_path = crate::config::Config::default_path().map_err(|e| {
            ServiceError::Io(format!(
                "Failed to resolve config path for Smithery search: {e}"
            ))
        })?;
        let config = crate::config::Config::load(&config_path).map_err(|e| {
            ServiceError::Io(format!("Failed to load config for Smithery search: {e}"))
        })?;

        config.smithery_auth.ok_or_else(|| {
            ServiceError::Validation(
                "Smithery search requires smithery_auth in config/settings.json".to_string(),
            )
        })
    }

    fn response_to_wrappers(
        response: crate::mcp::registry::McpSearchResult,
    ) -> Vec<McpRegistryServerWrapper> {
        response
            .entries
            .into_iter()
            .map(|entry| McpRegistryServerWrapper {
                server: entry.server,
                meta: entry.meta,
            })
            .collect()
    }

    async fn search_official_wrappers(
        &self,
        query: &str,
    ) -> ServiceResult<Vec<McpRegistryServerWrapper>> {
        let response = self
            .registry
            .search_registry(
                query,
                crate::mcp::registry::McpRegistrySource::Official,
                None,
            )
            .await
            .map_err(|e| ServiceError::Network(format!("Failed to search registry: {e}")))?;

        Ok(Self::response_to_wrappers(response))
    }

    async fn search_smithery_wrappers(
        &self,
        query: &str,
        smithery_key: &str,
    ) -> ServiceResult<Vec<McpRegistryServerWrapper>> {
        let response = self
            .registry
            .search_registry(
                query,
                crate::mcp::registry::McpRegistrySource::Smithery,
                Some(smithery_key),
            )
            .await
            .map_err(|e| ServiceError::Network(format!("Failed to search registry: {e}")))?;

        Ok(Self::response_to_wrappers(response))
    }

    async fn search_both_wrappers(
        &self,
        query: &str,
    ) -> ServiceResult<Vec<McpRegistryServerWrapper>> {
        let mut combined = self.search_official_wrappers(query).await?;

        if let Ok(smithery_key) = Self::load_smithery_key() {
            if let Ok(smithery) = self.search_smithery_wrappers(query, &smithery_key).await {
                combined.extend(smithery);
            }
        }

        Ok(combined)
    }

    fn dedupe_wrappers(results: Vec<McpRegistryServerWrapper>) -> Vec<McpRegistryServerWrapper> {
        let mut seen = std::collections::HashSet::new();
        results
            .into_iter()
            .filter(|entry| seen.insert(entry.server.name.clone()))
            .collect()
    }

    async fn cache_search_results(
        &self,
        wrappers: &[McpRegistryServerWrapper],
    ) -> Result<(), ServiceError> {
        self.save_to_disk(wrappers)?;
        *self.cached_results.write().await = wrappers.to_vec();
        Ok(())
    }

    /// Convert registry wrapper to entry
    fn wrapper_to_entry(wrapper: &McpRegistryServerWrapper) -> McpRegistryEntry {
        let remote_url = wrapper.server.remotes.first().map(|r| r.url.clone());
        let primary_package = wrapper.server.packages.first();
        let package_type =
            primary_package.and_then(|package| match package.registry_type.as_str() {
                "npm" => Some(crate::mcp::McpPackageType::Npm),
                "oci" => Some(crate::mcp::McpPackageType::Docker),
                _ => None,
            });
        let runtime_hint = match package_type {
            Some(crate::mcp::McpPackageType::Npm) => Some("npx".to_string()),
            Some(crate::mcp::McpPackageType::Docker) => Some("docker".to_string()),
            Some(crate::mcp::McpPackageType::Http) | None => None,
        };
        let env = primary_package.map(|package| {
            package
                .environment_variables
                .iter()
                .map(|var| (var.name.clone(), String::new()))
                .collect::<Vec<_>>()
        });
        let args = primary_package
            .map(|package| {
                package
                    .package_arguments
                    .iter()
                    .map(|arg| arg.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let tags = wrapper
            .meta
            .get("tags")
            .and_then(|value| value.as_array())
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let source = wrapper
            .meta
            .get("source")
            .and_then(|value| value.as_str())
            .unwrap_or("official")
            .to_string();

        McpRegistryEntry {
            name: wrapper.server.name.clone(),
            display_name: wrapper.server.name.clone(),
            description: wrapper.server.description.clone(),
            version: wrapper.server.version.clone(),
            author: wrapper.server.repository.url.clone().unwrap_or_default(),
            license: "Unknown".to_string(),
            repository: wrapper.server.repository.url.clone().unwrap_or_default(),
            command: primary_package
                .map(|p| p.identifier.clone())
                .unwrap_or_default(),
            args,
            env,
            tags,
            source,
            package_type,
            runtime_hint,
            url: remote_url,
        }
    }
}

#[async_trait::async_trait]
impl McpRegistryService for McpRegistryServiceImpl {
    /// Search for MCP servers in the registry
    async fn search(&self, query: &str) -> ServiceResult<Vec<McpRegistryEntry>> {
        self.search_registry(query, "official").await
    }

    async fn search_registry(
        &self,
        query: &str,
        source: &str,
    ) -> ServiceResult<Vec<McpRegistryEntry>> {
        self.prime_cache_from_disk().await?;

        let normalized_source = source.trim().to_lowercase();
        let query = query.trim();
        let wrappers = match normalized_source.as_str() {
            "smithery" => {
                let smithery_key = Self::load_smithery_key()?;
                self.search_smithery_wrappers(query, &smithery_key).await?
            }
            "both" => self.search_both_wrappers(query).await?,
            _ => self.search_official_wrappers(query).await?,
        };

        let wrappers = Self::dedupe_wrappers(wrappers);
        self.cache_search_results(&wrappers).await?;

        Ok(wrappers.iter().map(Self::wrapper_to_entry).collect())
    }

    /// Get detailed information about a specific MCP server
    async fn get_details(&self, name: &str) -> ServiceResult<Option<McpRegistryEntry>> {
        // Ensure we have cached data
        if self.cached_results.read().await.is_empty() {
            let cached = self.load_from_disk()?;
            *self.cached_results.write().await = cached;
        }

        let wrapper = self
            .cached_results
            .read()
            .await
            .iter()
            .find(|w| w.server.name == name)
            .cloned();

        Ok(wrapper.as_ref().map(Self::wrapper_to_entry))
    }

    /// List all MCP servers in the registry
    async fn list_all(&self) -> ServiceResult<Vec<McpRegistryEntry>> {
        // Ensure we have cached data
        if self.cached_results.read().await.is_empty() {
            let cached = self.load_from_disk()?;
            *self.cached_results.write().await = cached;
        }

        let results = self.cached_results.read().await;

        Ok(results.iter().map(Self::wrapper_to_entry).collect())
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
                w.meta
                    .get("tags")
                    .and_then(|t| t.as_array())
                    .is_some_and(|tags| {
                        tags.iter().any(|t| {
                            t.as_str()
                                .is_some_and(|s| s.to_lowercase().contains(&tag_lower))
                        })
                    })
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
        let all_servers = self
            .registry
            .fetch_official()
            .await
            .map_err(|e| ServiceError::Network(format!("Failed to fetch registry: {e}")))?;

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
        self.cached_results.write().await.clone_from(&wrappers);

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
        let wrapper = cache
            .iter()
            .find(|w| {
                w.server.name.eq_ignore_ascii_case(name)
                    || w.server.name.to_lowercase().contains(&name.to_lowercase())
            })
            .ok_or_else(|| {
                ServiceError::NotFound(format!("MCP server '{name}' not found in registry"))
            })?
            .clone();
        drop(cache);

        // Convert to the registry module's wrapper type and use entry_to_config
        let registry_wrapper = crate::mcp::registry::McpRegistryServerWrapper {
            server: wrapper.server.clone(),
            meta: wrapper.meta.clone(),
        };
        let mut mcp_config = McpRegistry::entry_to_config(&registry_wrapper).map_err(|e| {
            ServiceError::Internal(format!("Failed to convert server to config: {e}"))
        })?;

        // Override name if provided
        if let Some(display_name) = config_name {
            mcp_config.name = display_name;
        }

        // Load current app config
        let config_path = crate::config::Config::default_path()
            .map_err(|e| ServiceError::Internal(format!("Failed to get config path: {e}")))?;

        let mut config = crate::config::Config::load(&config_path)
            .map_err(|e| ServiceError::Internal(format!("Failed to load config: {e}")))?;

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
        config
            .save(&config_path)
            .map_err(|e| ServiceError::Internal(format!("Failed to save config: {e}")))?;

        // Reload MCP service to pick up new config
        crate::mcp::McpService::global()
            .lock()
            .await
            .reload()
            .await
            .map_err(|e| ServiceError::Internal(format!("Failed to reload MCP service: {e}")))?;

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
                    url: Some(format!("https://github.com/example/{name}")),
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

    #[tokio::test]
    async fn test_with_registry_uses_injected_registry() {
        let temp_dir = TempDir::new().unwrap();
        let registry = McpRegistry::with_url("http://localhost:1/fake");
        let service =
            McpRegistryServiceImpl::with_registry(temp_dir.path().to_path_buf(), registry);

        // Verify the service works for cache operations (disk-level, no network)
        let wrappers = vec![create_test_wrapper("injected-test")];
        service.save_to_disk(&wrappers).unwrap();

        let loaded = service.load_from_disk().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].server.name, "injected-test");
    }
}
