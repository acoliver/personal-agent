//! LLM client for making requests to AI providers
//!
//! This module bridges `PersonalAgent`'s profile system with `SerdesAI`,
//! using models.dev registry data for provider configuration.

use super::error::{debug_error_message, LlmError};
use super::provider_quirks::{effective_serdes_provider, resolve_provider_quirks, ProviderQuirks};
use crate::models::{AuthConfig, ModelProfile};
use crate::registry::RegistryCache;
use futures::StreamExt;
use reqwest::Client as HttpClient;
use serdes_ai::core::messages::ModelResponseStreamEvent;
use serdes_ai::models::ModelRequestParameters;
use serdes_ai::prelude::*;
use serdes_ai::ExtendedModelConfig;
use std::collections::HashMap;
use std::time::Duration;

// Use std Result to avoid conflict with serdes_ai::prelude::Result
type StdResult<T, E> = std::result::Result<T, E>;

/// Events emitted during streaming
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text content delta
    TextDelta(String),
    /// Thinking content delta (for reasoning models)
    ThinkingDelta(String),
    /// Tool use requested by the model
    ToolUse(crate::llm::tools::ToolUse),
    /// Tool call started (Agent mode)
    ToolCallStarted {
        /// Tool name being called
        tool_name: String,
        /// Unique call ID
        call_id: String,
    },
    /// Tool call completed (Agent mode)
    ToolCallCompleted {
        /// Tool name that was called
        tool_name: String,
        /// Unique call ID
        call_id: String,
        /// Whether the tool execution succeeded
        success: bool,
        /// Tool result (if successful)
        result: Option<String>,
        /// Error message (if failed)
        error: Option<String>,
    },
    /// Finalized tool transcript extracted from the completed turn.
    ToolTranscript {
        /// All tool calls emitted by the assistant during the turn.
        tool_calls: Vec<crate::llm::tools::ToolUse>,
        /// All tool results returned during the turn.
        tool_results: Vec<crate::llm::tools::ToolResult>,
    },
    /// Stream completed
    Complete {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
    },
    /// Error occurred
    Error(String),
}

/// Sampling and length controls for one turn.
///
/// All `None` means the endpoint refuses them and the request must go out
/// without them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplingSettings {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<u32>,
}

impl SamplingSettings {
    /// Carry nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            temperature: None,
            top_p: None,
            max_tokens: None,
        }
    }
}

/// Token usage accumulated from a provider's terminal stream event.
///
/// Providers that confirm completion on the wire report usage there; the rest
/// leave both fields `None`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StreamUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// LLM client that uses `SerdesAI`
#[derive(Clone)]
pub struct LlmClient {
    pub(crate) profile: ModelProfile,
    pub(crate) api_key: String,
    /// Base URL from models.dev registry (if available)
    pub(crate) registry_base_url: Option<String>,
    pub(crate) quirks: ProviderQuirks,
    /// Conversation this client is serving, when it is serving one.
    ///
    /// Session-stateful transports keep the wire conversation alive between
    /// turns and key it on this. Absent means "build a throwaway model",
    /// which is what connection tests and one-shot calls want.
    pub(crate) conversation_id: Option<uuid::Uuid>,
}

impl LlmClient {
    /// Create a new LLM client from a model profile
    ///
    /// This looks up the provider in the models.dev registry to get
    /// the correct API base URL and configuration.
    ///
    /// # Errors
    ///
    /// Returns `LlmError` when the API key cannot be resolved.
    pub fn from_profile(profile: &ModelProfile) -> StdResult<Self, LlmError> {
        let api_key = Self::resolve_api_key(profile)?;

        // Look up provider info from models.dev registry
        let registry_base_url = Self::get_registry_base_url(&profile.provider_id);

        Ok(Self {
            profile: profile.clone(),
            api_key,
            registry_base_url,
            quirks: resolve_provider_quirks(profile),
            conversation_id: None,
        })
    }

