//! A `Model` wrapper that normalizes SSE stream formatting.
//!
//! Some providers (notably Kimi) send SSE chunks as `data:{json}` without the
//! space after `data:` that serdes-ai's `OpenAIStreamParser` expects.
//! This wrapper intercepts `request_stream`, normalizes the byte stream,
//! and re-parses using the standard `OpenAIStreamParser`.
//!
//! Non-streaming `request()` is delegated to the inner model unchanged.

use super::error::LlmError;
use super::sse_normalize::NormalizeSseStream;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use serdes_ai::core::messages::request::ModelRequestPart;
use serdes_ai::core::messages::ModelResponsePart;
use serdes_ai::core::{ModelRequest, ModelResponse, ModelSettings};
use serdes_ai_core::messages::content::UserContent;
use serdes_ai_core::UserContentPart;
use serdes_ai_models::error::ModelError;
use serdes_ai_models::model::{Model, ModelRequestParameters, StreamedResponse};
use serdes_ai_models::openai::stream::OpenAIStreamParser;
use serdes_ai_models::openai::types::{
    ChatCompletionRequest, ChatMessage, ChatTool, FunctionCall, MessageContent, StreamOptions,
    ToolCall, ToolChoiceValue,
};
use serdes_ai_models::profile::ModelProfile;
use serdes_ai_models::ToolChoice;
use std::sync::Arc;
use std::time::Duration;

/// Default idle-read window for streaming requests: a stream fails only when no
/// bytes arrive for this long, no matter how long the total generation takes.
///
/// reqwest's client-level `read_timeout` covers the connect + response-header
/// phase and then resets after every successful body read, which is exactly the
/// idle semantics an SSE stream needs. There must be no total deadline: thinking
/// models legitimately stream for many minutes (issue #213).
pub const DEFAULT_STREAM_IDLE_READ_TIMEOUT: Duration = Duration::from_mins(5);

/// Configuration for constructing a [`NormalizingSseModel`].
pub struct NormalizingSseModelConfig {
    pub inner: Arc<dyn Model>,
    /// Default headers (e.g. provider-quirk `User-Agent`) applied to every
    /// streaming request. The wrapper builds its own client from these.
    pub default_headers: reqwest::header::HeaderMap,
    /// Idle-read window for the streaming client.
    pub idle_read_timeout: Duration,
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
    /// Whether the model supports thinking/reasoning (affects token field used).
    pub enable_thinking: bool,
    /// Optional thinking budget.
    pub thinking_budget: Option<u64>,
    /// Optional request field-name override for the token limit.
    pub max_tokens_field_name: Option<String>,
    /// Optional provider-specific JSON fields to merge into the outgoing request.
    pub extra_request_fields: Option<serde_json::Value>,
}

/// Model wrapper that normalizes non-standard SSE formatting in streaming
/// responses. Delegates non-streaming requests to the inner model.
pub struct NormalizingSseModel {
    inner: Arc<dyn Model>,
    client: Client,
    api_key: String,
    base_url: String,
    model_name: String,
    enable_thinking: bool,
    thinking_budget: Option<u64>,
    max_tokens_field_name: Option<String>,
    extra_request_fields: Option<serde_json::Value>,
}

impl NormalizingSseModel {
    pub fn new(config: NormalizingSseModelConfig) -> Result<Self, LlmError> {
        let client = Client::builder()
            .default_headers(config.default_headers)
            .read_timeout(config.idle_read_timeout)
            .build()
            .map_err(|e| {
                LlmError::InvalidConfig(format!("failed to build streaming HTTP client: {e}"))
            })?;
        Ok(Self {
            inner: config.inner,
            client,
            api_key: config.api_key,
            base_url: config.base_url,
            model_name: config.model_name,
            enable_thinking: config.enable_thinking,
            thinking_budget: config.thinking_budget,
            max_tokens_field_name: config.max_tokens_field_name,
            extra_request_fields: config.extra_request_fields,
        })
    }
}

#[async_trait]
impl Model for NormalizingSseModel {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn system(&self) -> &str {
        self.inner.system()
    }

    fn profile(&self) -> &ModelProfile {
        self.inner.profile()
    }

