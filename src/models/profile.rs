//! Model profile definitions

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProfile {
    pub id: Uuid,
    pub name: String,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: String,
    pub auth: AuthConfig,
    pub parameters: ModelParameters,
    /// System prompt to prepend to conversations
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_context_window_size")]
    pub context_window_size: usize,
}

pub const DEFAULT_SYSTEM_PROMPT: &str = "Unless instructed otherwise, respond solely in English.";

fn default_system_prompt() -> String {
    DEFAULT_SYSTEM_PROMPT.to_string()
}

const fn default_context_window_size() -> usize {
    128_000
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    /// API key stored in the OS keychain, referenced by label.
    Keychain { label: String },
    /// No authentication required (for local/offline models).
    None,
}

impl<'de> serde::Deserialize<'de> for AuthConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::collections::HashMap;

        let map = HashMap::<String, serde_json::Value>::deserialize(deserializer)?;
        match map.get("type").and_then(|v| v.as_str()) {
            Some("keychain") => {
                let label = map
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(Self::Keychain { label })
            }
            Some("none") => Ok(Self::None),
            // Legacy and unknown formats map to empty keychain labels so the secret
            // must be re-stored before use.
            _ => Ok(Self::Keychain {
                label: String::new(),
            }),
        }
    }
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keychain { label } => f.debug_struct("Keychain").field("label", label).finish(),
            Self::None => f.debug_struct("None").finish(),
        }
    }
}

impl AuthConfig {
    /// Returns `true` if this auth config requires an API key.
    #[must_use]
    pub const fn requires_api_key(&self) -> bool {
        match self {
            Self::Keychain { .. } => true,
            Self::None => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelParameters {
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub thinking_budget: Option<u32>,
    pub enable_thinking: bool,
    pub show_thinking: bool,
}

impl Default for ModelProfile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Default Profile".to_string(),
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            auth: AuthConfig::Keychain {
                label: String::new(),
            },
            parameters: ModelParameters::default(),
            system_prompt: default_system_prompt(),
            context_window_size: default_context_window_size(),
        }
    }
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 4096,
            thinking_budget: None,
            enable_thinking: false,
            show_thinking: false,
        }
    }
}

impl ModelProfile {
    /// Create a new profile with a unique ID
    #[must_use]
    pub fn new(
        name: String,
        provider_id: String,
        model_id: String,
        base_url: String,
        auth: AuthConfig,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            provider_id,
            model_id,
            base_url,
            auth,
            parameters: ModelParameters::default(),
            system_prompt: default_system_prompt(),
            context_window_size: default_context_window_size(),
        }
    }

    /// Create a profile with custom parameters
    #[must_use]
    pub const fn with_parameters(mut self, parameters: ModelParameters) -> Self {
        self.parameters = parameters;
        self
    }

    /// Update the profile name
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Update the auth config
    pub fn set_auth(&mut self, auth: AuthConfig) {
        self.auth = auth;
    }

    /// Update the parameters
    pub const fn set_parameters(&mut self, parameters: ModelParameters) {
        self.parameters = parameters;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_config_requires_api_key_returns_correct_value() {
        assert!(AuthConfig::Keychain {
            label: "test".to_string()
        }
        .requires_api_key());
        assert!(AuthConfig::Keychain {
            label: String::new()
        }
        .requires_api_key());
        assert!(!AuthConfig::None.requires_api_key());
    }

    #[test]
    fn auth_config_none_serializes_correctly() {
        let auth = AuthConfig::None;
        let json = serde_json::to_string(&auth).expect("serialize None");
        assert_eq!(json, r#"{"type":"none"}"#);
    }

    #[test]
    fn auth_config_none_deserializes_correctly() {
        let json = r#"{"type":"none"}"#;
        let auth: AuthConfig = serde_json::from_str(json).expect("deserialize None");
        assert_eq!(auth, AuthConfig::None);
    }

    #[test]
    fn auth_config_keychain_serializes_correctly() {
        let auth = AuthConfig::Keychain {
            label: "my-key".to_string(),
        };
        let json = serde_json::to_string(&auth).expect("serialize Keychain");
        assert_eq!(json, r#"{"type":"keychain","label":"my-key"}"#);
    }
}
