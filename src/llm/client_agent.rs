//! Agent-based LLM client with MCP tool integration
//!
//! This module provides Agent integration for `PersonalAgent` using `SerdesAI` Agent.

use crate::agent::tool_approval_policy::{ToolApprovalDecision, ToolApprovalPolicy};
use crate::llm::{LlmError, Message, Role, StreamEvent};
use crate::presentation::view_command::ViewCommand;
use futures::StreamExt;
use serdes_ai::core::messages::{
    ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart, TextPart, ThinkingPart,
    ToolCallArgs, ToolCallPart, ToolReturnContent, ToolReturnPart,
};
use serdes_ai::UserContent;
use serdes_ai_agent::prelude::*;
use serdes_ai_agent::ToolExecutor;
use serdes_ai_tools::{ToolDefinition, ToolError, ToolReturn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{oneshot, Mutex as AsyncMutex};

// Use std Result to avoid conflict with serdes_ai::prelude::Result
type StdResult<T, E> = std::result::Result<T, E>;

/// Gate for coordinating tool approval requests between tool executors and the presenter.
///
/// This uses oneshot channels keyed by `request_id` to pause tool execution pending
/// user approval decisions.
#[derive(Debug)]
pub struct ApprovalGate {
    pending: Arc<Mutex<HashMap<String, PendingApproval>>>,
}

#[derive(Debug)]
struct PendingApproval {
    tx: oneshot::Sender<bool>,
    tool_identifiers: Vec<String>,
}

#[derive(Debug)]
pub struct ApprovalWaiter {
    request_id: String,
    pending: Weak<Mutex<HashMap<String, PendingApproval>>>,
    receiver: Option<oneshot::Receiver<bool>>,
}

impl ApprovalWaiter {
    /// Await the approval decision for this pending request.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the resolver side is dropped before sending a decision.
    ///
    /// # Panics
    ///
    /// Panics if called after the internal receiver has already been taken.
    pub async fn wait(mut self) -> Result<bool, oneshot::error::RecvError> {
        let receiver = self
            .receiver
            .take()
            .expect("ApprovalWaiter receiver should be present");
        receiver.await
    }
}

impl Drop for ApprovalWaiter {
    fn drop(&mut self) {
        let Some(_receiver) = self.receiver.as_ref() else {
            return;
        };

        let Some(pending) = self.pending.upgrade() else {
            return;
        };

        {
            let mut pending_guard = pending
                .lock()
                .expect("approval gate pending map lock should not be poisoned");
            pending_guard.remove(&self.request_id);
        }
    }
}

impl ApprovalGate {
    /// Create a new approval gate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a pending approval and return a waiter to await the user's decision.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen in normal operation).
    #[must_use]
    pub fn wait_for_approval(&self, request_id: String, tool_identifier: String) -> ApprovalWaiter {
        self.wait_for_approvals(request_id, vec![tool_identifier])
    }

    /// Register a pending approval with multiple identifiers and return a waiter.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen in normal operation).
    #[must_use]
    pub fn wait_for_approvals(
        &self,
        request_id: String,
        tool_identifiers: Vec<String>,
    ) -> ApprovalWaiter {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(
                request_id.clone(),
                PendingApproval {
                    tx,
                    tool_identifiers,
                },
            );
        }

        ApprovalWaiter {
            request_id,
            pending: Arc::downgrade(&self.pending),
            receiver: Some(rx),
        }
    }

    /// Resolve a pending approval and return all claimed tool identifiers
    /// only when a live waiter exists.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen in normal operation).
    #[must_use]
    pub fn resolve_and_take_identifiers(
        &self,
        request_id: &str,
        approved: bool,
    ) -> Option<Vec<String>> {
        let pending_approval = {
            let mut pending = self.pending.lock().unwrap();
            pending.remove(request_id)
        }?;

        if pending_approval.tx.send(approved).is_ok() {
            Some(pending_approval.tool_identifiers)
        } else {
            None
        }
    }

    /// Resolve a pending approval and return the first claimed tool identifier.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen in normal operation).
    #[must_use]
    pub fn resolve_and_take_identifier(&self, request_id: &str, approved: bool) -> Option<String> {
        self.resolve_and_take_identifiers(request_id, approved)
            .and_then(|tool_identifiers| tool_identifiers.into_iter().next())
    }

    /// Resolve a pending approval with the user's decision.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen in normal operation).
    #[must_use]
    pub fn resolve(&self, request_id: &str, approved: bool) -> Option<String> {
        self.resolve_and_take_identifier(request_id, approved)
    }

    /// Resolve all pending approvals with a shared decision.
    ///
    /// Returns the request IDs that were resolved.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned (which should never happen in normal operation).
    #[must_use]
    pub fn resolve_all(&self, approved: bool) -> Vec<String> {
        let pending_approvals = {
            let mut pending = self.pending.lock().unwrap();
            pending.drain().collect::<Vec<_>>()
        };

        let mut request_ids = Vec::with_capacity(pending_approvals.len());
        for (request_id, pending_approval) in pending_approvals {
            let _ = pending_approval.tx.send(approved);
            request_ids.push(request_id);
        }

        request_ids
    }
}

