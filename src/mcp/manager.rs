//! MCP Manager - handles spawning, lifecycle, and tool routing for MCP servers

use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;
use thiserror::Error;

use crate::mcp::{McpConfig, McpAuthType, McpPackageType, SecretsManager};

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
}

pub type McpResult<T> = Result<T, McpError>;

/// Information about an active MCP server
struct ActiveMcp {
    config: McpConfig,
    started_at: Instant,
    last_used: Instant,
    restart_count: u32,
}

/// Manager for MCP server lifecycle
pub struct McpManager {
    secrets: SecretsManager,
    active: HashMap<Uuid, ActiveMcp>,
    idle_timeout: Duration,
    max_restart_attempts: u32,
}

impl McpManager {
    pub fn new(secrets: SecretsManager) -> Self {
        Self {
            secrets,
            active: HashMap::new(),
            idle_timeout: Duration::from_secs(30 * 60), // 30 minutes
            max_restart_attempts: 3,
        }
    }

    pub fn with_idle_timeout(secrets: SecretsManager, timeout: Duration) -> Self {
        Self {
            secrets,
            active: HashMap::new(),
            idle_timeout: timeout,
            max_restart_attempts: 3,
        }
    }

    pub fn with_max_restarts(secrets: SecretsManager, max_restarts: u32) -> Self {
        Self {
            secrets,
            active: HashMap::new(),
            idle_timeout: Duration::from_secs(30 * 60),
            max_restart_attempts: max_restarts,
        }
    }

    /// Build environment variables for an MCP based on its auth config
    pub fn build_env(&self, config: &McpConfig) -> McpResult<HashMap<String, String>> {
        let mut env = HashMap::new();

        match config.auth_type {
            McpAuthType::None => {}
            McpAuthType::ApiKey => {
                // Load API keys for each env var
                for var in &config.env_vars {
                    let key = if config.env_vars.len() == 1 {
                        self.secrets.load_api_key(config.id)?
                    } else {
                        self.secrets.load_api_key_named(config.id, &var.name)?
                    };
                    env.insert(var.name.clone(), key);
                }
            }
            McpAuthType::Keyfile => {
                if let Some(ref path) = config.keyfile_path {
                    let key = self.secrets.read_keyfile(path)?;
                    // Use the first env var name, or a default
                    let var_name = config.env_vars.first()
                        .map(|v| v.name.clone())
                        .unwrap_or_else(|| "API_KEY".to_string());
                    env.insert(var_name, key);
                }
            }
            McpAuthType::OAuth => {
                // OAuth tokens would be loaded from oauth token storage
                // For now, treat like API key (the access_token)
                for var in &config.env_vars {
                    if let Ok(key) = self.secrets.load_api_key_named(config.id, &var.name) {
                        env.insert(var.name.clone(), key);
                    }
                }
            }
        }

        Ok(env)
    }

    /// Build the command and arguments for an MCP based on its package type
    pub fn build_command(config: &McpConfig) -> (String, Vec<String>) {
        match config.package.package_type {
            McpPackageType::Npm => {
                let runtime = config.package.runtime_hint.as_deref().unwrap_or("npx");
                let args = vec!["-y".to_string(), config.package.identifier.clone()];
                (runtime.to_string(), args)
            }
            McpPackageType::Docker => {
                let args = vec![
                    "run".to_string(),
                    "-i".to_string(),
                    "--rm".to_string(),
                    config.package.identifier.clone(),
                ];
                ("docker".to_string(), args)
            }
            McpPackageType::Http => {
                // HTTP transport doesn't spawn a process
                (String::new(), Vec::new())
            }
        }
    }

    /// Check if an MCP is currently active
    pub fn is_active(&self, id: &Uuid) -> bool {
        self.active.contains_key(id)
    }

    /// Get count of active MCPs
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Get the last used time for an MCP
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
    pub fn get_restart_count(&self, id: &Uuid) -> u32 {
        self.active.get(id).map(|a| a.restart_count).unwrap_or(0)
    }

    /// Register an MCP as active (for tracking purposes)
    pub fn register_active(&mut self, config: McpConfig) {
        let now = Instant::now();
        self.active.insert(config.id, ActiveMcp {
            config,
            started_at: now,
            last_used: now,
            restart_count: 0,
        });
    }

    /// Stop an MCP
    pub fn stop(&mut self, id: &Uuid) -> McpResult<()> {
        self.active.remove(id);
        Ok(())
    }

    /// Shutdown all MCPs
    pub fn shutdown_all(&mut self) -> McpResult<()> {
        self.active.clear();
        Ok(())
    }

    /// Clean up idle MCPs
    pub fn cleanup_idle(&mut self) {
        let now = Instant::now();
        let idle_ids: Vec<Uuid> = self.active.iter()
            .filter(|(_, a)| now.duration_since(a.last_used) > self.idle_timeout)
            .map(|(id, _)| *id)
            .collect();

        for id in idle_ids {
            self.active.remove(&id);
        }
    }

