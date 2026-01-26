# Chat Service Requirements

The Chat Service handles sending messages to LLMs, managing streaming responses, and executing tool calls. **It is responsible for all stream processing** - the UI receives clean, ready-to-render events.

---

## Responsibilities


## Canonical Terminology

- Conversation and Message are defined in `dev-docs/requirements/data-models.md`.
- ContextState is owned by ConversationService.
- ModelProfile is owned by ProfileService.

- Build LLM requests from conversation context
- Stream responses from LLM providers
- **Parse and clean all stream events before emitting to UI**
- **Strip markers, split thinking from response**
- Handle tool calls via MCP integration
- Manage cancellation

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
        profile: &ModelProfile,
    ) -> Result<StreamHandle>;
    
    /// Cancel an active stream
    /// Will emit a Complete event with partial content
    fn cancel(&self, handle: &StreamHandle) -> Result<()>;
}

pub struct StreamHandle {
    pub id: Uuid,
    pub receiver: mpsc::Receiver<StreamEvent>,
}
```

---

## Stream Events (Clean Output)

These events are **clean and ready for UI rendering**. No parsing, marker stripping, or buffer management needed by the UI.

```rust
pub enum StreamEvent {
    /// Clean text content chunk
    /// - No end markers (␄)
    /// - No thinking content mixed in
    /// - Ready to append directly to UI
    TextDelta { content: String },
    
    /// Clean thinking content chunk
    /// - No end markers
    /// - Separated from response text
    /// - Ready to append directly to thinking UI
    ThinkingDelta { content: String },
    
    /// Tool call initiated
    ToolCallStart { 
        id: String, 
        name: String,
    },
    
    /// Tool call arguments (may come in chunks)
    ToolCallDelta { 
        id: String, 
        arguments: String,
    },
    
    /// Tool result received
    ToolResult { 
        id: String, 
        result: String,
        is_error: bool,
    },
    
    /// Stream completed successfully
    /// Contains final clean content for persistence
    Complete { 
        text: String,              // Final response text (clean)
        thinking: Option<String>,  // Final thinking content (clean)
        tool_calls: Vec<ToolCall>, // All tool calls made
    },
    
    /// Error occurred
    Error { message: String },
}
```

---

## Internal Processing

The service handles all the messy parsing internally. The UI never sees raw provider data.

### Stream Processing Pipeline

```
Provider Stream
     │
     ▼