impl Default for ApprovalGate {
    fn default() -> Self {
        Self::new()
    }
}

/// Context for MCP tool execution
///
/// This provides access to the global MCP service for tool execution,
/// as well as the approval gate and view channel for tool approval flow.
#[derive(Clone)]
pub struct McpToolContext {
    /// Channel for sending view commands to the UI layer
    pub view_tx: tokio::sync::mpsc::Sender<ViewCommand>,
    /// Approval gate for coordinating user approval of tool execution
    pub approval_gate: Arc<ApprovalGate>,
    /// Policy for evaluating tool approval requirements
    pub policy: Arc<AsyncMutex<ToolApprovalPolicy>>,
}

impl Default for McpToolContext {
    fn default() -> Self {
        let (view_tx, _) = tokio::sync::mpsc::channel(1);
        Self {
            view_tx,
            approval_gate: Arc::new(ApprovalGate::new()),
            policy: Arc::new(AsyncMutex::new(ToolApprovalPolicy::default())),
        }
    }
}

/// Register native (built-in) tools with the agent builder.
///
/// Native tools are registered before MCP tools so they appear first
/// in the tool list. These tools bypass the MCP layer for direct
/// local operations.
fn register_native_tools(
    mut builder: AgentBuilder<McpToolContext>,
) -> AgentBuilder<McpToolContext> {
    // Register ReadFile tool
    let read_file_def = crate::agent::tools::get_read_file_tool_definition();
    builder = builder.tool_with_executor(read_file_def, crate::agent::tools::ReadFileExecutor);

    // Register Search tool
    let search_def = crate::agent::tools::get_search_tool_definition();
    builder = builder.tool_with_executor(search_def, crate::agent::tools::SearchExecutor);

    // Register WriteFile tool
    let write_file_def = crate::agent::tools::get_write_file_tool_definition();
    builder = builder.tool_with_executor(write_file_def, crate::agent::tools::WriteFileExecutor);

    // Register EditFile tool
    let edit_file_def = crate::agent::tools::get_edit_file_tool_definition();
    builder = builder.tool_with_executor(edit_file_def, crate::agent::tools::EditFileExecutor);

    // Register ShellExec tool
    let shell_exec_def = crate::agent::tools::get_shell_exec_tool_definition();
    builder = builder.tool_with_executor(shell_exec_def, crate::agent::tools::ShellExecExecutor);

    builder
}

/// Executor that bridges Agent tools to MCP
struct McpToolExecutor {
    tool_name: String,
}

