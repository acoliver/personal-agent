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

/// Model wrapper that normalizes non-standard SSE formatting in streaming
/// responses. Delegates non-streaming requests to the inner model.
pub struct NormalizingSseModel {
    inner: Arc<dyn Model>,
    /// HTTP client (carries custom headers like User-Agent via `default_headers`).
    client: Client,
    api_key: String,
    base_url: String,
    model_name: String,
    default_timeout: Duration,
}

impl NormalizingSseModel {
    pub fn new(
        inner: Arc<dyn Model>,
        client: Client,
        api_key: String,
        base_url: String,
        model_name: String,
    ) -> Self {
        Self {
            inner,
            client,
            api_key,
            base_url,
            model_name,
            default_timeout: Duration::from_secs(120),
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
        let body = build_chat_request(&self.model_name, messages, settings, params);
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
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Request-body construction
// ---------------------------------------------------------------------------

fn build_chat_request(
    model_name: &str,
    messages: &[ModelRequest],
    settings: &ModelSettings,
    params: &ModelRequestParameters,
) -> ChatCompletionRequest {
    let chat_messages: Vec<ChatMessage> = messages.iter().flat_map(convert_request).collect();

    let tools = if params.tools.is_empty() {
        None
    } else {
        Some(convert_tools(&params.tools))
    };

    let tool_choice = params.tool_choice.as_ref().map(convert_tool_choice);

    ChatCompletionRequest {
        model: model_name.to_string(),
        messages: chat_messages,
        temperature: settings.temperature,
        top_p: settings.top_p,
        max_tokens: settings.max_tokens,
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

fn convert_request(req: &ModelRequest) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    for part in &req.parts {
        match part {
            ModelRequestPart::SystemPrompt(sys) => {
                messages.push(ChatMessage {
                    role: "system".to_string(),
                    content: Some(MessageContent::Text(sys.content.clone())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
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
                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: Some(content),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            ModelRequestPart::ToolReturn(tool_ret) => {
                messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text(tool_ret.content.to_string_content())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: tool_ret.tool_call_id.clone(),
                });
            }
            ModelRequestPart::RetryPrompt(retry) => {
                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: Some(MessageContent::Text(retry.content.message().to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            ModelRequestPart::BuiltinToolReturn(builtin) => {
                let content_str = serde_json::to_string(&builtin.content)
                    .unwrap_or_else(|_| builtin.content_type().to_string());
                messages.push(ChatMessage::tool(content_str, builtin.tool_call_id.clone()));
            }
            ModelRequestPart::ModelResponse(response) => {
                let mut text_parts = Vec::new();
                let mut tool_calls = Vec::new();
                for rp in &response.parts {
                    match rp {
                        ModelResponsePart::Text(t) => text_parts.push(t.content.clone()),
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
                messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(MessageContent::Text(text_parts.join(""))),
                    name: None,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                });
            }
        }
    }
    messages
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
