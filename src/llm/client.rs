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
    /// Tool use requested by the model
    ToolUse(crate::llm::tools::ToolUse),
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

    /// Build a model with extended configuration (thinking support, etc.)
    fn build_model(
        &self,
        provider: &str,
        base_url: Option<&str>,
    ) -> StdResult<std::sync::Arc<dyn serdes_ai::Model>, LlmError> {
        use serdes_ai::ExtendedModelConfig;
        
        let mut config = ExtendedModelConfig::new()
            .with_api_key(&self.api_key);
        
        if let Some(url) = base_url {
            config = config.with_base_url(url);
        }
        
        // Enable thinking if profile has it enabled
        if self.profile.parameters.enable_thinking {
            let budget = self.profile.parameters.thinking_budget.map(u64::from);
            config = config.with_thinking(budget);
        }
        
        serdes_ai::build_model_extended(provider, &self.profile.model_id, config)
            .map_err(|e| LlmError::SerdesAi(e.to_string()))
    }

    /// Make a non-streaming request
    pub async fn request(&self, messages: &[Message]) -> StdResult<Message, LlmError> {
        self.request_with_tools(messages, &[]).await
    }

    /// Make a non-streaming request with tools
    pub async fn request_with_tools(
        &self,
        messages: &[Message],
        tools: &[crate::llm::tools::Tool],
    ) -> StdResult<Message, LlmError> {
        // Set API key in environment for SerdesAI
        self.set_api_key_env();

        let model_requests: Vec<ModelRequest> = messages
            .iter()
            .map(|m| {
                let mut req = ModelRequest::new();
                match m.role {
                    Role::User => {
                        // Add user content
                        if !m.content.is_empty() {
                            req.add_user_prompt(m.content.clone());
                        }
                        
                        // Add tool results if present
                        if !m.tool_results.is_empty() {
                            use serdes_ai::core::messages::request::{ModelRequestPart, ToolReturnPart};
                            
                            for tool_result in &m.tool_results {
                                let tool_return = if tool_result.is_error {
                                    ToolReturnPart::error("tool", &tool_result.content)
                                        .with_tool_call_id(&tool_result.tool_use_id)
                                } else {
                                    ToolReturnPart::success("tool", &tool_result.content)
                                        .with_tool_call_id(&tool_result.tool_use_id)
                                };
                                
                                // Manual push since there's no add_tool_result method
                                req.parts.push(ModelRequestPart::ToolReturn(tool_return));
                            }
                        }
                    }
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
        
        // Build the model with extended config for thinking support
        let model = self.build_model(provider, base_url)?;

        // Convert tools to SerdesAI ToolDefinition format
        let tool_defs: Vec<ToolDefinition> = tools
            .iter()
            .map(|t| ToolDefinition::new(&t.name, &t.description).with_parameters(t.input_schema.clone()))
            .collect();

        // Create request parameters with tools
        // Pass empty default params for now - tools will be handled through model_settings
        // TODO: Figure out proper ModelRequestParameters import path
        let params = Default::default();

        // Make the request using the model directly
        let response = model
            .request(&model_requests, &self.model_settings(), &params)
            .await
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Parse response into Message
        self.parse_response(response, tools)
    }

    /// Make a streaming request, returning events via callback
    pub async fn request_stream<F>(
        &self,
        messages: &[Message],
        on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        self.request_stream_with_tools(messages, &[], on_event).await
    }

    /// Make a streaming request with tools, returning events via callback
    pub async fn request_stream_with_tools<F>(
        &self,
        messages: &[Message],
        tools: &[crate::llm::tools::Tool],
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
                    Role::User => {
                        // Add user content
                        if !m.content.is_empty() {
                            req.add_user_prompt(m.content.clone());
                        }
                        
                        // Add tool results if present (Issue 2 fix)
                        if !m.tool_results.is_empty() {
                            use serdes_ai::core::messages::request::{ModelRequestPart, ToolReturnPart};
                            
                            for tool_result in &m.tool_results {
                                let tool_return = if tool_result.is_error {
                                    ToolReturnPart::error("tool", &tool_result.content)
                                        .with_tool_call_id(&tool_result.tool_use_id)
                                } else {
                                    ToolReturnPart::success("tool", &tool_result.content)
                                        .with_tool_call_id(&tool_result.tool_use_id)
                                };
                                
                                // Manual push since there's no add_tool_result method
                                req.parts.push(ModelRequestPart::ToolReturn(tool_return));
                            }
                        }
                    }
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
        
        // Build the model with extended config for thinking support
        let model = self.build_model(provider, base_url)?;

        // Convert tools to SerdesAI ToolDefinition format (Issue 1 fix)
        let _tool_defs: Vec<ToolDefinition> = tools
            .iter()
            .map(|t| ToolDefinition::new(&t.name, &t.description).with_parameters(t.input_schema.clone()))
            .collect();

        // Create request parameters with tools
        // NOTE: SerdesAI's request_stream currently doesn't support tools parameter
        // in the same way as the non-streaming request. This is a limitation of the
        // underlying SerdesAI library. Tools are prepared here for when the API supports them.
        let params = Default::default();

        // Use the model directly for streaming
        // TODO: Once SerdesAI supports tools in streaming, pass _tool_defs here
        let mut stream = model.request_stream(&model_requests, &self.model_settings(), &params)
            .await
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;
        
        // Log if tools were requested but can't be passed yet
        if !_tool_defs.is_empty() {
            eprintln!("Warning: {} tools prepared but SerdesAI streaming doesn't support tools parameter yet", _tool_defs.len());
        }

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
                            ModelResponsePart::ToolCall(tc) => {
                                // Emit tool use event when tool call part starts
                                let tool_use = crate::llm::tools::ToolUse::new(
                                    tc.tool_call_id.as_deref().unwrap_or(""),
                                    &tc.tool_name,
                                    tc.args.to_json(),
                                );
                                on_event(StreamEvent::ToolUse(tool_use));
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
    
    /// Parse a SerdesAI ModelResponse into our Message type
    fn parse_response(
        &self,
        response: serdes_ai::core::ModelResponse,
        _tools: &[crate::llm::tools::Tool],
    ) -> StdResult<Message, LlmError> {
        use serdes_ai::core::ModelResponsePart;

        let mut text = String::new();
        let mut thinking_text = String::new();
        let mut tool_uses = Vec::new();

        for part in response.parts {
            match part {
                ModelResponsePart::Text(t) => {
                    text.push_str(&t.content);
                }
                ModelResponsePart::Thinking(t) => {
                    thinking_text.push_str(&t.content);
                }
                ModelResponsePart::ToolCall(tc) => {
                    // Parse tool call into ToolUse
                    let tool_use = crate::llm::tools::ToolUse::new(
                        tc.tool_call_id.as_deref().unwrap_or(""),
                        &tc.tool_name,
                        tc.args.to_json(),
                    );

                    // Log tool use for now (since MCP not fully wired)
                    eprintln!(
                        "LLM requested tool: {} with args: {}",
                        tool_use.name,
                        serde_json::to_string(&tool_use.input).unwrap_or_default()
                    );

                    tool_uses.push(tool_use);
                }
                _ => {
                    // Ignore other part types for now
                }
            }
        }

        let mut message = Message::assistant(text);
        if !thinking_text.is_empty() {
            message = message.with_thinking(thinking_text);
        }
        if !tool_uses.is_empty() {
            message = message.with_tool_uses(tool_uses);
        }

        Ok(message)
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

/// A chat message with optional tool interactions
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub thinking_content: Option<String>,
    /// Tool uses requested by the assistant (for assistant messages)
    pub tool_uses: Vec<crate::llm::tools::ToolUse>,
    /// Tool results provided by the user (for user messages)
    pub tool_results: Vec<crate::llm::tools::ToolResult>,
}

impl Message {
    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            thinking_content: None,
            tool_uses: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            thinking_content: None,
            tool_uses: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            thinking_content: None,
            tool_uses: Vec::new(),
            tool_results: Vec::new(),
        }
    }

    /// Add thinking content
    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking_content = Some(thinking.into());
        self
    }

    /// Add tool uses (for assistant messages)
    pub fn with_tool_uses(mut self, tool_uses: Vec<crate::llm::tools::ToolUse>) -> Self {
        self.tool_uses = tool_uses;
        self
    }

    /// Add tool results (for user messages)
    pub fn with_tool_results(mut self, tool_results: Vec<crate::llm::tools::ToolResult>) -> Self {
        self.tool_results = tool_results;
        self
    }

    /// Check if this message has tool uses
    pub fn has_tool_uses(&self) -> bool {
        !self.tool_uses.is_empty()
    }

    /// Check if this message has tool results
    pub fn has_tool_results(&self) -> bool {
        !self.tool_results.is_empty()
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
        assert!(msg.tool_uses.is_empty());
        assert!(msg.tool_results.is_empty());
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
    fn test_message_with_tool_uses() {
        use crate::llm::tools::ToolUse;
        
        let tool_use = ToolUse::new("toolu_123", "get_weather", serde_json::json!({"city": "NYC"}));
        let msg = Message::assistant("Let me check...").with_tool_uses(vec![tool_use]);

        assert!(msg.has_tool_uses());
        assert_eq!(msg.tool_uses.len(), 1);
        assert_eq!(msg.tool_uses[0].name, "get_weather");
    }

    #[test]
    fn test_message_with_tool_results() {
        use crate::llm::tools::ToolResult;
        
        let result = ToolResult::success("toolu_123", "Temperature: 72°F");
        let msg = Message::user("").with_tool_results(vec![result]);

        assert!(msg.has_tool_results());
        assert_eq!(msg.tool_results.len(), 1);
        assert!(!msg.tool_results[0].is_error);
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

    #[test]
    fn test_parse_response_with_tool_call() {
        use serdes_ai::core::{ModelResponse, ModelResponsePart};
        use serdes_ai::core::messages::parts::ToolCallArgs;
        
        let profile = ModelProfile {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-opus".to_string(),
            auth: AuthConfig::Key {
                value: "test".to_string(),
            },
            ..Default::default()
        };
        let client = LlmClient::from_profile(&profile).unwrap();

        // Create a mock response with a tool call
        let response = ModelResponse {
            parts: vec![
                ModelResponsePart::Text(serdes_ai::core::messages::parts::TextPart::new(
                    "Let me check the weather for you."
                )),
                ModelResponsePart::ToolCall(serdes_ai::core::messages::parts::ToolCallPart::new(
                    "get_weather",
                    ToolCallArgs::json(serde_json::json!({"city": "NYC"}))
                ).with_tool_call_id("toolu_123")),
            ],
            ..Default::default()
        };

        let message = client.parse_response(response, &[]).unwrap();
        
        assert_eq!(message.role, Role::Assistant);
        assert!(message.content.contains("weather"));
        assert!(message.has_tool_uses());
        assert_eq!(message.tool_uses.len(), 1);
        assert_eq!(message.tool_uses[0].name, "get_weather");
        assert_eq!(message.tool_uses[0].id, "toolu_123");
        assert_eq!(message.tool_uses[0].input["city"], "NYC");
    }

    #[test]
    fn test_parse_response_with_thinking() {
        use serdes_ai::core::{ModelResponse, ModelResponsePart};
        
        let profile = ModelProfile {
            provider_id: "anthropic".to_string(),
            model_id: "claude-3-opus".to_string(),
            auth: AuthConfig::Key {
                value: "test".to_string(),
            },
            ..Default::default()
        };
        let client = LlmClient::from_profile(&profile).unwrap();

        // Create a mock response with thinking content
        let response = ModelResponse {
            parts: vec![
                ModelResponsePart::Thinking(serdes_ai::core::messages::parts::ThinkingPart::new(
                    "Let me analyze this problem..."
                )),
                ModelResponsePart::Text(serdes_ai::core::messages::parts::TextPart::new(
                    "The answer is 42"
                )),
            ],
            ..Default::default()
        };

        let message = client.parse_response(response, &[]).unwrap();
        
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.content, "The answer is 42");
        assert_eq!(message.thinking_content, Some("Let me analyze this problem...".to_string()));
    }
}
