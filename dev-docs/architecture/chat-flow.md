# Chat Flow Architecture

This document describes the end-to-end data flow from the Chat View UI through services to the SerdesAI Agent and back.

---

## Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CHAT VIEW (UI)                                  │
│  - Renders messages, input field, buttons                                    │
│  - Receives StreamEvent from ChatService                                     │
│  - Forwards user actions to services                                         │
│  - Purely presentational - no business logic                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CHAT SERVICE                                    │
│  - Orchestrates message sending via SerdesAI Agent                           │
│  - Maps agent events to clean StreamEvents for UI                            │
│  - Coordinates with collaborating services                                   │
│  - Handles cancellation                                                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
            ┌─────────────────────────┼─────────────────────────┐
            │                         │                         │
            ▼                         ▼                         ▼
┌───────────────────┐    ┌───────────────────┐    ┌───────────────────┐
│  ProfileService   │    │ ConversationSvc   │    │    McpService     │
│                   │    │                   │    │                   │
│ • Model config    │    │ • Load history    │    │ • Get toolsets    │
│ • System prompt   │    │ • Save messages   │    │ • Tool execution  │
│ • Parameters      │    │ • Manage metadata │    │                   │
│ • Context limit   │    │                   │    │                   │
└───────────────────┘    └───────────────────┘    └───────────────────┘
            │                                                 │
            ▼                                                 │
┌───────────────────┐                                         │
│  SecretsService   │                                         │
│                   │                                         │
│ • API keys        │                                         │
│ • OAuth tokens    │                                         │
└───────────────────┘                                         │
                                      │                       │
                                      ▼                       │
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SERDES-AI AGENT                                    │
│                                                                              │
│  • Applies HistoryProcessor (context compression)                            │
│  • Sends request to LLM via Model                                            │
│  • Handles tool calls with retry logic  ◄────────────────────────────────────┘
│  • Streams events back to ChatService                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Service Responsibilities

### Chat View (UI Layer)

**Responsibility:** Render UI and forward user actions. No business logic.

| Owns | Delegates To |
|------|--------------|
| Message bubble rendering | - |
| Input field state | - |
| Button enable/disable | - |
| Scroll behavior | - |
| Sending messages | ChatService |
| Loading conversations | ConversationService |
| Managing conversations | ConversationService |
| Profile/model info | ProfileService |

### ChatService

**Responsibility:** Orchestrate LLM interactions via SerdesAI Agent.

| Owns | Collaborates With |
|------|-------------------|
| Agent building/configuration | ProfileService, SecretsService |
| Stream event mapping | SerdesAI |
| Cancellation handling | - |
| Message persistence timing | ConversationService |
| Tool attachment | McpService |

### ConversationService

**Responsibility:** Persist and retrieve conversations.

| Owns | Storage |
|------|---------|
| Conversation CRUD | ~/Library/Application Support/PersonalAgent/conversations/ |
| Message history | JSON files per conversation |
| Metadata (title, timestamps) | Embedded in conversation files |

### ProfileService

**Responsibility:** Manage model profiles and settings.

| Owns | Storage |
|------|---------|
| Profile CRUD | ~/Library/Application Support/PersonalAgent/profiles/ |
| Model configuration | provider, model_id, base_url |
| Parameters | temperature, max_tokens, thinking |
| System prompts | Per-profile |
| Context limits | Per-profile |

### McpService

**Responsibility:** Manage MCP server connections and provide toolsets.

| Owns | Provides |
|------|----------|
| MCP lifecycle (start/stop) | - |
| Toolset management | `get_toolsets() → Vec<McpToolset>` |
| Tool execution | Via toolsets attached to agent |

### SecretsService

**Responsibility:** Secure credential storage.

| Owns | Storage |
|------|---------|
| API keys | Encrypted files |
| OAuth tokens | With refresh support |
| Keyfile reading | Runtime access |

---

## Data Flow: Send Message

### Sequence

