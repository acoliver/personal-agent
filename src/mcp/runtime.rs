//! MCP Runtime - spawns servers and handles tool calls

use serdes_ai::mcp::McpClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

use crate::config::Config;
use crate::mcp::{
    McpConfig, McpManager, McpStatus, McpStatusManager, McpTransport, SecretsManager,
};

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
            McpTransport::Http => Ok(Self::create_http_client(config, &env)),
            McpTransport::Stdio => self.create_stdio_client(config, env).await,
        }
    }

    fn create_http_client(config: &McpConfig, env: &HashMap<String, String>) -> McpClient {
        let mut headers = std::collections::HashMap::new();

        // Check for OAuth token first (highest priority for Smithery servers)
        if let Some(ref oauth_token) = config.oauth_token {
            headers.insert("Authorization".to_string(), format!("Bearer {oauth_token}"));
        } else {
            // Check if we have auth data that should be passed as headers
            // For Smithery and other HTTP MCPs, auth is typically passed via Authorization header
            for (key, value) in env {
                // Convert env var names to header names
                // Common patterns: API_KEY, TOKEN, ACCESS_TOKEN -> Authorization: Bearer <value>
                let key_lower = key.to_lowercase();
                if key_lower.contains("token")
                    || key_lower.contains("api_key")
                    || key_lower.contains("key")
                {
                    headers.insert("Authorization".to_string(), format!("Bearer {value}"));
                } else {
                    // Pass other env vars as custom headers with X- prefix
                    headers.insert(format!("X-{key}"), value.clone());
                }
            }
        }

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
        McpClient::new(transport)
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
        for (id, conn) in &self.connections {
            if conn.tools.iter().any(|t| t.name == tool_name) {
                return Some(*id);
            }
        }
        None
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