    async fn request(
        &self,
        messages: &[ModelRequest],
        settings: &ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<ModelResponse, ModelError> {
        self.inner.request(messages, settings, params).await
    }

    async fn request_stream(
        &self,
        messages: &[ModelRequest],
        settings: &ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<StreamedResponse, ModelError> {
        let body = build_chat_request_payload(
            &self.model_name,
            messages,
            settings,
            params,
            self.enable_thinking,
            self.thinking_budget,
            self.max_tokens_field_name.as_deref(),
            self.extra_request_fields.as_ref(),
        )?;

        // No per-request total timeout: reqwest's per-request `.timeout()` is a
        // whole-body deadline that kills long SSE streams mid-generation. Idle
        // enforcement comes from the client-level `read_timeout` set in `new`.
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(ModelError::from)?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(ModelError::http(status, body_text));
        }

        let byte_stream = response.bytes_stream();
        let normalized = NormalizeSseStream::new(byte_stream);
        let parser = OpenAIStreamParser::new(normalized);

        Ok(Box::pin(parser))
    }
}

impl std::fmt::Debug for NormalizingSseModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NormalizingSseModel")
            .field("name", &self.inner.name())
            .field("base_url", &self.base_url)
            .field("model_name", &self.model_name)
            .field("enable_thinking", &self.enable_thinking)
            .field("thinking_budget", &self.thinking_budget)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Request-body construction
// ---------------------------------------------------------------------------

/// Reserved request field names that must not be overwritten by extra fields.
const RESERVED_REQUEST_KEYS: [&str; 14] = [
    "model",
    "messages",
    "stream",
    "stream_options",
    "temperature",
    "top_p",
    "presence_penalty",
    "frequency_penalty",
    "seed",
    "stop",
    "tools",
    "tool_choice",
    "max_tokens",
    "max_completion_tokens",
];

/// Build a streaming chat request payload.
#[allow(clippy::too_many_arguments)]
fn build_chat_request_payload(
    model_name: &str,
    messages: &[ModelRequest],
    settings: &ModelSettings,
    params: &ModelRequestParameters,
    enable_thinking: bool,
    thinking_budget: Option<u64>,
    max_tokens_field_name: Option<&str>,
    extra_request_fields: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ModelError> {
    let chat_messages: Vec<OutboundChatMessage> =
        messages.iter().flat_map(convert_request).collect();

    let tools = if params.tools.is_empty() {
        None
    } else {
        Some(convert_tools(&params.tools))
    };
    let tool_choice = params.tool_choice.as_ref().map(convert_tool_choice);
    let token_limit = if enable_thinking {
        thinking_budget.or(settings.max_tokens)
    } else {
        settings.max_tokens
    };

    let request = build_chat_request_struct(model_name, settings, tools, tool_choice);
    let mut request_value = serde_json::to_value(request)?;
    if !request_value.is_object() {
        return Err(ModelError::from(serde_json::Error::io(
            std::io::Error::other(format!(
                "ChatCompletionRequest must serialize to a JSON object, got: {request_value}"
            )),
        )));
    }

    let encoded_messages = serde_json::to_value(chat_messages)?;
    let request_object = request_value
        .as_object_mut()
        .expect("request_value object checked above");
    request_object.insert("messages".to_string(), encoded_messages);

    let token_field = resolve_token_field(max_tokens_field_name);
    apply_token_limit(request_object, token_limit, &token_field);
    merge_extra_fields(request_object, extra_request_fields, &token_field);

    Ok(request_value)
}

/// Build the `ChatCompletionRequest` struct with standard streaming settings.
fn build_chat_request_struct(
    model_name: &str,
    settings: &ModelSettings,
    tools: Option<Vec<ChatTool>>,
    tool_choice: Option<ToolChoiceValue>,
) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: model_name.to_string(),
        messages: Vec::new(),
        temperature: settings.temperature,
        top_p: settings.top_p,
        max_tokens: None,
        max_completion_tokens: None,
        stop: settings.stop.clone(),
        presence_penalty: settings.presence_penalty,
        frequency_penalty: settings.frequency_penalty,
        seed: settings.seed,
        tools,
        tool_choice,
        parallel_tool_calls: settings.parallel_tool_calls,
        response_format: None,
        user: None,
        stream: Some(true),
        stream_options: Some(StreamOptions {
            include_usage: true,
        }),
        logprobs: None,
        top_logprobs: None,
    }
}

/// Reserved keys that must not be overwritten by token field name override.
const RESERVED_TOKEN_FIELD_NAMES: &[&str] = &[
    "model",
    "messages",
    "stream",
    "stream_options",
    "tools",
    "tool_choice",
    "temperature",
    "top_p",
    "presence_penalty",
    "frequency_penalty",
    "seed",
    "stop",
];

/// Resolve the token field from the user override, falling back to `max_tokens`.
fn resolve_token_field(max_tokens_field_name: Option<&str>) -> String {
    max_tokens_field_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| !RESERVED_TOKEN_FIELD_NAMES.contains(name))
        .map_or_else(|| "max_tokens".to_string(), str::to_string)
}

/// Apply token limit to the request object using the resolved field name.
fn apply_token_limit(
    request_object: &mut serde_json::Map<String, serde_json::Value>,
    token_limit: Option<u64>,
    token_field_name: &str,
) {
    request_object.remove("max_tokens");
    request_object.remove("max_completion_tokens");
    if let Some(limit) = token_limit {
        request_object.insert(token_field_name.to_string(), serde_json::Value::from(limit));
    }
}

