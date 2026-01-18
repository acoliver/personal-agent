//! Toolset bridge - converts `McpConfig` to `SerdesAI` `McpToolset` format

use crate::mcp::manager::McpError;
use crate::mcp::secrets::SecretsManager;
use crate::mcp::{McpAuthType, McpConfig, McpPackageArgType, McpPackageType, McpTransport};
use std::collections::HashMap;

/// Build command and arguments for an MCP based on its package type
#[must_use]
pub fn build_command(config: &McpConfig) -> (String, Vec<String>) {
    let (cmd, mut args) = match config.package.package_type {
        McpPackageType::Npm => {
            let runtime = config.package.runtime_hint.as_deref().unwrap_or("npx");
            (
                runtime.to_string(),
                vec!["-y".to_string(), config.package.identifier.clone()],
            )
        }
        McpPackageType::Docker => (
            "docker".to_string(),
            vec![
                "run".to_string(),
                "-i".to_string(),
                "--rm".to_string(),
                config.package.identifier.clone(),
            ],
        ),
        McpPackageType::Http => (String::new(), vec![]),
    };

    if !config.package_args.is_empty() {
        let package_arg_values = config
            .config
            .get("package_args")
            .and_then(|value| value.as_object());

        for arg in &config.package_args {
            let value = package_arg_values
                .and_then(|values| values.get(&arg.name))
                .and_then(|value| value.as_str())
                .or(arg.default.as_deref());

            if let Some(value) = value {
                let values = value
                    .split(',')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty());

                for entry in values {
                    match arg.arg_type {
                        McpPackageArgType::Named => {
                            args.push(format!("--{}", arg.name));
                            args.push(entry.to_string());
                        }
                        McpPackageArgType::Positional => {
                            args.push(entry.to_string());
                        }
                    }
                }
            }
        }
    }

    (cmd, args)
}

/// Build environment variables for an MCP based on its auth config
///
/// # Errors
///
/// Returns `McpError` if secrets cannot be loaded.
pub fn build_env_for_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<HashMap<String, String>, McpError> {
    let mut env = HashMap::new();

    match config.auth_type {
        McpAuthType::None => {}
        McpAuthType::ApiKey => {
            // Load API keys for each env var
            for var in &config.env_vars {
                let key = if config.env_vars.len() == 1 {
                    secrets.load_api_key(config.id)?
                } else {
                    secrets.load_api_key_named(config.id, &var.name)?
                };
                env.insert(var.name.clone(), key);
            }
        }
        McpAuthType::Keyfile => {
            if let Some(ref path) = config.keyfile_path {
                let key = secrets.read_keyfile(path)?;
                // Use the first env var name, or a default
                let var_name = config
                    .env_vars
                    .first()
                    .map_or_else(|| "API_KEY".to_string(), |v| v.name.clone());
                env.insert(var_name, key);
            }
        }
        McpAuthType::OAuth => {
            // OAuth tokens would be loaded from oauth token storage
            // For now, treat like API key (the access_token)
            for var in &config.env_vars {
                if let Ok(key) = secrets.load_api_key_named(config.id, &var.name) {
                    env.insert(var.name.clone(), key);
                }
            }
        }
    }

    Ok(env)
}

/// Build HTTP headers for an MCP (primarily for OAuth tokens)
#[must_use]
pub fn build_headers_for_config(config: &McpConfig) -> HashMap<String, String> {
    let mut headers = HashMap::new();

    // Priority: oauth_token > keyfile
    if let Some(ref token) = config.oauth_token {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    } else if let Some(ref keyfile) = config.keyfile_path {
        if let Ok(token) = std::fs::read_to_string(keyfile) {
            headers.insert(
                "Authorization".to_string(),
                format!("Bearer {}", token.trim()),
            );
        }
    }

    headers
}

