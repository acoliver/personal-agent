# Conversation Service Requirements

The Conversation Service manages conversation persistence, loading, and lifecycle. It provides the data layer for chat history. **This is a pure storage service** - it does not coordinate with ProfileService or other services.

---

## Responsibilities

- CRUD operations for conversations
- Message appending (append-only storage)
- Metadata updates (title, context state)
- Conversation listing and sorting
- File I/O coordination

**Note:** Conversations do NOT store which profile to use. The "selected profile" is a global app setting managed by ProfileService. ChatService coordinates fetching the current profile and passing the `model_id` when saving messages.

---

## Service Interface

```rust
pub trait ConversationService: Send + Sync {
    /// Create a new conversation with default title
    fn create(&self) -> Result<Conversation>;
    
    /// Load conversation by ID (metadata + messages)
    fn load(&self, id: Uuid) -> Result<Option<Conversation>>;
    
    /// Load all conversations (metadata only, for listing)
    fn list(&self) -> Result<Vec<ConversationMetadata>>;
    
    /// Append a message to conversation (append to .jsonl)
    fn append_message(&self, id: Uuid, message: &Message) -> Result<()>;
    
    /// Update conversation metadata (rewrite .meta.json)
    fn update_metadata(&self, id: Uuid, metadata: &ConversationMetadata) -> Result<()>;
    
    /// Update context compression state
    fn update_context_state(&self, id: Uuid, state: &ContextState) -> Result<()>;
    
    /// Rename conversation
    fn rename(&self, id: Uuid, title: &str) -> Result<()>;
    
    /// Delete conversation (both files)
    fn delete(&self, id: Uuid) -> Result<()>;
    
    /// Get message count without loading all messages
    fn message_count(&self, id: Uuid) -> Result<usize>;
}
```

---

## Data Model

### Conversation

```rust
pub struct Conversation {
    /// Unique identifier
    pub id: Uuid,
    
    /// Metadata (persisted in .meta.json)
    pub metadata: ConversationMetadata,
    
    /// Messages (persisted in .jsonl)
    pub messages: Vec<Message>,
}

impl Conversation {
    pub fn new() -> Self {
        let now = Utc::now();
        let id = Uuid::new_v4();
        
        Self {
            id,
            metadata: ConversationMetadata {
                id,
                title: Some(format!("New {}", now.format("%Y-%m-%d %H:%M"))),
                created_at: now,
                updated_at: now,
                message_count: 0,
                context_state: None,
            },
            messages: Vec::new(),
        }
    }
}
```

### Conversation Metadata

```rust
pub struct ConversationMetadata {
    /// Unique identifier
    pub id: Uuid,
    
    /// User-visible title (None = untitled)
    pub title: Option<String>,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,
    
    /// Cached message count
    pub message_count: usize,
    
    /// Context compression state (future use)
    pub context_state: Option<ContextState>,
}
```

**Note:** Conversations do NOT store `profile_id`. The profile is a global selection that applies to all conversations. Each assistant MESSAGE stores its `model_id` to record which model generated it.

### Message

```rust
pub struct Message {
    /// Message role
    pub role: MessageRole,
    
    /// Message content
    pub content: String,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Model that generated this message (assistant only)
    /// This is the model_id from the profile used at generation time
    pub model_id: Option<String>,
    
    /// Thinking content (assistant only, always stored even if not displayed)
    pub thinking: Option<String>,
    
    /// Tool calls made (assistant only)
    pub tool_calls: Option<Vec<ToolCallRecord>>,
    
    /// Tool results (tool role only)
    pub tool_results: Option<Vec<ToolResultRecord>>,
    
    /// Whether this message was from a cancelled stream (assistant only)
    pub cancelled: bool,
}

pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

pub struct ToolCallRecord {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolResultRecord {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

impl Message {
    pub fn user(content: &str) -> Self {
        Self {
            role: MessageRole::User,
            content: content.to_string(),
            timestamp: Utc::now(),
            model_id: None,
            thinking: None,
            tool_calls: None,
            tool_results: None,
            cancelled: false,
        }
    }
    
    pub fn assistant(content: &str, model_id: &str) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.to_string(),
            timestamp: Utc::now(),
            model_id: Some(model_id.to_string()),
            thinking: None,
            tool_calls: None,
            tool_results: None,
            cancelled: false,
        }
    }
    
    pub fn with_thinking(mut self, thinking: &str) -> Self {
        self.thinking = Some(thinking.to_string());
        self
    }
    
    pub fn with_tool_calls(mut self, calls: Vec<ToolCallRecord>) -> Self {
        self.tool_calls = Some(calls);
        self
    }
    
    pub fn as_cancelled(mut self) -> Self {
        self.cancelled = true;
        self
    }
}
```

