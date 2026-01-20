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
