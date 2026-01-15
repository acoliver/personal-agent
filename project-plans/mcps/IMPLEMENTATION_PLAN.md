# MCP Implementation Plan - Test-First Approach

## Overview

This document outlines a test-first implementation plan for adding MCP (Model Context Protocol) support to PersonalAgent. Each phase has tests written BEFORE implementation with clear success criteria.

**Total Estimated Timeline**: 10-14 weeks (50-70 days)

## Revision History

| Date | Changes |
|------|---------|
| 2026-01-15 | Initial plan |
| 2026-01-15 | Rev 1: Added auth type detection, multiple env vars, error handling tests, lifecycle tests per review |

## Deferred to Future Phases

The following SPEC requirements are explicitly deferred:
- **configSchema UI rendering** - Phase 8 (future) - MCPs with custom config will show raw JSON editor as fallback
- **HTTP transport MCPs** - Phase 8 (future)

---

## Phase 0: SerdesAI Environment Variable PR (DRAFT)

**Duration**: 3-4 days  
**Status**: BLOCKER - Must complete before Phase 3  
**Deliverable**: Draft PR to SerdesAI repo

### Problem

`StdioTransport::spawn()` in `serdes-ai-mcp/src/transport.rs` doesn't accept environment variables:

```rust
// Current - NO env var support
pub async fn spawn(command: &str, args: &[&str]) -> McpResult<Self> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::piped())
        // Missing: .envs(env)
        .spawn()?;
}
```

### Tests to Write FIRST

```rust
// tests/transport_env_test.rs

#[tokio::test]
async fn test_spawn_with_env_sets_environment_variables() {
    // Spawn 'env' command which prints environment
    let env = HashMap::from([
        ("TEST_VAR".to_string(), "test_value".to_string()),
        ("API_KEY".to_string(), "secret123".to_string()),
    ]);
    
    let transport = StdioTransport::spawn_with_env("env", &[], env).await.unwrap();
    // Verify child process has access to env vars
    // (implementation detail: may need a test helper MCP)
}

#[tokio::test]
async fn test_spawn_with_env_inherits_parent_env() {
    std::env::set_var("PARENT_VAR", "parent_value");
    let env = HashMap::from([("CHILD_VAR".to_string(), "child_value".to_string())]);
    
    let transport = StdioTransport::spawn_with_env("env", &[], env).await.unwrap();
    // Should have both PARENT_VAR and CHILD_VAR
}

#[tokio::test]
async fn test_spawn_with_env_empty_map_works() {
    let transport = StdioTransport::spawn_with_env("echo", &["hello"], HashMap::new()).await;
    assert!(transport.is_ok());
}

#[tokio::test]
async fn test_spawn_with_env_overrides_parent_env() {
    std::env::set_var("PATH", "/original");
    let env = HashMap::from([("PATH".to_string(), "/custom".to_string())]);
    
    let transport = StdioTransport::spawn_with_env("printenv", &["PATH"], env).await.unwrap();
    // Should print /custom, not /original
}
```

### Implementation

Add to `serdes-ai-mcp/src/transport.rs`:

```rust
impl StdioTransport {
    /// Spawn a new process with custom environment variables.
    ///
    /// Environment variables from `env` are merged with the parent process environment,
    /// with `env` values taking precedence for any conflicts.
    pub async fn spawn_with_env(
        command: &str,
        args: &[&str],
        env: HashMap<String, String>,
    ) -> McpResult<Self> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env)  // Key addition
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| McpError::Transport(format!("Failed to spawn {}: {}", command, e)))?;
        
        // ... rest unchanged
    }
}
```

### Success Criteria

- [ ] All 4 tests pass
- [ ] Existing `spawn()` tests still pass (backward compatible)
- [ ] PR created as DRAFT on SerdesAI repo
- [ ] PR description explains use case (MCP credential injection)

### PR Description Template

```markdown
## Add spawn_with_env() to StdioTransport

### Motivation

When using MCP servers, we need to inject credentials (API keys, tokens) via
environment variables. The current `spawn()` method doesn't support this.

### Changes

- Add `spawn_with_env(command, args, env)` to `StdioTransport`
- Environment variables merge with parent process env
- Child env values override parent for conflicts
- Backward compatible - existing `spawn()` unchanged

### Testing

Added 4 new tests covering:
- Basic env var injection
- Parent env inheritance
- Empty env map
- Override behavior

### Status

DRAFT - Will mark ready once used in downstream project (PersonalAgent)
```

---

## Phase 1: Data Models & Storage

**Duration**: 5-7 days  
**Dependencies**: None (can start in parallel with Phase 0)  
**Deliverable**: `src/mcp/` module with types and storage

### Tests to Write FIRST

```rust
// src/mcp/types.rs - tests at bottom

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_serialize_deserialize() {
        let config = McpConfig {
            id: Uuid::new_v4(),
            name: "GitHub".to_string(),
            enabled: true,
            source: McpSource::Official {
                name: "github/github-mcp-server".to_string(),
                version: "0.28.1".to_string(),
            },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@github/mcp-server".to_string(),
                runtime_hint: Some("npx".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::ApiKey,
            env_var_name: Some("GITHUB_TOKEN".to_string()),
            keyfile_path: None,
            config: serde_json::json!({}),
        };
        
        let json = serde_json::to_string(&config).unwrap();
        let parsed: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "GitHub");
        assert_eq!(parsed.auth_type, McpAuthType::ApiKey);
    }

    #[test]
    fn test_mcp_auth_type_variants() {
        assert_eq!(
            serde_json::to_string(&McpAuthType::None).unwrap(),
            "\"none\""
        );
        assert_eq!(
            serde_json::to_string(&McpAuthType::ApiKey).unwrap(),
            "\"api_key\""
        );
        assert_eq!(
            serde_json::to_string(&McpAuthType::Keyfile).unwrap(),
            "\"keyfile\""
        );
        assert_eq!(
            serde_json::to_string(&McpAuthType::OAuth).unwrap(),
            "\"oauth\""
        );
    }

    #[test]
    fn test_mcp_source_official() {
        let source = McpSource::Official {
            name: "test/server".to_string(),
            version: "1.0.0".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"official\""));
    }

    #[test]
    fn test_mcp_source_smithery() {
        let source = McpSource::Smithery {
            qualified_name: "@owner/server".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"smithery\""));
    }

    #[test]
    fn test_mcp_source_manual() {
        let source = McpSource::Manual {
            url: "https://example.com/mcp".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(json.contains("\"type\":\"manual\""));
    }

    #[test]
    fn test_mcp_package_npm() {
        let pkg = McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@github/mcp-server".to_string(),
            runtime_hint: Some("npx".to_string()),
        };
        let json = serde_json::to_string(&pkg).unwrap();
        assert!(json.contains("\"type\":\"npm\""));
        assert!(json.contains("\"runtime_hint\":\"npx\""));
    }

    #[test]
    fn test_mcp_package_docker() {
        let pkg = McpPackage {
            package_type: McpPackageType::Docker,
            identifier: "ghcr.io/owner/server:latest".to_string(),
            runtime_hint: None,
        };
        let json = serde_json::to_string(&pkg).unwrap();
        assert!(json.contains("\"type\":\"docker\""));
    }
}
```

