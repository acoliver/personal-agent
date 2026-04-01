## Problem

Conversations are currently stored as monolithic JSON files (one file per conversation with all messages embedded). This has several scaling and feature problems:

1. **Every list operation loads every message of every conversation.** `ConversationServiceImpl::list()` reads and fully deserializes every `.json` file on disk just to show a conversation list. With 50 conversations averaging 100 messages, that's 5,000 message objects parsed to render a sidebar.

2. **Every message append rewrites the entire file.** Adding one assistant response to a 200-message conversation means serializing and writing all 200+ messages back to disk.

3. **No search capability exists.** There is no way to search conversation titles or message content. The upcoming popout mode (with a conversation sidebar) requires both title search and full-text content search with ranked results.

4. **No metadata-only queries.** The current `ConversationService` trait has no way to fetch just titles/dates/counts without loading full message arrays.

## Proposed Solution

Migrate conversation storage from JSON files to SQLite using `rusqlite` with FTS5 for full-text search.

### Why rusqlite + FTS5

- SQLite is the most tested software on earth -- proven on billions of devices
- `rusqlite` is the mature, well-maintained Rust binding (pure wrapper, no ORM)
- FTS5 is built into SQLite, supports BM25 ranking and snippet generation
- Single-file database at `~/Library/Application Support/PersonalAgent/personalagent.db`
- No server process, no network, fits the single-user desktop app model perfectly

### Database Schema

```sql
-- Conversations table (lightweight metadata for fast listing)
CREATE TABLE conversations (
    id          TEXT PRIMARY KEY,   -- UUID as text
    title       TEXT,
    profile_id  TEXT,               -- UUID of the model profile used
    created_at  TEXT NOT NULL,      -- ISO8601 timestamp
    updated_at  TEXT NOT NULL       -- ISO8601 timestamp
);

CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);

-- Messages table (one row per message, append-only in practice)
CREATE TABLE messages (
    id              TEXT PRIMARY KEY,   -- UUID as text
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,      -- 'user', 'assistant', 'system'
    content         TEXT NOT NULL,
    thinking_content TEXT,              -- assistant thinking (nullable)
    model_id        TEXT,              -- which model generated this (nullable, assistant only)
    created_at      TEXT NOT NULL,      -- ISO8601 timestamp
    seq             INTEGER NOT NULL    -- ordering within conversation (0-based)
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id, seq);

-- FTS5 virtual table for full-text search across titles and message content
CREATE VIRTUAL TABLE search_index USING fts5(
    conversation_id,   -- not searched, used for joins
    title,             -- conversation title (weighted higher)
    content,           -- message content
    content_rowid=rowid
);
```

### FTS5 Index Strategy

The search index is a **denormalized view** optimized for search. Each row represents one searchable unit:

- When a conversation is created/renamed, upsert a row with the title and empty content
- When a message is appended, insert a row with the conversation_id, title, and message content
- This means a conversation with 50 messages has ~50 rows in the FTS index

Search query for the sidebar:

```sql
-- Search with title matches ranked higher via bm25() weights
-- bm25() weights: conversation_id=0 (not ranked), title=10.0, content=1.0
SELECT DISTINCT
    c.id,
    c.title,
    c.updated_at,
    snippet(search_index, 2, '[', ']', '...', 32) AS match_context,
    CASE
        WHEN search_index.title MATCH :query THEN 'title'
        ELSE 'content'
    END AS match_type,
    bm25(search_index, 0.0, 10.0, 1.0) AS rank
FROM search_index
JOIN conversations c ON c.id = search_index.conversation_id
WHERE search_index MATCH :query
ORDER BY
    match_type ASC,  -- title matches first
    rank ASC         -- bm25 returns negative values, lower = better match
LIMIT 50;
```

### ConversationService Trait Changes

```rust
#[async_trait]
pub trait ConversationService: Send + Sync {
    // --- Existing (signatures unchanged) ---
    async fn create(&self, title: Option<String>, model_profile_id: Uuid) -> ServiceResult<Conversation>;
    async fn load(&self, id: Uuid) -> ServiceResult<Conversation>;
    async fn delete(&self, id: Uuid) -> ServiceResult<()>;
    async fn rename(&self, id: Uuid, new_title: String) -> ServiceResult<()>;
    async fn set_active(&self, id: Uuid) -> ServiceResult<()>;
    async fn get_active(&self) -> ServiceResult<Option<Uuid>>;

    // --- Modified (lighter return types or new params) ---
    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<ConversationMetadata>>;

    async fn add_message(
        &self,
        conversation_id: Uuid,
        message: Message,
    ) -> ServiceResult<Message>;

    async fn get_messages(&self, conversation_id: Uuid) -> ServiceResult<Vec<Message>>;

    // --- New ---
    async fn search(&self, query: &str) -> ServiceResult<Vec<SearchResult>>;

    async fn message_count(&self, conversation_id: Uuid) -> ServiceResult<usize>;

    async fn update_title(&self, id: Uuid, title: String) -> ServiceResult<()>;
}
```

New types:

```rust
pub struct ConversationMetadata {
    pub id: Uuid,
    pub title: Option<String>,
    pub profile_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub last_message_preview: Option<String>,  // first ~100 chars of last message
}

pub struct SearchResult {
    pub conversation_id: Uuid,
    pub title: String,
    pub match_type: SearchMatchType,    // Title or Content
    pub match_context: String,          // snippet with surrounding context
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

pub enum SearchMatchType {
    Title,
    Content,
}
```

### Migration Strategy

On first launch after update:

1. Check if `personalagent.db` exists. If yes, skip migration.
2. Scan `~/Library/Application Support/PersonalAgent/conversations/` for `.json` files.
3. For each file:
   a. Parse the `Conversation` struct
   b. INSERT into `conversations` table
   c. INSERT each message into `messages` table
   d. INSERT search index rows
4. After successful migration, rename the `conversations/` directory to `conversations.migrated/` (keep as backup, don't delete).
5. Log the migration result (count of conversations migrated, any parse failures).

Parse failures for individual files should be logged and skipped, not abort the entire migration.

### Threading Model

SQLite requires all access from a single connection (or use WAL mode with careful connection pooling). For a single-user desktop app:

- Use WAL mode for better concurrent read performance
- Wrap the connection in a dedicated thread with a channel-based command interface, OR
- Use `tokio::task::spawn_blocking` for all database calls (simpler, sufficient for this workload)
- The `ConversationServiceImpl` holds an `Arc<Mutex<Connection>>` or similar

### What This Enables

- The popout mode sidebar can list conversations with a single fast metadata query
- Title and content search with ranked results and context snippets
- Message append is one INSERT, not a full file rewrite
- Conversation delete cascades cleanly
- Future: conversation export, statistics, date range filtering all become trivial SQL queries
