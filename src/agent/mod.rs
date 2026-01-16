pub mod runtime;
pub use runtime::{agent_runtime, run_in_agent_runtime, spawn_in_agent_runtime};

use crate::models::ModelProfile;
use crate::mcp::McpConfig;

#[derive(Debug)]
pub enum AgentError {
    ModelError(String),
    ToolsetError(String),
    BuildError(String),
}

pub struct PersonalAgent {
    // For now, just track tool count since we don't have full SerdesAI integration
    tool_count: usize,
}

impl PersonalAgent {
    pub async fn new(
        _profile: &ModelProfile,
        mcp_configs: &[McpConfig],
    ) -> Result<Self, AgentError> {
        // Count enabled MCPs (placeholder for actual toolset creation)
        let enabled_count = mcp_configs.iter().filter(|c| c.enabled).count();
        
        // For now, just create a placeholder agent
        // Full implementation will use SerdesAI Agent with McpToolset
        Ok(Self {
            tool_count: enabled_count,
        })
    }
    
    pub fn tool_count(&self) -> usize {
        self.tool_count
    }
}

/// Global agent singleton
static GLOBAL_AGENT: once_cell::sync::OnceCell<tokio::sync::RwLock<Option<PersonalAgent>>> = 
    once_cell::sync::OnceCell::new();

/// Get the global agent lock
pub fn global_agent() -> &'static tokio::sync::RwLock<Option<PersonalAgent>> {
    GLOBAL_AGENT.get_or_init(|| tokio::sync::RwLock::new(None))
}

/// Initialize or reinitialize the global agent
pub async fn init_global_agent(
    profile: &ModelProfile,
    mcp_configs: &[McpConfig],
) -> Result<(), AgentError> {
    let agent = PersonalAgent::new(profile, mcp_configs).await?;
    let mut lock = global_agent().write().await;
    *lock = Some(agent);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation_no_mcps() {
        let profile = ModelProfile::default();
        let agent = PersonalAgent::new(&profile, &[]).await.unwrap();
        assert!(agent.tool_count() == 0);
    }

    #[tokio::test]
    async fn test_agent_creation_with_disabled_mcp() {
        let profile = ModelProfile::default();
        let config = McpConfig {
            enabled: false,
            id: uuid::Uuid::new_v4(),
            name: "test".to_string(),
            source: crate::mcp::McpSource::Official {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            },
            package: crate::mcp::McpPackage {
                package_type: crate::mcp::McpPackageType::Npm,
                identifier: "@test/mcp".to_string(),
                runtime_hint: Some("node".to_string()),
            },
            transport: crate::mcp::McpTransport::Stdio,
            auth_type: crate::mcp::McpAuthType::None,
            env_vars: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        };
        
        let agent = PersonalAgent::new(&profile, &[config]).await.unwrap();
        // Disabled MCPs should not create toolsets
        assert!(agent.tool_count() == 0);
    }

    #[test]
    fn test_agent_can_be_shared_across_threads() {
        use std::sync::Arc;
        use crate::agent::runtime::run_in_agent_runtime;
        
        let agent = Arc::new(run_in_agent_runtime(async {
            let profile = ModelProfile::default();
            PersonalAgent::new(&profile, &[]).await.unwrap()
        }));
        
        let handles: Vec<_> = (0..5).map(|_| {
            let agent = Arc::clone(&agent);
            std::thread::spawn(move || {
                // Just verify we can access the agent from multiple threads
                agent.tool_count()
            })
        }).collect();
        
        for handle in handles {
            assert_eq!(handle.join().unwrap(), 0);
        }
    }
}