**model_id Storage:** Each assistant message records which model generated it. This allows the UI to display the correct model label for each response, even if the user changed profiles mid-conversation. User messages have `model_id: None`.

### Context State

**Note:** Context compression is now handled by SerdesAI's `HistoryProcessor` at runtime rather than persisted. This field is retained for potential future use with `SummarizeHistory` processor but is currently optional/unused.

```rust
/// Compression state for context management (future use)
pub struct ContextState {
    /// Strategy name that created this state
    pub strategy: String,
    
    /// Compressed summary of middle messages
    pub summary: String,
    
    /// Range of message indices covered [start, end)
    pub summary_range: (usize, usize),
    
    /// When compression was performed
    pub compressed_at: DateTime<Utc>,
}
```

---

## Storage Format

### File Structure

```
~/Library/Application Support/PersonalAgent/conversations/
├── 20250121193000001.jsonl        # Messages (append-only)
├── 20250121193000001.meta.json    # Metadata
├── 20250121183000002.jsonl
├── 20250121183000002.meta.json
└── ...
```

### Filename Format

```
YYYYMMDDHHMMSS###.jsonl
YYYYMMDDHHMMSS###.meta.json

Where:
- YYYYMMDDHHMMSS = creation timestamp
- ### = random suffix for uniqueness
```

### Messages File (.jsonl)

One JSON object per line, append-only:

