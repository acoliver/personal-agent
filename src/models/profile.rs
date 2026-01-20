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
            auth: AuthConfig::Key {
                value: String::new(),
            },
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
