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
fn build_chat_request_payload(
    model_name: &str,
    messages: &[ModelRequest],
    settings: &ModelSettings,
    params: &ModelRequestParameters,
    enable_thinking: bool,
    thinking_budget: Option<u64>,
) -> Result<serde_json::Value, ModelError> {
    let chat_messages: Vec<OutboundChatMessage> =
        messages.iter().flat_map(convert_request).collect();

    let tools = if params.tools.is_empty() {
        None
    } else {
        Some(convert_tools(&params.tools))
    };

    let tool_choice = params.tool_choice.as_ref().map(convert_tool_choice);

    let (max_tokens, max_completion_tokens) = if enable_thinking {
        (None, thinking_budget.or(settings.max_tokens))
    } else {
        (settings.max_tokens, None)
    };

    let request = ChatCompletionRequest {
        model: model_name.to_string(),
        messages: Vec::new(),
        temperature: settings.temperature,
        top_p: settings.top_p,
        max_tokens,
        max_completion_tokens,
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
    if let Some(obj) = request_value.as_object_mut() {
        let encoded_messages = serde_json::to_value(chat_messages)?;
        obj.insert("messages".to_string(), encoded_messages);
    }

    Ok(request_value)
}

#[allow(clippy::too_many_lines)]
fn convert_request(req: &ModelRequest) -> Vec<OutboundChatMessage> {
    let mut messages = Vec::new();
    for part in &req.parts {
        match part {
            ModelRequestPart::SystemPrompt(sys) => {
                messages.push(OutboundChatMessage::from_chat_message(ChatMessage {
                    role: "system".to_string(),
                    content: Some(MessageContent::Text(sys.content.clone())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                }));
            }
            ModelRequestPart::UserPrompt(user) => {
                let content = match &user.content {
                    UserContent::Text(text) => MessageContent::Text(text.clone()),
                    UserContent::Parts(parts) => {
                        let text: String = parts
                            .iter()
                            .filter_map(|p| match p {
                                UserContentPart::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("");
                        MessageContent::Text(text)
                    }
                };
                messages.push(OutboundChatMessage::from_chat_message(ChatMessage {
                    role: "user".to_string(),
                    content: Some(content),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                }));
            }
            ModelRequestPart::ToolReturn(tool_ret) => {
                messages.push(OutboundChatMessage::from_chat_message(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text(tool_ret.content.to_string_content())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: tool_ret.tool_call_id.clone(),
                }));
            }
            ModelRequestPart::RetryPrompt(retry) => {
                messages.push(OutboundChatMessage::from_chat_message(ChatMessage {
                    role: "user".to_string(),
                    content: Some(MessageContent::Text(retry.content.message().to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                }));
            }
            ModelRequestPart::BuiltinToolReturn(builtin) => {
                let content_str = serde_json::to_string(&builtin.content)
                    .unwrap_or_else(|_| builtin.content_type().to_string());
                messages.push(OutboundChatMessage::from_chat_message(ChatMessage::tool(
                    content_str,
                    builtin.tool_call_id.clone(),
                )));
            }
            ModelRequestPart::ModelResponse(response) => {
                let mut text_parts = Vec::new();
                let mut tool_calls = Vec::new();
                let mut reasoning_parts = Vec::new();
                for rp in &response.parts {
                    match rp {
                        ModelResponsePart::Text(t) => text_parts.push(t.content.clone()),
                        ModelResponsePart::Thinking(t) => reasoning_parts.push(t.content.clone()),
                        ModelResponsePart::ToolCall(tc) => {
                            tool_calls.push(ToolCall {
                                id: tc.tool_call_id.clone().unwrap_or_default(),
                                tool_type: "function".to_string(),
                                function: FunctionCall {
                                    name: tc.tool_name.clone(),
                                    arguments: tc.args.to_json_string().unwrap_or_default(),
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

                let assistant_message = ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(MessageContent::Text(text_parts.join(""))),
                    name: None,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                };

                messages.push(OutboundChatMessage {
                    role: assistant_message.role,
                    content: assistant_message.content,
                    name: assistant_message.name,
                    tool_calls: assistant_message.tool_calls,
                    tool_call_id: assistant_message.tool_call_id,
                    reasoning_content,
                });
            }
        }
    }
    messages
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
}
