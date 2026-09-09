use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVarConfig {
    pub name: String,
    pub required: bool,
    /// Whether the value is a secret resolved from the OS keychain at runtime.
    pub is_secret: bool,
    /// Plain value for non-secret vars, serialized into the config.
    ///
    /// Invariant: when `is_secret` is true this MUST stay `None` and the
    /// value is resolved from the OS keychain at runtime; only non-secret
    /// vars may carry a serialized plain value. The manual `Serialize` and
    /// `Deserialize` impls below enforce that invariant at the serde
    /// boundary, so a hand-edited config file can neither persist nor load
    /// a secret into `value`.
    pub value: Option<String>,
}

/// Serde shadow of [`EnvVarConfig`]; exists so the secret invariant can be
/// sanitized in one place before the fields cross the serde boundary.
#[derive(Serialize, Deserialize)]
struct EnvVarConfigFields {
    name: String,
    required: bool,
    #[serde(default)]
    is_secret: bool,
    #[serde(default)]
    value: Option<String>,
}

impl Serialize for EnvVarConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        EnvVarConfigFields {
            name: self.name.clone(),
            required: self.required,
            is_secret: self.is_secret,
            value: if self.is_secret {
                None
            } else {
                self.value.clone()
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EnvVarConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut fields = EnvVarConfigFields::deserialize(deserializer)?;
        if fields.is_secret {
            fields.value = None;
        }
        Ok(Self {
            name: fields.name,
            required: fields.required,
            is_secret: fields.is_secret,
            value: fields.value,
        })
    }
}

/// Keywords that mark an env var name as holding a secret.
const SECRET_NAME_KEYWORDS: [&str; 4] = ["TOKEN", "KEY", "SECRET", "PAT"];

/// Infer whether an env var holds a secret from its name.
///
/// Registry search results flatten env metadata to name/value pairs, so this
/// restores the secret flag for drafts that never see the full registry
/// metadata. [`detect_auth_type`] applies the same matcher, keeping the
/// derived flag and the inferred auth type consistent.
///
/// A keyword counts only as a delimited word: preceded by the string start,
/// an underscore, or a camelCase transition, and followed by the string end
/// or an underscore. `API_KEY`, `ACCESS_TOKEN`, and `CLIENT_SECRET` match;
/// the `PAT` in `BUNDLE_PATH` and the `KEY` in `KEYCLOAK` do not.
#[must_use]
pub fn env_var_name_is_secret(name: &str) -> bool {
    SECRET_NAME_KEYWORDS
        .iter()
        .any(|keyword| contains_keyword_word(name, keyword))
}

/// ASCII-case-insensitive search for `keyword` in `name` as a delimited
/// word: preceded by the start, an underscore, or a camelCase transition,
/// and followed by the end or an underscore. Boundaries are judged on the
/// original casing so `apiKey` matches `KEY` while `KEYCLOAK` does not.
fn contains_keyword_word(name: &str, keyword: &str) -> bool {
    let bytes = name.as_bytes();
    let needle = keyword.as_bytes();
    let mut start = 0;
    while start + needle.len() <= bytes.len() {
        if !bytes[start..start + needle.len()].eq_ignore_ascii_case(needle) {
            start += 1;
            continue;
        }
        let end = start + needle.len();
        // A keyword match is all-ASCII, so `start` is a char boundary and
        // slicing the original text here cannot panic.
        let preceded_by_boundary = start == 0
            || bytes[start - 1] == b'_'
            || name[..start].ends_with(|c: char| c.is_lowercase());
        let followed_by_boundary = end == bytes.len() || bytes[end] == b'_';
        if preceded_by_boundary && followed_by_boundary {
            return true;
        }
        start += 1;
    }
    false
}