    /// Bind this client to a conversation.
    ///
    /// Session-stateful transports keep one wire conversation per chat, so
    /// callers that have a conversation should say so; otherwise every turn
    /// opens a fresh session and replays the whole history.
    #[must_use]
    pub const fn for_conversation(mut self, conversation_id: uuid::Uuid) -> Self {
        self.conversation_id = Some(conversation_id);
        self
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

    /// Resolve the API key from profile auth config (OS keychain lookup).
    fn resolve_api_key(profile: &ModelProfile) -> StdResult<String, LlmError> {
        // Non-interactive E2E override for CI and local ignored-test runs.
        if let Ok(api_key_override) = std::env::var("PA_E2E_API_KEY") {
            let trimmed = api_key_override.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        match &profile.auth {
            AuthConfig::None => {
                // Local models don't require API keys
                Ok(String::new())
            }
            AuthConfig::Keychain { label } => {
                let trimmed = label.trim();
                if trimmed.is_empty() {
                    return Err(LlmError::NoApiKey);
                }
                let key = crate::services::secure_store::api_keys::get(trimmed)
                    .map_err(|e| LlmError::Auth(format!("Keychain lookup failed: {e}")))?
                    .ok_or(LlmError::NoApiKey)?;
                if key.trim().is_empty() {
                    return Err(LlmError::NoApiKey);
                }
                Ok(key.trim().to_string())
            }
            // OAuth profiles carry a bearer token, not an API key. The token
            // is resolved per request in the open-responses transport, which
            // is the only transport that accepts an OAuth profile.
            AuthConfig::OAuth { .. } => Ok(String::new()),
        }
    }

    /// Get the model spec string for `SerdesAI` (e.g., "openai:gpt-4o")
    #[must_use]
    #[allow(dead_code)]
    pub fn model_spec(&self) -> String {
        // Use get_serdes_provider to handle OpenAI-compatible providers
        let provider = self.get_serdes_provider();
        format!("{provider}:{}", self.profile.model_id)
    }

    /// Build model settings from profile parameters
    /// The sampling and length controls a turn should carry, if any.
    ///
    /// There are two ways to start a turn, the direct client and the agent,
    /// and they build their requests separately. Both ask here, because
    /// answering this question twice is how a fixed bug came back: the direct
    /// path stopped sending `temperature` while the agent path kept doing it,
    /// and the agent path is the one the app uses.
    #[must_use]
    pub fn sampling(&self) -> SamplingSettings {
        // The codex backend rejects these outright, answering
        // `Unsupported parameter: temperature` and then the same for
        // `max_output_tokens`, so the turn never starts. Reasoning models take
        // a reasoning effort instead, which the transport sets from the
        // profile's thinking settings.
        if self.uses_open_responses() {
            return SamplingSettings::none();
        }

        SamplingSettings {
            temperature: Some(self.profile.parameters.temperature),
            top_p: Some(self.profile.parameters.top_p),
            max_tokens: self.profile.parameters.max_tokens,
        }
    }

    fn model_settings(&self) -> ModelSettings {
        let sampling = self.sampling();
        ModelSettings {
            temperature: sampling.temperature,
            top_p: sampling.top_p,
            max_tokens: sampling.max_tokens.map(u64::from),
            ..ModelSettings::default()
        }
    }

    /// Whether this profile talks the Responses protocol.
    pub(crate) fn uses_open_responses(&self) -> bool {
        self.quirks.serdes_provider.as_deref() == Some(crate::llm::open_responses::TRANSPORT)
    }

    fn build_model_requests(messages: &[Message]) -> Vec<ModelRequest> {
        messages
            .iter()
            .map(|m| {
                let mut req = ModelRequest::new();
                match m.role {
                    Role::User => {
                        if !m.content.is_empty() {
                            req.add_user_prompt(m.content.clone());
                        }

                        if !m.tool_results.is_empty() {
                            use serdes_ai::core::messages::request::{
                                ModelRequestPart, ToolReturnPart,
                            };

                            for tool_result in &m.tool_results {
                                let tool_return = if tool_result.is_error {
                                    ToolReturnPart::error("tool", &tool_result.content)
                                        .with_tool_call_id(&tool_result.tool_use_id)
                                } else {
                                    ToolReturnPart::success("tool", &tool_result.content)
                                        .with_tool_call_id(&tool_result.tool_use_id)
                                };

                                req.parts.push(ModelRequestPart::ToolReturn(tool_return));
                            }
                        }
                    }
                    Role::Assistant => {
                        // The assistant's own turn has to go back as an
                        // assistant turn. Flattening it into a user prompt
                        // breaks role alternation, and on session-stateful
                        // transports it makes the client resend a reply the
                        // server already holds.
                        use serdes_ai::core::messages::request::ModelRequestPart;
                        req.parts.push(ModelRequestPart::ModelResponse(Box::new(
                            Self::assistant_response(m),
                        )));
                    }
                    Role::System => req.add_system_prompt(m.content.clone()),
                }
                req
            })
            .collect()
    }

    /// Rebuild a stored assistant turn as a model response, including any tool
    /// calls it made, so the provider sees the turn it actually produced.
    fn assistant_response(message: &Message) -> ModelResponse {
        use serdes_ai::core::messages::{ModelResponsePart, TextPart, ToolCallPart};

        let mut response = ModelResponse::new();
        if !message.content.is_empty() {
            response
                .parts
                .push(ModelResponsePart::Text(TextPart::new(&message.content)));
        }
        for tool_use in &message.tool_uses {
            let mut call = ToolCallPart::new(&tool_use.name, tool_use.input.to_string());
            call.tool_call_id = Some(tool_use.id.clone());
            response.parts.push(ModelResponsePart::ToolCall(call));
        }
        response
    }

    pub(crate) fn base_url_override(&self) -> Option<&str> {
        if !self.profile.base_url.is_empty() {
            return Some(self.profile.base_url.as_str());
        }

        if let Some(url) = self.quirks.base_url_override.as_deref() {
            return Some(url);
        }

        self.registry_base_url.as_deref()
    }

    fn build_tool_definitions(tools: &[crate::llm::tools::Tool]) -> Vec<ToolDefinition> {
        tools
            .iter()
            .map(|t| {
                ToolDefinition::new(&t.name, &t.description).with_parameters(t.input_schema.clone())
            })
            .collect()
    }

    async fn build_model_and_params(
        &self,
        tools: &[crate::llm::tools::Tool],
    ) -> StdResult<(std::sync::Arc<dyn serdes_ai::Model>, ModelRequestParameters), LlmError> {
        let base_url = self.base_url_override();
        let provider = self.get_serdes_provider();
        let model = self.build_model(provider, base_url).await?;
        let tool_defs = Self::build_tool_definitions(tools);
        let params = ModelRequestParameters::new().with_tools(tool_defs);
        Ok((model, params))
    }

    fn parse_tool_call_args(args_str: &str) -> serde_json::Value {
        serde_json::from_str(args_str)
            .unwrap_or_else(|_| serde_json::json!({"_raw": args_str, "_error": "parse_failed"}))
    }

    fn emit_tool_use<F>(
        pending_tool_calls: &mut HashMap<usize, (String, String, String)>,
        index: usize,
        on_event: &mut F,
    ) where
        F: FnMut(StreamEvent) + Send,
    {
        if let Some((id, name, args_str)) = pending_tool_calls.remove(&index) {
            let args = Self::parse_tool_call_args(&args_str);
            let tool_use = crate::llm::tools::ToolUse::new(&id, &name, args);
            on_event(StreamEvent::ToolUse(tool_use));
        }
    }

    fn handle_stream_event<F>(
        event: ModelResponseStreamEvent,
        pending_tool_calls: &mut HashMap<usize, (String, String, String)>,
        usage: &mut StreamUsage,
        on_event: &mut F,
    ) where
        F: FnMut(StreamEvent) + Send,
    {
        match event {
            ModelResponseStreamEvent::PartDelta(delta) => {
                use serdes_ai::core::messages::ModelResponsePartDelta;
                match &delta.delta {
                    ModelResponsePartDelta::Text(t) => {
                        on_event(StreamEvent::TextDelta(t.content_delta.clone()));
                    }
                    ModelResponsePartDelta::Thinking(t) => {
                        on_event(StreamEvent::ThinkingDelta(t.content_delta.clone()));
                    }
                    ModelResponsePartDelta::ToolCall(tc_delta) => {
                        if let Some((_, _, ref mut args_str)) =
                            pending_tool_calls.get_mut(&delta.index)
                        {
                            args_str.push_str(&tc_delta.args_delta);
                        }
                    }
                    ModelResponsePartDelta::BuiltinToolCall(_) => {}
                }
            }
            ModelResponseStreamEvent::PartStart(start) => {
                use serdes_ai::core::ModelResponsePart;
                match &start.part {
                    ModelResponsePart::Text(t) if !t.content.is_empty() => {
                        on_event(StreamEvent::TextDelta(t.content.clone()));
                    }
                    ModelResponsePart::Thinking(t) if !t.content.is_empty() => {
                        on_event(StreamEvent::ThinkingDelta(t.content.clone()));
                    }
                    ModelResponsePart::ToolCall(tc) => {
                        let id = tc.tool_call_id.as_deref().unwrap_or("").to_string();
                        let name = tc.tool_name.clone();
                        pending_tool_calls.insert(start.index, (id, name, String::new()));
                    }
                    _ => {}
                }
            }
            ModelResponseStreamEvent::PartEnd(end) => {
                Self::emit_tool_use(pending_tool_calls, end.index, on_event);
            }
            ModelResponseStreamEvent::StreamComplete(complete) => {
                // Providers that confirm completion on the wire (Anthropic
                // `message_stop`, the Responses `response.completed` frame)
                // report usage here. Record it; the single terminal
                // `Complete` is still emitted once the stream is drained, so
                // providers that never send this event behave as before.
                usage.input_tokens = complete.input_tokens.and_then(|v| u32::try_from(v).ok());
                usage.output_tokens = complete.output_tokens.and_then(|v| u32::try_from(v).ok());
            }
        }
    }

    /// Build a model with extended configuration (thinking support, etc.)
    pub(crate) async fn build_model(
        &self,
        provider: &str,
        base_url: Option<&str>,
    ) -> StdResult<std::sync::Arc<dyn serdes_ai::Model>, LlmError> {
        if provider == super::open_responses::TRANSPORT {
            return self.build_open_responses_model(base_url).await;
        }

        if self.profile.auth.requires_oauth_account() {
            return Err(LlmError::Auth(format!(
                "OAuth profiles need the Responses transport; provider '{}' speaks API keys",
                self.profile.provider_id
            )));
        }

        if provider == "openai" {
            return self.build_openai_model_with_quirks(base_url);
        }

        let mut config = ExtendedModelConfig::new()
            .with_api_key(&self.api_key)
            .with_timeout(Self::request_timeout());

        if let Some(url) = base_url {
            config = config.with_base_url(url);
        }

        // Enable thinking if profile has it enabled
        if self.profile.parameters.enable_thinking {
            let budget = self.profile.parameters.thinking_budget.map(u64::from);
            config = config.with_thinking(budget);
        }

        let inner = serdes_ai::build_model_extended(provider, &self.profile.model_id, config)
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Wrap with normalizer to apply max_tokens_field_name and extra_request_fields
        // for all providers, not just OpenAI. This ensures consistent behavior.
        let resolved_base_url = base_url.unwrap_or("").to_string();
        let http_client = HttpClient::builder()
            .build()
            .map_err(|e| LlmError::InvalidConfig(format!("failed to build HTTP client: {e}")))?;

        let wrapper = super::normalizing_model::NormalizingSseModel::new(
            super::normalizing_model::NormalizingSseModelConfig {
                inner,
                client: http_client,
                api_key: self.api_key.clone(),
                base_url: resolved_base_url,
                model_name: self.profile.model_id.clone(),
                enable_thinking: self.profile.parameters.enable_thinking,
                thinking_budget: self.profile.parameters.thinking_budget.map(u64::from),
                max_tokens_field_name: self.profile.parameters.max_tokens_field_name.clone(),
                extra_request_fields: self.profile.parameters.extra_request_fields.clone(),
            },
        );

        Ok(std::sync::Arc::new(wrapper))
    }

    /// Build (or reuse) an `OpenAI` Responses model.
    ///
    /// Deliberately not wrapped in `NormalizingSseModel`: that wrapper repairs
    /// Chat Completions SSE, and this client already emits well-formed stream
    /// events.
    async fn build_open_responses_model(
        &self,
        base_url: Option<&str>,
    ) -> StdResult<std::sync::Arc<dyn serdes_ai::Model>, LlmError> {
        let endpoint = base_url.unwrap_or(self.profile.base_url.as_str());
        super::open_responses::model_for(super::open_responses::SessionRequest {
            profile: &self.profile,
            quirks: &self.quirks,
            endpoint,
            conversation_id: self.conversation_id,
            api_key: &self.api_key,
        })
        .await
    }

    fn build_openai_model_with_quirks(
        &self,
        base_url: Option<&str>,
    ) -> StdResult<std::sync::Arc<dyn serdes_ai::Model>, LlmError> {
        let mut client_builder = HttpClient::builder();

        if let Some(headers) = self.quirks_header_map()? {
            client_builder = client_builder.default_headers(headers);
        }

        let http_client = client_builder
            .build()
            .map_err(|e| LlmError::InvalidConfig(format!("failed to build HTTP client: {e}")))?;

        let mut config = ExtendedModelConfig::new()
            .with_api_key(&self.api_key)
            .with_client(http_client.clone())
            .with_timeout(Self::request_timeout());

        let resolved_base_url = base_url.unwrap_or("https://api.openai.com/v1").to_string();
        config = config.with_base_url(&resolved_base_url);

        if self.profile.parameters.enable_thinking {
            let budget = self.profile.parameters.thinking_budget.map(u64::from);
            config = config.with_thinking(budget);
        }

        let inner = serdes_ai::build_model_extended("openai", &self.profile.model_id, config)
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Wrap with SSE normalizer — some providers (e.g. Kimi) send `data:{json}`
        // without the space that serdes-ai's parser requires.
        let wrapper = super::normalizing_model::NormalizingSseModel::new(
            super::normalizing_model::NormalizingSseModelConfig {
                inner,
                client: http_client,
                api_key: self.api_key.clone(),
                base_url: resolved_base_url,
                model_name: self.profile.model_id.clone(),
                enable_thinking: self.profile.parameters.enable_thinking,
                thinking_budget: self.profile.parameters.thinking_budget.map(u64::from),
                max_tokens_field_name: self.profile.parameters.max_tokens_field_name.clone(),
                extra_request_fields: self.profile.parameters.extra_request_fields.clone(),
            },
        );

        Ok(std::sync::Arc::new(wrapper))
    }

    const fn request_timeout() -> Duration {
        Duration::from_mins(2)
    }

    fn quirks_header_map(&self) -> StdResult<Option<reqwest::header::HeaderMap>, LlmError> {
        if !self.quirks.has_custom_headers() {
            return Ok(None);
        }

        self.quirks
            .header_map()
            .map(Some)
            .map_err(|e| LlmError::InvalidConfig(format!("invalid provider quirk header: {e}")))
    }

    /// Make a non-streaming request
    ///
    /// # Errors
    ///
    /// Returns `LlmError` if the request fails.
    pub async fn request(&self, messages: &[Message]) -> StdResult<Message, LlmError> {
        self.request_with_tools(messages, &[]).await
    }

    /// Make a non-streaming request with tools
    ///
    /// # Errors
    ///
    /// Returns `LlmError` if the request fails.
    pub async fn request_with_tools(
        &self,
        messages: &[Message],
        tools: &[crate::llm::tools::Tool],
    ) -> StdResult<Message, LlmError> {
        self.set_api_key_env();

        let model_requests = Self::build_model_requests(messages);
        let (model, params) = self.build_model_and_params(tools).await?;

        // Make the request using the model directly
        let response = model
            .request(&model_requests, &self.model_settings(), &params)
            .await
            .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Parse response into Message
        Ok(Self::parse_response(response, tools))
    }

    /// Make a streaming request, returning events via callback
    ///
    /// # Errors
    ///
    /// Returns `LlmError` if the request fails.
    pub async fn request_stream<F>(
        &self,
        messages: &[Message],
        on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        self.request_stream_with_tools(messages, &[], on_event)
            .await
    }

    /// Make a streaming request with tools, returning events via callback
    ///
    /// # Errors
    ///
    /// Returns `LlmError` if the request fails.
    pub async fn request_stream_with_tools<F>(
        &self,
        messages: &[Message],
        tools: &[crate::llm::tools::Tool],
        mut on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        self.set_api_key_env();

        let model_requests = Self::build_model_requests(messages);
        let (model, params) = self.build_model_and_params(tools).await?;

        // Use the model directly for streaming
        let mut stream = match model
            .request_stream(&model_requests, &self.model_settings(), &params)
            .await
        {
            Ok(stream) => stream,
            Err(e) => {
                let err_msg = debug_error_message(&e);
                on_event(StreamEvent::Error(err_msg.clone()));
                return Err(LlmError::Stream(err_msg));
            }
        };

        let mut pending_tool_calls: HashMap<usize, (String, String, String)> = HashMap::new();
        let mut usage = StreamUsage::default();

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    Self::handle_stream_event(
                        event,
                        &mut pending_tool_calls,
                        &mut usage,
                        &mut on_event,
                    );
                }
                Err(e) => {
                    let err_msg = debug_error_message(&e);
                    on_event(StreamEvent::Error(err_msg.clone()));
                    return Err(LlmError::Stream(err_msg));
                }
            }
        }

