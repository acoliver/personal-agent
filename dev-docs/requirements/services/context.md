# Context Service Requirements

> **Status: Superseded by SerdesAI HistoryProcessor**
>
> Context compression is now handled by SerdesAI's built-in `HistoryProcessor` at runtime. This document is retained for reference and potential future use with `SummarizeHistory` processor, but **no custom ContextService implementation is needed** for the initial release.

---

## Current Approach: SerdesAI HistoryProcessor

ChatService configures context management when building the SerdesAI Agent:

```rust
// In ChatService.build_agent()
let context_limit = profile.parameters.context_limit.unwrap_or(128_000);

let agent = AgentBuilder::from_config(model_config)?
    .history_processor(
        TruncateByTokens::new(context_limit)
            .keep_first(true)  // Preserve system prompt
            .chars_per_token(4.0)
    )
    // ...
    .build();
```

### Available HistoryProcessors in SerdesAI

| Processor | Description | Use Case |
|-----------|-------------|----------|
| `TruncateByTokens` | Limits by estimated token count | **Primary - used by default** |
| `TruncateHistory` | Limits by message count | Simple truncation |
| `FilterHistory` | Removes specific message types | Clean up retries, tool returns |
| `SummarizeHistory` | Placeholder for LLM summarization | Future enhancement |
| `ChainedProcessor` | Combines multiple processors | Complex strategies |
| `FnProcessor` | Custom function | Custom logic |

### Why SerdesAI Over Custom Service?

| Custom Service (Old) | SerdesAI HistoryProcessor (New) |
|----------------------|--------------------------------|
| Separate service call | Integrated into agent runtime |
| Persist compression state | Stateless, recompute each time |
| Complex summarization | Simple truncation (for now) |
| Custom token counting | Built-in estimation |
| Tight coupling | Loose coupling via trait |

---

## Future: LLM-Based Summarization

SerdesAI's `SummarizeHistory` processor is a placeholder. When implemented, it could provide sandwich-style compression:

```rust
// Future: When SerdesAI SummarizeHistory is implemented
let agent = AgentBuilder::from_config(model_config)?
    .history_processor(
        SummarizeHistory::new(
            keep_recent: 10,
            threshold_tokens: 100_000,
        )
        .with_summarization_model("claude-3-haiku")
    )
    .build();
```

Until then, `TruncateByTokens` provides a simpler solution that:
- Keeps the system prompt (first message)
- Keeps most recent messages that fit
- Drops older messages when over limit

---

## Retained: Data Model

These types may be used for future summarization state:

```rust
/// Compression state (future use with SummarizeHistory)
pub struct ContextState {
    /// Strategy that created this state
    pub strategy: String,
    
    /// Compressed summary of omitted messages
    pub summary: String,
    
    /// Range of message indices covered [start, end)
    pub summary_range: (usize, usize),
    
    /// When compression was performed
    pub compressed_at: DateTime<Utc>,
}
```

This is stored in `ConversationMetadata.context_state` but is **not currently used**.

---

## Retained: Token Estimation Reference

For reference, here's how token estimation works:

```rust
// SerdesAI TruncateByTokens uses ~4 chars per token
fn estimate_tokens(text: &str) -> u64 {
    (text.len() as f64 / 4.0).ceil() as u64
}

// For more accuracy, could use tiktoken
fn estimate_tokens_tiktoken(text: &str) -> u64 {
    let encoding = tiktoken::get_encoding("cl100k_base");
    encoding.encode(text).len() as u64
}
```

---

## Retained: Default Context Limits

Reference for model context windows:

| Model | Context Window |
|-------|---------------|
| claude-3-opus | 200,000 |
| claude-3-sonnet | 200,000 |
| claude-sonnet-4 | 200,000 |
| gpt-4-turbo | 128,000 |
| gpt-4o | 128,000 |
| gpt-4 | 8,192 |
| gpt-3.5-turbo | 16,385 |
| Default | 100,000 |

These are used when `profile.parameters.context_limit` is not set.

---

## Migration Notes

### From Custom Strategy to HistoryProcessor

If you had custom context strategy code:

**Before (Custom Service):**
```rust
let context = context_service.build_context(
    &conversation.messages,
    conversation.metadata.context_state.as_ref(),
    profile.context_limit,
);

if let Some(state) = context.new_state {
    conversation_service.update_context_state(id, &state)?;
}
```

**After (SerdesAI):**
```rust
// Just configure the agent - it handles everything
let agent = AgentBuilder::from_config(model_config)?
    .history_processor(TruncateByTokens::new(profile.context_limit))
    .build();

// Pass full message history - agent compresses internally
let stream = AgentStream::new(&agent, prompt, (), RunOptions::new()
    .message_history(all_messages)
).await?;
```

---

## Future Sandwich Strategy (Optional)

If we want sandwich-style compression (protect first + last, summarize middle), we could implement a custom HistoryProcessor:

```rust
pub struct SandwichProcessor {
    keep_first: usize,
    keep_last: usize,
    max_tokens: u64,
}

#[async_trait]
impl<Deps: Send + Sync> HistoryProcessor<Deps> for SandwichProcessor {
    async fn process(
        &self,
        ctx: &RunContext<Deps>,
        messages: Vec<ModelRequest>,
    ) -> Vec<ModelRequest> {
        let total_tokens = estimate_total_tokens(&messages);
        
        if total_tokens <= self.max_tokens {
            return messages;
        }
        
        let n = messages.len();
        let first = &messages[..self.keep_first.min(n)];
        let last_start = n.saturating_sub(self.keep_last);
        let last = &messages[last_start..];
        
        // For now, just drop middle
        // Future: Generate summary using ctx
        let mut result = first.to_vec();
        result.extend(last.iter().cloned());
        result
    }
}
```

This is **not needed for initial release** - `TruncateByTokens` is sufficient.

---

## Test Requirements

Since we're using SerdesAI's built-in processors, tests focus on configuration:

| ID | Test |
|----|------|
| CX-T1 | ChatService configures HistoryProcessor with profile context_limit |
| CX-T2 | Default context limit used when not specified |
| CX-T3 | keep_first(true) preserves system prompt |
| CX-T4 | Agent handles long conversations without error |
| CX-T5 | Messages truncated when over limit |
