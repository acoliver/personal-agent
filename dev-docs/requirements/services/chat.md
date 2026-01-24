# Chat Service Requirements

The Chat Service handles sending messages to LLMs, managing streaming responses, and coordinating tool execution. **It uses SerdesAI Agent mode** for the core LLM interaction loop, delegating tool execution and response streaming to the agent framework.

---

## Responsibilities

- Coordinate message sending through SerdesAI Agent
- Manage streaming response events from agent
- Provide clean, pre-processed events to UI
- Handle cancellation (drop stream, return partial content)
- Coordinate with ConversationService for persistence
- Coordinate with McpService for tool availability
- Apply context compression via HistoryProcessor

---

## Architecture: Agent Mode

### Why Agent Mode?

The previous approach had manual tool execution loops in the UI layer. Agent mode moves this responsibility to SerdesAI:

| Manual Loop (Old) | Agent Mode (New) |
|-------------------|------------------|
| UI manages tool execution | Agent manages tool execution |
| Complex state in view controller | State in agent runtime |
| 39KB chat_view.rs | Thin UI + service layer |
| Difficult to test | Testable service layer |

### SerdesAI Agent Integration

```rust
pub struct ChatServiceImpl {
    profile_service: Arc<dyn ProfileService>,
    conversation_service: Arc<dyn ConversationService>,
    mcp_service: Arc<dyn McpService>,
}
```

**Key Decision:** McpService owns and manages MCP toolsets. ChatService requests available tools via `mcp_service.get_toolsets()`. This allows:
- Settings view to show MCP status
- MCPs shared across conversations
- Central lifecycle management

**Key Decision:** ChatService gets API keys through ProfileService.get_model_config(), NOT directly from SecretsService. ProfileService owns the secrets relationship.

---

## Service Interface

```rust
pub trait ChatService: Send + Sync {
    /// Send a message and get streaming response
    /// Returns a handle for receiving events and cancellation
    fn send_message(
        &self,
        conversation_id: Uuid,
        user_message: &str,
    ) -> Result<StreamHandle>;
    
    /// Cancel an active stream
    /// Will emit a Cancelled event with partial content
    fn cancel(&self, handle: &StreamHandle) -> Result<()>;
    
    /// Check if a stream is still active
    fn is_streaming(&self, handle: &StreamHandle) -> bool;
}

pub struct StreamHandle {
    pub id: Uuid,
    pub receiver: mpsc::Receiver<StreamEvent>,
    cancel_sender: oneshot::Sender<()>,
}

impl StreamHandle {
    /// Get the next event (blocks until available)
    pub async fn next(&mut self) -> Option<StreamEvent> {
        self.receiver.recv().await
    }
}
```

---

## Stream Events (Clean Output)

These events are **clean and ready for UI rendering**. The agent and service handle all parsing internally.

```rust
pub enum StreamEvent {
    /// Stream has started
    Started { 
        /// Model being used for this response
        model_id: String,
    },
    
    /// Clean text content chunk
    /// Ready to append directly to UI
    TextDelta { content: String },
    
    /// Clean thinking content chunk
    /// Ready to append directly to thinking UI
    ThinkingDelta { content: String },
    
    /// Tool execution started
    ToolStart { 
        id: String, 
        name: String,
    },
    
    /// Tool execution completed
    ToolComplete { 
        id: String, 
        name: String,
        success: bool,
        /// Brief result summary for UI (not full output)
        summary: Option<String>,
    },
    
    /// Stream completed successfully
    Complete { 
        /// Final response text (clean, no markers)
        text: String,
        /// Final thinking content (clean, always included if model provided it)
        thinking: Option<String>,
        /// All tool calls made during this response
        tool_calls: Vec<ToolCallSummary>,
        /// Model that generated this response
        model_id: String,
    },
    
    /// Stream was cancelled
    /// Partial content is persisted with [cancelled] marker
    Cancelled {
        /// Partial text accumulated before cancel
        partial_text: String,
        /// Partial thinking accumulated (always included if present)
        partial_thinking: Option<String>,
        /// Model that was generating this response
        model_id: String,
    },
    
    /// Error occurred
    Error { 
        message: String,
        /// If true, SerdesAI retry logic applies
        retryable: bool,
    },
}

pub struct ToolCallSummary {
    pub id: String,
    pub name: String,
    pub success: bool,
}
```

**Decision:** Usage stats (tokens) not included in events for now. Will add later as message footer along with context size.

---

## Message Flow

### Send Message Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                         ChatService                              │
└─────────────────────────────────────────────────────────────────┘
                              │
    1. send_message(conversation_id, user_message)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Load conversation from ConversationService                      │