```rust
// src/mcp/secrets.rs - tests

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_and_load_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let mcp_id = Uuid::new_v4();
        secrets.store_api_key(mcp_id, "test_secret_key").unwrap();
        
        let loaded = secrets.load_api_key(mcp_id).unwrap();
        assert_eq!(loaded, "test_secret_key");
    }

    #[test]
    fn test_api_key_file_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let mcp_id = Uuid::new_v4();
        secrets.store_api_key(mcp_id, "secret").unwrap();
        
        let path = temp_dir.path().join(format!("mcp_{}.key", mcp_id));
        let metadata = std::fs::metadata(&path).unwrap();
        let permissions = metadata.permissions();
        
        // On Unix, check mode is 0600
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn test_delete_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let mcp_id = Uuid::new_v4();
        secrets.store_api_key(mcp_id, "secret").unwrap();
        secrets.delete_api_key(mcp_id).unwrap();
        
        assert!(secrets.load_api_key(mcp_id).is_err());
    }

    #[test]
    fn test_load_nonexistent_key_fails() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let result = secrets.load_api_key(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn test_read_keyfile() {
        let temp_dir = TempDir::new().unwrap();
        let keyfile_path = temp_dir.path().join("my_token");
        std::fs::write(&keyfile_path, "keyfile_content\n").unwrap();
        
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        let key = secrets.read_keyfile(&keyfile_path).unwrap();
        
        assert_eq!(key, "keyfile_content"); // Should trim whitespace
    }
}
```

```rust
// src/config.rs - add tests for mcps field

#[test]
fn test_config_with_mcps() {
    let config = Config {
        version: "1.0".to_string(),
        profiles: vec![],
        mcps: vec![McpConfig {
            id: Uuid::new_v4(),
            name: "Test MCP".to_string(),
            enabled: true,
            // ... other fields
        }],
        ..Default::default()
    };
    
    let json = serde_json::to_string(&config).unwrap();
    let parsed: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.mcps.len(), 1);
}

#[test]
fn test_config_backward_compatible_no_mcps() {
    // Old config without mcps field should still load
    let json = r#"{"version":"1.0","profiles":[]}"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert!(config.mcps.is_empty());
}
```

### Implementation Tasks

1. Create `src/mcp/mod.rs`:
   ```rust
   pub mod types;
   pub mod secrets;
   pub mod manager;
   ```

2. Create `src/mcp/types.rs` with data models from SPEC.md Section 7

3. Create `src/mcp/secrets.rs`:
   - `SecretsManager` struct
   - `store_api_key(mcp_id, key)` 
   - `load_api_key(mcp_id)`
   - `delete_api_key(mcp_id)`
   - `read_keyfile(path)`
   - File permissions handling

4. Update `src/models.rs` or `src/config.rs`:
   - Add `mcps: Vec<McpConfig>` to Config
   - Ensure backward compatible with `#[serde(default)]`

5. Add to `src/lib.rs`:
   ```rust
   pub mod mcp;
   ```

### Additional Tests (Rev 1)

```rust
// Multiple env vars per MCP (e.g., AWS)
#[test]
fn test_mcp_config_multiple_env_vars() {
    let config = McpConfig {
        env_vars: vec![
            EnvVarConfig { name: "AWS_ACCESS_KEY_ID".to_string(), required: true },
            EnvVarConfig { name: "AWS_SECRET_ACCESS_KEY".to_string(), required: true },
            EnvVarConfig { name: "AWS_REGION".to_string(), required: false },
        ],
        ..Default::default()
    };
    
    let json = serde_json::to_string(&config).unwrap();
    let parsed: McpConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.env_vars.len(), 3);
}

// Auth type detection from registry metadata
#[test]
fn test_detect_auth_type_from_token_env_var() {
    let env_vars = vec![
        RegistryEnvVar { name: "GITHUB_TOKEN".to_string(), is_secret: true, is_required: true }
    ];
    assert_eq!(detect_auth_type(&env_vars), McpAuthType::ApiKey);
}

#[test]
fn test_detect_auth_type_from_api_key_env_var() {
    let env_vars = vec![
        RegistryEnvVar { name: "OPENAI_API_KEY".to_string(), is_secret: true, is_required: true }
    ];
    assert_eq!(detect_auth_type(&env_vars), McpAuthType::ApiKey);
}

#[test]
fn test_detect_auth_type_oauth_from_client_credentials() {
    let env_vars = vec![
        RegistryEnvVar { name: "CLIENT_ID".to_string(), is_secret: false, is_required: true },
        RegistryEnvVar { name: "CLIENT_SECRET".to_string(), is_secret: true, is_required: true },
    ];
    assert_eq!(detect_auth_type(&env_vars), McpAuthType::OAuth);
}

#[test]
fn test_detect_auth_type_none_when_no_secrets() {
    let env_vars = vec![
        RegistryEnvVar { name: "LOG_LEVEL".to_string(), is_secret: false, is_required: false }
    ];
    assert_eq!(detect_auth_type(&env_vars), McpAuthType::None);
}

// Keyfile error handling
#[test]
fn test_read_keyfile_not_found() {
    let secrets = SecretsManager::new(TempDir::new().unwrap().path().to_path_buf());
    let result = secrets.read_keyfile(Path::new("/nonexistent/path"));
    assert!(matches!(result, Err(SecretsError::KeyfileNotFound(_))));
}

#[test]
#[cfg(unix)]
fn test_read_keyfile_permission_denied() {
    let temp_dir = TempDir::new().unwrap();
    let keyfile = temp_dir.path().join("secret");
    std::fs::write(&keyfile, "content").unwrap();
    std::fs::set_permissions(&keyfile, std::fs::Permissions::from_mode(0o000)).unwrap();
    
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let result = secrets.read_keyfile(&keyfile);
    assert!(matches!(result, Err(SecretsError::PermissionDenied(_))));
    
    // Cleanup - restore permissions so temp dir can be deleted
    std::fs::set_permissions(&keyfile, std::fs::Permissions::from_mode(0o600)).unwrap();
}
```

### Updated Data Model

```rust
// Support multiple env vars per MCP (Rev 1)
pub struct McpConfig {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub source: McpSource,
    pub package: McpPackage,
    pub transport: McpTransport,
    pub auth_type: McpAuthType,
    pub env_vars: Vec<EnvVarConfig>,  // Changed from single env_var_name
    pub keyfile_path: Option<PathBuf>,
    pub config: serde_json::Value,
}

pub struct EnvVarConfig {
    pub name: String,
    pub required: bool,
}
```

### Success Criteria

- [ ] All type serialization tests pass
- [ ] All secrets storage tests pass
- [ ] Config loads existing config.json without mcps field
- [ ] Config saves and loads with mcps array
- [ ] Multiple env vars per MCP supported
- [ ] Auth type detection from registry metadata works
- [ ] Keyfile error handling (not found, permission denied)
- [ ] `cargo test` passes with no regressions

---

## Phase 2: MCP Spawning & Manager

**Duration**: 5-7 days  
**Dependencies**: Phase 0 (SerdesAI PR), Phase 1  
**Deliverable**: `src/mcp/manager.rs` that can spawn MCPs with credentials

### Tests to Write FIRST