        on_event(StreamEvent::Complete {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
        });
        Ok(())
    }

    /// Set the API key in the environment for the provider
    ///
    /// Uses the `env` field from models.dev registry to determine the correct
    /// environment variable name for the provider.
    pub(crate) fn set_api_key_env(&self) {
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

    /// Parse a `SerdesAI` `ModelResponse` into our Message type
    #[must_use]
    fn parse_response(
        response: serdes_ai::core::ModelResponse,
        _tools: &[crate::llm::tools::Tool],
    ) -> Message {
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
                    // Ignore other parts (tool returns, images, etc.)
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

        message
    }

    /// Determine the provider type for `SerdesAI`
    ///
    /// Uses models.dev registry `npm` field to detect OpenAI-compatible providers:
    /// - `@ai-sdk/openai-compatible` -> use "openai" provider with custom `base_url`
    /// - `@ai-sdk/openai` -> native openai
    /// - `@ai-sdk/anthropic` -> native anthropic
    pub(crate) fn get_serdes_provider(&self) -> &str {
        if let Ok(cache_path) = RegistryCache::default_path() {
            let cache = RegistryCache::new(cache_path, 24);
            if let Ok(Some(registry)) = cache.load() {
                return effective_serdes_provider(&self.profile, Some(&registry));
            }
        }

        effective_serdes_provider(&self.profile, None)
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
    #[must_use]
    pub fn with_thinking(mut self, thinking: impl Into<String>) -> Self {
        self.thinking_content = Some(thinking.into());
        self
    }

    /// Add tool uses (for assistant messages)
    #[must_use]
    pub fn with_tool_uses(mut self, tool_uses: Vec<crate::llm::tools::ToolUse>) -> Self {
        self.tool_uses = tool_uses;
        self
    }

    /// Add tool results (for user messages)
    #[must_use]
    pub fn with_tool_results(mut self, tool_results: Vec<crate::llm::tools::ToolResult>) -> Self {
        self.tool_results = tool_results;
        self
    }

    /// Check if this message has tool uses
    #[must_use]
    pub const fn has_tool_uses(&self) -> bool {
        !self.tool_uses.is_empty()
    }

    /// Check if this message has tool results
    #[must_use]
    pub const fn has_tool_results(&self) -> bool {
        !self.tool_results.is_empty()
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