│  Get resolved config from ProfileService.get_model_config()      │
│  (includes profile AND resolved API key)                         │
│  Get toolsets from McpService                                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Save user message to ConversationService                        │
│  (persist immediately before LLM call)                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Build SerdesAI Agent                                            │
│  - ModelConfig with profile settings                             │
│  - System prompt from profile                                    │
│  - Parameters (temp, max_tokens, thinking)                       │
│  - HistoryProcessor for context management                       │
│  - Attach MCP toolsets                                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  AgentStream::new() with message_history from conversation       │
│                                                                  │
│  Agent internally:                                               │
│  - Applies HistoryProcessor (context compression)                │
│  - Sends to LLM                                                  │
│  - Handles tool calls with retry logic                           │
│  - Feeds results back                                            │
│  - Continues until done                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Map agent events to StreamEvent                                 │
│  Send to UI via channel                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  On Complete: Save assistant message to ConversationService      │
│  On Cancelled: Save partial message with [cancelled] marker      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Agent Configuration

### Building the Agent

```rust
async fn build_agent(
    &self,
    config: &ResolvedModelConfig,  // From ProfileService.get_model_config()
    conversation: &Conversation,
) -> Result<Agent<(), String>> {
    let profile = &config.profile;
    
    // 1. Build model spec: "provider:model_id"
    // SerdesAI uses format like "openai:gpt-4o" or "anthropic:claude-3-5-sonnet"
    let spec = format!("{}:{}", profile.api_type.as_str(), profile.model_id);
    
    // 2. Build model config with thinking/reasoning support
    // API key comes from ResolvedModelConfig - ChatService doesn't touch SecretsService
    // api_key is Option<String> - None for AuthMethod::None (local models)
    let mut model_config = ModelConfig::new(&spec)
        .with_base_url(&profile.base_url);
    
    if let Some(ref api_key) = config.api_key {
        model_config = model_config.with_api_key(api_key);
    }
    
    // Enable thinking if profile specifies (otherwise model default)
    if profile.parameters.enable_thinking {
        model_config = model_config.with_thinking(profile.parameters.thinking_budget);
    }
    
    if let Some(ref effort) = profile.parameters.reasoning_effort {
        model_config = model_config.with_reasoning_effort(effort);
    }
    
    // 4. Build agent
    let mut builder = AgentBuilder::from_config(model_config)?
        .system_prompt(&profile.system_prompt);
    
    // 5. Add model parameters
    if let Some(temp) = profile.parameters.temperature {
        builder = builder.temperature(temp);
    }
    if let Some(max) = profile.parameters.max_tokens {
        builder = builder.max_tokens(max);
    }
    
    // 6. Add context management via HistoryProcessor
    // context_limit is required (u32, not Option) - pre-filled from models.dev
    let context_limit = profile.parameters.context_limit;
    builder = builder.history_processor(
        TruncateByTokens::new(context_limit)
            .keep_first(true)  // Keep system prompt
    );
    
    // 7. Add MCP toolsets
    let toolsets = self.mcp_service.get_toolsets();
    for toolset in toolsets {
        builder = builder.toolset(toolset);
    }
    
    Ok(builder.build())
}
```

### Running with Message History

```rust
async fn run_stream(
    &self,
    agent: &Agent<(), String>,
    conversation: &Conversation,
    user_message: &str,
) -> Result<AgentStream<(), String>> {
    // Convert conversation messages to SerdesAI format
    let history = self.convert_to_model_requests(&conversation.messages);
    
    // Create run options with history
    let options = RunOptions::new()
        .message_history(history);
    
    // Start streaming
    let stream = AgentStream::new(
        agent,
        UserContent::text(user_message),
        (),  // No deps needed
        options,
    ).await?;
    
    Ok(stream)
}
```

---

## Context Management

**Decision:** Use SerdesAI's built-in `HistoryProcessor` for context compression.

### Available Processors

| Processor | Use Case |
|-----------|----------|
| `TruncateByTokens` | Limit by estimated token count (our primary) |
| `TruncateHistory` | Limit by message count |
| `FilterHistory` | Remove system prompts, tool returns, retries |
| `SummarizeHistory` | Placeholder for future LLM-based summarization |
| `ChainedProcessor` | Combine multiple processors |
| `FnProcessor` | Custom logic |

### Default Configuration

```rust
// Primary: Token-based truncation
// context_limit is required (u32) - comes from models.dev via profile
TruncateByTokens::new(profile.parameters.context_limit as u64)
    .keep_first(true)  // Always keep system prompt
    .chars_per_token(4.0)  // Reasonable estimate for English
```

### Future: Sandwich Strategy

For more sophisticated context management (important messages protected):

```rust
ChainedProcessor::new()
    .add(FnProcessor::new(|ctx, msgs| {
        // Mark important messages (first, last N, tool results)
        // Compress middle section
        // Return optimized history
    }))
    .add(TruncateByTokens::new(limit))
```

