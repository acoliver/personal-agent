//! Toolset bridge - converts `McpConfig` to `SerdesAI` `McpToolset` format

use crate::mcp::manager::{McpError, McpResult};
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
        // Keyfile auth is deprecated — treated as an API key lookup.
        McpAuthType::ApiKey | McpAuthType::Keyfile => {
            // Secrets are stored per env var name (`mcp:{id}:{var_name}`);
            // load each one by name so store and load sides agree.
            for var in &config.env_vars {
                let key = secrets.load_api_key_named(config.id, &var.name)?;
                env.insert(var.name.clone(), key);
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

/// Build HTTP headers for an MCP (OAuth token, keyfile bearer, or keychain API key)
///
/// For `Http` transport with `ApiKey` auth, the keychain secret behind the
/// single configured env var is delivered as an `x-api-key` header. A missing
/// secret is an error so misconfigured remote MCPs fail at startup instead of
/// failing per-request with a provider 401.
///
/// # Errors
///
/// Returns `McpError` if the `ApiKey` secret cannot be loaded from the keychain.
pub fn build_headers_for_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> McpResult<HashMap<String, String>> {
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

    if config.transport == McpTransport::Http && config.auth_type == McpAuthType::ApiKey {
        if let [var] = config.env_vars.as_slice() {
            let key = secrets.load_api_key_named(config.id, &var.name)?;
            headers.insert("x-api-key".to_string(), key);
        }
    }

    Ok(headers)
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
    let _ = build_headers_for_config(config, secrets)?;
    let (cmd, _args) = build_command(config);

    if config.transport == McpTransport::Stdio && cmd.is_empty() {
        return Err(McpError::Config(
            "Stdio transport requires a command".to_string(),
        ));
    }

    Ok(())
}
