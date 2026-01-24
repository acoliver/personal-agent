# Conversation Service Requirements

The Conversation Service manages conversation persistence, loading, and lifecycle.

---

## Responsibilities

- CRUD operations for conversations
- File I/O (read/write to disk)
- Message appending
- Title updates
- Conversation listing and sorting

---

## Storage Format

### File Structure

```
~/Library/Application Support/PersonalAgent/conversations/
├── 20250121193000001.jsonl        # Messages (append-only)
├── 20250121193000001.meta.json    # Metadata + context state
├── 20250121183000002.jsonl
├── 20250121183000002.meta.json
└── ...
```

### Messages File (.jsonl)

One JSON object per line, append-only:

```jsonl
{"role":"user","content":"What is Rust?","ts":"2025-01-21T19:30:00Z"}
{"role":"assistant","content":"Rust is a systems programming language...","ts":"2025-01-21T19:30:05Z","thinking":"Let me explain..."}
{"role":"user","content":"Tell me more","ts":"2025-01-21T19:31:00Z"}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| role | string | Yes | "user", "assistant", "system" |
| content | string | Yes | Message text |
| ts | ISO8601 | Yes | Timestamp |
| thinking | string | No | Assistant thinking content |
| tool_calls | array | No | Tool invocations |
| tool_results | array | No | Tool responses |

### Metadata File (.meta.json)

Small JSON, rewritten on updates:

```json
{
  "id": "a1b2c3d4-e5f6-...",
  "title": "Rust async discussion",
  "created_at": "2025-01-21T19:30:00Z",
  "updated_at": "2025-01-21T20:45:00Z",
  "profile_id": "uuid-of-profile",
  "message_count": 47,
  "context_state": {
    "strategy": "sandwich",
    "summary": "Discussed Rust async runtimes...",
    "summary_range": [5, 42],
    "compressed_at": "2025-01-21T20:30:00Z"
  }
}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| id | UUID | Yes | Unique identifier |
| title | string | No | User-visible title |
| created_at | ISO8601 | Yes | Creation timestamp |
| updated_at | ISO8601 | Yes | Last modification |
| profile_id | UUID | No | Associated profile |
| message_count | number | Yes | Cached count |
| context_state | object | No | Compression state |

---

## Service Interface

```rust
pub trait ConversationService: Send + Sync {
    /// Create a new conversation
    fn create(&self) -> Result<Conversation>;
    
    /// Load conversation by ID
    fn load(&self, id: Uuid) -> Result<Option<Conversation>>;
    
    /// Load all conversations (metadata only for listing)
    fn load_all_metadata(&self) -> Result<Vec<ConversationMetadata>>;
    
    /// Load full conversation with messages
    fn load_full(&self, id: Uuid) -> Result<Option<Conversation>>;
    
    /// Append a message (append to .jsonl)
    fn append_message(&self, id: Uuid, message: &Message) -> Result<()>;
    
    /// Update metadata (rewrite .meta.json)
    fn update_metadata(&self, id: Uuid, metadata: &ConversationMetadata) -> Result<()>;
    
    /// Update context state (part of metadata)
    fn update_context_state(&self, id: Uuid, state: &ContextState) -> Result<()>;
    
    /// Delete conversation (both files)
    fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Get message count without loading all messages
    fn message_count(&self, id: Uuid) -> Result<usize>;
}
```

---

## Operations

### Create Conversation

| Step | Action |
|------|--------|
| 1 | Generate UUID |
| 2 | Generate filename from current timestamp |
| 3 | Create empty .jsonl file |
| 4 | Create .meta.json with defaults |
| 5 | Return Conversation with empty messages |

### Append Message

| Step | Action |
|------|--------|
| 1 | Serialize message to JSON |
| 2 | Append line to .jsonl (with newline) |
| 3 | Update message_count in .meta.json |
| 4 | Update updated_at timestamp |

### Load Conversation

| Step | Action |
|------|--------|
| 1 | Read .meta.json |
| 2 | Read .jsonl line by line |
| 3 | Parse each line as Message |
| 4 | Combine into Conversation struct |

### Update Title

| Step | Action |
|------|--------|
| 1 | Read .meta.json |
| 2 | Update title field |
| 3 | Update updated_at |
| 4 | Write .meta.json |

### Delete Conversation

| Step | Action |
|------|--------|
| 1 | Delete .jsonl file |
| 2 | Delete .meta.json file |
| 3 | Return success |

---

## Error Handling

| Error | Handling |
|-------|----------|
| File not found | Return None (not error) |
| Parse error | Log and skip malformed line |
| Write error | Return error, don't corrupt |
| Disk full | Return error |

---

## Performance Considerations

| Scenario | Approach |
|----------|----------|
| Large conversation | Stream .jsonl, don't load all at once |
| Many conversations | Load metadata only for listing |
| Frequent appends | Append-only, no rewrite |
| Message count | Cache in metadata, update on append |

---

## Test Requirements

| ID | Test |
|----|------|
| CS-T1 | Create conversation creates both files |
| CS-T2 | Append message adds line to .jsonl |
| CS-T3 | Load returns correct message count |
| CS-T4 | Update title only rewrites .meta.json |
| CS-T5 | Delete removes both files |
| CS-T6 | Load handles missing files gracefully |
| CS-T7 | Malformed .jsonl line is skipped |
