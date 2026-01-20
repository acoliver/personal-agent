use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
pub struct McpConfig {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub source: McpSource,
    pub package: McpPackage,
    pub transport: McpTransport,
    pub auth_type: McpAuthType,
    /// Environment variables this MCP requires (from registry metadata)
    #[serde(default)]
    pub env_vars: Vec<EnvVarConfig>,
    /// Package arguments this MCP requires (from registry metadata)
    #[serde(default)]
    pub package_args: Vec<McpPackageArg>,
    /// Path to keyfile if `auth_type` is Keyfile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyfile_path: Option<PathBuf>,
    /// MCP-specific configuration from configSchema
    #[serde(default)]
    pub config: serde_json::Value,
    /// OAuth token for Smithery or other OAuth-based MCPs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvVarConfig {
    pub name: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpPackageArgType {
    Named,
    Positional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpPackageArg {
    pub arg_type: McpPackageArgType,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpSource {
    Official { name: String, version: String },
    Smithery { qualified_name: String },
    Manual { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpPackage {
    #[serde(rename = "type")]
    pub package_type: McpPackageType,
    pub identifier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpPackageType {
    Npm,
    Docker,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthType {
    #[default]
    None,
    ApiKey,
    Keyfile,
    OAuth,
}

/// Registry environment variable metadata (from Official MCP registry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEnvVar {
    pub name: String,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(default)]
    pub is_required: bool,
}

/// Detect auth type from registry environment variable metadata
#[must_use]
pub fn detect_auth_type(env_vars: &[RegistryEnvVar]) -> McpAuthType {
    let has_client_id = env_vars.iter().any(|v| v.name.contains("CLIENT_ID"));
    let has_client_secret = env_vars
        .iter()
        .any(|v| v.name.contains("CLIENT_SECRET") && v.is_secret);

    if has_client_id && has_client_secret {
        return McpAuthType::OAuth;
    }

    let has_secret_token = env_vars.iter().any(|v| {
        v.is_secret
            && (v.name.contains("TOKEN")
                || v.name.contains("API_KEY")
                || v.name.contains("_KEY")
                || v.name.contains("_PAT"))
    });

    if has_secret_token {
        return McpAuthType::ApiKey;
    }

    McpAuthType::None
}