```jsonl
{"role":"user","content":"What is Rust?","ts":"2025-01-21T19:30:00Z"}
{"role":"assistant","content":"Rust is a systems programming language...","ts":"2025-01-21T19:30:05Z","model_id":"claude-sonnet-4-20250514","thinking":"Let me explain..."}
{"role":"user","content":"Tell me more","ts":"2025-01-21T19:31:00Z"}
{"role":"assistant","content":"Certainly!...","ts":"2025-01-21T19:31:10Z","model_id":"gpt-4o","tool_calls":[{"id":"tc1","name":"search","arguments":{"query":"rust"}}]}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| role | string | Yes | "user", "assistant", "system", "tool" |
| content | string | Yes | Message text |
| ts | ISO8601 | Yes | Timestamp |
| model_id | string | No | Model that generated response (assistant only) |
| thinking | string | No | Assistant thinking content (always stored) |
| tool_calls | array | No | Tool invocations |
| tool_results | array | No | Tool responses |
| cancelled | bool | No | True if stream was cancelled (default false) |

### Metadata File (.meta.json)

Small JSON, rewritten on updates:

```json
{
  "id": "a1b2c3d4-e5f6-...",
  "title": "Rust async discussion",
  "created_at": "2025-01-21T19:30:00Z",
  "updated_at": "2025-01-21T20:45:00Z",
  "message_count": 47,
  "context_state": null
}
```

**Note:** No `profile_id` in metadata. The profile is a global app setting, not per-conversation.

---

## Operations

### Create Conversation

| Step | Action |
|------|--------|
| 1 | Generate UUID |
| 2 | Generate filename from current timestamp + random suffix |
| 3 | Create empty .jsonl file |
| 4 | Create .meta.json with default title "New YYYY-MM-DD HH:MM" |
| 5 | Return Conversation with empty messages |

```rust
fn create(&self) -> Result<Conversation> {
    let conversation = Conversation::new();
    let base_path = self.file_path(&conversation.id);
    
    // Create empty messages file
    std::fs::File::create(format!("{}.jsonl", base_path))?;
    
    // Write metadata
    let meta_json = serde_json::to_string_pretty(&conversation.metadata)?;
    std::fs::write(format!("{}.meta.json", base_path), meta_json)?;
    
    Ok(conversation)
}
```

### Load Conversation

| Step | Action |
|------|--------|
| 1 | Find files by ID (scan directory or use index) |
| 2 | Read .meta.json |
| 3 | Read .jsonl line by line |
| 4 | Parse each line as Message |
| 5 | Combine into Conversation struct |

```rust
fn load(&self, id: Uuid) -> Result<Option<Conversation>> {
    let base_path = self.find_file_path(&id)?;
    
    let Some(base_path) = base_path else {
        return Ok(None);
    };
    
    // Read metadata
    let meta_content = std::fs::read_to_string(format!("{}.meta.json", base_path))?;
    let metadata: ConversationMetadata = serde_json::from_str(&meta_content)?;
    
    // Read messages
    let messages_content = std::fs::read_to_string(format!("{}.jsonl", base_path))?;
    let messages: Vec<Message> = messages_content
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            serde_json::from_str(line)
                .map_err(|e| log::warn!("Skipping malformed message: {}", e))
                .ok()
        })
        .collect();
    
    Ok(Some(Conversation { id, metadata, messages }))
}
```

### List Conversations

| Step | Action |
|------|--------|
| 1 | Scan conversations directory |
| 2 | Read only .meta.json files |
| 3 | Parse metadata |
| 4 | Sort by created_at descending (newest first) |
| 5 | Return Vec<ConversationMetadata> |

```rust
fn list(&self) -> Result<Vec<ConversationMetadata>> {
    let dir = self.conversations_dir();
    let mut conversations = Vec::new();
    
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().map(|e| e == "meta.json").unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(meta) = serde_json::from_str::<ConversationMetadata>(&content) {
                    conversations.push(meta);
                }
            }
        }
    }
    
    // Sort newest first
    conversations.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    
    Ok(conversations)
}
```

### Append Message

| Step | Action |
|------|--------|
| 1 | Serialize message to JSON (single line) |
| 2 | Open .jsonl in append mode |
| 3 | Write line with newline |
| 4 | Update message_count in .meta.json |
| 5 | Update updated_at timestamp |

```rust
fn append_message(&self, id: Uuid, message: &Message) -> Result<()> {
    let base_path = self.find_file_path(&id)?
        .ok_or(Error::NotFound)?;
    
    // Append to messages file
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(format!("{}.jsonl", base_path))?;
    
    let json = serde_json::to_string(message)?;
    writeln!(file, "{}", json)?;
    
    // Update metadata
    let mut metadata = self.load_metadata(&id)?;
    metadata.message_count += 1;
    metadata.updated_at = Utc::now();
    self.save_metadata(&base_path, &metadata)?;
    
    Ok(())
}
```

### Update Metadata

| Step | Action |
|------|--------|
| 1 | Validate metadata |
| 2 | Update updated_at timestamp |
| 3 | Write .meta.json atomically |

```rust
fn update_metadata(&self, id: Uuid, metadata: &ConversationMetadata) -> Result<()> {
    let base_path = self.find_file_path(&id)?
        .ok_or(Error::NotFound)?;
    
    let mut metadata = metadata.clone();
    metadata.updated_at = Utc::now();
    
    self.save_metadata(&base_path, &metadata)?;
    
    Ok(())
}