---

## Retry Support

**Decision:** Use SerdesAI's built-in retry logic rather than implementing our own.

### What SerdesAI Provides

| Feature | Configuration | Default |
|---------|---------------|---------|
| Output validation retries | `builder.max_output_retries(n)` | 3 |
| Tool execution retries | `builder.max_tool_retries(n)` | 3 |
| Per-tool retry limit | `tool.max_retries` | Inherits from agent |
| Retryable errors | `error.is_retryable()` | Model/tool dependent |

### How It Works

1. **Output Validation Fails:** Agent sends `RetryPrompt` to model, model tries again
2. **Tool Execution Fails:** If `error.is_retryable()`, agent retries up to `max_retries`
3. **Max Retries Exceeded:** `AgentRunError::MaxRetriesExceeded` emitted

### ChatService Integration

```rust
// In agent builder
let agent = AgentBuilder::from_config(model_config)?
    .max_output_retries(3)  // Retry output validation
    .max_tool_retries(2)    // Retry failed tools
    // ...
    .build();

// In stream event mapping
match agent_event {
    AgentStreamEvent::Error(e) => {
        StreamEvent::Error {
            message: e.to_string(),
            retryable: e.is_retryable(),  // Pass through for UI info
        }
    }
    // ...
}
```

**Note:** We don't auto-retry at ChatService level. SerdesAI handles retries internally. If it still fails after retries, we surface the error to UI and let user decide.

---

## Cancellation

### Current Implementation (Drop Stream)

SerdesAI doesn't have native cancellation support yet (Issue #6). For now:

```rust
fn cancel(&self, handle: &StreamHandle) -> Result<()> {
    // Signal cancellation
    let _ = handle.cancel_sender.send(());
    Ok(())
}

// In streaming task
async fn stream_task(
    stream: AgentStream,
    sender: mpsc::Sender<StreamEvent>,
    mut cancel_receiver: oneshot::Receiver<()>,
) {
    let mut text_buffer = String::new();
    let mut thinking_buffer = String::new();
    
    loop {
        tokio::select! {
            // Check for cancellation
            _ = &mut cancel_receiver => {
                // Drop the stream (closes HTTP connection)
                drop(stream);
                
                // Emit cancelled event with partial content
                let _ = sender.send(StreamEvent::Cancelled {
                    partial_text: text_buffer,
                    partial_thinking: if thinking_buffer.is_empty() { 
                        None 
                    } else { 
                        Some(thinking_buffer) 
                    },
                }).await;
                break;
            }
            
            // Process next agent event
            event = stream.next() => {
                match event {
                    Some(Ok(agent_event)) => {
                        // Accumulate for cancellation
                        if let AgentStreamEvent::TextDelta { text } = &agent_event {
                            text_buffer.push_str(text);
                        }
                        if let AgentStreamEvent::ThinkingDelta { text } = &agent_event {
                            thinking_buffer.push_str(text);
                        }
                        
                        // Map and send
                        if let Some(stream_event) = map_agent_event(agent_event) {
                            if sender.send(stream_event).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        let _ = sender.send(StreamEvent::Error {
                            message: e.to_string(),
                            retryable: e.is_retryable(),
                        }).await;
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}
```

### Pydantic-AI Cancellation Pattern

Pydantic-AI uses Python's native `asyncio.CancelledError`:

```python
task = asyncio.create_task(run_agent())

try:
    async with receive_stream:
        async for message in receive_stream:
            yield message
    result = await task
except asyncio.CancelledError as e:
    task.cancel(msg=e.args[0] if len(e.args) != 0 else None)
    raise
```

**Key insight:** When the async context manager exits (due to cancellation), the task is cancelled and the exception propagates. No special cancellation token - just language-native task cancellation.

### Future: SerdesAI Cancellation (Issue #6)

When implemented, SerdesAI should add:

```rust
// Option A: CancellationToken (recommended)
let cancel_token = CancellationToken::new();
let stream = AgentStream::new_with_cancel(&agent, prompt, deps, options, cancel_token.clone())?;

// Cancel from another task
cancel_token.cancel();

// Option B: Stream method
stream.cancel();
```

The stream should:
1. Stop the HTTP connection
2. Cancel pending tool calls via MCP `notifications/cancelled` (see McpService.cancel_tool_call)
3. Emit `AgentStreamEvent::Cancelled { partial_text, partial_thinking, pending_tools }`

**MCP Tool Cancellation:** When cancelling during a tool call, SerdesAI should call through to `McpService.cancel_tool_call()` which sends the MCP-standard `notifications/cancelled` message. This is best-effort - MCP servers SHOULD honor it but may not.

