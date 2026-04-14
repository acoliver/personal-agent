//! A `Model` wrapper that normalizes SSE stream formatting.
//!
//! Some providers (notably Kimi) send SSE chunks as `data:{json}` without the
//! space after `data:` that serdes-ai's `OpenAIStreamParser` expects.
//! This wrapper intercepts `request_stream`, normalizes the byte stream,
//! and re-parses using the standard `OpenAIStreamParser`.
//!
//! Non-streaming `request()` is delegated to the inner model unchanged.

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
    ToolCall,
};
use serdes_ai_models::profile::ModelProfile;
use serdes_ai_models::ToolChoice;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for constructing a [`NormalizingSseModel`].
pub struct NormalizingSseModelConfig {
    pub inner: Arc<dyn Model>,
    /// HTTP client (carries custom headers like User-Agent via `default_headers`).
    pub client: Client,
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
    default_timeout: Duration,
    enable_thinking: bool,
    thinking_budget: Option<u64>,
    max_tokens_field_name: Option<String>,
    extra_request_fields: Option<serde_json::Value>,
}

impl NormalizingSseModel {
    pub fn new(config: NormalizingSseModelConfig) -> Self {
        Self {
            inner: config.inner,
            client: config.client,
            api_key: config.api_key,
            base_url: config.base_url,
            model_name: config.model_name,
            default_timeout: Duration::from_secs(120),
            enable_thinking: config.enable_thinking,
            thinking_budget: config.thinking_budget,
            max_tokens_field_name: config.max_tokens_field_name,
            extra_request_fields: config.extra_request_fields,
        }
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
        let timeout = settings.timeout.unwrap_or(self.default_timeout);

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .timeout(timeout)
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
            .field("default_timeout", &self.default_timeout)
            .field("enable_thinking", &self.enable_thinking)
            .field("thinking_budget", &self.thinking_budget)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Request-body construction
// ---------------------------------------------------------------------------

/// Build a streaming chat request payload.
///
/// When `enable_thinking` is set, `max_completion_tokens` is used instead of
/// `max_tokens` (the `OpenAI` reasoning API requirement).
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

    let request = ChatCompletionRequest {
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
    };

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

    let default_token_field_name = if enable_thinking {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };
    let token_field_name = max_tokens_field_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(default_token_field_name)
        .to_string();

    request_object.remove("max_tokens");
    request_object.remove("max_completion_tokens");
    if let Some(token_limit) = token_limit {
        request_object.insert(
            token_field_name.clone(),
            serde_json::Value::from(token_limit),
        );
    }

    if let Some(serde_json::Value::Object(extra_fields)) = extra_request_fields {
        let reserved_keys = [
            "model",
            "messages",
            "stream",
            "temperature",
            "top_p",
            "presence_penalty",
            "frequency_penalty",
            "seed",
            "stop",
            "tools",
            "tool_choice",
        ];
        for (key, value) in extra_fields {
            if !reserved_keys.contains(&key.as_str()) && key != &token_field_name {
                request_object.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(request_value)
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
mod tests {
    use super::*;
    use serdes_ai::core::messages::parts::{ThinkingPart, ToolCallArgs, ToolCallPart};
    use serdes_ai::core::messages::{ModelRequestPart, ModelResponse, ModelResponsePart};

    #[test]
    fn convert_request_includes_reasoning_content_for_assistant_history() {
        let mut response = ModelResponse::new();
        response.add_part(ModelResponsePart::Thinking(ThinkingPart::new("step one")));
        response.add_part(ModelResponsePart::Text(
            serdes_ai::core::messages::parts::TextPart::new("final"),
        ));
        response.add_part(ModelResponsePart::ToolCall(
            ToolCallPart::new(
                "read_file",
                ToolCallArgs::json(serde_json::json!({ "path": "a" })),
            )
            .with_tool_call_id("call_1"),
        ));

        let mut request = ModelRequest::new();
        request.add_part(ModelRequestPart::ModelResponse(Box::new(response)));

        let converted = convert_request(&request);
        assert_eq!(converted.len(), 1);

        let assistant = &converted[0];
        assert_eq!(assistant.role, "assistant");
        assert_eq!(assistant.reasoning_content.as_deref(), Some("step one"));
        assert!(assistant
            .tool_calls
            .as_ref()
            .is_some_and(|calls| calls.len() == 1));
    }

    #[test]
    fn build_chat_request_payload_serializes_reasoning_content_field() {
        let mut response = ModelResponse::new();
        response.add_part(ModelResponsePart::Thinking(ThinkingPart::new("chain")));
        response.add_part(ModelResponsePart::Text(
            serdes_ai::core::messages::parts::TextPart::new("answer"),
        ));

        let mut history_turn = ModelRequest::new();
        history_turn.add_part(ModelRequestPart::ModelResponse(Box::new(response)));

        let payload = build_chat_request_payload(
            "kimi-k2-0711-preview",
            &[history_turn],
            &ModelSettings::default(),
            &ModelRequestParameters::default(),
            true,
            Some(512),
            None,
            None,
        )
        .expect("payload should serialize");

        let messages = payload
            .get("messages")
            .and_then(serde_json::Value::as_array)
            .expect("messages array should be present");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0]
                .get("reasoning_content")
                .and_then(serde_json::Value::as_str),
            Some("chain")
        );
    }

    #[test]
    fn build_chat_request_payload_uses_configured_max_tokens_field_name() {
        let settings = ModelSettings {
            max_tokens: Some(2048),
            ..ModelSettings::default()
        };

        let payload = build_chat_request_payload(
            "gpt-4.1",
            &[],
            &settings,
            &ModelRequestParameters::default(),
            false,
            None,
            Some("max_completion_tokens"),
            None,
        )
        .expect("payload should serialize");

        assert_eq!(
            payload
                .get("max_completion_tokens")
                .and_then(serde_json::Value::as_u64),
            Some(2048)
        );
        assert!(payload.get("max_tokens").is_none());
    }

    #[test]
    fn build_chat_request_payload_omits_token_limit_when_max_tokens_is_absent() {
        let payload = build_chat_request_payload(
            "gpt-4.1",
            &[],
            &ModelSettings::default(),
            &ModelRequestParameters::default(),
            false,
            None,
            Some("max_completion_tokens"),
            None,
        )
        .expect("payload should serialize");

        assert!(payload.get("max_tokens").is_none());
        assert!(payload.get("max_completion_tokens").is_none());
    }
}
