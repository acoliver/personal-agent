//! LLM client for making requests to AI providers
//!
//! This module bridges `PersonalAgent`'s profile system with `SerdesAI`,
//! using models.dev registry data for provider configuration.

use crate::models::{AuthConfig, ModelProfile};
use crate::registry::RegistryCache;
use futures::StreamExt;
use serdes_ai::prelude::*;
use std::fs;
use thiserror::Error;

// Use std Result to avoid conflict with serdes_ai::prelude::Result
type StdResult<T, E> = std::result::Result<T, E>;

/// Errors from LLM operations
#[derive(Debug, Error)]
pub enum LlmError {
    /// Failed to read API key file
    #[error("Failed to read keyfile {path}: {source}")]
    KeyfileRead {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// `SerdesAI` error
    #[error("LLM error: {0}")]
    SerdesAi(String),

    /// Invalid provider
    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    /// No API key configured
    #[error("No API key configured for profile")]
    NoApiKey,
}

/// Events emitted during streaming
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text content delta
    TextDelta(String),
    /// Thinking content delta (for reasoning models)
    ThinkingDelta(String),
    /// Stream completed
    Complete,
    /// Error occurred
    Error(String),
}

/// LLM client that uses `SerdesAI`
pub struct LlmClient {
    profile: ModelProfile,
    api_key: String,
    /// Base URL from models.dev registry (if available)
    registry_base_url: Option<String>,
}

impl LlmClient {
    /// Create a new LLM client from a model profile
    /// 
    /// This looks up the provider in the models.dev registry to get
    /// the correct API base URL and configuration.
    pub fn from_profile(profile: &ModelProfile) -> StdResult<Self, LlmError> {
        let api_key = Self::resolve_api_key(profile)?;
        
        // Look up provider info from models.dev registry
        let registry_base_url = Self::get_registry_base_url(&profile.provider_id);

        Ok(Self {
            profile: profile.clone(),
            api_key,
            registry_base_url,
        })
    }
    
    /// Get the base URL from models.dev registry for a provider
    fn get_registry_base_url(provider_id: &str) -> Option<String> {
        let cache_path = RegistryCache::default_path().ok()?;
        let cache = RegistryCache::new(cache_path, 24); // 24 hour expiry
        if let Ok(Some(registry)) = cache.load() {
            if let Some(provider) = registry.providers.get(provider_id) {
                return provider.api.clone();
            }
        }
        None
    }

    /// Resolve the API key from profile auth config
    fn resolve_api_key(profile: &ModelProfile) -> StdResult<String, LlmError> {
        let key = match &profile.auth {
            AuthConfig::Key { value } => {
                if value.is_empty() {
                    return Err(LlmError::NoApiKey);
                }
                value.clone()
            }
            AuthConfig::Keyfile { path } => {
                fs::read_to_string(path)
                    .map_err(|e| LlmError::KeyfileRead {
                        path: path.clone(),
                        source: e,
                    })?
                    .trim()
                    .to_string()
            }
        };

        if key.is_empty() {
            return Err(LlmError::NoApiKey);
        }

        Ok(key)
    }

    /// Get the model spec string for `SerdesAI` (e.g., "openai:gpt-4o")
    fn model_spec(&self) -> String {
        // Use get_serdes_provider to handle OpenAI-compatible providers
        let provider = self.get_serdes_provider();
        format!("{}:{}", provider, self.profile.model_id)
    }

    /// Build model settings from profile parameters
    fn model_settings(&self) -> ModelSettings {
        ModelSettings {
            temperature: Some(self.profile.parameters.temperature),
            top_p: Some(self.profile.parameters.top_p),
            max_tokens: Some(u64::from(self.profile.parameters.max_tokens)),
            ..ModelSettings::default()
        }
    }