    /// Handle config change (e.g., MCP disabled)
    pub fn handle_config_change(&mut self, config: &McpConfig) -> McpResult<()> {
        if !config.enabled && self.is_active(&config.id) {
            self.stop(&config.id)?;
        }
        Ok(())
    }

    /// Delete an MCP (stop + delete credentials)
    pub fn delete_mcp(&mut self, config: &McpConfig) -> McpResult<()> {
        self.stop(&config.id)?;
        self.secrets.delete_api_key(config.id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::mcp::{McpSource, McpPackage, McpTransport, EnvVarConfig};

    fn create_test_config() -> McpConfig {
        McpConfig {
            id: Uuid::new_v4(),
            name: "Test MCP".to_string(),
            enabled: true,
            source: McpSource::Manual { url: "test".to_string() },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@test/mcp".to_string(),
                runtime_hint: Some("npx".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::ApiKey,
            env_vars: vec![EnvVarConfig {
                name: "TEST_API_KEY".to_string(),
                required: true,
            }],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        }
    }

    fn create_secrets_manager() -> SecretsManager {
        let temp_dir = TempDir::new().unwrap();
        SecretsManager::new(temp_dir.path().to_path_buf())
    }

    #[test]
    fn test_new_manager() {
        let secrets = create_secrets_manager();
        let manager = McpManager::new(secrets);
        
        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.idle_timeout, Duration::from_secs(30 * 60));
        assert_eq!(manager.max_restart_attempts, 3);
    }

    #[test]
    fn test_with_idle_timeout() {
        let secrets = create_secrets_manager();
        let timeout = Duration::from_secs(60);
        let manager = McpManager::with_idle_timeout(secrets, timeout);
        
        assert_eq!(manager.idle_timeout, timeout);
    }

    #[test]
    fn test_with_max_restarts() {
        let secrets = create_secrets_manager();
        let manager = McpManager::with_max_restarts(secrets, 5);
        
        assert_eq!(manager.max_restart_attempts, 5);
    }

    #[test]
    fn test_build_command_npm() {
        let config = create_test_config();
        let (cmd, args) = McpManager::build_command(&config);
        
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "@test/mcp"]);
    }