```rust
// src/mcp/manager.rs - tests

#[cfg(test)]
mod tests {
    use super::*;
    use serdes_ai_mcp::transport::MemoryTransport;

    fn create_test_mcp_config() -> McpConfig {
        McpConfig {
            id: Uuid::new_v4(),
            name: "Test MCP".to_string(),
            enabled: true,
            source: McpSource::Manual { url: "test".to_string() },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "test-mcp".to_string(),
                runtime_hint: Some("npx".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::ApiKey,
            env_var_name: Some("TEST_API_KEY".to_string()),
            keyfile_path: None,
            config: serde_json::json!({}),
        }
    }

    #[test]
    fn test_build_env_for_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let config = create_test_mcp_config();
        secrets.store_api_key(config.id, "secret123").unwrap();
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("TEST_API_KEY"), Some(&"secret123".to_string()));
    }

    #[test]
    fn test_build_env_for_keyfile() {
        let temp_dir = TempDir::new().unwrap();
        let keyfile = temp_dir.path().join("token");
        std::fs::write(&keyfile, "keyfile_secret").unwrap();
        
        let mut config = create_test_mcp_config();
        config.auth_type = McpAuthType::Keyfile;
        config.keyfile_path = Some(keyfile);
        
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert_eq!(env.get("TEST_API_KEY"), Some(&"keyfile_secret".to_string()));
    }

    #[test]
    fn test_build_env_for_no_auth() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        
        let mut config = create_test_mcp_config();
        config.auth_type = McpAuthType::None;
        
        let manager = McpManager::new(secrets);
        let env = manager.build_env(&config).unwrap();
        
        assert!(!env.contains_key("TEST_API_KEY"));
    }

    #[test]
    fn test_build_command_npm() {
        let config = McpConfig {
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@github/mcp-server".to_string(),
                runtime_hint: Some("npx".to_string()),
            },
            ..create_test_mcp_config()
        };
        
        let (cmd, args) = McpManager::build_command(&config);
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "@github/mcp-server"]);
    }

    #[test]
    fn test_build_command_docker() {
        let config = McpConfig {
            package: McpPackage {
                package_type: McpPackageType::Docker,
                identifier: "ghcr.io/owner/server:latest".to_string(),
                runtime_hint: None,
            },
            ..create_test_mcp_config()
        };
        
        let (cmd, args) = McpManager::build_command(&config);
        assert_eq!(cmd, "docker");
        assert_eq!(args, vec!["run", "-i", "--rm", "ghcr.io/owner/server:latest"]);
    }

    #[tokio::test]
    async fn test_manager_tracks_active_mcps() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        let mut manager = McpManager::new(secrets);
        
        let config = create_test_mcp_config();
        
        // Use mock transport instead of actually spawning
        manager.register_mock_client(config.id, MemoryTransport::new()).await;
        
        assert!(manager.is_active(&config.id));
        
        manager.stop(&config.id).await.unwrap();
        assert!(!manager.is_active(&config.id));
    }

    #[tokio::test]
    async fn test_manager_shutdown_all() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
        let mut manager = McpManager::new(secrets);
        
        let config1 = create_test_mcp_config();
        let config2 = create_test_mcp_config();
        
        manager.register_mock_client(config1.id, MemoryTransport::new()).await;
        manager.register_mock_client(config2.id, MemoryTransport::new()).await;
        
        assert_eq!(manager.active_count(), 2);
        
        manager.shutdown_all().await.unwrap();
        assert_eq!(manager.active_count(), 0);
    }
}
```

### Integration Test (requires Phase 0)

```rust
// tests/mcp_spawn_integration.rs

#[tokio::test]
#[ignore] // Requires Phase 0 and npx installed
async fn test_spawn_real_echo_mcp() {
    // Use a simple echo MCP for testing
    let config = McpConfig {
        id: Uuid::new_v4(),
        name: "Echo".to_string(),
        enabled: true,
        source: McpSource::Manual { url: "test".to_string() },
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@anthropics/echo-mcp".to_string(),
            runtime_hint: Some("npx".to_string()),
        },
        transport: McpTransport::Stdio,
        auth_type: McpAuthType::None,
        env_var_name: None,
        keyfile_path: None,
        config: serde_json::json!({}),
    };
    
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let client = manager.start(&config).await.unwrap();
    
    // List tools
    let tools = client.list_tools().await.unwrap();
    assert!(!tools.is_empty());
    
    manager.stop(&config.id).await.unwrap();
}
```

### Implementation Tasks

1. Create `src/mcp/manager.rs`:
   ```rust
   pub struct McpManager {
       secrets: SecretsManager,
       active: HashMap<Uuid, ActiveMcp>,
       idle_timeout: Duration,
       max_restart_attempts: u32,
   }
   
   struct ActiveMcp {
       client: McpClient,
       config: McpConfig,
       started_at: Instant,
       last_used: Instant,
       restart_count: u32,
   }
   ```

2. Implement `build_env()` - reads credentials from secrets manager

3. Implement `build_command()` - returns (command, args) tuple

4. Implement `start()`:
   - Build env vars
   - Build command
   - Call `StdioTransport::spawn_with_env()` (from Phase 0 PR)
   - Initialize McpClient
   - Store in active map

5. Implement lifecycle methods:
   - `stop(id)`
   - `shutdown_all()`
   - `is_active(id)`
   - `active_count()`

### Additional Tests (Rev 1) - Error Handling

```rust
// Spawn failure handling
#[tokio::test]
async fn test_spawn_command_not_found() {
    let config = McpConfig {
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "test".to_string(),
            runtime_hint: Some("nonexistent_command_xyz".to_string()),
        },
        ..create_test_mcp_config()
    };
    
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let result = manager.start(&config).await;
    assert!(matches!(result, Err(McpError::SpawnFailed(_))));
}

#[tokio::test]
async fn test_spawn_npm_package_not_found() {
    let config = McpConfig {
        package: McpPackage {
            package_type: McpPackageType::Npm,
            identifier: "@nonexistent/package-that-doesnt-exist-12345".to_string(),
            runtime_hint: Some("npx".to_string()),
        },
        ..create_test_mcp_config()
    };
    
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let result = manager.start(&config).await;
    // npx exits with error code when package not found
    assert!(result.is_err());
}

// Multiple env vars
#[test]
fn test_build_env_multiple_vars() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    
    let config = McpConfig {
        env_vars: vec![
            EnvVarConfig { name: "AWS_ACCESS_KEY_ID".to_string(), required: true },
            EnvVarConfig { name: "AWS_SECRET_ACCESS_KEY".to_string(), required: true },
        ],
        ..create_test_mcp_config()
    };
    
    // Store both keys
    secrets.store_api_key_named(config.id, "AWS_ACCESS_KEY_ID", "AKIAIOSFODNN7EXAMPLE").unwrap();
    secrets.store_api_key_named(config.id, "AWS_SECRET_ACCESS_KEY", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY").unwrap();
    
    let manager = McpManager::new(secrets);
    let env = manager.build_env(&config).unwrap();
    
    assert_eq!(env.get("AWS_ACCESS_KEY_ID"), Some(&"AKIAIOSFODNN7EXAMPLE".to_string()));
    assert_eq!(env.get("AWS_SECRET_ACCESS_KEY"), Some(&"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()));
}

// Lifecycle: disable MCP
#[tokio::test]
async fn test_disable_mcp_stops_running() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let mut config = create_test_mcp_config();
    config.enabled = true;
    manager.register_mock_client(config.id, MemoryTransport::new()).await;
    
    assert!(manager.is_active(&config.id));
    
    // Disable the MCP
    config.enabled = false;
    manager.handle_config_change(&config).await.unwrap();
    
    assert!(!manager.is_active(&config.id));
}

// Lifecycle: delete MCP
#[tokio::test]
async fn test_delete_mcp_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    
    let config = create_test_mcp_config();
    secrets.store_api_key(config.id, "secret").unwrap();
    
    let mut manager = McpManager::new(secrets.clone());
    manager.register_mock_client(config.id, MemoryTransport::new()).await;
    
    // Delete the MCP
    manager.delete_mcp(&config).await.unwrap();
    
    // Should stop the MCP
    assert!(!manager.is_active(&config.id));
    
    // Should delete credentials
    assert!(secrets.load_api_key(config.id).is_err());
}

// Lifecycle: last_used timestamp
#[tokio::test]
async fn test_tool_call_updates_last_used() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let transport = MemoryTransport::new();
    transport.push_response(JsonRpcResponse::success(1, json!({"content": []}))).await;
    
    let config = create_test_mcp_config();
    manager.register_mock_client(config.id, transport).await;
    
    let before = manager.get_last_used(&config.id).unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Call a tool
    manager.call_tool(&config.id, "test", json!({})).await.unwrap();
    
    let after = manager.get_last_used(&config.id).unwrap();
    assert!(after > before);
}
```

