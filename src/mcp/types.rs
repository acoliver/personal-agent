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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_serialization() {
        let config = McpConfig {
            id: Uuid::new_v4(),
            name: "test-mcp".to_string(),
            enabled: true,
            source: McpSource::Official {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@test/mcp".to_string(),
                runtime_hint: Some("node".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::ApiKey,
            env_vars: vec![EnvVarConfig {
                name: "API_KEY".to_string(),
                required: true,
            }],
            package_args: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_mcp_source_serialization() {
        let official = McpSource::Official {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
        };
        let json = serde_json::to_string(&official).unwrap();
        assert!(json.contains(r#""type":"official""#));

        let smithery = McpSource::Smithery {
            qualified_name: "org/mcp".to_string(),
        };
        let json = serde_json::to_string(&smithery).unwrap();
        assert!(json.contains(r#""type":"smithery""#));

        let manual = McpSource::Manual {
            url: "https://example.com".to_string(),
        };
        let json = serde_json::to_string(&manual).unwrap();
        assert!(json.contains(r#""type":"manual""#));
    }

    #[test]
    fn test_detect_auth_type_oauth() {
        let env_vars = vec![
            RegistryEnvVar {
                name: "CLIENT_ID".to_string(),
                is_secret: false,
                is_required: true,
            },
            RegistryEnvVar {
                name: "CLIENT_SECRET".to_string(),
                is_secret: true,
                is_required: true,
            },
        ];

        assert_eq!(detect_auth_type(&env_vars), McpAuthType::OAuth);
    }

    #[test]
    fn test_detect_auth_type_api_key() {
        let env_vars = vec![RegistryEnvVar {
            name: "API_KEY".to_string(),
            is_secret: true,
            is_required: true,
        }];

        assert_eq!(detect_auth_type(&env_vars), McpAuthType::ApiKey);
    }

    #[test]
    fn test_detect_auth_type_token() {
        let env_vars = vec![RegistryEnvVar {
            name: "ACCESS_TOKEN".to_string(),
            is_secret: true,
            is_required: true,
        }];

        assert_eq!(detect_auth_type(&env_vars), McpAuthType::ApiKey);
    }

    #[test]
    fn test_detect_auth_type_pat() {
        let env_vars = vec![RegistryEnvVar {
            name: "GITHUB_PAT".to_string(),
            is_secret: true,
            is_required: true,
        }];

        assert_eq!(detect_auth_type(&env_vars), McpAuthType::ApiKey);
    }

    #[test]
    fn test_detect_auth_type_none() {
        let env_vars = vec![RegistryEnvVar {
            name: "CONFIG_VAR".to_string(),
            is_secret: false,
            is_required: false,
        }];

        assert_eq!(detect_auth_type(&env_vars), McpAuthType::None);
    }

    #[test]
    fn test_detect_auth_type_empty() {
        let env_vars: Vec<RegistryEnvVar> = vec![];
        assert_eq!(detect_auth_type(&env_vars), McpAuthType::None);
    }

    #[test]
    fn test_mcp_auth_type_default() {
        assert_eq!(McpAuthType::default(), McpAuthType::None);
    }

    #[test]
    fn test_package_type_serialization() {
        let npm = McpPackageType::Npm;
        let json = serde_json::to_string(&npm).unwrap();
        assert_eq!(json, r#""npm""#);

        let docker = McpPackageType::Docker;
        let json = serde_json::to_string(&docker).unwrap();
        assert_eq!(json, r#""docker""#);

        let http = McpPackageType::Http;
        let json = serde_json::to_string(&http).unwrap();
        assert_eq!(json, r#""http""#);
    }

    #[test]
    fn test_transport_serialization() {
        let stdio = McpTransport::Stdio;
        let json = serde_json::to_string(&stdio).unwrap();
        assert_eq!(json, r#""stdio""#);

        let http = McpTransport::Http;
        let json = serde_json::to_string(&http).unwrap();
        assert_eq!(json, r#""http""#);
    }

    #[test]
    fn test_env_var_config() {
        let env_var = EnvVarConfig {
            name: "TEST_VAR".to_string(),
            required: true,
        };

        let json = serde_json::to_string(&env_var).unwrap();
        let deserialized: EnvVarConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(env_var, deserialized);
    }

    #[test]
    fn test_mcp_config_with_keyfile() {
        let config = McpConfig {
            id: Uuid::new_v4(),
            name: "test-mcp".to_string(),
            enabled: true,
            source: McpSource::Manual {
                url: "https://example.com".to_string(),
            },
            package: McpPackage {
                package_type: McpPackageType::Docker,
                identifier: "test/mcp:latest".to_string(),
                runtime_hint: None,
            },
            transport: McpTransport::Http,
            auth_type: McpAuthType::Keyfile,
            env_vars: vec![],
            package_args: vec![],
            keyfile_path: Some(PathBuf::from("/path/to/keyfile")),
            config: serde_json::json!({"key": "value"}),
            oauth_token: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_registry_env_var_defaults() {
        let env_var: RegistryEnvVar = serde_json::from_str(r#"{"name": "TEST"}"#).unwrap();
        assert_eq!(env_var.name, "TEST");
        assert!(!env_var.is_secret);
        assert!(!env_var.is_required);
    }
}