/// Whether `name` is safe to use as an MCP env var name.
///
/// Env var names are free-form text typed in the UI or read from remote
/// registry metadata, and they flow into HTTP header material. A name with
/// whitespace or control characters would produce malformed or injectable
/// headers, so the accepted set is deliberately narrow: non-empty, at most
/// 64 chars, starting with an ASCII letter or underscore, and containing
/// only ASCII letters, digits, or underscores.
#[must_use]
pub fn env_var_name_is_valid(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    name.len() <= 64 && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
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

/// Detect auth type from registry environment variable metadata.
///
/// OAuth requires a `CLIENT_ID`/`CLIENT_SECRET` pair with the secret flagged
/// in the registry metadata. Otherwise any secret-flagged var whose name
/// [`env_var_name_is_secret`] classifies as secret implies `ApiKey`. Both
/// branches share that matcher, so a name that yields a secret flag also
/// yields `ApiKey` auth.
#[must_use]
pub fn detect_auth_type(env_vars: &[RegistryEnvVar]) -> McpAuthType {
    let has_client_id = env_vars
        .iter()
        .any(|v| contains_keyword_word(&v.name, "CLIENT_ID"));
    let has_client_secret = env_vars
        .iter()
        .any(|v| v.is_secret && contains_keyword_word(&v.name, "CLIENT_SECRET"));

    if has_client_id && has_client_secret {
        return McpAuthType::OAuth;
    }

    let has_secret_var = env_vars
        .iter()
        .any(|v| v.is_secret && env_var_name_is_secret(&v.name));

    if has_secret_var {
        return McpAuthType::ApiKey;
    }

    McpAuthType::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_var_name_validation_accepts_safe_names() {
        assert!(env_var_name_is_valid("API_KEY"));
        assert!(env_var_name_is_valid("_private"));
        assert!(env_var_name_is_valid("a"));
        assert!(env_var_name_is_valid("A1_b2"));
        assert!(env_var_name_is_valid(&"A".repeat(64)));
    }

    #[test]
    fn env_var_name_validation_rejects_empty_and_too_long_names() {
        assert!(!env_var_name_is_valid(""));
        assert!(!env_var_name_is_valid(&"A".repeat(65)));
    }

    #[test]
    fn env_var_name_validation_rejects_names_with_whitespace() {
        assert!(!env_var_name_is_valid("EXA KEY"));
        assert!(!env_var_name_is_valid("KEY\r\n"));
        assert!(!env_var_name_is_valid("KEY\n"));
        assert!(!env_var_name_is_valid("\tKEY"));
    }

    #[test]
    fn env_var_name_validation_rejects_bad_first_or_body_chars() {
        assert!(!env_var_name_is_valid("1KEY"));
        assert!(!env_var_name_is_valid("KEY-NAME"));
        assert!(!env_var_name_is_valid("CLÉ"));
    }

    #[test]
    fn env_var_name_secret_detection_respects_word_boundaries() {
        assert!(env_var_name_is_secret("API_KEY"));
        assert!(env_var_name_is_secret("apiKey"));
        assert!(env_var_name_is_secret("ACCESS_TOKEN"));
        assert!(env_var_name_is_secret("CLIENT_SECRET"));
        assert!(env_var_name_is_secret("GITHUB_PAT"));
        assert!(env_var_name_is_secret("TOKEN"));

        // A trailing keyword fragment inside a longer word is not a secret.
        assert!(!env_var_name_is_secret("BUNDLE_PATH"));
        assert!(!env_var_name_is_secret("CONFIG_PATH"));
        assert!(!env_var_name_is_secret("NPM_PATH"));
        assert!(!env_var_name_is_secret("LOG_LEVEL"));
        assert!(!env_var_name_is_secret("KEYCLOAK_ID"));
    }

    #[test]
    fn secret_name_heuristic_and_auth_detection_agree() {
        let names = [
            "API_KEY",
            "apiKey",
            "ACCESS_TOKEN",
            "CLIENT_SECRET",
            "GITHUB_PAT",
            "TOKEN",
            "BUNDLE_PATH",
            "CONFIG_PATH",
            "NPM_PATH",
            "LOG_LEVEL",
        ];
        for name in names {
            let is_secret = env_var_name_is_secret(name);
            let detected = detect_auth_type(&[RegistryEnvVar {
                name: name.to_string(),
                is_secret,
                is_required: true,
            }]);
            assert_eq!(
                detected,
                if is_secret {
                    McpAuthType::ApiKey
                } else {
                    McpAuthType::None
                },
                "name {name}: secret heuristic and auth detection must agree"
            );
        }
    }

    #[test]
    fn serde_strips_plain_value_from_secret_env_vars() {
        let json = r#"{"name":"API_KEY","required":true,"is_secret":true,"value":"hunter2"}"#;
        let var: EnvVarConfig = serde_json::from_str(json).expect("deserialize env var");
        assert!(var.is_secret);
        assert_eq!(var.value, None, "a secret var must never load a value");
    }

    #[test]
    fn serde_defaults_missing_optional_fields() {
        let sparse: EnvVarConfig =
            serde_json::from_str(r#"{"name":"API_KEY","required":true,"is_secret":true}"#)
                .expect("deserialize env var without optional fields");
        assert!(sparse.is_secret);
        assert_eq!(sparse.value, None);
    }

    #[test]
    fn serde_never_writes_a_value_for_secret_env_vars() {
        let var = EnvVarConfig {
            name: "API_KEY".to_string(),
            required: true,
            is_secret: true,
            value: Some("hunter2".to_string()),
        };
        let json = serde_json::to_string(&var).expect("serialize env var");
        assert!(
            !json.contains("hunter2"),
            "the secret must stay out of the serialized config: {json}"
        );
        assert_eq!(
            json,
            r#"{"name":"API_KEY","required":true,"is_secret":true,"value":null}"#
        );
    }

    #[test]
    fn serde_round_trips_plain_env_vars_unchanged() {
        let var = EnvVarConfig {
            name: "NPM_PATH".to_string(),
            required: false,
            is_secret: false,
            value: Some("/opt/npm".to_string()),
        };
        let json = serde_json::to_string(&var).expect("serialize env var");
        assert_eq!(
            json,
            r#"{"name":"NPM_PATH","required":false,"is_secret":false,"value":"/opt/npm"}"#
        );
        let parsed: EnvVarConfig = serde_json::from_str(&json).expect("deserialize env var");
        assert_eq!(parsed, var);
    }
}