#[async_trait::async_trait]
impl ToolExecutor<McpToolContext> for McpToolExecutor {
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &RunContext<McpToolContext>,
    ) -> Result<ToolReturn, ToolError> {
        let provider = {
            let service_arc = crate::mcp::McpService::global();
            let provider = service_arc
                .lock()
                .await
                .find_tool_provider_metadata(&self.tool_name)
                .ok_or_else(|| {
                    ToolError::execution_failed(format!(
                        "No MCP provider found for tool {}",
                        self.tool_name
                    ))
                })?;
            provider
        };

        let (tool_identifier, decision) = {
            let policy = ctx.deps().policy.lock().await;
            let tool_identifier = policy.mcp_tool_identifier(&provider.mcp_name, &self.tool_name);
            let decision = policy.evaluate(&tool_identifier);
            drop(policy);
            (tool_identifier, decision)
        };

        match decision {
            ToolApprovalDecision::Allow => {}
            ToolApprovalDecision::Deny => {
                return Err(ToolError::execution_failed(
                    "Tool execution denied by policy",
                ));
            }
            ToolApprovalDecision::AskUser => {
                let request_id = uuid::Uuid::new_v4().to_string();
                let waiter = ctx
                    .deps()
                    .approval_gate
                    .wait_for_approval(request_id.clone(), tool_identifier.clone());

                if ctx
                    .deps()
                    .view_tx
                    .try_send(ViewCommand::ToolApprovalRequest {
                        request_id: request_id.clone(),
                        tool_name: self.tool_name.clone(),
                        tool_argument: args.to_string(),
                    })
                    .is_err()
                {
                    let _ = ctx.deps().approval_gate.resolve(&request_id, false);
                    return Err(ToolError::execution_failed(
                        "Failed to send approval request to UI (channel full or closed)",
                    ));
                }

                let approved = waiter.wait().await.unwrap_or(false);
                if !approved {
                    return Err(ToolError::execution_failed("Tool execution denied by user"));
                }
            }
        }

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
        context: McpToolContext,
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
        context: McpToolContext,
        mut on_event: F,
    ) -> StdResult<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        self.do_run_agent_stream(agent, messages, context, &mut on_event)
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
        let builder = self.build_agent_builder(model, system_prompt);
        // Register native tools first (they appear first in tool list)
        let builder = register_native_tools(builder);
        let builder = Self::register_mcp_tools(builder, mcp_tools);

        Ok(builder.build())
    }
}

impl crate::llm::LlmClient {
    fn collect_tool_transcript(
        messages: &[ModelRequest],
    ) -> (
        Vec<crate::llm::tools::ToolUse>,
        Vec<crate::llm::tools::ToolResult>,
    ) {
        let mut tool_calls = Vec::new();
        let mut tool_results = Vec::new();

        for request in messages {
            for part in &request.parts {
                match part {
                    ModelRequestPart::ModelResponse(response) => {
                        for response_part in &response.parts {
                            if let ModelResponsePart::ToolCall(tool_call) = response_part {
                                tool_calls.push(crate::llm::tools::ToolUse::new(
                                    tool_call.tool_call_id.clone().unwrap_or_default(),
                                    tool_call.tool_name.clone(),
                                    tool_call.args.to_json(),
                                ));
                            }
                        }
                    }
                    ModelRequestPart::ToolReturn(tool_return) => {
                        let content = match &tool_return.content {
                            ToolReturnContent::Text { content } => content.clone(),
                            ToolReturnContent::Json { content } => serde_json::to_string(content)
                                .unwrap_or_else(|_| content.to_string()),
                            ToolReturnContent::Image { .. } => "[image]".to_string(),
                            ToolReturnContent::Error { error } => error.message.clone(),
                            ToolReturnContent::Multiple { items } => {
                                serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string())
                            }
                        };
                        let is_error =
                            matches!(tool_return.content, ToolReturnContent::Error { .. });
                        let result = if is_error {
                            crate::llm::tools::ToolResult::error(
                                tool_return.tool_call_id.clone().unwrap_or_default(),
                                content,
                            )
                        } else {
                            crate::llm::tools::ToolResult::success(
                                tool_return.tool_call_id.clone().unwrap_or_default(),
                                content,
                            )
                        };
                        tool_results.push(result);
                    }
                    _ => {}
                }
            }
        }