┌─────────────────────────────────┐
│  1. Receive raw provider event  │
│     (TextDelta, ThinkingDelta,  │
│      ToolUse, etc.)             │
└─────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────┐
│  2. Parse and categorize        │
│     - Detect thinking vs text   │
│     - Parse tool calls          │
└─────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────┐
│  3. Clean content               │
│     - Strip end markers (␄)     │
│     - Trim whitespace edges     │
│     - Remove control chars      │
└─────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────┐
│  4. Emit clean StreamEvent      │
│     to UI via channel           │
└─────────────────────────────────┘
```

### Marker Handling

| Marker | Source | Handling |
|--------|--------|----------|
| `␄` (end of text) | Stream complete | Strip before emitting, trigger Complete event |
| `▌` (cursor) | UI display | **Not in stream** - UI adds/removes this |

### Buffer Management

The service maintains internal buffers:

```rust
struct StreamState {
    response_buffer: String,   // Accumulated response text
    thinking_buffer: String,   // Accumulated thinking text
    tool_calls: Vec<ToolCall>, // Accumulated tool calls
    is_complete: bool,
}
```

On stream end or cancellation:
1. Strip any trailing markers from buffers
2. Emit `Complete` event with clean final content
3. Clear buffers

---

## Request Building

### Steps

1. Load conversation from ConversationService
2. Get context from ContextStrategy (may compress)
3. Add system prompt from profile
4. Configure model settings
5. Add MCP tools from McpService
6. Build provider-specific request

### Context Integration

```rust
async fn build_request(
    &self,
    conversation_id: Uuid,
    profile: &ModelProfile,
) -> Result<ModelRequest> {
    // 1. Load conversation
    let conversation = self.conversation_service.load(conversation_id)?;
    
    // 2. Build context (may compress long conversations)
    let context = self.context_strategy.build_context(
        &conversation.messages,
        conversation.context_state.as_ref(),
        profile.context_limit(),
    );
    
    // 3. Build request with clean context
    let mut request = ModelRequest::new()
        .model(&profile.model_id)
        .messages(context.messages);
    
    // 4. Add system prompt
    if !profile.system_prompt.is_empty() {
        request = request.system_prompt(&profile.system_prompt);
    }
    
    // 5. Add model parameters
    if let Some(temp) = profile.parameters.temperature {
        request = request.temperature(temp);
    }
    if let Some(max) = profile.parameters.max_tokens {
        request = request.max_tokens(max);
    }
    
    // 6. Add thinking config
    if profile.parameters.enable_thinking {
        request = request.with_thinking(
            profile.parameters.thinking_budget.unwrap_or(10000)
        );
    }
    
    // 7. Add MCP tools
    let tools = self.mcp_service.available_tools();
    for tool in tools {
        request = request.add_tool(tool);
    }
    
    Ok(request)
}
```

---


## Context and Memory Lifecycle

- ContextState is created by ContextStrategy when compression is triggered.
- ChatService requests ConversationService.update_context_state(context_state).
- ContextState is cleared when:
  - conversation is deleted, or
  - strategy changes, or
  - summary_range no longer matches message count.
- Compression is triggered when estimated token usage exceeds 70% of model context limit.

## Cancellation

### Flow

1. UI calls `cancel(handle)`
2. Service sets cancellation flag
3. Stream loop detects flag
4. Service emits `Complete` with partial content
5. Buffers cleared

### Partial Content

On cancel, emit whatever was accumulated:

```rust
fn handle_cancel(&mut self, sender: &mpsc::Sender<StreamEvent>) {
    // Clean the buffers
    let text = self.response_buffer.trim_end_matches('␄').to_string();
    let thinking = if self.thinking_buffer.is_empty() {
        None
    } else {
        Some(self.thinking_buffer.trim_end_matches('␄').to_string())
    };
    
    // Emit complete with partial content
    let _ = sender.send(StreamEvent::Complete {
        text,
        thinking,
        tool_calls: self.tool_calls.clone(),
    });
}
```

---

## Tool Call Handling

### Flow

1. Provider emits tool call request
2. Service emits `ToolCallStart` to UI
3. Service calls MCP tool via McpService
4. Service emits `ToolResult` to UI
5. Service feeds result back to LLM
6. LLM continues response

### Error Handling

Tool errors don't stop the stream:

```rust
async fn execute_tool(&self, tool_call: &ToolCall) -> ToolResult {
    match self.mcp_service.call_tool(&tool_call.name, &tool_call.arguments).await {
        Ok(result) => ToolResult {
            id: tool_call.id.clone(),
            result,
            is_error: false,
        },
        Err(e) => ToolResult {
            id: tool_call.id.clone(),
            result: format!("Error: {}", e),
            is_error: true,
        },
    }
}
```

---

## Provider Abstraction

### Supported Providers

| Provider | Implementation | Notes |
|----------|----------------|-------|
| Anthropic | Native client | Messages API |
| OpenAI | Native client | Chat Completions |
| Google | Native client | Gemini API |
| OpenRouter | OpenAI-compatible | Via base_url |
| Ollama | OpenAI-compatible | Local |
| Custom | OpenAI-compatible | User-configured base_url |

### Client Selection

```rust
fn get_client(&self, profile: &ModelProfile) -> Box<dyn LlmClient> {
    match profile.provider_id.as_str() {
        "anthropic" => Box::new(AnthropicClient::new(&profile.api_key)),
        "openai" => Box::new(OpenAIClient::new(&profile.api_key)),
        "google" => Box::new(GoogleClient::new(&profile.api_key)),
        _ => {
            // OpenAI-compatible with custom base URL
            Box::new(OpenAIClient::with_base_url(
                &profile.api_key,
                profile.base_url.as_deref().unwrap_or(""),
            ))
        }
    }
}
```

---

## Error Handling

All errors use the standard error contract:

```json
{ "code": "string", "message": "string", "field": "string" }
```


## Validation Rules

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| conversation_id | Must exist | NOT_FOUND | Conversation not found |
| user_message | Non-empty after trim | VALIDATION_ERROR | Message cannot be empty |
| profile.id | Must exist | NOT_FOUND | Profile not found |
| profile.model_id | Non-empty | VALIDATION_ERROR | Model is required |
| profile.provider_id | Non-empty | VALIDATION_ERROR | Provider is required |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| CH-NT1 | send_message with empty content | Error code VALIDATION_ERROR, message shown in #error-banner |
| CH-NT2 | send_message with unknown conversation_id | Error code NOT_FOUND, chat area unchanged |
| CH-NT3 | provider returns 401 | Error code UNAUTHORIZED, "Invalid API key" in #error-banner |
| CH-NT4 | network timeout | Error code NETWORK_ERROR, retry enabled |
| CH-NT5 | cancel on completed stream | Error code CONFLICT, no UI change |

## End-to-End Flow (ChatService)

1. Validate inputs; return VALIDATION_ERROR on failure.
2. Append user message via ConversationService.append_message.
3. Build context via ContextStrategy.build_context.
4. Start provider stream; emit StreamingStarted.
5. On Complete: append assistant message and persist tool calls/results.
6. If ContextStrategy returned new ContextState, persist it.
7. On failure after step 2, emit Error and leave persisted user message intact.

| Error | StreamEvent | Notes |
|-------|-------------|-------|
| Network error | Error { message } | code=NETWORK_ERROR, message="Connection failed" |
| Auth error (401) | Error { message } | code=UNAUTHORIZED, message="Invalid API key" |
| Rate limit (429) | Error { message } | code=RATE_LIMITED, message="Rate limited, try again" |
| Model error | Error { message } | code=SERVICE_UNAVAILABLE, message=provider error |
| Tool timeout | ToolResult { is_error: true } | code=SERVICE_UNAVAILABLE in tool result payload |
| Parse error | Error { message } | code=SERVICE_UNAVAILABLE, message="Failed to parse response" |

---

## Persistence Integration

The ChatService coordinates with ConversationService for persistence:

| Action | When | Service Call |
|--------|------|--------------|
| Save user message | Before streaming | ConversationService.append_message() |
| Save assistant message | On Complete event | ConversationService.append_message() |
| Update context state | If compression occurred | ConversationService.update_context_state() |

---

## Test Requirements

| ID | Test |
|----|------|
| CH-T1 | TextDelta events have no ␄ markers |
| CH-T2 | ThinkingDelta events have no ␄ markers |
| CH-T3 | Complete event has clean final text |
| CH-T4 | Complete event has clean thinking (if present) |
| CH-T5 | Cancel emits Complete with partial content |
| CH-T6 | Tool errors don't stop the stream |
| CH-T7 | Context strategy applied to request |
| CH-T8 | System prompt included in request |
| CH-T9 | MCP tools added to request |
| CH-T10 | Provider-specific client selected |