### Success Criteria

- [ ] All unit tests pass
- [ ] `build_env()` correctly handles all auth types
- [ ] `build_env()` handles multiple env vars per MCP
- [ ] `build_command()` correctly builds npm and docker commands
- [ ] Manager tracks active MCPs
- [ ] Spawn failures handled gracefully with proper error types
- [ ] Disable MCP stops running instance
- [ ] Delete MCP cleans up credentials
- [ ] Tool calls update last_used timestamp
- [ ] Shutdown gracefully closes all MCPs
- [ ] Integration test passes (when Phase 0 ready)

---

## Phase 3: UI - Add MCP Flow

**Duration**: 7-10 days  
**Dependencies**: Phase 1  
**Deliverable**: Add MCP and Configure MCP screens

### Tests (UI State Management)

```rust
// src/ui/mcp_add_view.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_npx_url() {
        let result = parse_mcp_url("npx -y @github/mcp-server");
        assert!(matches!(result, Ok(ParsedMcp::Npm { identifier, .. })));
        if let Ok(ParsedMcp::Npm { identifier, .. }) = result {
            assert_eq!(identifier, "@github/mcp-server");
        }
    }

    #[test]
    fn test_parse_docker_url() {
        let result = parse_mcp_url("docker run ghcr.io/owner/server");
        assert!(matches!(result, Ok(ParsedMcp::Docker { image, .. })));
        if let Ok(ParsedMcp::Docker { image, .. }) = result {
            assert_eq!(image, "ghcr.io/owner/server");
        }
    }

    #[test]
    fn test_parse_http_url() {
        let result = parse_mcp_url("https://server.smithery.ai/mcp");
        assert!(matches!(result, Ok(ParsedMcp::Http { url, .. })));
    }

    #[test]
    fn test_parse_invalid_url() {
        let result = parse_mcp_url("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_next_button_disabled_initially() {
        let state = AddMcpState::default();
        assert!(!state.can_proceed());
    }

    #[test]
    fn test_next_button_enabled_with_url() {
        let mut state = AddMcpState::default();
        state.url_input = "npx -y @test/mcp".to_string();
        assert!(state.can_proceed());
    }

    #[test]
    fn test_next_button_enabled_with_selection() {
        let mut state = AddMcpState::default();
        state.selected_result = Some(0);
        state.search_results = vec![SearchResult::mock()];
        assert!(state.can_proceed());
    }
}
```

```rust
// src/ui/mcp_configure_view.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_validation_empty() {
        let state = ConfigureMcpState {
            auth_type: McpAuthType::ApiKey,
            api_key_input: "".to_string(),
            ..Default::default()
        };
        assert!(!state.is_valid());
    }

    #[test]
    fn test_api_key_validation_filled() {
        let state = ConfigureMcpState {
            auth_type: McpAuthType::ApiKey,
            api_key_input: "ghp_xxxxx".to_string(),
            ..Default::default()
        };
        assert!(state.is_valid());
    }

    #[test]
    fn test_keyfile_validation_empty() {
        let state = ConfigureMcpState {
            auth_type: McpAuthType::Keyfile,
            keyfile_path_input: "".to_string(),
            ..Default::default()
        };
        assert!(!state.is_valid());
    }

    #[test]
    fn test_keyfile_validation_filled() {
        let state = ConfigureMcpState {
            auth_type: McpAuthType::Keyfile,
            keyfile_path_input: "~/.github_token".to_string(),
            ..Default::default()
        };
        assert!(state.is_valid());
    }

    #[test]
    fn test_no_auth_always_valid() {
        let state = ConfigureMcpState {
            auth_type: McpAuthType::None,
            ..Default::default()
        };
        assert!(state.is_valid());
    }

    #[test]
    fn test_name_defaulted_from_identifier() {
        let state = ConfigureMcpState::from_parsed(ParsedMcp::Npm {
            identifier: "@github/mcp-server".to_string(),
            runtime_hint: "npx".to_string(),
        });
        assert_eq!(state.name_input, "github-mcp-server");
    }
}
```

### Implementation Tasks

1. Create `src/ui/mcp_add_view.rs`:
   - URL input field
   - Parse URL on change (npx/docker/http)
   - Validate input
   - Next button enabled state
   - Navigation to Configure screen

2. Create `src/ui/mcp_configure_view.rs`:
   - Name input (auto-filled from identifier)
   - Auth type detection from parsed URL
   - API Key / Keyfile radio buttons
   - Credential input fields
   - Save button
   - Cancel button

3. Update `src/main_menubar.rs`:
   - Add notification handlers for MCP screens
   - Wire up navigation

4. Update Settings view:
   - Add "+" button handler to show Add MCP
   - Add "Edit" button handler
   - Add "-" button with confirmation

### Manual Test Checklist

- [ ] Click "+" in MCPs section → shows Add MCP screen
- [ ] Type `npx -y @github/mcp-server` → Next enabled
- [ ] Click Next → shows Configure screen with "GitHub MCP Server" name
- [ ] Select API Key, enter key → Save enabled
- [ ] Click Save → returns to Settings, MCP in list
- [ ] Click Edit → shows Configure with saved values
- [ ] Click "-" → shows confirmation dialog

### Success Criteria

- [ ] All state management tests pass
- [ ] URL parsing handles npx, docker, http formats
- [ ] Configure screen validates based on auth type
- [ ] Manual test checklist passes
- [ ] MCP saved to config.json
- [ ] Credentials saved to secrets/

---

## Phase 4: Agent Integration

**Duration**: 5-7 days  
**Dependencies**: Phase 2, Phase 3  
**Deliverable**: Agent can use MCP tools

### Tests to Write FIRST