```
User                Chat View           ChatService         Collaborators        SerdesAI
  │                     │                    │                    │                 │
  │─── Type message ───►│                    │                    │                 │
  │─── Click Send ─────►│                    │                    │                 │
  │                     │                    │                    │                 │
  │                     │── send_message() ─►│                    │                 │
  │                     │                    │                    │                 │
  │                     │                    │── load(conv_id) ──►│ Conversation    │
  │                     │                    │◄── Conversation ───│ Service         │
  │                     │                    │                    │                 │
  │                     │                    │── get_default() ──►│ Profile         │
  │                     │                    │◄── ModelProfile ───│ Service         │
  │                     │                    │                    │                 │
  │                     │                    │── get(api_key) ───►│ Secrets         │
  │                     │                    │◄── "sk-..." ───────│ Service         │
  │                     │                    │                    │                 │
  │                     │                    │── get_toolsets() ─►│ MCP             │
  │                     │                    │◄── Vec<Toolset> ───│ Service         │
  │                     │                    │                    │                 │
  │                     │                    │── append_message() ►│ Conversation   │
  │                     │                    │   (user message)    │ Service         │
  │                     │                    │                    │                 │
  │                     │                    │─── Build Agent ────────────────────►│
  │                     │                    │    • ModelConfig                     │
  │                     │                    │    • HistoryProcessor                │
  │                     │                    │    • Toolsets                        │
  │                     │                    │                                      │
  │                     │                    │─── AgentStream::new() ─────────────►│
  │                     │                    │    with message_history              │
  │                     │                    │                                      │
  │                     │◄─ StreamHandle ────│                                      │
  │                     │                    │                                      │
  │◄── Add user bubble ─│                    │                                      │
  │◄── Add placeholder ─│                    │                                      │
  │                     │                    │                                      │
  │                     │                    │◄──────── TextDelta ─────────────────│
  │                     │◄─ TextDelta ───────│                                      │
  │◄── Append text ─────│                    │                                      │
  │                     │                    │                                      │
  │                     │                    │◄──────── ThinkingDelta ─────────────│
  │                     │◄─ ThinkingDelta ───│                                      │
  │◄── Append thinking ─│                    │                                      │
  │                     │                    │                                      │
  │                     │                    │◄──────── ToolStart ─────────────────│
  │                     │◄─ ToolStart ───────│         (agent executes tool)       │
  │◄── Show indicator ──│                    │◄──────── ToolComplete ──────────────│
  │                     │◄─ ToolComplete ────│                                      │
  │◄── Update indicator │                    │                                      │
  │                     │                    │                                      │
  │                     │                    │◄──────── RunComplete ───────────────│
  │                     │◄─ Complete ────────│                                      │
  │◄── Finalize bubble ─│                    │                                      │
  │                     │                    │                                      │
  │                     │                    │── append_message() ─►│ Conversation  │
  │                     │                    │   (assistant msg)    │ Service        │
  │                     │                    │                    │                 │
```

### StreamEvent Types

| SerdesAI Event | ChatService Maps To | UI Action |
|----------------|--------------------|-----------| 
| `RunStarted` | `Started` | Add placeholder with cursor |
| `TextDelta { text }` | `TextDelta { content }` | Append to bubble |
| `ThinkingDelta { text }` | `ThinkingDelta { content }` | Append to thinking section |
| `ToolCallStarted { id, name }` | `ToolStart { id, name }` | Show tool indicator |
| `ToolCallCompleted { id, name, result }` | `ToolComplete { id, name, success, summary }` | Update indicator |
| `RunCompleted { response }` | `Complete { text, thinking, tool_calls }` | Finalize bubble |
| `Error { error }` | `Error { message, retryable }` | Show error |
| (drop stream) | `Cancelled { partial_text, partial_thinking }` | Show partial + marker |

---

## Data Flow: Cancel Streaming

```
User                Chat View           ChatService         SerdesAI
  │                     │                    │                 │
  │─── Click Stop ─────►│                    │                 │
  │                     │── cancel(handle) ─►│                 │
  │                     │                    │── drop stream ─►│
  │                     │                    │                 │
  │                     │◄─ Cancelled ───────│                 │
  │                     │   { partial_text,  │                 │
  │                     │     partial_think} │                 │
  │                     │                    │                 │
  │◄── Show partial ────│                    │                 │
  │    + [cancelled]    │                    │                 │
  │                     │                    │                 │
  │                     │                    │── append_message() ─► ConversationSvc
  │                     │                    │   (partial + marker)
```

---

## Data Flow: Load Conversation

```
User                Chat View           ConversationSvc     ProfileService
  │                     │                    │                    │
  │─── Select from ────►│                    │                    │
  │    dropdown         │                    │                    │
  │                     │── load(id) ───────►│                    │
  │                     │◄── Conversation ───│                    │
  │                     │    { messages,     │                    │
  │                     │      profile_id }  │                    │
  │                     │                    │                    │
  │                     │── get(profile_id) ─────────────────────►│
  │                     │◄── ModelProfile ───────────────────────│
  │                     │                    │                    │
  │◄── Clear chat ──────│                    │                    │
  │◄── Render messages ─│                    │                    │
  │◄── Update model ────│                    │                    │
  │    label            │                    │                    │
```