/// Create a toolset from MCP configuration
/// Note: This is a placeholder for `SerdesAI` integration
///
/// # Errors
///
/// Returns `McpError` if config validation fails.
pub async fn create_toolset_from_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<(), McpError> {
    // This will be implemented when we integrate with SerdesAI McpToolset
    // For now, validate the config and return Ok
    let _ = build_env_for_config(config, secrets)?;
    let _ = build_headers_for_config(config);
    let (cmd, _args) = build_command(config);

    if config.transport == McpTransport::Stdio && cmd.is_empty() {
        return Err(McpError::Config(
            "Stdio transport requires a command".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{EnvVarConfig, McpPackage, McpSource};
    use tempfile::TempDir;
    use uuid::Uuid;

    fn create_test_config() -> McpConfig {
        McpConfig {
            id: Uuid::new_v4(),
            name: "test_mcp".to_string(),
            enabled: true,
            source: McpSource::Manual {
                url: "test".to_string(),
            },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@mcp/server-filesystem".to_string(),
                runtime_hint: Some("npx".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::None,
            env_vars: vec![],
            package_args: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        }
    }

    #[tokio::test]
    async fn test_build_env_from_config() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let mut config = create_test_config();
        config.name = "test_mcp".to_string();
        config.auth_type = McpAuthType::ApiKey;
        config.env_vars = vec![EnvVarConfig {
            name: "API_KEY".to_string(),
            required: true,
        }];

        secrets.store_api_key(config.id, "secret123").unwrap();

        let env = build_env_for_config(&config, &secrets).unwrap();
        assert_eq!(env.get("API_KEY"), Some(&"secret123".to_string()));
    }

    #[tokio::test]
    async fn test_build_env_missing_required_secret() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let mut config = create_test_config();
        config.auth_type = McpAuthType::ApiKey;
        config.env_vars = vec![EnvVarConfig {
            name: "API_KEY".to_string(),
            required: true,
        }];

        let result = build_env_for_config(&config, &secrets);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_build_headers_with_oauth_token() {
        let mut config = create_test_config();
        config.oauth_token = Some("oauth_token_123".to_string());

        let headers = build_headers_for_config(&config);
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer oauth_token_123".to_string())
        );
    }

    #[tokio::test]
    async fn test_build_headers_without_oauth_token() {
        let config = create_test_config();

        let headers = build_headers_for_config(&config);
        assert!(headers.is_empty());
    }

    #[tokio::test]
    async fn test_build_command_npm() {
        let config = create_test_config();

        let (cmd, args) = build_command(&config);
        assert_eq!(cmd, "npx");
        assert_eq!(
            args,
            vec!["-y".to_string(), "@mcp/server-filesystem".to_string()]
        );
        assert!(args.contains(&"-y".to_string()));
    }

    #[tokio::test]
    async fn test_build_command_with_package_args() {
        let mut config = create_test_config();
        config.package_args = vec![crate::mcp::McpPackageArg {
            arg_type: McpPackageArgType::Named,
            name: "allowed-directories".to_string(),
            description: None,
            required: true,
            default: None,
        }];
        config.config = serde_json::json!({
            "package_args": {
                "allowed-directories": "/tmp, /var"
            }
        });

        let (cmd, args) = build_command(&config);
        assert_eq!(cmd, "npx");
        assert!(args.contains(&"--allowed-directories".to_string()));
        assert!(args.contains(&"/tmp".to_string()));
        assert!(args.contains(&"/var".to_string()));
    }

    #[tokio::test]
    async fn test_build_command_npm_default_runtime() {
        let mut config = create_test_config();
        config.package.runtime_hint = None;

        let (cmd, args) = build_command(&config);
        assert_eq!(cmd, "npx");
        assert_eq!(
            args,
            vec!["-y".to_string(), "@mcp/server-filesystem".to_string()]
        );
    }

    #[tokio::test]
    async fn test_build_command_docker() {
        let mut config = create_test_config();
        config.package.package_type = McpPackageType::Docker;
        config.package.identifier = "test/mcp:latest".to_string();

        let (cmd, args) = build_command(&config);
        assert_eq!(cmd, "docker");
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "-i".to_string(),
                "--rm".to_string(),
                "test/mcp:latest".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_build_command_http() {
        let mut config = create_test_config();
        config.package.package_type = McpPackageType::Http;

        let (cmd, args) = build_command(&config);
        assert_eq!(cmd, "");
        assert!(args.is_empty());
    }

    #[tokio::test]
    async fn test_build_env_no_auth() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        let env = build_env_for_config(&config, &secrets).unwrap();
        assert!(env.is_empty());
    }

    #[tokio::test]
    async fn test_build_env_multiple_api_keys() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let mut config = create_test_config();
        config.auth_type = McpAuthType::ApiKey;
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

        secrets
            .store_api_key_named(config.id, "CLIENT_ID", "id-123")
            .unwrap();
        secrets
            .store_api_key_named(config.id, "CLIENT_SECRET", "secret-456")
            .unwrap();

        let env = build_env_for_config(&config, &secrets).unwrap();

        assert_eq!(env.get("CLIENT_ID").unwrap(), "id-123");
        assert_eq!(env.get("CLIENT_SECRET").unwrap(), "secret-456");
    }

    #[tokio::test]
    async fn test_build_env_keyfile() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let keyfile_path = temp_dir.path().join("test.key");
        std::fs::write(&keyfile_path, "keyfile-content").unwrap();

        let mut config = create_test_config();
        config.auth_type = McpAuthType::Keyfile;
        config.keyfile_path = Some(keyfile_path);
        config.env_vars = vec![EnvVarConfig {
            name: "SERVICE_KEY".to_string(),
            required: true,
        }];

        let env = build_env_for_config(&config, &secrets).unwrap();

        assert_eq!(env.get("SERVICE_KEY").unwrap(), "keyfile-content");
    }

    #[tokio::test]
    async fn test_build_env_oauth() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let mut config = create_test_config();
        config.auth_type = McpAuthType::OAuth;
        config.env_vars = vec![EnvVarConfig {
            name: "ACCESS_TOKEN".to_string(),
            required: true,
        }];

        secrets
            .store_api_key_named(config.id, "ACCESS_TOKEN", "oauth-token-123")
            .unwrap();

        let env = build_env_for_config(&config, &secrets).unwrap();

        assert_eq!(env.get("ACCESS_TOKEN").unwrap(), "oauth-token-123");
    }

    #[tokio::test]
    async fn test_build_env_oauth_missing_token() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let mut config = create_test_config();
        config.auth_type = McpAuthType::OAuth;
        config.env_vars = vec![EnvVarConfig {
            name: "ACCESS_TOKEN".to_string(),
            required: true,
        }];

        // Don't store the token
        let env = build_env_for_config(&config, &secrets).unwrap();

        // OAuth missing tokens are silently skipped
        assert!(env.is_empty());
    }

    #[tokio::test]
    async fn test_build_headers_from_keyfile() {
        let temp_keyfile = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_keyfile.path(), "keyfile_token_456").unwrap();

        let mut config = create_test_config();
        config.keyfile_path = Some(temp_keyfile.path().to_path_buf());

        let headers = build_headers_for_config(&config);
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer keyfile_token_456".to_string())
        );
    }

    #[tokio::test]
    async fn test_create_toolset_from_config() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let config = create_test_config();

        let result = create_toolset_from_config(&config, &secrets).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_toolset_stdio_requires_command() {
        let temp_dir = TempDir::new().unwrap();
        let secrets = SecretsManager::new(temp_dir.path().to_path_buf());

        let mut config = create_test_config();
        config.package.package_type = McpPackageType::Http;
        config.transport = McpTransport::Stdio;

        let result = create_toolset_from_config(&config, &secrets).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::Config(_)));
    }
}
