//! Agent-based LLM client with MCP tool integration
//!
//! This module provides Agent integration for `PersonalAgent` using `SerdesAI` Agent.

use crate::llm::{LlmError, Message, Role, StreamEvent};
use futures::StreamExt;
use serdes_ai::core::messages::{
    ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart, TextPart, ThinkingPart,
    ToolCallArgs, ToolCallPart, ToolReturnPart,
};
use serdes_ai::UserContent;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};

// Use std Result to avoid conflict with serdes_ai::prelude::Result
type StdResult<T, E> = std::result::Result<T, E>;

/// Context for MCP tool execution
///
/// This provides access to the global MCP service for tool execution
#[derive(Clone)]
pub struct McpToolContext;

/// Executor that bridges Agent tools to MCP
struct McpToolExecutor {
    tool_name: String,
}

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for McpToolExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        // Get the global MCP service and call the tool
        let service_arc = crate::mcp::McpService::global();
        let result = service_arc
            .lock()
            .await
            .call_tool(&self.tool_name, args.clone())
            .await
            .map_err(|e| {
                ToolError::execution_failed(format!("MCP tool {} failed: {}", self.tool_name, e))
            })?;

        // Convert the JSON result to a ToolReturn
        Ok(ToolReturn::text(result.to_string()))
    }
}

/// Agent client extensions for `LlmClient`
pub trait AgentClientExt {
    /// Run an agent with streaming
    fn run_agent_stream<F>(
        &self,
        agent: &Agent<McpToolContext>,
        messages: &[Message],
        on_event: F,
    ) -> impl std::future::Future<Output = StdResult<(), LlmError>> + Send
    where
        F: FnMut(StreamEvent) + Send;

    /// Create an Agent with MCP tools integrated
    fn create_agent(
        &self,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        system_prompt: &str,
    ) -> impl std::future::Future<Output = StdResult<Agent<McpToolContext>, LlmError>> + Send;
}

impl AgentClientExt for crate::llm::LlmClient {
    async fn run_agent_stream<F>(
        &self,
        agent: &Agent<McpToolContext>,
        messages: &[Message],
        mut on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        self.do_run_agent_stream(agent, messages, &mut on_event)
            .await
    }

    async fn create_agent(
        &self,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        system_prompt: &str,
    ) -> StdResult<Agent<McpToolContext>, LlmError> {
        tracing::info!(
            "create_agent: model={}, base_url={}",
            self.profile.model_id,
            self.profile.base_url
        );
        self.set_api_key_env();

        let model = self.build_agent_model()?;
        let mut builder = self.build_agent_builder(model, system_prompt);
        builder = Self::register_mcp_tools(builder, mcp_tools);

        Ok(builder.build())
    }
}

