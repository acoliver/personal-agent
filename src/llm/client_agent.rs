//! Agent-based LLM client with MCP tool integration
//!
//! This module provides Agent integration for `PersonalAgent` using `SerdesAI` Agent.

use crate::llm::{LlmError, Message, Role, StreamEvent};
use futures::StreamExt;
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
    /// Create an Agent with MCP tools integrated
    ///
    /// This builds a `SerdesAI` Agent with the current profile's model and system prompt,
    /// and registers MCP tools as native Agent tools using a bridge executor.
    fn create_agent(
        &self,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        system_prompt: &str,
    ) -> impl std::future::Future<Output = StdResult<Agent<McpToolContext>, LlmError>> + Send;

    /// Run an agent with streaming
    fn run_agent_stream<F>(
        &self,
        agent: &Agent<McpToolContext>,
        messages: &[Message],
        on_event: F,
    ) -> impl std::future::Future<Output = StdResult<(), LlmError>> + Send
    where
        F: FnMut(StreamEvent) + Send;
}

impl crate::llm::LlmClient {
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
}

impl AgentClientExt for crate::llm::LlmClient {
    async fn create_agent(
        &self,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        system_prompt: &str,
    ) -> StdResult<Agent<McpToolContext>, LlmError> {
        tracing::info!("create_agent: model={}, base_url={}", self.profile.model_id, self.profile.base_url);
        self.set_api_key_env();

        let model = self.build_agent_model()?;
        let mut builder = self.build_agent_builder(model, system_prompt);
        builder = Self::register_mcp_tools(builder, mcp_tools);

        Ok(builder.build())
    }

    async fn run_agent_stream<F>(
        &self,
        agent: &Agent<McpToolContext>,
        messages: &[Message],
        mut on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        // Convert messages to a prompt (for now, just use the last user message)
        // TODO: Support full conversation history in Agent
        let prompt = messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, Role::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();
        
        tracing::info!("run_agent_stream: prompt='{}'", prompt);

        // Create the McpToolContext
        let context = McpToolContext;

        // Create the agent stream
        tracing::info!("run_agent_stream: creating AgentStream...");
        let mut stream = AgentStream::new(
            agent,
            UserContent::text(prompt),
            context,
            RunOptions::default(),
        )
        .await
        .map_err(|e| {
            tracing::error!("run_agent_stream: AgentStream creation failed: {}", e);
            LlmError::SerdesAi(e.to_string())
        })?;
        tracing::info!("run_agent_stream: AgentStream created, processing events...");

        // Process stream events
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
                    AgentStreamEvent::ToolCallStart { tool_name, tool_call_id, .. } => {
                        // Emit tool call started event
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
                        // Emit tool call completed event
                        on_event(StreamEvent::ToolCallCompleted {
                            tool_name,
                            call_id: tool_call_id.unwrap_or_default(),
                            success,
                            result: None, // SerdesAI doesn't include result in event
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
                    } // Ignore other events
                },
                Err(e) => {
                    on_event(StreamEvent::Error(e.to_string()));
                    return Err(LlmError::SerdesAi(e.to_string()));
                }
            }
        }

        Ok(())
    }
}
