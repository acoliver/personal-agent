//! MCP Registry client for discovering servers

use crate::mcp::{
    detect_auth_type, EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageArg,
    McpPackageArgType, McpPackageType, McpSource, McpTransport, RegistryEnvVar,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Resolve Smithery API key from either a path or raw key
fn resolve_smithery_key(key_or_path: &str) -> Result<String, String> {
    let trimmed = key_or_path.trim();

    // Check if it looks like a path
    if trimmed.starts_with('/') || trimmed.starts_with("~/") || trimmed.starts_with("./") {
        // Expand ~ to home dir
        let path = if let Some(stripped) = trimmed.strip_prefix("~/") {
            dirs::home_dir().ok_or("No home directory")?.join(stripped)
        } else {
            std::path::PathBuf::from(trimmed)
        };

        std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read keyfile {}: {}", path.display(), e))
            .map(|s| s.trim().to_string())
    } else {
        // It's a raw key
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_smithery_key;
    use tempfile::TempDir;

    #[test]
    fn resolve_smithery_key_reads_file_contents() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("smithery.key");
        std::fs::write(&key_path, "secret").unwrap();

        let key = resolve_smithery_key(&key_path.to_string_lossy()).unwrap();
        assert_eq!(key, "secret");
    }
}

/// Response from the official MCP registry
#[derive(Debug, Clone, Deserialize)]
pub struct McpRegistryResponse {
    pub servers: Vec<McpRegistryServerWrapper>,
}

/// Smithery server response
#[derive(Debug, Clone, Deserialize)]
pub struct SmitheryResponse {
    pub servers: Vec<SmitheryServer>,
    pub pagination: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SmitheryServer {
    pub id: String,
    #[serde(rename = "qualifiedName")]
    pub qualified_name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: String,
    #[serde(rename = "iconUrl")]
    pub icon_url: Option<String>,
    pub verified: bool,
    #[serde(rename = "useCount")]
    pub use_count: i64,
    pub remote: bool,
    #[serde(rename = "isDeployed")]
    pub is_deployed: bool,
    pub homepage: String,
}

/// Wrapper for a server entry
#[derive(Debug, Clone, Deserialize)]
pub struct McpRegistryServerWrapper {
    pub server: McpRegistryServer,
    #[serde(rename = "_meta")]
    pub meta: serde_json::Value,
}

/// Server definition from the official MCP registry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryServer {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub repository: McpRegistryRepository,
    pub version: String,
    #[serde(default)]
    pub packages: Vec<McpRegistryPackage>,
    #[serde(default)]
    pub remotes: Vec<McpRegistryRemote>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct McpRegistryRepository {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryPackage {
    #[serde(rename = "registryType")]
    pub registry_type: String,
    pub identifier: String,
    #[serde(default)]
    pub version: Option<String>,
    pub transport: McpRegistryTransport,
    #[serde(default, rename = "environmentVariables")]
    pub environment_variables: Vec<McpRegistryEnvVar>,
    #[serde(default, rename = "packageArguments", alias = "package_arguments")]
    pub package_arguments: Vec<McpRegistryPackageArgument>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryPackageArgument {
    #[serde(rename = "type")]
    pub argument_type: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "isRequired")]
    pub is_required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryTransport {
    #[serde(rename = "type")]
    pub transport_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryRemote {
    #[serde(rename = "type")]
    pub remote_type: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryEnvVar {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "isSecret")]
    pub is_secret: bool,
    #[serde(default, rename = "isRequired")]
    pub is_required: bool,
}

/// Search results
#[derive(Debug, Clone)]
pub struct McpSearchResult {
    pub entries: Vec<McpRegistryServerWrapper>,
    pub source: McpRegistrySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpRegistrySource {
    Official,
    Smithery,
}

/// MCP Registry client
pub struct McpRegistry {
    http_client: reqwest::Client,
    official_url: String,
}

impl McpRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            official_url: "https://registry.modelcontextprotocol.io/v0.1/servers".to_string(),
        }
    }

    /// Search official registry with server-side search
    ///
    /// # Errors
    ///
    /// Returns an error if the registry request fails.
    pub async fn search_official(
        &self,
        query: &str,
    ) -> Result<Vec<McpRegistryServerWrapper>, String> {
        let url = format!(
            "{}?search={}&limit=100",
            self.official_url,
            urlencoding::encode(query)
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch official registry: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Official registry returned {}", response.status()));
        }

        let registry_response: McpRegistryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse official registry: {e}"))?;

        Ok(registry_response.servers)
    }

    /// Fetch all servers from official registry (no search, for browsing)
    ///
    /// # Errors
    ///
    /// Returns an error if the registry request fails.
    pub async fn fetch_official(&self) -> Result<Vec<McpRegistryServerWrapper>, String> {
        let url = format!("{}?limit=100", self.official_url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch official registry: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Official registry returned {}", response.status()));
        }

        let registry_response: McpRegistryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse official registry: {e}"))?;

        Ok(registry_response.servers)
    }

    /// Fetch from Smithery registry
    ///
    /// # Errors
    ///
    /// Returns an error if the registry request fails.
    pub async fn fetch_smithery(
        &self,
        query: &str,
        key_or_path: &str,
    ) -> Result<Vec<McpRegistryServerWrapper>, String> {
        let api_key = resolve_smithery_key(key_or_path)?;

        if api_key.is_empty() {
            return Err("Smithery API key is empty".to_string());
        }

        let url = format!(
            "https://registry.smithery.ai/servers?q={}",
            urlencoding::encode(query)
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Smithery: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Smithery returned {}", response.status()));
        }

        let smithery_response: SmitheryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Smithery response: {e}"))?;

        // Convert to our wrapper format
        // Note: Smithery hosted servers require OAuth, but the search API doesn't tell us
        // what specific auth is needed. We mark them as needing OAuth so the user
        // is prompted to configure.
        Ok(smithery_response
            .servers
            .into_iter()
            .map(|s| {
                McpRegistryServerWrapper {
                    server: McpRegistryServer {
                        name: s.display_name.clone(),
                        description: s.description,
                        repository: McpRegistryRepository::default(),
                        version: "latest".to_string(),
                        packages: vec![],
                        remotes: if s.remote {
                            vec![McpRegistryRemote {
                                remote_type: "smithery-oauth".to_string(), // Mark as needing Smithery OAuth
                                url: format!("https://server.smithery.ai/{}", s.qualified_name),
                            }]
                        } else {
                            vec![]
                        },
                    },
                    meta: serde_json::json!({
                        "source": "smithery",
                        "qualified_name": s.qualified_name,
                        "verified": s.verified,
                        "use_count": s.use_count,
                    }),
                }
            })
            .collect())
    }

    /// Search registries by query
    ///
    /// # Errors
    ///
    /// Returns an error if the registry request fails.
    pub async fn search(&self, query: &str) -> Result<McpSearchResult, String> {
        // Fetch from official registry
        let all_entries = self.fetch_official().await?;

        let query_lower = query.to_lowercase();
        // Deduplicate by server name (official registry has duplicates)
        let mut seen = std::collections::HashSet::new();
        let deduped: Vec<McpRegistryServerWrapper> = all_entries
            .into_iter()
            .filter(|e| {
                e.server.name.to_lowercase().contains(&query_lower)
                    || e.server.description.to_lowercase().contains(&query_lower)
                    || e.server
                        .repository
                        .url
                        .as_ref()
                        .is_some_and(|u| u.to_lowercase().contains(&query_lower))
            })
            .filter(|e| seen.insert(e.server.name.clone()))
            .collect();

        Ok(McpSearchResult {
            entries: deduped,
            source: McpRegistrySource::Official,
        })
    }

    /// Search with registry selection
    ///
    /// # Errors
    ///
    /// Returns an error if the registry request fails.
    pub async fn search_registry(
        &self,
        query: &str,
        registry: McpRegistrySource,
        smithery_key: Option<&str>,
    ) -> Result<McpSearchResult, String> {
        match registry {
            McpRegistrySource::Official => {
                // Use server-side search
                let results = self.search_official(query).await?;

                // Dedupe by name
                let mut seen = std::collections::HashSet::new();
                let deduped = results
                    .into_iter()
                    .filter(|e| seen.insert(e.server.name.clone()))
                    .collect();

                Ok(McpSearchResult {
                    entries: deduped,
                    source: McpRegistrySource::Official,
                })
            }
            McpRegistrySource::Smithery => {
                let key = smithery_key.ok_or("Smithery API key required")?;
                let entries = self.fetch_smithery(query, key).await?;
                Ok(McpSearchResult {
                    entries,
                    source: McpRegistrySource::Smithery,
                })
            }
        }
    }

    /// Convert registry server to `McpConfig`
    ///
    /// # Errors
    ///
    /// Returns an error if the entry cannot be mapped.
    pub fn entry_to_config(wrapper: &McpRegistryServerWrapper) -> Result<McpConfig, String> {
        let server = &wrapper.server;

        // Prefer packages over remotes
        if let Some(package) = server.packages.first() {
            return Self::package_entry_to_config(server, package);
        }

        if let Some(remote) = server.remotes.first() {
            return Self::remote_entry_to_config(server, remote);
        }

        Err("Server has neither packages nor remotes".to_string())
    }

    fn package_entry_to_config(
        server: &McpRegistryServer,
        package: &McpRegistryPackage,
    ) -> Result<McpConfig, String> {
        // Convert package type
        let package_type = match package.registry_type.as_str() {
            "npm" => McpPackageType::Npm,
            "oci" => McpPackageType::Docker,
            _ => {
                return Err(format!(
                    "Unsupported registry type: {}",
                    package.registry_type
                ))
            }
        };

        // Convert transport type
        let transport = match package.transport.transport_type.as_str() {
            "stdio" => McpTransport::Stdio,
            "http" | "streamable-http" => McpTransport::Http,
            _ => {
                return Err(format!(
                    "Unsupported transport type: {}",
                    package.transport.transport_type
                ))
            }
        };

        // Convert env vars
        let env_vars: Vec<EnvVarConfig> = package
            .environment_variables
            .iter()
            .map(|v| EnvVarConfig {
                name: v.name.clone(),
                required: v.is_required,
            })
            .collect();

        // Detect auth type from env vars
        let registry_env_vars: Vec<RegistryEnvVar> = package
            .environment_variables
            .iter()
            .map(|v| RegistryEnvVar {
                name: v.name.clone(),
                is_secret: v.is_secret,
                is_required: v.is_required,
            })
            .collect();

        let auth_type = detect_auth_type(&registry_env_vars);

        let package_args = package
            .package_arguments
            .iter()
            .map(|arg| McpPackageArg {
                arg_type: match arg.argument_type.as_str() {
                    "named" => McpPackageArgType::Named,
                    _ => McpPackageArgType::Positional,
                },
                name: arg.name.clone(),
                description: arg.description.clone(),
                required: arg.is_required,
                default: arg.default.clone(),
            })
            .collect();

        // Determine runtime hint based on package type
        let runtime_hint = match package_type {
            McpPackageType::Npm => Some("npx".to_string()),
            McpPackageType::Docker => Some("docker".to_string()),
            McpPackageType::Http => None,
        };

        Ok(McpConfig {
            id: Uuid::new_v4(),
            name: server.name.clone(),
            enabled: true,
            source: McpSource::Official {
                name: server.name.clone(),
                version: server.version.clone(),
            },
            package: McpPackage {
                package_type,
                identifier: package.identifier.clone(),
                runtime_hint,
            },
            transport,
            auth_type,
            env_vars,
            package_args,
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        })
    }

    fn remote_entry_to_config(
        server: &McpRegistryServer,
        remote: &McpRegistryRemote,
    ) -> Result<McpConfig, String> {
        // Handle remote servers
        let (transport, auth_type) = match remote.remote_type.as_str() {
            "http" | "streamable-http" => (McpTransport::Http, McpAuthType::None),
            "smithery-oauth" => (McpTransport::Http, McpAuthType::OAuth), // Smithery hosted servers need OAuth
            _ => return Err(format!("Unsupported remote type: {}", remote.remote_type)),
        };

        Ok(McpConfig {
            id: Uuid::new_v4(),
            name: server.name.clone(),
            enabled: true,
            source: McpSource::Manual {
                url: remote.url.clone(),
            },
            package: McpPackage {
                package_type: McpPackageType::Http,
                identifier: remote.url.clone(),
                runtime_hint: None,
            },
            transport,
            auth_type,
            env_vars: vec![],
            package_args: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        })
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}
