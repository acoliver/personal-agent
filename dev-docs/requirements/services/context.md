# Context Management Service Requirements

The Context Service manages conversation context for LLM requests, including compression and token management.

---

## Canonical Terminology

- ContextState is defined in `dev-docs/requirements/data-models.md` and owned by ConversationService.
- ContextStrategy produces ContextState but does not persist it directly.


## Responsibilities

- Estimate token count for messages
- Determine when compression is needed
- Build context for LLM requests
- Compress/summarize middle messages
- Cache and manage summaries

---

## Strategy Interface

```rust
/// Pluggable strategy for managing conversation context
pub trait ContextStrategy: Send + Sync {
    /// Strategy name for display/config
    fn name(&self) -> &str;
    
    /// Number of messages to preserve at start
    fn preserve_top(&self) -> usize;
    
    /// Number of messages to preserve at end
    fn preserve_bottom(&self) -> usize;
    
    /// Threshold (0.0-1.0) of context usage to trigger compression
    fn compression_threshold(&self) -> f64;
    
    /// Check if compression is needed
    fn needs_compression(
        &self,
        messages: &[Message],
        max_context_tokens: usize,
    ) -> bool;
    
    /// Build context to send to model
    fn build_context(
        &self,
        messages: &[Message],
        cached_summary: Option<&ContextState>,
        max_context_tokens: usize,
    ) -> ContextResult;
    
    /// Compress messages into summary
    async fn compress(
        &self,
        messages: &[Message],
        model: &dyn ChatModel,
    ) -> Result<String>;
}
```

---

## Default Strategy: Sandwich

Preserves conversation beginning and end, compresses middle.

### Configuration

| Parameter | Default | Notes |
|-----------|---------|-------|
| preserve_top | 5 | Messages to keep at start |
| preserve_bottom | 5 | Messages to keep at end |
| compression_threshold | 0.7 | Trigger at 70% of context |

### Algorithm

```
IF token_count < (max_tokens * 0.7):
    RETURN full message history
    
IF message_count <= (preserve_top + preserve_bottom):
    RETURN full message history (not enough to compress)

top_messages = messages[0..preserve_top]
middle_messages = messages[preserve_top..message_count-preserve_bottom]
bottom_messages = messages[message_count-preserve_bottom..]

IF cached_summary covers middle_messages:
    USE cached_summary
ELSE:
    summary = compress(middle_messages)
    CACHE summary with range

RETURN [
    top_messages,
    system_message("[Earlier conversation summary: {summary}]"),
    bottom_messages
]
```

### Context Result

```rust
pub struct ContextResult {
    /// Messages to send to the model
    pub messages: Vec<Message>,
    
    /// Whether a summary was prepended
    pub summary_used: bool,
    
    /// Estimated token count
    pub tokens_used: usize,
    
    /// New context state to cache (if compression occurred)
    pub new_state: Option<ContextState>,
}
```

---

## Context State (Cached)

Stored in conversation's `.meta.json`:

```rust
pub struct ContextState {
    /// Strategy name that created this state
    pub strategy: String,
    
    /// Compressed summary of middle messages
    pub summary: String,

## Context Lifecycle

- ContextState is created when compression is triggered (token usage > threshold and enough messages).
- ContextState is updated whenever the middle section grows beyond summary_range.
- ContextState is reused only if summary_range matches preserve_top and preserve_bottom for current message count.
- ContextState is cleared when strategy changes or conversation is deleted.

    
    /// Range of message indices covered by summary [start, end)
    pub summary_range: (usize, usize),
    
    /// When compression was performed
    pub compressed_at: DateTime<Utc>,
}
```

### Cache Validity

Summary is valid if:
1. `summary_range.0 == preserve_top` (starts after top messages)
2. `summary_range.1 == message_count - preserve_bottom` (ends before bottom messages)

If new messages were added, the middle has grown and re-compression is needed.

---

## Token Estimation

### Approach

Simple character-based estimation (accurate enough for triggering):

```rust
fn estimate_tokens(messages: &[Message]) -> usize {
    messages.iter()
        .map(|m| m.content.len() / 4)  // ~4 chars per token
        .sum()
}
```

### Per-Model Context Limits

| Provider | Model | Context Window |
|----------|-------|----------------|
| Anthropic | claude-3-5-sonnet | 200,000 |
| Anthropic | claude-3-opus | 200,000 |
| OpenAI | gpt-4-turbo | 128,000 |
| OpenAI | gpt-4o | 128,000 |
| Google | gemini-pro | 32,000 |

Context limit should come from profile or models.dev data.

---

## Compression Prompt

```
Summarize this conversation excerpt concisely. Preserve:
- Key facts and decisions made
- Important context needed to continue the discussion
- User preferences or requirements mentioned
- Any commitments or action items

Do not include greetings or filler. Focus on information density.

Conversation:
---
{formatted_messages}
---
```

---

## Operations

### Check If Compression Needed

| Input | Output |
|-------|--------|

## Validation Rules

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| max_context_tokens | > 0 | VALIDATION_ERROR | Context limit must be positive |
| messages | At least 1 message | VALIDATION_ERROR | Conversation has no messages |

## Negative Test Cases

| ID | Scenario | Expected Result |
|----|----------|----------------|
| CX-NT1 | build_context with zero max_context_tokens | VALIDATION_ERROR, no context returned |
| CX-NT2 | compress with empty messages | VALIDATION_ERROR, no summary created |
| CX-NT3 | compression model failure | SERVICE_UNAVAILABLE, cached summary unchanged |

| messages, max_tokens | bool |

```rust
fn needs_compression(&self, messages: &[Message], max_tokens: usize) -> bool {
    let tokens = estimate_tokens(messages);
    let threshold = (max_tokens as f64 * self.compression_threshold) as usize;
    tokens > threshold && messages.len() > (self.preserve_top + self.preserve_bottom)
}
```

### Build Context

| Input | Output |
|-------|--------|
| messages, cached_state, max_tokens | ContextResult |

1. Check if compression needed
2. If not, return full history
3. If cached summary valid, use it
4. Otherwise, compress and return new state

### Compress Middle

| Input | Output |
|-------|--------|
| middle_messages, model | summary string |

1. Format messages for summarization
2. Call model with compression prompt
3. Return summary text

---

## Alternative Strategies (Future)

### Sliding Window

```rust
pub struct SlidingWindowStrategy {
    pub keep_last_n: usize,  // e.g., 20
}
```

Simple: just keep last N messages. Loses beginning context.

### Full History

```rust
pub struct FullHistoryStrategy;
```

Never compress. For models with huge context windows (200K+).

### Tree Summarization

```rust
pub struct TreeSummaryStrategy {
    pub chunk_size: usize,
    pub levels: usize,
}
```

Hierarchical summarization for very long conversations.

---

## Test Requirements

| ID | Test |
|----|------|
| CX-T1 | Under threshold returns full history |
| CX-T2 | Over threshold triggers compression |
| CX-T3 | Top 5 messages always preserved |
| CX-T4 | Bottom 5 messages always preserved |
| CX-T5 | Cached summary reused when valid |
| CX-T6 | Cache invalidated when middle grows |
| CX-T7 | Summary prepended as system message |
| CX-T8 | Token estimation reasonable accuracy |
