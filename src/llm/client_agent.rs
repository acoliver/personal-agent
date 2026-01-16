//! Agent-based LLM client with MCP tool integration
//!
//! This module provides Agent integration for PersonalAgent using SerdesAI Agent.

use crate::llm::{LlmError, Message, Role, StreamEvent};
use crate::registry::RegistryCache;
use futures::StreamExt;
use serdes_ai::UserContent;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::sync::Arc;

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
        let mut svc = service_arc.lock().await;
        
        let result = svc.call_tool(&self.tool_name, args.clone())
            .await
            .map_err(|e| ToolError::execution_failed(format!("MCP tool {} failed: {}", self.tool_name, e)))?;

        // Convert the JSON result to a ToolReturn
        Ok(ToolReturn::text(result.to_string()))
    }
}

/// Agent client extensions for LlmClient
pub trait AgentClientExt {
    /// Create an Agent with MCP tools integrated
    ///
    /// This builds a SerdesAI Agent with the current profile's model and system prompt,
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

impl AgentClientExt for crate::llm::LlmClient {
    async fn create_agent(
        &self,
        mcp_tools: Vec<crate::llm::tools::Tool>,
        system_prompt: &str,
    ) -> StdResult<Agent<McpToolContext>, LlmError> {
        // Set API key in environment
        self.set_api_key_env();

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

        // Create Agent builder
        let mut builder = AgentBuilder::from_arc(model)
            .temperature(self.profile.parameters.temperature)
            .top_p(self.profile.parameters.top_p)
            .max_tokens(u64::from(self.profile.parameters.max_tokens));

        // Add system prompt if not empty
        if !system_prompt.is_empty() {
            builder = builder.system_prompt(system_prompt);
        }

        // Add each MCP tool as an async tool using the bridge executor
        for tool in mcp_tools {
            let tool_name = tool.name.clone();
            let tool_def = ToolDefinition::new(&tool_name, &tool.description)
                .with_parameters(tool.input_schema.clone());

            // Create a bridge executor that calls MCP
            let executor = McpToolExecutor {
                tool_name: tool_name.clone(),
            };

            builder = builder.tool_with_executor(tool_def, executor);
        }

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

        // Create the McpToolContext
        let context = McpToolContext;

        // Create the agent stream
        let mut stream = AgentStream::new(
            agent,
            UserContent::text(prompt),
            context,
            RunOptions::default(),
        )
        .await
        .map_err(|e| LlmError::SerdesAi(e.to_string()))?;

        // Process stream events
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => match event {
                    AgentStreamEvent::TextDelta { text } => {
                        on_event(StreamEvent::TextDelta(text));
                    }
                    AgentStreamEvent::ThinkingDelta { text } => {
                        on_event(StreamEvent::ThinkingDelta(text));
                    }
                    AgentStreamEvent::ToolCallStart { tool_name, .. } => {
                        // Log tool execution start
                        eprintln!("Tool call started: {}", tool_name);
                    }
                    AgentStreamEvent::ToolExecuted {
                        tool_name,
                        success,
                        error,
                        ..
                    } => {
                        if success {
                            eprintln!("Tool completed: {}", tool_name);
                        } else {
                            eprintln!("Tool failed: {} - {:?}", tool_name, error);
                        }
                    }
                    AgentStreamEvent::RunComplete { .. } => {
                        on_event(StreamEvent::Complete);
                    }
                    AgentStreamEvent::Error { message } => {
                        on_event(StreamEvent::Error(message));
                    }
                    _ => {} // Ignore other events
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