    /// Make a non-streaming request
    pub async fn request(&self, messages: &[Message]) -> StdResult<String, LlmError> {
        // Set API key in environment for SerdesAI
        self.set_api_key_env();

        let model_requests: Vec<ModelRequest> = messages
            .iter()
            .map(|m| {
                let mut req = ModelRequest::new();
                match m.role {
                    Role::User => req.add_user_prompt(m.content.clone()),
                    Role::Assistant => {
                        // For assistant messages, we'd typically use ModelResponse
                        // but for simplicity, add as user context with prefix
                        req.add_user_prompt(format!("[Assistant]: {}", &m.content));
                    }
                    Role::System => req.add_system_prompt(m.content.clone()),
                }
                req
            })
            .collect();

        // Determine base URL: profile override > registry > none
        let base_url = if self.profile.base_url.is_empty() {
            self.registry_base_url.as_deref()
        } else {
            Some(self.profile.base_url.as_str())
        };

        // Determine provider type from registry
        let provider = self.get_serdes_provider();
        
        // Build the model with custom base URL if needed
        let model = serdes_ai::models::build_model_with_config(
            provider,
            &self.profile.model_id,
            Some(&self.api_key),
            base_url,
            None,
        ).map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Make the request using the model directly
        let response = model
            .request(&model_requests, &self.model_settings(), &Default::default())
            .await
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Extract text from response parts
        let text = response
            .parts
            .iter()
            .filter_map(|p| {
                if let serdes_ai::core::ModelResponsePart::Text(t) = p {
                    Some(t.content.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        Ok(text)
    }

    /// Make a streaming request, returning events via callback
    pub async fn request_stream<F>(
        &self,
        messages: &[Message],
        mut on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        // Set API key in environment for SerdesAI
        self.set_api_key_env();

        let model_requests: Vec<ModelRequest> = messages
            .iter()
            .map(|m| {
                let mut req = ModelRequest::new();
                match m.role {
                    Role::User => req.add_user_prompt(m.content.clone()),
                    Role::Assistant => {
                        req.add_user_prompt(format!("[Assistant]: {}", &m.content));
                    }
                    Role::System => req.add_system_prompt(m.content.clone()),
                }
                req
            })
            .collect();

        // Determine base URL: profile override > registry > none
        let base_url = if self.profile.base_url.is_empty() {
            self.registry_base_url.as_deref()
        } else {
            Some(self.profile.base_url.as_str())
        };

        // Determine provider type from registry
        let provider = self.get_serdes_provider();
        
        // Build the model with custom base URL if needed
        let model = serdes_ai::models::build_model_with_config(
            provider,
            &self.profile.model_id,
            Some(&self.api_key),
            base_url,
            None,
        ).map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Use the model directly for streaming
        let mut stream = model.request_stream(&model_requests, &self.model_settings(), &Default::default())
            .await
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        use serdes_ai::core::messages::ModelResponseStreamEvent;

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => match event {
                    ModelResponseStreamEvent::PartDelta(delta) => {
                        use serdes_ai::core::messages::ModelResponsePartDelta;
                        match &delta.delta {
                            ModelResponsePartDelta::Text(t) => {
                                on_event(StreamEvent::TextDelta(t.content_delta.clone()));
                            }
                            ModelResponsePartDelta::Thinking(t) => {
                                on_event(StreamEvent::ThinkingDelta(t.content_delta.clone()));
                            }
                            _ => {}
                        }
                    }
                    ModelResponseStreamEvent::PartStart(start) => {
                        use serdes_ai::core::ModelResponsePart;
                        match &start.part {
                            ModelResponsePart::Text(t) => {
                                if !t.content.is_empty() {
                                    on_event(StreamEvent::TextDelta(t.content.clone()));
                                }
                            }
                            ModelResponsePart::Thinking(t) => {
                                if !t.content.is_empty() {
                                    on_event(StreamEvent::ThinkingDelta(t.content.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                    ModelResponseStreamEvent::PartEnd(_) => {}
                },
                Err(e) => {
                    on_event(StreamEvent::Error(e.to_string()));
                    return Err(LlmError::SerdesAi(e.to_string()));
                }
            }
        }

        on_event(StreamEvent::Complete);
        Ok(())
    }

    /// Set the API key in the environment for the provider
    /// 
    /// Uses the `env` field from models.dev registry to determine the correct
    /// environment variable name for the provider.
    fn set_api_key_env(&self) {
        // Look up the env var name from registry, or use provider-specific defaults
        let env_var = self.get_env_var_name();
        std::env::set_var(&env_var, &self.api_key);
    }
    
    /// Get the environment variable name for API key from models.dev registry
    fn get_env_var_name(&self) -> String {
        // First try to get from registry
        if let Ok(cache_path) = RegistryCache::default_path() {
            let cache = RegistryCache::new(cache_path, 24);
            if let Ok(Some(registry)) = cache.load() {
                if let Some(provider) = registry.providers.get(&self.profile.provider_id) {
                    // Use the first env var (typically the API key)
                    if let Some(env_var) = provider.env.first() {
                        return env_var.clone();
                    }
                }
            }
        }
        
        // Fallback to OPENAI_API_KEY for OpenAI-compatible providers
        "OPENAI_API_KEY".to_string()
    }
    
    /// Determine the provider type for `SerdesAI`
    /// 
    /// Uses models.dev registry `npm` field to detect OpenAI-compatible providers:
    /// - `@ai-sdk/openai-compatible` -> use "openai" provider with custom `base_url`
    /// - `@ai-sdk/openai` -> native openai
    /// - `@ai-sdk/anthropic` -> native anthropic
    fn get_serdes_provider(&self) -> &str {
        if let Ok(cache_path) = RegistryCache::default_path() {
            let cache = RegistryCache::new(cache_path, 24);
            if let Ok(Some(registry)) = cache.load() {
                if let Some(provider) = registry.providers.get(&self.profile.provider_id) {
                    if let Some(npm) = &provider.npm {
                        if npm.contains("openai-compatible") {
                            return "openai"; // Use OpenAI provider with custom base URL
                        }
                    }
                }
            }
        }
        
        // Use provider_id directly for known SerdesAI providers
        match self.profile.provider_id.as_str() {
            "anthropic" | "claude" => "anthropic",
            "groq" => "groq",
            "mistral" => "mistral",
            _ => "openai", // Default to OpenAI-compatible
        }
    }
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// A chat message
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub thinking_content: Option<String>,
}

impl Message {
    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            thinking_content: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            thinking_content: None,
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            thinking_content: None,
        }
    }

    /// Add thinking content
    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking_content = Some(thinking.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "Hello");
        assert!(msg.thinking_content.is_none());
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("Hi there");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content, "Hi there");
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("You are helpful");
        assert_eq!(msg.role, Role::System);
        assert_eq!(msg.content, "You are helpful");
    }

    #[test]
    fn test_message_with_thinking() {
        let msg = Message::assistant("Answer").with_thinking("Let me think...");
        assert_eq!(msg.thinking_content, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_client_from_profile_no_key() {
        let profile = ModelProfile {
            auth: AuthConfig::Key {
                value: String::new(),
            },
            ..Default::default()
        };
        let result = LlmClient::from_profile(&profile);
        assert!(matches!(result, Err(LlmError::NoApiKey)));
    }

    #[test]
    fn test_client_from_profile_with_key() {
        let profile = ModelProfile {
            auth: AuthConfig::Key {
                value: "test-key".to_string(),
            },
            ..Default::default()
        };
        let result = LlmClient::from_profile(&profile);
        assert!(result.is_ok());
    }

    #[test]
    fn test_model_spec() {
        let profile = ModelProfile {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-opus".to_string(),
            auth: AuthConfig::Key {
                value: "test".to_string(),
            },
            ..Default::default()
        };
        let client = LlmClient::from_profile(&profile).unwrap();
        assert_eq!(client.model_spec(), "anthropic:claude-3-opus");
    }
}