---

## Data Flow: New Conversation

```
User                Chat View           ConversationSvc     ProfileService
  │                     │                    │                    │
  │─── Click [+] ──────►│                    │                    │
  │                     │── get_default() ───────────────────────►│
  │                     │◄── ModelProfile ───────────────────────│
  │                     │                    │                    │
  │                     │── create() ───────►│                    │
  │                     │◄── Conversation ───│                    │
  │                     │    { id, title }   │                    │
  │                     │                    │                    │
  │◄── Clear chat ──────│                    │                    │
  │◄── Show edit field ─│                    │                    │
  │◄── Update dropdown ─│                    │                    │
  │                     │                    │                    │
  │─── Type title ─────►│                    │                    │
  │─── Press Enter ────►│                    │                    │
  │                     │── update_metadata()►│                   │
  │                     │                    │                    │
  │◄── Hide edit field ─│                    │                    │
  │◄── Show dropdown ───│                    │                    │
```

---

## Agent Configuration

ChatService builds the SerdesAI Agent with configuration from multiple sources:

```rust
async fn build_agent(&self, profile: &ModelProfile, conversation: &Conversation) -> Result<Agent> {
    // 1. Model configuration from profile + secrets
    let api_key = self.secrets_service.get(&SecretKey::profile_api_key(profile.id))?;
    
    let mut model_config = ModelConfig::new(&profile.model_spec())
        .with_api_key(api_key);
    
    if let Some(url) = &profile.base_url {
        model_config = model_config.with_base_url(url);
    }
    
    if profile.parameters.enable_thinking {
        model_config = model_config.with_thinking(profile.parameters.thinking_budget);
    }
    
    // 2. Build agent with configuration
    let mut builder = AgentBuilder::from_config(model_config)?
        .system_prompt(&profile.system_prompt);
    
    // 3. Model parameters
    if let Some(temp) = profile.parameters.temperature {
        builder = builder.temperature(temp);
    }
    if let Some(max) = profile.parameters.max_tokens {
        builder = builder.max_tokens(max);
    }
    
    // 4. Context management (compression)
    let context_limit = profile.context_limit.unwrap_or(128_000);
    builder = builder.history_processor(
        TruncateByTokens::new(context_limit).keep_first(true)
    );
    
    // 5. Retry configuration
    builder = builder
        .max_output_retries(3)
        .max_tool_retries(2);
    
    // 6. MCP toolsets
    for toolset in self.mcp_service.get_toolsets() {
        builder = builder.toolset(toolset);
    }
    
    Ok(builder.build())
}
```

---

## Context Management

SerdesAI's `HistoryProcessor` handles context compression before LLM calls:

| Processor | Configuration | Use Case |
|-----------|---------------|----------|
| `TruncateByTokens` | `profile.context_limit` | Primary - limit by token count |
| `keep_first(true)` | Always | Preserve system prompt |

### Flow

```
Conversation History (all messages)
         │
         ▼
┌─────────────────────────┐
│    HistoryProcessor     │
│  TruncateByTokens       │
│  (context_limit tokens) │
│  keep_first = true      │
└─────────────────────────┘
         │
         ▼
Compressed History (fits in context)
         │
         ▼
    LLM Request
```

---

## Error Handling

| Error Source | Handling | UI Result |
|--------------|----------|-----------|
| Network error | `Error { retryable: true }` | Show error, Send enabled |
| Auth error (401) | `Error { retryable: false }` | Show error, suggest settings |
| Rate limit (429) | `Error { retryable: true }` | Show error, Send enabled |
| Tool error | Agent retries up to `max_tool_retries` | May succeed on retry |
| Output validation | Agent retries up to `max_output_retries` | May succeed on retry |
| Max retries exceeded | `Error { retryable: false }` | Show error |
| User cancellation | `Cancelled` event | Show partial + marker |

---

## Message Persistence

| Event | Action | When |
|-------|--------|------|
| User sends message | Save user message | Before agent starts |
| Stream completes | Save assistant message | After Complete event |
| Stream cancelled | Save partial + "[cancelled]" | After Cancelled event |
| Stream errors | Don't save | User can retry |

---

## Future Enhancements

1. **Tool Indicators** - Visual display of tool execution in chat
2. **Usage Stats** - Token counts in message footer
3. **Context Size Display** - Show how much context is used
4. **Proper Cancellation** - SerdesAI Issue #6 for CancellationToken support
5. **Toolset Integration** - SerdesAI Issue #5 for `.toolset()` method
6. **Sandwich Strategy** - Protect important messages during compression