fn save_metadata(&self, base_path: &str, metadata: &ConversationMetadata) -> Result<()> {
    let path = format!("{}.meta.json", base_path);
    let json = serde_json::to_string_pretty(metadata)?;
    
    // Atomic write: write to temp, then rename
    let temp_path = format!("{}.tmp", path);
    std::fs::write(&temp_path, &json)?;
    std::fs::rename(&temp_path, &path)?;
    
    Ok(())
}
```

### Rename Conversation

| Step | Action |
|------|--------|
| 1 | Load current metadata |
| 2 | Update title field |
| 3 | Save metadata |

```rust
fn rename(&self, id: Uuid, title: &str) -> Result<()> {
    let base_path = self.find_file_path(&id)?
        .ok_or(Error::NotFound)?;
    
    let mut metadata = self.load_metadata(&id)?;
    metadata.title = Some(title.to_string());
    metadata.updated_at = Utc::now();
    
    self.save_metadata(&base_path, &metadata)?;
    
    Ok(())
}
```

### Delete Conversation

| Step | Action |
|------|--------|
| 1 | Find files by ID |
| 2 | Delete .jsonl file |
| 3 | Delete .meta.json file |
| 4 | Return success |

```rust
fn delete(&self, id: Uuid) -> Result<()> {
    let base_path = self.find_file_path(&id)?
        .ok_or(Error::NotFound)?;
    
    std::fs::remove_file(format!("{}.jsonl", base_path))?;
    std::fs::remove_file(format!("{}.meta.json", base_path))?;
    
    Ok(())
}
```

### Update Context State

| Step | Action |
|------|--------|
| 1 | Load current metadata |
| 2 | Set context_state field |
| 3 | Save metadata |

```rust
fn update_context_state(&self, id: Uuid, state: &ContextState) -> Result<()> {
    let base_path = self.find_file_path(&id)?
        .ok_or(Error::NotFound)?;
    
    let mut metadata = self.load_metadata(&id)?;
    metadata.context_state = Some(state.clone());
    metadata.updated_at = Utc::now();
    
    self.save_metadata(&base_path, &metadata)?;
    
    Ok(())
}
```

---

## Error Handling

| Error | Handling |
|-------|----------|
| File not found | Return None (load) or NotFound error (update/delete) |
| Parse error in .jsonl | Log warning, skip malformed line, continue |
| Parse error in .meta.json | Return error (metadata required) |
| Write error | Return error, don't corrupt existing file |
| Disk full | Return error |
| Permission denied | Return error |

---

## Performance Considerations

| Scenario | Approach |
|----------|----------|
| Large conversation | Stream .jsonl, don't load all at once for list |
| Many conversations | Load metadata only for listing (no messages) |
| Frequent appends | Append-only, no rewrite of messages |
| Message count | Cache in metadata, update on append |
| Finding files | Consider maintaining index file for large directories |

---

## UI Integration

### Chat View

| Action | Service Call |
|--------|--------------|
| Load conversation | `load(id)` |
| Send message | ChatService calls `append_message(id, user_msg)` before streaming |
| On assistant complete | ChatService calls `append_message(id, assistant_msg)` with model_id |
| On stream cancelled | ChatService calls `append_message(id, msg.as_cancelled())` with model_id |
| New conversation | `create()` |
| Rename | `rename(id, title)` |
| List for dropdown | `list()` |

**Note:** ChatService coordinates persistence timing. The UI doesn't call ConversationService directly for message operations - it uses ChatService.send_message() which handles both streaming and persistence. ChatService is responsible for passing the correct `model_id` from the current profile to the assistant message.

### History View

| Action | Service Call |
|--------|--------------|
| Show conversations | `list()` |
| Load conversation | `load(id)` then navigate |
| Delete conversation | `delete(id)` |

---

## Test Requirements

| ID | Test |
|----|------|
| CS-T1 | create() creates both .jsonl and .meta.json files |
| CS-T2 | create() uses correct filename format |
| CS-T3 | load() returns None for non-existent ID |
| CS-T4 | load() returns correct message count |
| CS-T5 | load() handles malformed .jsonl lines gracefully |
| CS-T6 | append_message() adds line to .jsonl |
| CS-T7 | append_message() updates message_count |
| CS-T8 | append_message() updates updated_at |
| CS-T9 | rename() only rewrites .meta.json |
| CS-T10 | delete() removes both files |
| CS-T11 | list() returns metadata sorted newest first |
| CS-T12 | list() doesn't load messages |
| CS-T13 | update_context_state() persists compression state |
| CS-T14 | Atomic write prevents corruption |
| CS-T15 | Cancelled message has cancelled=true flag |
| CS-T16 | as_cancelled() builder sets flag correctly |
| CS-T17 | Assistant message stores model_id |
| CS-T18 | User message has model_id=None |
| CS-T19 | model_id persisted in .jsonl |
| CS-T20 | Metadata has no profile_id field |
