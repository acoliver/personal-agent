//! MCP Manager - handles spawning, lifecycle, and tool routing for MCP servers

use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;
use uuid::Uuid;

use crate::mcp::toolset;
use crate::mcp::{McpConfig, SecretsManager};

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Failed to spawn MCP server: {0}")]
    SpawnFailed(String),
    #[error("MCP not found: {0}")]
    NotFound(Uuid),
    #[error("Secrets error: {0}")]
    Secrets(#[from] crate::mcp::secrets::SecretsError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("MCP server error: {0}")]
    ServerError(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

pub type McpResult<T> = Result<T, McpError>;

/// Information about an active MCP server
struct ActiveMcp {
    last_used: Instant,
    restart_count: u32,
}

/// Manager for MCP server lifecycle
pub struct McpManager {
    secrets: SecretsManager,
    active: HashMap<Uuid, ActiveMcp>,
    idle_timeout: Duration,
}

impl McpManager {
    #[must_use]
    pub fn new(secrets: SecretsManager) -> Self {
        Self {
            secrets,
            active: HashMap::new(),
            idle_timeout: Duration::from_secs(30 * 60), // 30 minutes
        }
    }

    #[must_use]
    pub fn with_idle_timeout(secrets: SecretsManager, timeout: Duration) -> Self {
        Self {
            secrets,
            active: HashMap::new(),
            idle_timeout: timeout,
        }
    }

    /// Build environment variables for an MCP based on its auth config
    ///
    /// # Errors
    ///
    /// Returns `McpError` if secrets cannot be loaded.
    pub fn build_env(&self, config: &McpConfig) -> McpResult<HashMap<String, String>> {
        toolset::build_env_for_config(config, &self.secrets)
    }

    /// Build the command and arguments for an MCP based on its package type
    #[must_use]
    pub fn build_command(config: &McpConfig) -> (String, Vec<String>) {
        toolset::build_command(config)
    }

    /// Check if an MCP is currently active
    #[must_use]
    pub fn is_active(&self, id: &Uuid) -> bool {
        self.active.contains_key(id)
    }

    /// Get count of active MCPs
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Get the last used time for an MCP
    #[must_use]
    pub fn get_last_used(&self, id: &Uuid) -> Option<Instant> {
        self.active.get(id).map(|a| a.last_used)
    }

    /// Update the last used time for an MCP
    pub fn touch(&mut self, id: &Uuid) {
        if let Some(active) = self.active.get_mut(id) {
            active.last_used = Instant::now();
        }
    }

    /// Get restart count for an MCP
    #[must_use]
    pub fn get_restart_count(&self, id: &Uuid) -> u32 {
        self.active.get(id).map_or(0, |a| a.restart_count)
    }

    /// Register an MCP as active (for tracking purposes)
    pub fn register_active(&mut self, config: &McpConfig) {
        let now = Instant::now();
        self.active.insert(
            config.id,
            ActiveMcp {
                last_used: now,
                restart_count: 0,
            },
        );
    }

    /// Stop an MCP
    ///
    /// # Errors
    ///
    /// Returns `McpError` if shutdown fails.
    pub fn stop(&mut self, id: &Uuid) -> McpResult<()> {
        self.active.remove(id);
        Ok(())
    }

    /// Shutdown all MCPs
    ///
    /// # Errors
    ///
    /// Returns `McpError` if shutdown fails.
    pub fn shutdown_all(&mut self) -> McpResult<()> {
        self.active.clear();
        Ok(())
    }

    /// Clean up idle MCPs
    pub fn cleanup_idle(&mut self) {
        let now = Instant::now();
        let idle_ids: Vec<Uuid> = self
            .active
            .iter()
            .filter(|(_, a)| now.duration_since(a.last_used) > self.idle_timeout)
            .map(|(id, _)| *id)
            .collect();

        for id in idle_ids {
            self.active.remove(&id);
        }
    }

    /// Handle config change (e.g., MCP disabled)
    ///
    /// # Errors
    ///
    /// Returns `McpError` if shutdown fails.
    pub fn handle_config_change(&mut self, config: &McpConfig) -> McpResult<()> {
        if !config.enabled && self.is_active(&config.id) {
            self.stop(&config.id)?;
        }
        Ok(())
    }

    /// Delete an MCP (stop + delete credentials)
    ///
    /// # Errors
    ///
    /// Returns `McpError` if credentials cannot be removed.
    pub fn delete_mcp(&mut self, config: &McpConfig) -> McpResult<()> {
        self.stop(&config.id)?;
        self.secrets.delete_api_key(config.id)?;
        Ok(())
    }
}