```rust
// src/mcp/toolset.rs - tests

#[cfg(test)]
mod tests {
    use super::*;
    use serdes_ai_mcp::transport::MemoryTransport;

    #[tokio::test]
    async fn test_tools_with_prefix() {
        let transport = MemoryTransport::new();
        // Mock tools/list response
        transport.push_response(JsonRpcResponse::success(1, json!({
            "tools": [
                {"name": "search", "description": "Search repos"},
                {"name": "create_issue", "description": "Create issue"}
            ]
        }))).await;
        
        let client = McpClient::with_transport(Box::new(transport));
        let toolset = McpToolset::new("github", client);
        
        let tools = toolset.list_tools_prefixed().await.unwrap();
        assert_eq!(tools[0].name, "github.search");
        assert_eq!(tools[1].name, "github.create_issue");
    }

    #[tokio::test]
    async fn test_call_tool_routes_correctly() {
        let transport = MemoryTransport::new();
        // Mock tools/call response
        transport.push_response(JsonRpcResponse::success(1, json!({
            "content": [{"type": "text", "text": "result"}]
        }))).await;
        
        let client = McpClient::with_transport(Box::new(transport));
        let toolset = McpToolset::new("github", client);
        
        let result = toolset.call_tool("github.search", json!({"q": "test"})).await.unwrap();
        
        // Verify the actual call stripped the prefix
        let requests = transport.get_requests().await;
        let call_params: serde_json::Value = serde_json::from_value(requests[0].params.clone()).unwrap();
        assert_eq!(call_params["name"], "search"); // Not "github.search"
    }

    #[tokio::test]
    async fn test_call_tool_wrong_prefix_fails() {
        let transport = MemoryTransport::new();
        let client = McpClient::with_transport(Box::new(transport));
        let toolset = McpToolset::new("github", client);
        
        let result = toolset.call_tool("filesystem.read", json!({})).await;
        assert!(result.is_err());
    }
}
```

```rust
// src/mcp/loader.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_enabled_mcps_only() {
        let config = Config {
            mcps: vec![
                McpConfig { enabled: true, name: "A".to_string(), .. },
                McpConfig { enabled: false, name: "B".to_string(), .. },
                McpConfig { enabled: true, name: "C".to_string(), .. },
            ],
            ..Default::default()
        };
        
        // With mock manager
        let loaded = load_enabled_mcps(&config, &mock_manager).await;
        assert_eq!(loaded.len(), 2); // Only A and C
    }

    #[test]
    fn test_aggregate_tools_from_multiple_mcps() {
        let toolsets = vec![
            MockToolset::with_tools(vec!["github.search", "github.create_issue"]),
            MockToolset::with_tools(vec!["filesystem.read", "filesystem.write"]),
        ];
        
        let all_tools = aggregate_tools(&toolsets);
        assert_eq!(all_tools.len(), 4);
    }
}
```

### Implementation Tasks

1. Create `src/mcp/toolset.rs`:
   - Wrap `McpToolset` with prefix handling
   - `list_tools_prefixed()` - adds MCP name prefix
   - `call_tool(prefixed_name, args)` - strips prefix, routes call

2. Create `src/mcp/loader.rs`:
   - `load_enabled_mcps(config, manager)` - start enabled MCPs
   - `aggregate_tools(toolsets)` - combine all tools for agent

3. Update `src/llm/client.rs`:
   - Pass MCP toolsets to agent builder
   - Handle tool calls in stream response

4. Update Chat view:
   - Display tool call in progress
   - Display tool call result
   - Display tool call error

### Additional Tests (Rev 1) - Tool Display & System Prompt

```rust
// Tool call display states
#[test]
fn test_tool_call_state_in_progress() {
    let state = ToolCallDisplayState::InProgress {
        tool_name: "github.search".to_string(),
        args: json!({"q": "test"}),
        started_at: Instant::now(),
    };
    assert_eq!(state.status_text(), "Using github.search...");
    assert!(state.show_spinner());
}

#[test]
fn test_tool_call_state_success() {
    let state = ToolCallDisplayState::Success {
        tool_name: "github.search".to_string(),
        result: json!({"items": []}),
        duration_ms: 150,
    };
    assert_eq!(state.status_text(), "github.search completed");
    assert!(state.show_checkmark());
}

#[test]
fn test_tool_call_state_error() {
    let state = ToolCallDisplayState::Error {
        tool_name: "github.search".to_string(),
        error: "Rate limited".to_string(),
    };
    assert_eq!(state.status_text(), "github.search failed");
    assert!(state.show_error_icon());
}

#[test]
fn test_tool_call_state_timeout() {
    let state = ToolCallDisplayState::Timeout {
        tool_name: "github.search".to_string(),
        timeout_seconds: 30,
    };
    assert_eq!(state.status_text(), "github.search timed out after 30s");
}

// System prompt includes MCP tools
#[test]
fn test_system_prompt_includes_mcp_tools() {
    let base_prompt = "You are a helpful assistant.";
    let tools = vec![
        ToolInfo { name: "github.search".to_string(), description: "Search GitHub repositories".to_string() },
        ToolInfo { name: "filesystem.read".to_string(), description: "Read a local file".to_string() },
    ];
    
    let prompt = build_system_prompt_with_tools(base_prompt, &tools);
    
    assert!(prompt.contains("You are a helpful assistant."));
    assert!(prompt.contains("Available tools:"));
    assert!(prompt.contains("- github.search: Search GitHub repositories"));
    assert!(prompt.contains("- filesystem.read: Read a local file"));
}

#[test]
fn test_system_prompt_no_tools() {
    let base_prompt = "You are a helpful assistant.";
    let tools: Vec<ToolInfo> = vec![];
    
    let prompt = build_system_prompt_with_tools(base_prompt, &tools);
    
    assert_eq!(prompt, "You are a helpful assistant.");
    assert!(!prompt.contains("Available tools:"));
}

// Tool call timeout
#[tokio::test]
async fn test_tool_call_timeout() {
    let transport = MemoryTransport::new();
    // Don't push a response - simulates timeout
    
    let client = McpClient::with_transport(Box::new(transport));
    let toolset = McpToolset::new("test", client);
    
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        toolset.call_tool("test.slow", json!({}))
    ).await;
    
    assert!(result.is_err()); // Timeout
}

// Tool call error response from MCP
#[tokio::test]
async fn test_tool_call_mcp_error_response() {
    let transport = MemoryTransport::new();
    transport.push_response(JsonRpcResponse::error(1, -32000, "Tool execution failed")).await;
    
    let client = McpClient::with_transport(Box::new(transport));
    let toolset = McpToolset::new("test", client);
    
    let result = toolset.call_tool("test.broken", json!({})).await;
    
    assert!(matches!(result, Err(McpError::ToolError(_))));
}
```

### Success Criteria

- [ ] All unit tests pass
- [ ] Tool names correctly prefixed
- [ ] Tool calls routed to correct MCP
- [ ] Agent can call MCP tools
- [ ] System prompt includes available MCP tools with descriptions
- [ ] Tool call timeout handled (30s default)
- [ ] Tool call MCP errors handled
- [ ] Tool call display states (in progress, success, error, timeout)
- [ ] Tool results shown in chat

---

## Phase 5: Registry Search

**Duration**: 7-10 days  
**Dependencies**: Phase 3  
**Deliverable**: Search Official + Smithery registries

### Tests to Write FIRST

