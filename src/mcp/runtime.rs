//! MCP Runtime - spawns servers and handles tool calls

use serdes_ai::mcp::McpClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::config::Config;
use crate::mcp::{
    McpAuthType, McpConfig, McpManager, McpStatus, McpStatusManager, McpTransport, SecretsManager,
};

/// Active MCP connection
pub struct McpConnection {
    pub config: McpConfig,
    pub client: Arc<Mutex<McpClient>>,
    pub tools: Vec<McpTool>,
}

/// Provider metadata for an MCP tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolProvider {
    pub mcp_id: Uuid,
    pub mcp_name: String,
}

/// MCP Tool definition  
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub mcp_id: Uuid,
}

const MCP_INIT_TIMEOUT: Duration = Duration::from_secs(30);
const MCP_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// MCP Runtime manages active connections
pub struct McpRuntime {
    manager: McpManager,
    connections: HashMap<Uuid, McpConnection>,
    status_manager: McpStatusManager,
}

impl McpRuntime {
    #[must_use]
    pub fn new(secrets: SecretsManager) -> Self {
        Self {
            manager: McpManager::new(secrets),
            connections: HashMap::new(),
            status_manager: McpStatusManager::new(),
        }
    }

    /// Get a clone of the status manager for UI access
    #[must_use]
    pub fn status_manager(&self) -> McpStatusManager {
        self.status_manager.clone()
    }

    /// Start an MCP server
    ///
    /// # Errors
    ///
    /// Returns an error if the MCP is disabled, misconfigured, or fails to start.
    pub async fn start_mcp(&mut self, config: &McpConfig) -> Result<(), String> {
        if !config.enabled {
            self.status_manager
                .set_status(config.id, McpStatus::Stopped);
            return Err("MCP is disabled".to_string());
        }

        if self.connections.contains_key(&config.id) {
            return Ok(()); // Already running
        }

        // Update status to Starting
        self.status_manager
            .set_status(config.id, McpStatus::Starting);

        let env = self.prepare_env(config)?;
        let client = self.create_client(config, env).await?;
        let tools = self.initialize_client(config, &client).await?;

        // Register as active
        self.manager.register_active(config);

        // Store connection
        self.connections.insert(
            config.id,
            McpConnection {
                config: config.clone(),
                client: Arc::new(Mutex::new(client)),
                tools,
            },
        );

        self.status_manager
            .set_status(config.id, McpStatus::Running);

        Ok(())
    }

    fn prepare_env(&self, config: &McpConfig) -> Result<HashMap<String, String>, String> {
        // Validate required package_args before spawning
        for arg in &config.package_args {
            if arg.required {
                let arg_value = config
                    .config
                    .get("package_args")
                    .and_then(|args| args.get(&arg.name))
                    .and_then(|v| v.as_str());

                if arg_value.is_none_or(|value| value.trim().is_empty()) {
                    let arg_name = &arg.name;
                    let err = format!("Missing required package argument: {arg_name}");
                    self.status_manager
                        .set_status(config.id, McpStatus::Error(err.clone()));
                    return Err(err);
                }
            }
        }

        // Build environment
        let env = self.manager.build_env(config).map_err(|e| {
            let err = e.to_string();
            self.status_manager
                .set_status(config.id, McpStatus::Error(err.clone()));
            err
        })?;

        Ok(env)
    }

    async fn create_client(
        &self,
        config: &McpConfig,
        env: HashMap<String, String>,
    ) -> Result<McpClient, String> {
        match config.transport {
            McpTransport::Http => Self::create_http_client(config, &env).inspect_err(|e| {
                self.status_manager
                    .set_status(config.id, McpStatus::Error(e.clone()));
            }),
            McpTransport::Stdio => self.create_stdio_client(config, env).await,
        }
    }

    fn create_http_client(
        config: &McpConfig,
        env: &HashMap<String, String>,
    ) -> Result<McpClient, String> {
        let headers = Self::http_auth_headers(config, env)?;

        // Create HTTP transport with custom headers if needed
        let transport = if headers.is_empty() {
            serdes_ai::mcp::transport::HttpTransport::new(&config.package.identifier)
        } else {
            // Use with_headers for custom auth headers
            serdes_ai::mcp::transport::HttpTransport::with_headers(
                &config.package.identifier,
                headers,
            )
        };
        Ok(McpClient::new(transport))
    }