---

## Persistence

### When to Persist

| Event | Persistence Action |
|-------|-------------------|
| Before `AgentStream::new()` | Save user message |
| `StreamEvent::Complete` | Save assistant message |
| `StreamEvent::Cancelled` | Save partial message with `[cancelled]` marker |
| `StreamEvent::Error` | Don't save (user can retry) |

### Cancelled Message Format

**Decision:** Save cancelled messages so user can see what was generated.

```rust
// On Cancelled event
if let StreamEvent::Cancelled { partial_text, partial_thinking } = event {
    let content = if partial_text.is_empty() {
        "[cancelled]".to_string()
    } else {
        format!("{}\n\n[cancelled]", partial_text)
    };
    
    let mut message = Message::assistant(&content);
    message.cancelled = true;
    
    if let Some(thinking) = partial_thinking {
        message = message.with_thinking(&thinking);
    }
    
    self.conversation_service.append_message(conversation_id, &message)?;
}
```

---

## Conversation Management

**Decision:** Conversation management is **our responsibility**, not SerdesAI's.

### What SerdesAI Provides

- `message_history` parameter in `RunOptions` - accepts previous messages
- `HistoryProcessor` - processes history before sending to model
- No persistence, no conversation state

### What We Manage

| Responsibility | Service |
|----------------|---------|
| Store conversations | ConversationService |
| Store messages | ConversationService |
| Convert to/from SerdesAI format | ChatService |
| Track conversation metadata | ConversationService |

### Message Format Conversion

```rust
fn convert_to_model_requests(&self, messages: &[Message]) -> Vec<ModelRequest> {
    messages.iter().map(|m| {
        let mut req = ModelRequest::new();
        match m.role {
            MessageRole::User => {
                req.add_user_prompt(&m.content);
            }
            MessageRole::Assistant => {
                let mut response = ModelResponse::new();
                response.add_text(&m.content);
                if let Some(thinking) = &m.thinking {
                    response.add_thinking(thinking);
                }
                req.add_response(response);
            }
            MessageRole::System => {
                req.add_system_prompt(&m.content);
            }
        }
        req
    }).collect()
}
```

---

## UI Integration

### Chat View Usage

```rust
// In ChatPresenter
async fn send_message(&self, content: String) {
    let conversation_id = self.current_conversation_id()?;
    
    // Get stream handle
    let mut handle = self.chat_service.send_message(
        conversation_id,
        &content,
    )?;
    
    // Update UI immediately
    self.view.add_user_message(&content);
    self.view.clear_input();
    self.view.set_streaming(true);
    
    // Process events
    while let Some(event) = handle.next().await {
        match event {
            StreamEvent::Started => {
                self.view.add_assistant_placeholder();
            }
            StreamEvent::TextDelta { content } => {
                self.view.append_to_assistant(&content);
            }
            StreamEvent::ThinkingDelta { content } => {
                self.view.append_to_thinking(&content);
            }
            StreamEvent::ToolStart { name, .. } => {
                self.view.show_tool_indicator(&name);
            }
            StreamEvent::ToolComplete { name, success, .. } => {
                self.view.update_tool_indicator(&name, success);
            }
            StreamEvent::Complete { .. } => {
                self.view.finalize_assistant_message();
                self.view.set_streaming(false);
            }
            StreamEvent::Cancelled { partial_text, .. } => {
                // Show what was cancelled
                self.view.finalize_assistant_message_cancelled(&partial_text);
                self.view.set_streaming(false);
            }
            StreamEvent::Error { message, retryable } => {
                self.view.show_error(&message, retryable);
                self.view.set_streaming(false);
            }
        }
    }
}

async fn cancel_streaming(&self) {
    if let Some(handle) = &self.stream_handle {
        self.chat_service.cancel(handle)?;
    }
}
```

---

## Test Requirements

| ID | Test |
|----|------|
| CH-T1 | send_message returns StreamHandle |
| CH-T2 | Started event emitted first |
| CH-T3 | TextDelta events have clean content |
| CH-T4 | ThinkingDelta events have clean content |
| CH-T5 | Complete event has final text and thinking |
| CH-T6 | Cancel emits Cancelled with partial content |
| CH-T7 | Cancelled message persisted with marker |
| CH-T8 | Tool errors handled by agent retry logic |
| CH-T9 | User message persisted before streaming |
| CH-T10 | Assistant message persisted on Complete |
| CH-T11 | HistoryProcessor applied to messages |
| CH-T12 | Profile parameters passed to agent |
| CH-T13 | MCP toolsets attached from McpService |
| CH-T14 | Agent handles multi-turn tool execution |
| CH-T15 | Error events include retryable flag |
| CH-T16 | Context limit from profile applied |