```rust
// src/mcp/registry/official.rs - tests

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

    #[tokio::test]
    async fn test_search_official_registry() {
        let server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/v0.1/servers"))
            .and(query_param("search", "github"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "servers": [{
                    "server": {
                        "name": "github/github-mcp-server",
                        "title": "GitHub",
                        "description": "GitHub MCP Server"
                    }
                }]
            })))
            .mount(&server)
            .await;
        
        let client = OfficialRegistryClient::with_url(server.uri());
        let results = client.search("github").await.unwrap();
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "github/github-mcp-server");
    }

    #[tokio::test]
    async fn test_search_empty_results() {
        let server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "servers": []
            })))
            .mount(&server)
            .await;
        
        let client = OfficialRegistryClient::with_url(server.uri());
        let results = client.search("nonexistent").await.unwrap();
        
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_registry_unavailable() {
        let server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        
        let client = OfficialRegistryClient::with_url(server.uri());
        let result = client.search("test").await;
        
        assert!(result.is_err());
    }
}
```

```rust
// src/mcp/registry/smithery.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_smithery_registry() {
        // Similar structure to official tests
    }

    #[tokio::test]
    async fn test_get_server_details() {
        // Smithery has a details endpoint
    }
}
```

```rust
// src/mcp/registry/merged.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_merge_results_dedupes() {
        let official = vec![
            SearchResult { name: "github".to_string(), source: Source::Official, .. },
        ];
        let smithery = vec![
            SearchResult { name: "github".to_string(), source: Source::Smithery, .. },
            SearchResult { name: "brave".to_string(), source: Source::Smithery, .. },
        ];
        
        let merged = merge_results(official, smithery);
        
        // Should have 2 results, github deduplicated (prefer official)
        assert_eq!(merged.len(), 2);
        assert!(merged[0].source == Source::Official); // github from official
        assert_eq!(merged[1].name, "brave");
    }

    #[tokio::test]
    async fn test_parallel_search() {
        // Both registries queried in parallel
        let results = search_all_registries("github").await.unwrap();
        // Should have results from both
    }
}
```

### Implementation Tasks

1. Create `src/mcp/registry/mod.rs`
2. Create `src/mcp/registry/official.rs` - Official MCP registry client
3. Create `src/mcp/registry/smithery.rs` - Smithery registry client
4. Create `src/mcp/registry/merged.rs` - Merge and dedupe logic

5. Update Add MCP UI:
   - Registry dropdown
   - Search field (enabled after registry selected)
   - Results list with source badges
   - Select result → populate URL

### Success Criteria

- [ ] All registry client tests pass
- [ ] Search Official registry works
- [ ] Search Smithery registry works
- [ ] Results merged and deduplicated
- [ ] UI shows search results with badges
- [ ] Selecting result populates configure screen

---

## Phase 6: Polish & Error Recovery

**Duration**: 10-14 days  
**Dependencies**: All previous phases  
**Deliverable**: Production-ready MCP support

### Tests to Write FIRST

```rust
// src/mcp/manager.rs - lifecycle tests

#[tokio::test]
async fn test_auto_restart_on_crash() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let config = create_test_mcp_config();
    let transport = CrashingMockTransport::new(); // Simulates crash
    manager.register_mock_client(config.id, transport).await;
    
    // Trigger crash detection
    manager.health_check(&config.id).await;
    
    // Should have attempted restart
    assert!(manager.get_restart_count(&config.id) >= 1);
}

#[tokio::test]
async fn test_max_restart_attempts_respected() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::with_max_restarts(secrets, 3);
    
    let config = create_test_mcp_config();
    
    // Simulate 4 crashes
    for _ in 0..4 {
        manager.simulate_crash(&config.id).await;
    }
    
    // Should give up after 3 restarts
    assert!(!manager.is_active(&config.id));
    assert_eq!(manager.get_restart_count(&config.id), 3);
    assert!(manager.has_permanent_error(&config.id));
}

#[tokio::test]
async fn test_idle_timeout_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::with_idle_timeout(secrets, Duration::from_millis(100));
    
    let config = create_test_mcp_config();
    manager.register_mock_client(config.id, MemoryTransport::new()).await;
    
    assert!(manager.is_active(&config.id));
    
    // Wait past idle timeout
    tokio::time::sleep(Duration::from_millis(150)).await;
    
    // Run cleanup
    manager.cleanup_idle().await;
    
    assert!(!manager.is_active(&config.id));
}

#[tokio::test]
async fn test_graceful_shutdown_timeout() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let config = create_test_mcp_config();
    let transport = HangingMockTransport::new(); // Doesn't respond to close
    manager.register_mock_client(config.id, transport).await;
    
    // Should not hang forever
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        manager.stop(&config.id)
    ).await;
    
    assert!(result.is_ok()); // Completed within timeout (force killed)
}

#[tokio::test]
async fn test_health_check_ping() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut manager = McpManager::new(secrets);
    
    let transport = MemoryTransport::new();
    transport.push_response(JsonRpcResponse::success(1, json!({}))).await; // ping response
    
    let config = create_test_mcp_config();
    manager.register_mock_client(config.id, transport).await;
    
    let status = manager.health_check(&config.id).await;
    assert_eq!(status, HealthStatus::Healthy);
}
```

```rust
// src/ui/settings_view.rs - status indicator tests

#[test]
fn test_mcp_status_connected() {
    let state = McpListItemState {
        config: McpConfig { enabled: true, ..Default::default() },
        is_running: true,
        has_error: false,
        error_message: None,
    };
    assert_eq!(state.status_badge(), StatusBadge::Connected);
    assert_eq!(state.status_color(), Color::GREEN);
}

#[test]
fn test_mcp_status_idle() {
    let state = McpListItemState {
        config: McpConfig { enabled: true, ..Default::default() },
        is_running: false,
        has_error: false,
        error_message: None,
    };
    assert_eq!(state.status_badge(), StatusBadge::Idle);
    assert_eq!(state.status_color(), Color::GRAY);
}

#[test]
fn test_mcp_status_error() {
    let state = McpListItemState {
        config: McpConfig { enabled: true, ..Default::default() },
        is_running: false,
        has_error: true,
        error_message: Some("Connection refused".to_string()),
    };
    assert_eq!(state.status_badge(), StatusBadge::Error);
    assert_eq!(state.status_color(), Color::RED);
    assert_eq!(state.error_tooltip(), Some("Connection refused"));
}

#[test]
fn test_mcp_status_disabled() {
    let state = McpListItemState {
        config: McpConfig { enabled: false, ..Default::default() },
        is_running: false,
        has_error: false,
        error_message: None,
    };
    assert_eq!(state.status_badge(), StatusBadge::None);
}

// Delete confirmation dialog
#[test]
fn test_delete_confirmation_state() {
    let state = DeleteConfirmationState {
        mcp_name: "GitHub".to_string(),
        is_visible: true,
    };
    assert!(state.is_visible);
    assert_eq!(state.message(), "Delete "GitHub"?

This will remove the MCP and its stored credentials.");
}

#[test]
fn test_delete_confirmation_confirm() {
    let mut state = DeleteConfirmationState::new("GitHub");
    state.show();
    assert!(state.is_visible);
    
    let result = state.confirm();
    assert_eq!(result, DeleteAction::Delete);
    assert!(!state.is_visible);
}

#[test]
fn test_delete_confirmation_cancel() {
    let mut state = DeleteConfirmationState::new("GitHub");
    state.show();
    
    state.cancel();
    assert!(!state.is_visible);
}
```

