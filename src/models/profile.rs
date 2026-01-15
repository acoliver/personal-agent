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
}

fn default_system_prompt() -> String {
    "You are a helpful assistant, be direct and to the point. Respond in English.".to_string()
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    Key { value: String },
    Keyfile { path: String },
}

// Custom Debug impl to redact API keys from logs
impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key { .. } => f.debug_struct("Key").field("value", &"[REDACTED]").finish(),
            Self::Keyfile { path } => f.debug_struct("Keyfile").field("path", path).finish(),
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
            auth: AuthConfig::Key { value: String::new() },
            parameters: ModelParameters::default(),
            system_prompt: default_system_prompt(),
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
    fn test_default_profile() {
        let profile = ModelProfile::default();
        assert_eq!(profile.name, "Default Profile");
        assert_eq!(profile.provider_id, "openai");
        assert_eq!(profile.model_id, "gpt-4");
        assert_eq!(profile.parameters.temperature, 0.7);
        assert!(!profile.parameters.enable_thinking);
    }

    #[test]
    fn test_new_profile() {
        let profile = ModelProfile::new(
            "Claude".to_string(),
            "anthropic".to_string(),
            "claude-3-opus".to_string(),
            "https://api.anthropic.com/v1".to_string(),
            AuthConfig::Key { value: "test-key".to_string() },
        );

        assert_eq!(profile.name, "Claude");
        assert_eq!(profile.provider_id, "anthropic");
        assert_eq!(profile.model_id, "claude-3-opus");
        assert_eq!(profile.base_url, "https://api.anthropic.com/v1");
    }

    #[test]
    fn test_with_parameters() {
        let params = ModelParameters {
            temperature: 0.5,
            top_p: 0.9,
            max_tokens: 2048,
            thinking_budget: Some(1000),
            enable_thinking: true,
            show_thinking: true,
        };

        let profile = ModelProfile::default().with_parameters(params.clone());
        assert_eq!(profile.parameters, params);
    }

    #[test]
    fn test_set_name() {
        let mut profile = ModelProfile::default();
        profile.set_name("New Name".to_string());
        assert_eq!(profile.name, "New Name");
    }

    #[test]
    fn test_set_auth() {
        let mut profile = ModelProfile::default();
        let new_auth = AuthConfig::Keyfile { path: "/path/to/key".to_string() };
        profile.set_auth(new_auth.clone());
        assert_eq!(profile.auth, new_auth);
    }

    #[test]
    fn test_set_parameters() {
        let mut profile = ModelProfile::default();
        let new_params = ModelParameters {
            temperature: 0.9,
            top_p: 0.95,
            max_tokens: 8192,
            thinking_budget: Some(2000),
            enable_thinking: true,
            show_thinking: false,
        };
        profile.set_parameters(new_params.clone());
        assert_eq!(profile.parameters, new_params);
    }

    #[test]
    fn test_auth_key_serialization() {
        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"key\""));
        assert!(json.contains("\"value\":\"test-key\""));
    }

    #[test]
    fn test_auth_keyfile_serialization() {
        let auth = AuthConfig::Keyfile { path: "/path/to/key".to_string() };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"keyfile\""));
        assert!(json.contains("\"path\":\"/path/to/key\""));
    }

    #[test]
    fn test_profile_serialization() {
        let profile = ModelProfile::default();
        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: ModelProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, deserialized);
    }

    #[test]
    fn test_unique_ids() {
        let profile1 = ModelProfile::default();
        let profile2 = ModelProfile::default();
        assert_ne!(profile1.id, profile2.id);
    }
}