impl crate::llm::LlmClient {
    async fn do_run_agent_stream<F>(
        &self,
        agent: &Agent<McpToolContext>,
        messages: &[Message],
        on_event: &mut F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        let (prompt, history_messages) = Self::split_prompt_and_history(messages);
        let message_history = Self::build_agent_message_history(history_messages);

        tracing::info!(
            "run_agent_stream: prompt='{}' history_messages={} history_requests={}",
            prompt,
            history_messages.len(),
            message_history.len()
        );

        let context = McpToolContext;
        let options = if message_history.is_empty() {
            RunOptions::default()
        } else {
            RunOptions::default().message_history(message_history)
        };

        tracing::info!("run_agent_stream: creating AgentStream...");
        let mut stream = AgentStream::new(agent, UserContent::text(prompt), context, options)
            .await
            .map_err(|e| {
                tracing::error!("run_agent_stream: AgentStream creation failed: {}", e);
                LlmError::SerdesAi(e.to_string())
            })?;
        tracing::info!("run_agent_stream: AgentStream created, processing events...");

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => match event {
                    AgentStreamEvent::TextDelta { text } => {
                        tracing::info!("run_agent_stream: TextDelta: '{}'", text);
                        on_event(StreamEvent::TextDelta(text));
                    }
                    AgentStreamEvent::ThinkingDelta { text } => {
                        on_event(StreamEvent::ThinkingDelta(text));
                    }
                    AgentStreamEvent::ToolCallStart {
                        tool_name,
                        tool_call_id,
                        ..
                    } => {
                        on_event(StreamEvent::ToolCallStarted {
                            tool_name: tool_name.clone(),
                            call_id: tool_call_id.clone().unwrap_or_default(),
                        });
                    }
                    AgentStreamEvent::ToolExecuted {
                        tool_name,
                        tool_call_id,
                        success,
                        error,
                        ..
                    } => {
                        on_event(StreamEvent::ToolCallCompleted {
                            tool_name,
                            call_id: tool_call_id.unwrap_or_default(),
                            success,
                            result: None,
                            error,
                        });
                    }
                    AgentStreamEvent::RunComplete { .. } => {
                        tracing::info!("run_agent_stream: RunComplete");
                        on_event(StreamEvent::Complete);
                    }
                    AgentStreamEvent::Error { message } => {
                        tracing::error!("run_agent_stream: Error: {}", message);
                        on_event(StreamEvent::Error(message));
                    }
                    other => {
                        tracing::debug!("run_agent_stream: other event: {:?}", other);
                    }
                },
                Err(e) => {
                    on_event(StreamEvent::Error(e.to_string()));
                    return Err(LlmError::SerdesAi(e.to_string()));
                }
            }
        }

        Ok(())
    }

    fn build_agent_model(&self) -> StdResult<std::sync::Arc<dyn serdes_ai::Model>, LlmError> {
        let base_url = self.base_url_override();
        let provider = self.get_serdes_provider();
        self.build_model(provider, base_url)
    }

    fn build_agent_builder(
        &self,
        model: std::sync::Arc<dyn serdes_ai::Model>,
        system_prompt: &str,
    ) -> AgentBuilder<McpToolContext> {
        let mut builder = AgentBuilder::from_arc(model)
            .temperature(self.profile.parameters.temperature)
            .top_p(self.profile.parameters.top_p)
            .max_tokens(u64::from(self.profile.parameters.max_tokens));

        if !system_prompt.is_empty() {
            builder = builder.system_prompt(system_prompt);
        }

        builder
    }

    fn register_mcp_tools(
        mut builder: AgentBuilder<McpToolContext>,
        mcp_tools: Vec<crate::llm::tools::Tool>,
    ) -> AgentBuilder<McpToolContext> {
        for tool in mcp_tools {
            let tool_name = tool.name.clone();
            let tool_def = ToolDefinition::new(&tool_name, &tool.description)
                .with_parameters(tool.input_schema.clone());
            let executor = McpToolExecutor { tool_name };
            builder = builder.tool_with_executor(tool_def, executor);
        }
        builder
    }

    fn split_prompt_and_history(messages: &[Message]) -> (String, &[Message]) {
        messages
            .iter()
            .rposition(|message| matches!(message.role, Role::User))
            .map_or_else(
                || {
                    let prompt = messages
                        .last()
                        .map(|message| message.content.clone())
                        .unwrap_or_default();
                    (prompt, messages)
                },
                |last_user_idx| {
                    let fallback_prompt = messages
                        .last()
                        .map(|message| message.content.clone())
                        .unwrap_or_default();
                    let prompt = messages[last_user_idx].content.clone();
                    let prompt = if prompt.is_empty() {
                        fallback_prompt
                    } else {
                        prompt
                    };
                    (prompt, &messages[..last_user_idx])
                },
            )
    }

    fn message_to_agent_history_request(message: &Message) -> Option<ModelRequest> {
        match message.role {
            Role::System => None,
            Role::User => {
                let mut request = ModelRequest::new();

                if !message.content.is_empty() {
                    request.add_user_prompt(message.content.clone());
                }

                for tool_result in &message.tool_results {
                    let mut tool_return = if tool_result.is_error {
                        ToolReturnPart::error("tool", tool_result.content.clone())
                    } else {
                        ToolReturnPart::success("tool", tool_result.content.clone())
                    };

                    if !tool_result.tool_use_id.is_empty() {
                        tool_return =
                            tool_return.with_tool_call_id(tool_result.tool_use_id.clone());
                    }

                    request.add_part(ModelRequestPart::ToolReturn(tool_return));
                }

                if request.parts.is_empty() {
                    None
                } else {
                    Some(request)
                }
            }
            Role::Assistant => {
                let mut response = ModelResponse::new();

                if !message.content.is_empty() {
                    response.add_part(ModelResponsePart::Text(TextPart::new(
                        message.content.clone(),
                    )));
                }

                if let Some(thinking) = &message.thinking_content {
                    if !thinking.is_empty() {
                        response.add_part(ModelResponsePart::Thinking(ThinkingPart::new(
                            thinking.clone(),
                        )));
                    }
                }

                for tool_use in &message.tool_uses {
                    let mut tool_call = ToolCallPart::new(
                        tool_use.name.clone(),
                        ToolCallArgs::json(tool_use.input.clone()),
                    );

                    if !tool_use.id.is_empty() {
                        tool_call = tool_call.with_tool_call_id(tool_use.id.clone());
                    }

                    response.add_part(ModelResponsePart::ToolCall(tool_call));
                }

                if response.parts.is_empty() {
                    None
                } else {
                    let mut request = ModelRequest::new();
                    request.add_part(ModelRequestPart::ModelResponse(Box::new(response)));
                    Some(request)
                }
            }
        }
    }

    fn build_agent_message_history(messages: &[Message]) -> Vec<ModelRequest> {
        messages
            .iter()
            .filter_map(Self::message_to_agent_history_request)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_prompt_and_history_uses_last_user_message() {
        let messages = vec![
            Message::user("first question"),
            Message::assistant("first answer"),
            Message::user("second question"),
        ];

        let (prompt, history) = crate::llm::LlmClient::split_prompt_and_history(&messages);

        assert_eq!(prompt, "second question");
        assert_eq!(history.len(), 2);
        assert!(matches!(history[0].role, Role::User));
        assert!(matches!(history[1].role, Role::Assistant));
    }

    #[test]
    fn build_agent_message_history_preserves_assistant_responses() {
        let assistant_message = Message::assistant("tool result summary").with_tool_uses(vec![
            crate::llm::tools::ToolUse::new(
                "tool-call-1",
                "web_search",
                serde_json::json!({"query": "weather"}),
            ),
        ]);

        let history = crate::llm::LlmClient::build_agent_message_history(&[
            Message::user("what's the weather"),
            assistant_message,
        ]);

        assert_eq!(history.len(), 2);

        assert!(matches!(
            history[0].parts.first(),
            Some(ModelRequestPart::UserPrompt(_))
        ));

        match history[1].parts.first() {
            Some(ModelRequestPart::ModelResponse(response)) => {
                assert!(response
                    .parts
                    .iter()
                    .any(|part| matches!(part, ModelResponsePart::Text(_))));
                assert!(response
                    .parts
                    .iter()
                    .any(|part| matches!(part, ModelResponsePart::ToolCall(_))));
            }
            other => panic!("expected ModelResponse history part, got {other:?}"),
        }
    }

    #[test]
    fn build_agent_message_history_preserves_user_tool_results() {
        let tool_result_message =
            Message::user("").with_tool_results(vec![crate::llm::tools::ToolResult::success(
                "tool-call-1",
                "{\"ok\":true}",
            )]);

        let history = crate::llm::LlmClient::build_agent_message_history(&[tool_result_message]);

        assert_eq!(history.len(), 1);
        assert!(history[0]
            .parts
            .iter()
            .any(|part| matches!(part, ModelRequestPart::ToolReturn(_))));
    }
}