        (tool_calls, tool_results)
    }

    async fn do_run_agent_stream<F>(
        &self,
        agent: &Agent<McpToolContext>,
        messages: &[Message],
        context: McpToolContext,
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
                    AgentStreamEvent::RunComplete { messages, .. } => {
                        tracing::info!("run_agent_stream: RunComplete");
                        let (tool_calls, tool_results) = Self::collect_tool_transcript(&messages);
                        on_event(StreamEvent::ToolTranscript {
                            tool_calls,
                            tool_results,
                        });
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
                let mut request = ModelRequest::new();

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

                if !response.parts.is_empty() {
                    request.add_part(ModelRequestPart::ModelResponse(Box::new(response)));
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
    use serdes_ai::core::messages::parts::ToolCallArgs;
    use uuid::Uuid;

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
    #[tokio::test]
    async fn approval_waiter_drop_cleans_pending_entry() {
        let gate = ApprovalGate::new();
        let request_id = Uuid::new_v4().to_string();
        let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        drop(waiter);

        assert!(gate.resolve(&request_id, true).is_none());
    }

    #[tokio::test]
    async fn resolve_returns_none_when_waiter_was_dropped() {
        let gate = ApprovalGate::new();
        let request_id = Uuid::new_v4().to_string();
        let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        drop(waiter);

        assert!(gate
            .resolve_and_take_identifier(&request_id, true)
            .is_none());
    }

    #[tokio::test]
    async fn resolve_returns_identifier_when_waiter_is_alive() {
        let gate = ApprovalGate::new();
        let request_id = Uuid::new_v4().to_string();
        let waiter = gate.wait_for_approval(request_id.clone(), "WriteFile".to_string());

        let resolver = {
            let gate = gate;
            let request_id = request_id.clone();
            tokio::spawn(async move { gate.resolve_and_take_identifier(&request_id, true) })
        };

        let approved = waiter.wait().await.expect("waiter should receive decision");
        let identifier = resolver
            .await
            .expect("resolver task should complete")
            .expect("identifier should be returned for live waiter");

        assert!(approved);
        assert_eq!(identifier, "WriteFile");
    }

    #[tokio::test]
    async fn resolve_returns_all_identifiers_when_waiter_is_alive() {
        let gate = ApprovalGate::new();
        let request_id = Uuid::new_v4().to_string();
        let waiter = gate.wait_for_approvals(
            request_id.clone(),
            vec!["ls".to_string(), "pwd".to_string()],
        );

        let resolver = {
            let gate = gate;
            let request_id = request_id.clone();
            tokio::spawn(async move { gate.resolve_and_take_identifiers(&request_id, true) })
        };

        let approved = waiter.wait().await.expect("waiter should receive decision");
        let identifiers = resolver
            .await
            .expect("resolver task should complete")
            .expect("identifiers should be returned for live waiter");

        assert!(approved);
        assert_eq!(identifiers, vec!["ls".to_string(), "pwd".to_string()]);
    }

    #[tokio::test]
    async fn resolve_all_resolves_every_pending_waiter() {
        let gate = ApprovalGate::new();

        let request_id_a = Uuid::new_v4().to_string();
        let request_id_b = Uuid::new_v4().to_string();

        let waiter_a = gate.wait_for_approval(request_id_a.clone(), "WriteFile".to_string());
        let waiter_b = gate.wait_for_approval(request_id_b.clone(), "Search".to_string());

        let resolved_ids = gate.resolve_all(false);

        assert_eq!(resolved_ids.len(), 2);
        assert!(resolved_ids.contains(&request_id_a));
        assert!(resolved_ids.contains(&request_id_b));

        let approved_a = waiter_a
            .wait()
            .await
            .expect("waiter a should receive resolution");
        let approved_b = waiter_b
            .wait()
            .await
            .expect("waiter b should receive resolution");

        assert!(!approved_a);
        assert!(!approved_b);

        assert!(gate.resolve(&request_id_a, false).is_none());
        assert!(gate.resolve(&request_id_b, false).is_none());
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
    fn collect_tool_transcript_extracts_calls_and_results() {
        let mut response = ModelResponse::new();
        response.add_part(ModelResponsePart::ToolCall(
            ToolCallPart::new(
                "web_search",
                ToolCallArgs::json(serde_json::json!({"query":"weather"})),
            )
            .with_tool_call_id("tool-call-1"),
        ));

        let mut request_with_tool_call = ModelRequest::new();
        request_with_tool_call.add_part(ModelRequestPart::ModelResponse(Box::new(response)));

        let mut request_with_tool_return = ModelRequest::new();
        request_with_tool_return.add_part(ModelRequestPart::ToolReturn(
            ToolReturnPart::new(
                "web_search",
                ToolReturnContent::json(serde_json::json!({"answer":"sunny"})),
            )
            .with_tool_call_id("tool-call-1"),
        ));

        let mut request_with_tool_error = ModelRequest::new();
        request_with_tool_error.add_part(ModelRequestPart::ToolReturn(
            ToolReturnPart::error("web_search", "request failed").with_tool_call_id("tool-call-2"),
        ));

        let (tool_calls, tool_results) = crate::llm::LlmClient::collect_tool_transcript(&[
            request_with_tool_call,
            request_with_tool_return,
            request_with_tool_error,
        ]);

        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "tool-call-1");
        assert_eq!(tool_calls[0].name, "web_search");

        assert_eq!(tool_results.len(), 2);
        assert_eq!(tool_results[0].tool_use_id, "tool-call-1");
        assert!(!tool_results[0].is_error);
        assert!(tool_results[0].content.contains("\"answer\":\"sunny\""));

        assert_eq!(tool_results[1].tool_use_id, "tool-call-2");
        assert!(tool_results[1].is_error);
        assert_eq!(tool_results[1].content, "request failed");
    }

    #[test]
    fn build_agent_message_history_preserves_assistant_tool_results() {
        let assistant_message = Message::assistant("tool summary").with_tool_results(vec![
            crate::llm::tools::ToolResult::success("tool-call-1", "{\"answer\":\"sunny\"}"),
            crate::llm::tools::ToolResult::error("tool-call-2", "request failed"),
        ]);

        let history = crate::llm::LlmClient::build_agent_message_history(&[assistant_message]);

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].parts.len(), 3);

        assert!(matches!(
            history[0].parts[0],
            ModelRequestPart::ModelResponse(_)
        ));

        match &history[0].parts[1] {
            ModelRequestPart::ToolReturn(tool_return) => {
                assert_eq!(tool_return.tool_call_id.as_deref(), Some("tool-call-1"));
                assert!(!matches!(
                    tool_return.content,
                    ToolReturnContent::Error { .. }
                ));
            }
            other => panic!("expected first tool return part, got {other:?}"),
        }

        match &history[0].parts[2] {
            ModelRequestPart::ToolReturn(tool_return) => {
                assert_eq!(tool_return.tool_call_id.as_deref(), Some("tool-call-2"));
                assert!(matches!(
                    tool_return.content,
                    ToolReturnContent::Error { .. }
                ));
            }
            other => panic!("expected second tool return part, got {other:?}"),
        }
    }

    #[test]
    fn mcp_tool_executor_stores_tool_name() {
        let executor = McpToolExecutor {
            tool_name: "weather/get_forecast".to_string(),
        };

        assert_eq!(executor.tool_name, "weather/get_forecast");
    }
}