```rust
// Toast notifications
#[test]
fn test_toast_mcp_disconnected() {
    let toast = McpToast::disconnected("GitHub");
    assert_eq!(toast.message(), "GitHub MCP disconnected. Reconnecting...");
    assert_eq!(toast.level(), ToastLevel::Warning);
    assert!(toast.show_spinner());
}

#[test]
fn test_toast_mcp_connected() {
    let toast = McpToast::connected("GitHub");
    assert_eq!(toast.message(), "GitHub MCP connected.");
    assert_eq!(toast.level(), ToastLevel::Success);
    assert!(toast.auto_dismiss());
}

#[test]
fn test_toast_mcp_restart_failed() {
    let toast = McpToast::restart_failed("GitHub");
    assert_eq!(toast.message(), "GitHub MCP failed to restart. Check settings.");
    assert_eq!(toast.level(), ToastLevel::Error);
    assert!(!toast.auto_dismiss());
}
```

### Implementation Tasks

1. MCP Lifecycle:
   - Idle timeout (30 min default)
   - Auto-restart on crash (max 3 attempts)
   - Graceful shutdown with timeout
   - Health check ping

2. Status Indicators:
   - Connected (green) - running
   - Idle (gray) - enabled but not started
   - Error (red) - crashed/failed
   - Update on state change

3. Error Recovery:
   - Toast notifications for MCP events
   - Error messages in chat for tool failures
   - Retry button for transient failures

4. Performance:
   - Lazy start (first tool call)
   - Connection pooling
   - Request timeouts

### Success Criteria

- [ ] MCPs restart on crash (up to 3 times)
- [ ] Idle MCPs cleaned up after 30 min
- [ ] Status badges update in real-time
- [ ] Toast notifications for MCP events
- [ ] Tool errors display in chat

---

## Timeline Summary

| Phase | Duration | Dependencies | Deliverable |
|-------|----------|--------------|-------------|
| **0: SerdesAI PR** | 3-4 days | None | Draft PR with spawn_with_env |
| **1: Data Models** | 7-9 days | None | Types, config, secrets, auth detection |
| **2: MCP Spawning** | 7-9 days | Phase 0, 1 | McpManager with error handling |
| **3: UI Add MCP** | 7-10 days | Phase 1 | Add/Configure screens |
| **4: Agent Integration** | 7-9 days | Phase 2, 3 | Tool routing, display, system prompt |
| **5: Registry Search** | 7-10 days | Phase 3 | Official + Smithery |
| **6: Polish** | 10-14 days | All | Lifecycle, errors, status |
| **7: OAuth** | 7-10 days | Phase 6 | OAuth flow for GitHub, Google, etc. |

**Total: 55-75 days (11-15 weeks)**

**Revised from original 37-52 days based on review feedback to account for:**
- Multiple env vars per MCP support
- Auth type detection from registry metadata
- Comprehensive error handling
- Lifecycle edge cases (disable, delete, last_used)
- Tool call display states
- System prompt builder
- OAuth authentication flow

### Parallel Work

- Phase 0 and Phase 1 can run in parallel
- Phase 3 (UI) can start once Phase 1 types are done
- Phase 5 (Registry) can start once Phase 3 UI scaffolding is done

### Critical Path

```
Phase 0 (SerdesAI PR) → Phase 2 (Spawning) → Phase 4 (Agent) → Phase 6 (Polish)
        ↓
      Phase 1 (Types) → Phase 3 (UI) → Phase 5 (Registry)
```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| SerdesAI PR not accepted | Use local fork, maintain indefinitely |
| MCP servers crash frequently | Robust restart logic, max attempts |
| Registry APIs change | Version endpoints, graceful degradation |
| OAuth complexity | Defer to future phase (API key first) |
| Performance issues | Lazy loading, idle cleanup |

---

---

## Phase 7: OAuth Authentication

**Duration**: 7-10 days  
**Dependencies**: Phase 6  
**Deliverable**: OAuth flow for MCPs that require it (GitHub, Google, etc.)

### Tests to Write FIRST

```rust
// src/mcp/oauth.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_oauth_authorize_url() {
        let config = OAuthConfig {
            client_id: "test_client_id".to_string(),
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            scope: "repo,read:org".to_string(),
        };
        
        let (url, state) = build_authorize_url(&config, "personalagent://oauth/callback");
        
        assert!(url.contains("client_id=test_client_id"));
        assert!(url.contains("redirect_uri=personalagent"));
        assert!(url.contains("scope=repo,read:org"));
        assert!(url.contains(&format!("state={}", state)));
        assert_eq!(state.len(), 32); // Random state should be 32 chars
    }

    #[test]
    fn test_parse_oauth_callback_url() {
        let url = "personalagent://oauth/callback?code=abc123&state=xyz789";
        let result = parse_callback_url(url).unwrap();
        
        assert_eq!(result.code, "abc123");
        assert_eq!(result.state, "xyz789");
    }

    #[test]
    fn test_parse_oauth_callback_error() {
        let url = "personalagent://oauth/callback?error=access_denied&error_description=User+denied";
        let result = parse_callback_url(url);
        
        assert!(matches!(result, Err(OAuthError::AccessDenied(_))));
    }

    #[test]
    fn test_validate_state_matches() {
        let original_state = "abc123xyz";
        let callback_state = "abc123xyz";
        
        assert!(validate_state(original_state, callback_state));
    }

    #[test]
    fn test_validate_state_mismatch_fails() {
        let original_state = "abc123xyz";
        let callback_state = "different";
        
        assert!(!validate_state(original_state, callback_state));
    }

    #[tokio::test]
    async fn test_exchange_code_for_token() {
        let server = MockServer::start().await;
        
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "gho_xxxxxxxxxxxx",
                "token_type": "bearer",
                "scope": "repo,read:org",
                "refresh_token": "ghr_yyyyyyyyyyyy",
                "expires_in": 28800
            })))
            .mount(&server)
            .await;
        
        let config = OAuthConfig {
            client_id: "test_id".to_string(),
            client_secret: "test_secret".to_string(),
            token_url: format!("{}/login/oauth/access_token", server.uri()),
            ..Default::default()
        };
        
        let tokens = exchange_code(&config, "auth_code_123").await.unwrap();
        
        assert_eq!(tokens.access_token, "gho_xxxxxxxxxxxx");
        assert_eq!(tokens.refresh_token, Some("ghr_yyyyyyyyyyyy".to_string()));
    }

    #[tokio::test]
    async fn test_refresh_token() {
        let server = MockServer::start().await;
        
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(body_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "gho_new_token",
                "token_type": "bearer",
                "refresh_token": "ghr_new_refresh",
                "expires_in": 28800
            })))
            .mount(&server)
            .await;
        
        let config = OAuthConfig {
            token_url: format!("{}/login/oauth/access_token", server.uri()),
            ..Default::default()
        };
        
        let old_tokens = OAuthTokens {
            access_token: "old".to_string(),
            refresh_token: Some("ghr_old_refresh".to_string()),
            expires_at: Some(Utc::now() - Duration::hours(1)), // Expired
            ..Default::default()
        };
        
        let new_tokens = refresh_tokens(&config, &old_tokens).await.unwrap();
        
        assert_eq!(new_tokens.access_token, "gho_new_token");
    }

    #[test]
    fn test_token_is_expired() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            expires_at: Some(Utc::now() - Duration::hours(1)),
            ..Default::default()
        };
        
        assert!(tokens.is_expired());
    }

    #[test]
    fn test_token_not_expired() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            ..Default::default()
        };
        
        assert!(!tokens.is_expired());
    }

    #[test]
    fn test_token_no_expiry_never_expires() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            expires_at: None,
            ..Default::default()
        };
        
        assert!(!tokens.is_expired());
    }
}

// URL scheme handler tests
#[cfg(test)]
mod url_scheme_tests {
    use super::*;

    #[test]
    fn test_url_scheme_registered() {
        // This would be an integration test on macOS
        // Verifies personalagent:// scheme is handled
    }
}
```

