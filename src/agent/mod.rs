//! Agent module for `PersonalAgent`.
//!
//! This module provides the main `PersonalAgent` struct that wraps `SerdesAI`'s Agent
//! with our application-specific configuration and MCP toolsets.
//!
//! # Architecture
//! - `runtime.rs`: Global tokio runtime that persists for application lifetime
//! - `mod.rs` (this file): `PersonalAgent` wrapper and global singleton
//!
//! # Global Runtime Pattern
//! Instead of creating temporary runtimes that get dropped, we use a single
//! global runtime accessed via `agent_runtime()`. This prevents MCP clients
//! from dying when their spawning runtime shuts down.
//!
//! # Usage
//! ```ignore
//! use personal_agent::agent::{init_global_agent, global_agent};
//!
//! // Initialize once at application startup
//! init_global_agent(&profile, &mcp_configs).await?;
//!
//! // Access from anywhere
//! let agent_lock = global_agent().read().await;
//! if let Some(agent) = &*agent_lock {
//!     // Use agent
//! }
//! ```

pub mod runtime;
pub use runtime::{agent_runtime, run_in_agent_runtime, spawn_in_agent_runtime};

use crate::mcp::McpConfig;
use crate::models::ModelProfile;

#[derive(Debug)]
pub enum AgentError {
    ModelError(String),
    ToolsetError(String),
    BuildError(String),
}

/// `PersonalAgent` wraps `SerdesAI`'s Agent with application-specific configuration.
///
/// This is a placeholder implementation until `SerdesAI` PR #5 (`McpToolset` support)
/// is merged. Once available, this will construct a full Agent with MCP toolsets.
pub struct PersonalAgent {
    // For now, just track tool count since we don't have full SerdesAI integration
    tool_count: usize,
}

impl PersonalAgent {
    /// Create a new `PersonalAgent` with the given profile and MCP configurations.
    ///
    /// Currently a placeholder that just counts enabled MCPs. Once `SerdesAI` PR #5
    /// is merged, this will create `McpToolsets` and build a full Agent.
    ///
    /// # Errors
    ///
    /// Returns `AgentError` if agent initialization fails.
    pub fn new(_profile: &ModelProfile, mcp_configs: &[McpConfig]) -> Result<Self, AgentError> {
        // Count enabled MCPs (placeholder for actual toolset creation)
        let enabled_count = mcp_configs.iter().filter(|c| c.enabled).count();

        // For now, just create a placeholder agent
        // Full implementation will use SerdesAI Agent with McpToolset
        Ok(Self {
            tool_count: enabled_count,
        })
    }

    /// Get the number of tools available to this agent.
    #[must_use]
    pub const fn tool_count(&self) -> usize {
        self.tool_count
    }
}

/// Global agent singleton.
///
/// Uses `OnceCell` to ensure thread-safe initialization and `RwLock` for
/// concurrent read access with exclusive write access when updating.
static GLOBAL_AGENT: once_cell::sync::OnceCell<tokio::sync::RwLock<Option<PersonalAgent>>> =
    once_cell::sync::OnceCell::new();

/// Get the global agent lock.
///
/// Returns a reference to the `RwLock` containing the optional `PersonalAgent`.
/// Use `.read().await` for concurrent access or `.write().await` to update.
pub fn global_agent() -> &'static tokio::sync::RwLock<Option<PersonalAgent>> {
    GLOBAL_AGENT.get_or_init(|| tokio::sync::RwLock::new(None))
}

/// Initialize or reinitialize the global agent.
///
/// This should be called once at application startup and can be called again
/// to reload the agent with new configuration (e.g., after MCP settings change).
///
/// # Example
/// ```ignore
/// init_global_agent(&profile, &mcp_configs).await?;
/// ```
///
/// # Errors
///
/// Returns `AgentError` if agent initialization fails.
pub async fn init_global_agent(
    profile: &ModelProfile,
    mcp_configs: &[McpConfig],
) -> Result<(), AgentError> {
    let agent = PersonalAgent::new(profile, mcp_configs)?;
    {
        let mut lock = global_agent().write().await;
        *lock = Some(agent);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{McpAuthType, McpPackage, McpPackageType, McpSource, McpTransport};
    use uuid::Uuid;

    #[tokio::test]
    async fn init_global_agent_tracks_enabled_mcps() {
        let profile = ModelProfile::default();
        let enabled = McpConfig {
            id: Uuid::new_v4(),
            name: "Enabled".to_string(),
            enabled: true,
            source: McpSource::Manual {
                url: "https://example.com".to_string(),
            },
            package: McpPackage {
                package_type: McpPackageType::Http,
                identifier: "https://example.com".to_string(),
                runtime_hint: None,
            },
            transport: McpTransport::Http,
            auth_type: McpAuthType::None,
            env_vars: vec![],
            package_args: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        };
        let mut disabled = enabled.clone();
        disabled.id = Uuid::new_v4();
        disabled.name = "Disabled".to_string();
        disabled.enabled = false;

        init_global_agent(&profile, &[enabled.clone(), disabled])
            .await
            .unwrap();

        let lock = global_agent().read().await;
        let agent = lock.as_ref().expect("agent initialized");
        assert_eq!(agent.tool_count(), 1);
    }
}