/// Merge extra request fields, skipping reserved keys and the token field.
fn merge_extra_fields(
    request_object: &mut serde_json::Map<String, serde_json::Value>,
    extra_request_fields: Option<&serde_json::Value>,
    token_field_name: &str,
) {
    if let Some(serde_json::Value::Object(extra_fields)) = extra_request_fields {
        for (key, value) in extra_fields {
            if !RESERVED_REQUEST_KEYS.contains(&key.as_str()) && key != token_field_name {
                request_object.insert(key.clone(), value.clone());
            }
        }
    }
}

fn convert_request(req: &ModelRequest) -> Vec<OutboundChatMessage> {
    req.parts.iter().map(convert_request_part).collect()
}

fn convert_request_part(part: &ModelRequestPart) -> OutboundChatMessage {
    match part {
        ModelRequestPart::SystemPrompt(sys) => {
            OutboundChatMessage::from_chat_message(ChatMessage {
                role: "system".to_string(),
                content: Some(MessageContent::Text(sys.content.clone())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            })
        }
        ModelRequestPart::UserPrompt(user) => OutboundChatMessage::from_chat_message(ChatMessage {
            role: "user".to_string(),
            content: Some(convert_user_content(&user.content)),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }),
        ModelRequestPart::ToolReturn(tool_ret) => {
            OutboundChatMessage::from_chat_message(ChatMessage {
                role: "tool".to_string(),
                content: Some(MessageContent::Text(tool_ret.content.to_string_content())),
                name: None,
                tool_calls: None,
                tool_call_id: tool_ret.tool_call_id.clone(),
            })
        }
        ModelRequestPart::RetryPrompt(retry) => {
            OutboundChatMessage::from_chat_message(ChatMessage {
                role: "user".to_string(),
                content: Some(MessageContent::Text(retry.content.message().to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            })
        }
        ModelRequestPart::BuiltinToolReturn(builtin) => {
            let content = serde_json::to_string(&builtin.content)
                .unwrap_or_else(|_| builtin.content_type().to_string());
            OutboundChatMessage::from_chat_message(ChatMessage::tool(
                content,
                builtin.tool_call_id.clone(),
            ))
        }
        ModelRequestPart::ModelResponse(response) => convert_model_response(response),
    }
}

fn convert_user_content(content: &UserContent) -> MessageContent {
    match content {
        UserContent::Text(text) => MessageContent::Text(text.clone()),
        UserContent::Parts(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| match part {
                    UserContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("");
            MessageContent::Text(text)
        }
    }
}

fn convert_model_response(response: &ModelResponse) -> OutboundChatMessage {
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    let mut reasoning_parts = Vec::new();

    for part in &response.parts {
        match part {
            ModelResponsePart::Text(text) => text_parts.push(text.content.clone()),
            ModelResponsePart::Thinking(thinking) => reasoning_parts.push(thinking.content.clone()),
            ModelResponsePart::ToolCall(tool_call) => {
                tool_calls.push(ToolCall {
                    id: tool_call.tool_call_id.clone().unwrap_or_default(),
                    tool_type: "function".to_string(),
                    function: FunctionCall {
                        name: tool_call.tool_name.clone(),
                        arguments: tool_call.args.to_json_string().unwrap_or_default(),
                    },
                });
            }
            _ => {}
        }
    }

    let reasoning_content = {
        let merged = reasoning_parts.join("");
        (!merged.is_empty()).then_some(merged)
    };

    OutboundChatMessage {
        role: "assistant".to_string(),
        content: Some(MessageContent::Text(text_parts.join(""))),
        name: None,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        tool_call_id: None,
        reasoning_content,
    }
}

#[derive(Debug, Clone, Serialize)]
struct OutboundChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
}

impl OutboundChatMessage {
    fn from_chat_message(message: ChatMessage) -> Self {
        Self {
            role: message.role,
            content: message.content,
            name: message.name,
            tool_calls: message.tool_calls,
            tool_call_id: message.tool_call_id,
            reasoning_content: None,
        }
    }
}

fn convert_tools(tools: &[serdes_ai::ToolDefinition]) -> Vec<ChatTool> {
    tools
        .iter()
        .map(|t| {
            let params =
                serde_json::to_value(&t.parameters_json_schema).unwrap_or(serde_json::json!({}));
            ChatTool::function(&t.name, &t.description, params)
        })
        .collect()
}

fn convert_tool_choice(choice: &ToolChoice) -> serdes_ai_models::openai::types::ToolChoiceValue {
    use serdes_ai_models::openai::types::ToolChoiceValue;
    match choice {
        ToolChoice::Auto => ToolChoiceValue::auto(),
        ToolChoice::Required => ToolChoiceValue::required(),
        ToolChoice::None => ToolChoiceValue::none(),
        ToolChoice::Specific(name) => ToolChoiceValue::function(name),
    }
}

#[cfg(test)]
mod tests;