```rust
// src/ui/mcp_oauth_view.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_state_not_connected() {
        let state = OAuthViewState {
            status: OAuthStatus::NotConnected,
            provider_name: "GitHub".to_string(),
            ..Default::default()
        };
        
        assert_eq!(state.button_text(), "Authorize with GitHub");
        assert_eq!(state.status_text(), "Not connected");
        assert!(!state.is_connected());
    }

    #[test]
    fn test_oauth_state_authorizing() {
        let state = OAuthViewState {
            status: OAuthStatus::Authorizing,
            provider_name: "GitHub".to_string(),
            ..Default::default()
        };
        
        assert!(state.show_spinner());
        assert_eq!(state.status_text(), "Waiting for authorization...");
        assert!(!state.button_enabled());
    }

    #[test]
    fn test_oauth_state_connected() {
        let state = OAuthViewState {
            status: OAuthStatus::Connected {
                username: Some("@acoliver".to_string()),
            },
            provider_name: "GitHub".to_string(),
            ..Default::default()
        };
        
        assert_eq!(state.button_text(), "Reauthorize with GitHub");
        assert_eq!(state.status_text(), "Connected as @acoliver");
        assert!(state.is_connected());
    }

    #[test]
    fn test_oauth_state_error() {
        let state = OAuthViewState {
            status: OAuthStatus::Error {
                message: "Authorization denied".to_string(),
            },
            provider_name: "GitHub".to_string(),
            ..Default::default()
        };
        
        assert_eq!(state.status_text(), "Error: Authorization denied");
        assert!(state.show_retry_button());
    }

    #[test]
    fn test_oauth_timeout() {
        let state = OAuthViewState {
            status: OAuthStatus::Timeout,
            provider_name: "GitHub".to_string(),
            ..Default::default()
        };
        
        assert_eq!(state.status_text(), "Authorization timed out after 2 minutes");
        assert!(state.show_retry_button());
    }
}
```

### Implementation Tasks

1. **Create `src/mcp/oauth.rs`**:
   - `build_authorize_url()` - Generate OAuth URL with state
   - `parse_callback_url()` - Parse code and state from callback
   - `validate_state()` - CSRF protection
   - `exchange_code()` - Exchange auth code for tokens
   - `refresh_tokens()` - Refresh expired tokens
   - `OAuthTokens` struct with expiry checking

2. **Register Custom URL Scheme**:
   - Add `CFBundleURLTypes` to `Info.plist` for bundled app
   - Implement Apple Event handler for `personalagent://` URLs
   - Route callbacks to OAuth flow

3. **Token Storage**:
   - Store OAuth tokens in `secrets/mcp_{uuid}.oauth`
   - JSON format with access_token, refresh_token, expires_at
   - Auto-refresh before expiry

4. **Update Configure UI**:
   - Detect OAuth auth type from registry metadata
   - Show "Authorize with {Provider}" button
   - Handle authorization flow in background
   - Display connected status with username

5. **Update McpManager**:
   - Check token expiry before spawning
   - Auto-refresh if needed
   - Inject access_token as env var

### OAuth Provider Configurations

```rust
// Built-in OAuth configs for common providers
pub fn github_oauth_config() -> OAuthConfig {
    OAuthConfig {
        provider: "github".to_string(),
        auth_url: "https://github.com/login/oauth/authorize".to_string(),
        token_url: "https://github.com/login/oauth/access_token".to_string(),
        scope: "repo,read:org".to_string(),
        // client_id/secret loaded from env or embedded
    }
}

pub fn google_oauth_config() -> OAuthConfig {
    OAuthConfig {
        provider: "google".to_string(),
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
        scope: "https://www.googleapis.com/auth/calendar".to_string(),
    }
}
```

### OAuth App Registration

For OAuth to work, we need client_id/client_secret for each provider:

| Provider | Registration URL | Notes |
|----------|------------------|-------|
| GitHub | https://github.com/settings/developers | Create OAuth App |
| Google | https://console.cloud.google.com | Create OAuth 2.0 credentials |

**Options for client credentials**:
1. **Embed in binary** - Acceptable for open-source, extractable but functional
2. **User provides own** - Power user option, add to settings
3. **Backend proxy** - Requires server infrastructure (future)

For Phase 7, we'll use option 1 (embedded) with option 2 as fallback.

### Success Criteria

- [ ] OAuth authorize URL generated correctly with state
- [ ] Custom URL scheme `personalagent://` registered and working
- [ ] Callback URL parsed correctly, state validated
- [ ] Token exchange works (tested with mock server)
- [ ] Token refresh works when expired
- [ ] Tokens stored securely in secrets/
- [ ] Configure UI shows OAuth flow
- [ ] Connected status displays username
- [ ] McpManager injects access_token correctly
- [ ] End-to-end flow works with real GitHub OAuth

---

## Out of Scope (Future - Phase 8+)

- **HTTP transport MCP servers** - Smithery hosted servers
- **configSchema-driven dynamic UI** - Will use JSON editor as fallback
- **Custom MCP tool filtering** - Per-MCP enable/disable specific tools
- **MCP metrics/telemetry** - Usage tracking, latency monitoring
- **Windows support** - File permissions, process management differences

---

## Appendix A: Test Dependencies

Add to `Cargo.toml` for testing:

```toml
[dev-dependencies]
tempfile = "3"
wiremock = "0.5"
tokio-test = "0.4"
```

## Appendix B: Review History

### Review 1 (2026-01-15)
**Reviewer**: rustreviewer subagent  
**Verdict**: NEEDS_REVISION

**Issues Identified**:
1. Missing auth type detection from registry metadata
2. Data model only supported single env var, SPEC requires multiple
3. Missing error handling tests (spawn failures, timeouts, rate limits)
4. Missing lifecycle edge cases (disable, delete, last_used update)
5. Missing configSchema UI (deferred to future)
6. Timeline too optimistic

**Fixes Applied**:
- Added auth type detection tests and implementation tasks
- Updated data model to support `Vec<EnvVarConfig>` instead of single `env_var_name`
- Added comprehensive error handling tests
- Added lifecycle tests (disable, delete, last_used)
- Explicitly deferred configSchema UI to future phase
- Extended timeline from 37-52 days to 48-65 days
- Extended Phase 6 from 5-7 days to 10-14 days
