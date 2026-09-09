//! Toolset bridge - converts `McpConfig` to `SerdesAI` `McpToolset` format

use crate::mcp::manager::{McpError, McpResult};
use crate::mcp::secrets::SecretsManager;
use crate::mcp::{
    EnvVarConfig, McpAuthType, McpConfig, McpPackageArgType, McpPackageType, McpTransport,
};
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

/// Reject env var names or values that cannot travel safely downstream.
///
/// Names feed HTTP header material and must satisfy
/// [`env_var_name_is_valid`]; plain values are serialized into the config
/// and must be single-line. Secrets have no plain value here (their keychain
/// value is checked at load time).
fn validate_declared_env_var(config: &McpConfig, var: &EnvVarConfig) -> Result<(), McpError> {
    if !crate::mcp::env_var_name_is_valid(&var.name) {
        return Err(McpError::Config(format!(
            "MCP {}: env var name {:?} is invalid; use ASCII letters, digits, or \
             underscores (max 64 chars, starting with a letter or underscore)",
            config.name, var.name
        )));
    }
    if let Some(value) = var.value.as_deref() {
        reject_line_breaks(config, &var.name, value)?;
    }
    Ok(())
}

fn reject_line_breaks(config: &McpConfig, var_name: &str, value: &str) -> Result<(), McpError> {
    if value.contains('\r') || value.contains('\n') {
        return Err(McpError::Config(format!(
            "MCP {}: env var {var_name} value must not contain line breaks",
            config.name
        )));
    }
    Ok(())
}

/// Resolve a non-secret var's configured plain value.
///
/// A missing or empty value is skipped for optional vars and rejected for
/// required ones, so a blank required var cannot silently launch as an
/// empty environment entry.
fn plain_var_entry(
    config: &McpConfig,
    var: &EnvVarConfig,
) -> Result<Option<(String, String)>, McpError> {
    match var.value.as_deref() {
        Some(value) if !value.is_empty() => Ok(Some((var.name.clone(), value.to_string()))),
        _ if var.required => Err(McpError::Config(format!(
            "MCP {}: required env var {} has no configured value",
            config.name, var.name
        ))),
        _ => Ok(None),
    }
}

/// Build environment variables for an MCP based on its auth config
///
/// # Errors
///
/// Returns `McpError` if secrets cannot be loaded, an env var name is not
/// header-safe, a value contains line breaks, a required non-secret var has
/// no value, or Keyfile auth is paired with a secret env var it cannot use.
pub fn build_env_for_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> Result<HashMap<String, String>, McpError> {
    let mut env = HashMap::new();

    for var in &config.env_vars {
        validate_declared_env_var(config, var)?;
    }

    match config.auth_type {
        McpAuthType::None => {}
        // Secret vars resolve from the OS keychain by name (stored at
        // `mcp:{id}:{var_name}`); a missing entry fails the MCP. Non-secret
        // vars carry their plain value in the config itself.
        McpAuthType::ApiKey => {
            for var in &config.env_vars {
                if var.is_secret {
                    let key = secrets.load_api_key_named(config.id, &var.name)?;
                    reject_line_breaks(config, &var.name, &key)?;
                    env.insert(var.name.clone(), key);
                } else if let Some((name, value)) = plain_var_entry(config, var)? {
                    env.insert(name, value);
                }
            }
        }
        // Keyfile auth resolves the credential from `keyfile_path`; the
        // keychain is never consulted. A declared secret var has no keyfile
        // slot to resolve into and would silently drop the credential, so
        // the config is rejected instead.
        McpAuthType::Keyfile => {
            for var in &config.env_vars {
                if var.is_secret {
                    return Err(McpError::Config(format!(
                        "MCP {}: Keyfile auth cannot use secret env var {}",
                        config.name, var.name
                    )));
                }
                if let Some((name, value)) = plain_var_entry(config, var)? {
                    env.insert(name, value);
                }
            }
        }
        McpAuthType::OAuth => {
            // OAuth tokens would be loaded from oauth token storage
            // For now, treat like API key (the access_token)
            for var in &config.env_vars {
                if let Ok(key) = secrets.load_api_key_named(config.id, &var.name) {
                    reject_line_breaks(config, &var.name, &key)?;
                    env.insert(var.name.clone(), key);
                }
            }
        }
    }

    Ok(env)
}

/// Derive the HTTP auth header for the single designated credential var.
///
/// Authorization-ish names (containing `token`, `api_key`, or `key`) become an
/// `Authorization: Bearer` header; everything else becomes an `X-{NAME}`
/// custom header.
///
/// Enforced contract: this rule applies to exactly one credential per
/// request — the OAuth token, the keyfile bearer, or the single `ApiKey`
/// secret var (see [`build_headers_for_config`] and
/// `McpRuntime::http_auth_headers`). Blanket-mapping every env var through
/// it is forbidden: two key-ish names would both produce `Authorization`
/// and map insertion order would silently pick the winner. All remaining
/// env vars must become `X-{NAME}` custom headers instead and can never
/// yield a second `Authorization` header.
#[must_use]
pub fn derive_http_auth_header(name: &str, value: &str) -> (String, String) {
    let lower = name.to_lowercase();
    if lower.contains("token") || lower.contains("api_key") || lower.contains("key") {
        ("Authorization".to_string(), format!("Bearer {value}"))
    } else {
        (format!("X-{}", name.to_uppercase()), value.to_string())
    }
}

/// Build HTTP headers for an MCP (OAuth token, keyfile bearer, or keychain API key)
///
/// For `Http` transport with `ApiKey` auth, the keychain secret behind the
/// single configured secret env var is delivered through the shared header
/// rule. Zero or multiple secret vars is a configuration error: an
/// unauthenticated request would only fail later with a provider 401.
/// Only that one secret var passes through [`derive_http_auth_header`];
/// non-credential env vars are environment for the process, not headers.
///
/// # Errors
///
/// Returns `McpError` when the keyfile cannot be read, the `ApiKey` secret
/// var count is not exactly one, or the secret cannot be loaded from the
/// keychain.
pub fn build_headers_for_config(
    config: &McpConfig,
    secrets: &SecretsManager,
) -> McpResult<HashMap<String, String>> {
    let mut headers = HashMap::new();

    // Priority: oauth_token > keyfile
    if let Some(ref token) = config.oauth_token {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
        return Ok(headers);
    }
    if let Some(ref keyfile) = config.keyfile_path {
        let token = std::fs::read_to_string(keyfile).map_err(|e| {
            McpError::Config(format!(
                "MCP {}: cannot read keyfile {}: {e}",
                config.name,
                keyfile.display()
            ))
        })?;
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", token.trim()),
        );
    }

    if config.transport == McpTransport::Http && config.auth_type == McpAuthType::ApiKey {
        let secret_vars: Vec<&crate::mcp::EnvVarConfig> =
            config.env_vars.iter().filter(|var| var.is_secret).collect();
        let [var] = secret_vars.as_slice() else {
            return Err(McpError::Config(format!(
                "MCP {} needs exactly one secret env var for HTTP API key auth, found {}",
                config.name,
                secret_vars.len()
            )));
        };
        let key = secrets.load_api_key_named(config.id, &var.name)?;
        let (header, value) = derive_http_auth_header(&var.name, &key);
        headers.insert(header, value);
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