    /// Compute the Authorization credential plus custom headers for an HTTP MCP.
    ///
    /// Exactly one credential source wins, in priority order: the OAuth
    /// token, the keyfile bearer (Keyfile auth reads `keyfile_path` here so
    /// the live path matches [`crate::mcp::toolset::build_headers_for_config`]),
    /// or the single `ApiKey` secret env var through the shared
    /// [`crate::mcp::toolset::derive_http_auth_header`] rule. Every other env
    /// var becomes an `X-{NAME}` custom header and can never produce a second
    /// `Authorization`, no matter how token-ish its name looks.
    fn http_auth_headers(
        config: &McpConfig,
        env: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, String> {
        let mut headers = HashMap::new();
        let mut credential_var: Option<&str> = None;

        // Priority: oauth_token > keyfile > single ApiKey secret var
        if let Some(ref oauth_token) = config.oauth_token {
            headers.insert("Authorization".to_string(), format!("Bearer {oauth_token}"));
        } else if config.auth_type == McpAuthType::Keyfile {
            let path = config.keyfile_path.as_ref().ok_or_else(|| {
                format!(
                    "MCP {} uses Keyfile auth without a keyfile path",
                    config.name
                )
            })?;
            let token = std::fs::read_to_string(path).map_err(|e| {
                format!(
                    "MCP {}: cannot read keyfile {}: {e}",
                    config.name,
                    path.display()
                )
            })?;
            headers.insert(
                "Authorization".to_string(),
                format!("Bearer {}", token.trim()),
            );
        } else if config.auth_type == McpAuthType::ApiKey {
            let secret_vars: Vec<&crate::mcp::EnvVarConfig> =
                config.env_vars.iter().filter(|var| var.is_secret).collect();
            let [var] = secret_vars.as_slice() else {
                return Err(format!(
                    "MCP {} needs exactly one secret env var for HTTP API key auth, found {}",
                    config.name,
                    secret_vars.len()
                ));
            };
            let value = env.get(&var.name).ok_or_else(|| {
                format!(
                    "MCP {}: secret env var {} is missing from the resolved environment",
                    config.name, var.name
                )
            })?;
            let (header, derived) = crate::mcp::toolset::derive_http_auth_header(&var.name, value);
            headers.insert(header, derived);
            credential_var = Some(var.name.as_str());
        }

        for (name, value) in env {
            if credential_var == Some(name.as_str()) {
                continue;
            }
            headers.insert(format!("X-{}", name.to_uppercase()), value.clone());
        }

        Ok(headers)
    }
    async fn create_stdio_client(
        &self,
        config: &McpConfig,
        env: HashMap<String, String>,
    ) -> Result<McpClient, String> {
        // Build command
        let (cmd, args) = McpManager::build_command(config);

        if cmd.is_empty() {
            self.status_manager
                .set_status(config.id, McpStatus::Error("Empty command".to_string()));
            return Err("Empty command for stdio transport".to_string());
        }

        // Convert args to &str
        let args_str: Vec<&str> = args.iter().map(String::as_str).collect();

        // Use spawn_with_env to pass environment variables to the MCP process
        let transport = serdes_ai::mcp::StdioTransport::spawn_with_env(&cmd, &args_str, env)
            .await
            .map_err(|e| {
                let err = format!("Failed to spawn MCP: {e}");
                self.status_manager
                    .set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?;

        Ok(McpClient::new(transport))
    }

    async fn initialize_client(
        &self,
        config: &McpConfig,
        client: &McpClient,
    ) -> Result<Vec<McpTool>, String> {
        // Initialize the client
        timeout(MCP_INIT_TIMEOUT, client.initialize())
            .await
            .map_err(|_| {
                let err = "Failed to initialize MCP: timeout".to_string();
                self.status_manager
                    .set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?
            .map_err(|e| {
                let err = format!("Failed to initialize MCP: {e}");
                self.status_manager
                    .set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?;

        // List tools from the MCP server
        let mcp_tools = timeout(MCP_INIT_TIMEOUT, client.list_tools())
            .await
            .map_err(|_| {
                let err = "Failed to list tools: timeout".to_string();
                self.status_manager
                    .set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?
            .map_err(|e| {
                let err = format!("Failed to list tools: {e}");
                self.status_manager
                    .set_status(config.id, McpStatus::Error(err.clone()));
                err
            })?;

        // Convert to our McpTool format
        let tools: Vec<McpTool> = mcp_tools
            .into_iter()
            .map(|t| McpTool {
                name: t.name,
                description: t.description.unwrap_or_default(),
                input_schema: t.input_schema,
                mcp_id: config.id,
            })
            .collect();

        Ok(tools)
    }

    /// Stop an MCP server
    ///
    /// # Errors
    ///
    /// Returns an error if the MCP cannot be stopped.
    pub fn stop_mcp(&mut self, id: &Uuid) -> Result<(), String> {
        self.connections.remove(id);
        self.status_manager.set_status(*id, McpStatus::Stopped);
        self.manager.stop(id).map_err(|e| e.to_string())
    }

    /// Start all enabled MCPs from config
    pub async fn start_all(&mut self, config: &Config) -> Vec<(Uuid, Result<(), String>)> {
        let mcps: Vec<McpConfig> = config.get_enabled_mcps().into_iter().cloned().collect();

        let mut results = Vec::new();
        for mcp in &mcps {
            let result = self.start_mcp(mcp).await;
            results.push((mcp.id, result));
        }
        results
    }

    /// Get all available tools from active MCPs
    #[must_use]
    pub fn get_all_tools(&self) -> Vec<McpTool> {
        self.connections
            .values()
            .flat_map(|c| c.tools.iter().cloned())
            .collect()
    }

    /// Find which MCP provides a tool
    #[must_use]
    pub fn find_tool_provider(&self, tool_name: &str) -> Option<Uuid> {
        self.find_tool_provider_metadata(tool_name)
            .map(|provider| provider.mcp_id)
    }

    /// Find MCP provider metadata for a tool.
    #[must_use]
    pub fn find_tool_provider_metadata(&self, tool_name: &str) -> Option<McpToolProvider> {
        self.connections.iter().find_map(|(id, conn)| {
            conn.tools
                .iter()
                .any(|tool| tool.name == tool_name)
                .then(|| McpToolProvider {
                    mcp_id: *id,
                    mcp_name: conn.config.name.clone(),
                })
        })
    }

    /// Call a tool on an MCP
    ///
    /// # Errors
    ///
    /// Returns an error if the tool cannot be executed or times out.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mcp_id = self
            .find_tool_provider(tool_name)
            .ok_or_else(|| format!("No MCP provides tool: {tool_name}"))?;

        // Update last used time
        self.manager.touch(&mcp_id);

        // Get the connection
        let conn = self
            .connections
            .get(&mcp_id)
            .ok_or_else(|| format!("MCP connection not found: {mcp_id}"))?;

        // Call the tool via SerdesAI MCP client
        let result = timeout(
            MCP_TOOL_TIMEOUT,
            conn.client.lock().await.call_tool(tool_name, arguments),
        )
        .await
        .map_err(|_| {
            let err = "MCP tool call timed out".to_string();
            self.status_manager
                .set_status(mcp_id, McpStatus::Error(err.clone()));
            err
        })?
        .map_err(|e| {
            let err = format!("MCP tool call failed: {e}");
            self.status_manager
                .set_status(mcp_id, McpStatus::Error(err.clone()));
            err
        })?;

        // Convert CallToolResult to JSON
        // The result contains content array with text/image/resource items
        Ok(serde_json::to_value(result).unwrap_or_default())
    }

    /// Check if any MCPs are active
    #[must_use]
    pub fn has_active_mcps(&self) -> bool {
        !self.connections.is_empty()
    }

    /// Get active MCP count
    #[must_use]
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
    use crate::mcp::{
        EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageArg, McpSource, McpTransport,
    };

    fn make_config(id: Uuid, name: &str) -> McpConfig {
        McpConfig {
            id,
            name: name.to_string(),
            enabled: true,
            source: McpSource::Manual {
                url: "https://example.com".to_string(),
            },
            package: McpPackage {
                package_type: crate::mcp::McpPackageType::Http,
                identifier: "https://example.com".to_string(),
                runtime_hint: None,
            },
            transport: McpTransport::Http,
            auth_type: McpAuthType::None,
            env_vars: Vec::<EnvVarConfig>::new(),
            package_args: Vec::<McpPackageArg>::new(),
            keyfile_path: None,
            config: serde_json::Value::Null,
            oauth_token: None,
        }
    }

    #[test]
    fn find_tool_provider_metadata_returns_name_and_id() {
        let mut runtime = McpRuntime::new(crate::mcp::SecretsManager::new());
        let mcp_id = Uuid::new_v4();

        runtime.connections.insert(
            mcp_id,
            McpConnection {
                config: make_config(mcp_id, "weather"),
                client: Arc::new(Mutex::new(McpClient::new(
                    serdes_ai::mcp::transport::HttpTransport::new("https://example.com"),
                ))),
                tools: vec![McpTool {
                    name: "get_forecast".to_string(),
                    description: "Get forecast".to_string(),
                    input_schema: serde_json::json!({"type":"object"}),
                    mcp_id,
                }],
            },
        );

        let provider = runtime
            .find_tool_provider_metadata("get_forecast")
            .expect("provider should be found");

        assert_eq!(provider.mcp_id, mcp_id);
        assert_eq!(provider.mcp_name, "weather");
    }

    #[test]
    fn find_tool_provider_returns_none_for_unknown_tool() {
        let runtime = McpRuntime::new(crate::mcp::SecretsManager::new());
        assert!(runtime.find_tool_provider_metadata("missing").is_none());
        assert!(runtime.find_tool_provider("missing").is_none());
    }

    fn secret_var(name: &str) -> EnvVarConfig {
        EnvVarConfig {
            name: name.to_string(),
            required: true,
            is_secret: true,
            value: None,
        }
    }

    fn plain_var(name: &str, value: &str) -> EnvVarConfig {
        EnvVarConfig {
            name: name.to_string(),
            required: true,
            is_secret: false,
            value: Some(value.to_string()),
        }
    }

    #[test]
    fn http_auth_headers_error_when_api_key_has_multiple_secrets() {
        let mut config = make_config(Uuid::new_v4(), "multi-secret");
        config.auth_type = McpAuthType::ApiKey;
        config.env_vars = vec![secret_var("PRIMARY_TOKEN"), secret_var("SECONDARY_TOKEN")];

        let err = McpRuntime::http_auth_headers(&config, &HashMap::new())
            .expect_err("zero or multiple secret vars must fail fast for Http + ApiKey");
        assert!(
            err.contains("multi-secret") && err.contains("found 2"),
            "the error must name the MCP and the offending count, got: {err}"
        );
    }

    #[test]
    fn http_auth_headers_single_secret_yields_one_authorization_and_x_headers() {
        let mut config = make_config(Uuid::new_v4(), "exa");
        config.auth_type = McpAuthType::ApiKey;
        config.env_vars = vec![
            secret_var("EXA_API_KEY"),
            plain_var("WORKSPACE_TOKEN", "acme"),
        ];
        let env = HashMap::from([
            ("EXA_API_KEY".to_string(), "sk-1".to_string()),
            ("WORKSPACE_TOKEN".to_string(), "acme".to_string()),
        ]);

        let headers = McpRuntime::http_auth_headers(&config, &env)
            .expect("single-secret ApiKey config must produce headers");

        // Exactly one Authorization: if the token-named plain var were also
        // run through the bearer rule it would overwrite this entry.
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer sk-1".to_string())
        );
        assert_eq!(headers.get("X-WORKSPACE_TOKEN"), Some(&"acme".to_string()));
    }

    #[test]
    fn http_auth_headers_keyfile_bearer_is_read_from_keyfile_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let keyfile = temp.path().join("key.txt");
        std::fs::write(&keyfile, "file-token\n").expect("write keyfile");

        let mut config = make_config(Uuid::new_v4(), "keyfile-mcp");
        config.auth_type = McpAuthType::Keyfile;
        config.keyfile_path = Some(keyfile);

        let headers = McpRuntime::http_auth_headers(&config, &HashMap::new())
            .expect("readable keyfile must yield a bearer header");
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer file-token".to_string())
        );
    }

    #[test]
    fn http_auth_headers_error_when_keyfile_is_missing_or_unreadable() {
        let mut config = make_config(Uuid::new_v4(), "keyfile-mcp");
        config.auth_type = McpAuthType::Keyfile;

        assert!(
            McpRuntime::http_auth_headers(&config, &HashMap::new()).is_err(),
            "Keyfile auth without a keyfile path must fail fast"
        );

        config.keyfile_path = Some(std::path::PathBuf::from("/nonexistent/dir/key.txt"));
        assert!(
            McpRuntime::http_auth_headers(&config, &HashMap::new()).is_err(),
            "an unreadable keyfile must fail fast instead of dropping the credential"
        );
    }

    #[test]
    fn http_auth_headers_oauth_token_wins_and_env_vars_stay_custom() {
        let mut config = make_config(Uuid::new_v4(), "oauth-mcp");
        config.oauth_token = Some("oauth-token".to_string());
        config.env_vars = vec![secret_var("EXA_API_KEY"), plain_var("WORKSPACE", "acme")];
        let env = HashMap::from([
            ("EXA_API_KEY".to_string(), "sk-1".to_string()),
            ("WORKSPACE".to_string(), "acme".to_string()),
        ]);

        let headers = McpRuntime::http_auth_headers(&config, &env)
            .expect("oauth config must produce headers");
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer oauth-token".to_string())
        );
        assert_eq!(headers.get("X-EXA_API_KEY"), Some(&"sk-1".to_string()));
        assert_eq!(headers.get("X-WORKSPACE"), Some(&"acme".to_string()));
    }

    #[test]
    fn http_auth_headers_error_when_api_key_secret_is_missing_from_env() {
        let mut config = make_config(Uuid::new_v4(), "exa");
        config.auth_type = McpAuthType::ApiKey;
        config.env_vars = vec![secret_var("EXA_API_KEY")];

        let err = McpRuntime::http_auth_headers(&config, &HashMap::new())
            .expect_err("a declared secret var absent from env must fail fast");
        assert!(
            err.contains("EXA_API_KEY"),
            "the error must name the missing secret var, got: {err}"
        );
    }
}