    #[test]
    fn test_build_command_npm_default_runtime() {
        let mut config = create_test_config();
        config.package.runtime_hint = None;
        
        let (cmd, args) = McpManager::build_command(&config);
        
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "@test/mcp"]);
    }

    #[test]
    fn test_build_command_docker() {
        let mut config = create_test_config();
        config.package.package_type = McpPackageType::Docker;
        config.package.identifier = "test/mcp:latest".to_string();
        
        let (cmd, args) = McpManager::build_command(&config);
        
        assert_eq!(cmd, "docker");
        assert_eq!(args, vec!["run", "-i", "--rm", "test/mcp:latest"]);
    }

    #[test]
    fn test_build_command_http() {
        let mut config = create_test_config();
        config.package.package_type = McpPackageType::Http;
        
        let (cmd, args) = McpManager::build_command(&config);
        
        assert_eq!(cmd, "");
        assert!(args.is_empty());
    }

    #[test]
    fn test_build_env_no_auth() {
        let secrets = create_secrets_manager();
        let manager = McpManager::new(secrets);
        
        let mut config = create_test_config();
        config.auth_type = McpAuthType::None;
        
        let env = manager.build_env(&config).unwrap();
        assert!(env.is_empty());
    }

    #[test]
    fn test_build_env_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let config = create_test_config();
        secrets.store_api_key(config.id, "test-key-123").unwrap();
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("TEST_API_KEY").unwrap(), "test-key-123");
    }

    #[test]
    fn test_build_env_multiple_api_keys() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let mut config = create_test_config();
        config.env_vars = vec![
            EnvVarConfig {
                name: "CLIENT_ID".to_string(),
                required: true,
            },
            EnvVarConfig {
                name: "CLIENT_SECRET".to_string(),
                required: true,
            },
        ];
        
        secrets.store_api_key_named(config.id, "CLIENT_ID", "id-123").unwrap();
        secrets.store_api_key_named(config.id, "CLIENT_SECRET", "secret-456").unwrap();
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("CLIENT_ID").unwrap(), "id-123");
        assert_eq!(env.get("CLIENT_SECRET").unwrap(), "secret-456");
    }

    #[test]
    fn test_build_env_keyfile() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let keyfile_path = temp_dir.path().join("test.key");
        std::fs::write(&keyfile_path, "keyfile-content").unwrap();
        
        let mut config = create_test_config();
        config.auth_type = McpAuthType::Keyfile;
        config.keyfile_path = Some(keyfile_path);
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("TEST_API_KEY").unwrap(), "keyfile-content");
    }

    #[test]
    fn test_build_env_keyfile_default_var_name() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let keyfile_path = temp_dir.path().join("test.key");
        std::fs::write(&keyfile_path, "keyfile-content").unwrap();
        
        let mut config = create_test_config();
        config.auth_type = McpAuthType::Keyfile;
        config.keyfile_path = Some(keyfile_path);
        config.env_vars.clear();
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("API_KEY").unwrap(), "keyfile-content");
    }

    #[test]
    fn test_register_active() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let config = create_test_config();
        let id = config.id;
        
        assert!(!manager.is_active(&id));
        
        manager.register_active(config);
        
        assert!(manager.is_active(&id));
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_touch() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let config = create_test_config();
        let id = config.id;
        
        manager.register_active(config);
        
        let first_time = manager.get_last_used(&id).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        
        manager.touch(&id);
        
        let second_time = manager.get_last_used(&id).unwrap();
        assert!(second_time > first_time);
    }

    #[test]
    fn test_get_restart_count() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let config = create_test_config();
        let id = config.id;
        
        assert_eq!(manager.get_restart_count(&id), 0);
        
        manager.register_active(config);
        assert_eq!(manager.get_restart_count(&id), 0);
    }

    #[test]
    fn test_stop() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let config = create_test_config();
        let id = config.id;
        
        manager.register_active(config);
        assert!(manager.is_active(&id));
        
        manager.stop(&id).unwrap();
        assert!(!manager.is_active(&id));
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_shutdown_all() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let config1 = create_test_config();
        let config2 = create_test_config();
        
        manager.register_active(config1);
        manager.register_active(config2);
        
        assert_eq!(manager.active_count(), 2);
        
        manager.shutdown_all().unwrap();
        
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_cleanup_idle() {
        let secrets = create_secrets_manager();
        let timeout = Duration::from_millis(50);
        let mut manager = McpManager::with_idle_timeout(secrets, timeout);
        
        let config1 = create_test_config();
        let config2 = create_test_config();
        let id1 = config1.id;
        let id2 = config2.id;
        
        manager.register_active(config1);
        manager.register_active(config2);
        
        std::thread::sleep(Duration::from_millis(60));
        
        manager.touch(&id2);
        
        manager.cleanup_idle();
        
        assert!(!manager.is_active(&id1));
        assert!(manager.is_active(&id2));
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_handle_config_change_disable() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let mut config = create_test_config();
        let id = config.id;
        
        manager.register_active(config.clone());
        assert!(manager.is_active(&id));
        
        config.enabled = false;
        manager.handle_config_change(&config).unwrap();
        
        assert!(!manager.is_active(&id));
    }

    #[test]
    fn test_handle_config_change_enable() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let config = create_test_config();
        let id = config.id;
        
        assert!(!manager.is_active(&id));
        
        manager.handle_config_change(&config).unwrap();
        
        assert!(!manager.is_active(&id));
    }

    #[test]
    fn test_delete_mcp() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let config = create_test_config();
        let id = config.id;
        
        secrets.store_api_key(id, "test-key").unwrap();
        
        let mut manager = McpManager::new(secrets);
        manager.register_active(config.clone());
        
        assert!(manager.is_active(&id));
        
        manager.delete_mcp(&config).unwrap();
        
        assert!(!manager.is_active(&id));
        assert!(manager.secrets.load_api_key(id).is_err());
    }

    #[test]
    fn test_get_last_used_nonexistent() {
        let secrets = create_secrets_manager();
        let manager = McpManager::new(secrets);
        
        let id = Uuid::new_v4();
        assert!(manager.get_last_used(&id).is_none());
    }

    #[test]
    fn test_touch_nonexistent() {
        let secrets = create_secrets_manager();
        let mut manager = McpManager::new(secrets);
        
        let id = Uuid::new_v4();
        manager.touch(&id); // Should not panic
    }

    #[test]
    fn test_build_env_oauth() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let mut config = create_test_config();
        config.auth_type = McpAuthType::OAuth;
        config.env_vars = vec![
            EnvVarConfig {
                name: "ACCESS_TOKEN".to_string(),
                required: true,
            },
        ];
        
        secrets.store_api_key_named(config.id, "ACCESS_TOKEN", "oauth-token-123").unwrap();
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("ACCESS_TOKEN").unwrap(), "oauth-token-123");
    }

    #[test]
    fn test_build_env_oauth_missing_token() {
        let secrets = create_secrets_manager();
        
        let mut config = create_test_config();
        config.auth_type = McpAuthType::OAuth;
        config.env_vars = vec![
            EnvVarConfig {
                name: "ACCESS_TOKEN".to_string(),
                required: true,
            },
        ];
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        // OAuth missing tokens are silently skipped
        assert!(env.is_empty());
    }
}
