# Issue #56: SQLite Conversation Storage with FTS5 Search

## Technical & Functional Specification

### Purpose

Replace the JSON-file-per-conversation storage with a single SQLite database using rusqlite and FTS5. This is the foundation for two downstream features:

- **#57 Popout mode** — needs `list_metadata()` for fast sidebar listing and `search()` for full-text search
- **#87 Background streaming** — needs atomic single-row INSERT for concurrent message persistence from multiple active streams

### Scope

This is a **clean replacement**, not a migration. There are no production users — all existing conversations are test data. The old JSON storage code (`ConversationServiceImpl`, `ConversationStorage`) will be deleted and replaced entirely. No backward compatibility shims, no JSON-to-SQLite migration, no backup/rename of old directories.

---

## 1. Database Layer

### 1.1 Database File Location

Single file: `{data_local_dir}/PersonalAgent/personalagent.db`

Created on first launch if it doesn't exist. Schema applied immediately.

### 1.2 Connection Configuration

Applied on every connection open:

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
```

### 1.3 Schema

```sql
CREATE TABLE conversations (
    id              TEXT PRIMARY KEY,
    title           TEXT,
    profile_id      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    context_state   TEXT
);

CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);
CREATE INDEX idx_conversations_profile ON conversations(profile_id);

CREATE TABLE messages (
    id                  INTEGER PRIMARY KEY,
    conversation_id     TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role                TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content             TEXT NOT NULL,
    thinking_content    TEXT,
    model_id            TEXT,
    tool_calls          TEXT,
    tool_results        TEXT,
    created_at          TEXT NOT NULL,
    seq                 INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_messages_ordering ON messages(conversation_id, seq);
CREATE INDEX idx_messages_conversation_ts ON messages(conversation_id, created_at);

CREATE VIRTUAL TABLE search_index USING fts5(
    title,
    content,
    conversation_id UNINDEXED,
    message_rowid UNINDEXED
);

-- FTS sync triggers
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO search_index(title, content, conversation_id, message_rowid)
    SELECT c.title, NEW.content, NEW.conversation_id, NEW.id
    FROM conversations c WHERE c.id = NEW.conversation_id;
END;

CREATE TRIGGER messages_au AFTER UPDATE OF content ON messages BEGIN
    DELETE FROM search_index WHERE message_rowid = OLD.id;
    INSERT INTO search_index(title, content, conversation_id, message_rowid)
    SELECT c.title, NEW.content, NEW.conversation_id, NEW.id
    FROM conversations c WHERE c.id = NEW.conversation_id;
END;

CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
    DELETE FROM search_index WHERE message_rowid = OLD.id;
END;

CREATE TRIGGER conversations_title_au AFTER UPDATE OF title ON conversations BEGIN
    UPDATE search_index SET title = NEW.title WHERE conversation_id = NEW.id;
END;

CREATE TRIGGER conversations_ad AFTER DELETE ON conversations BEGIN
    DELETE FROM search_index WHERE conversation_id = OLD.id;
END;
```

Schema version tracked via `PRAGMA user_version = 1`. Future schema changes increment this and add migration logic.

**Note on FTS5 table:** This is a **normal** (content-storing) FTS5 table, not external-content. Direct `INSERT`/`DELETE` against `search_index` works correctly. The triggers use standard DML, not FTS5 special commands.

**Note on `seq` column:** `seq` is an internal DB column used solely for deterministic message ordering within a conversation. It is **not** exposed on the `Message` struct or returned through any service API. Callers see messages in `seq` order via `get_messages()` (`ORDER BY seq ASC`) but have no visibility into the value itself. If downstream issues (e.g., #87 stream reconciliation) need to expose a stable message identity or ordering token, `seq` can be surfaced on `Message` at that time.

### 1.4 FTS5 Design Rationale

Title is **denormalized** into each FTS row (one per message). This enables weighted BM25 scoring: title matches score 10x higher than content matches. The title-update trigger keeps all rows in sync when a conversation is renamed. The title-update trigger is O(messages_in_conversation). For a single-user desktop app, even conversations with hundreds of messages complete in milliseconds.

**Accepted limitation:** FTS rows are created only by the `messages_ai` trigger on message INSERT. A conversation with zero messages has no FTS rows and will not appear in search results, even if its title matches. This is accepted behavior: `search()` returns only conversations that have at least one message. Empty conversations are still visible via `list_metadata()` but not via `search()`.

### 1.5 Threading Model: Closure-Based DB Handle

A single dedicated OS thread owns the `rusqlite::Connection`. All access goes through a `DbHandle` that sends closures to the thread and awaits results via oneshot channels.

```
Caller (async/tokio) ──send closure──► DB Thread (std::thread, blocking recv)
                      ◄──oneshot result──
```

**Why this over `Arc<Mutex<Connection>>`:**
- `rusqlite::Connection` is not `Send` — can't share across threads with a Mutex
- Single-writer serialization is natural — no contention, no deadlocks
- Async callers stay unblocked — they await the oneshot, not a mutex
- Critical for #87: concurrent background streams queue their INSERTs safely

**Concrete contract:**

```rust
/// A boxed closure sent to the DB thread. Each closure captures its own
/// oneshot sender internally, so the job type is a simple FnOnce.
type DbJob = Box<dyn FnOnce(&rusqlite::Connection) + Send + 'static>;

pub struct DbHandle {
    sender: std::sync::mpsc::Sender<DbJob>,
}

impl DbHandle {
    /// Execute a closure on the DB thread and return the result.
    /// The closure runs with exclusive access to the Connection.
    pub async fn execute<F, R>(&self, f: F) -> Result<R, ServiceError>
    where
        F: FnOnce(&rusqlite::Connection) -> Result<R, rusqlite::Error> + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let job: DbJob = Box::new(move |conn| {
            let result = f(conn);
            let _ = tx.send(result);
        });
        self.sender.send(job).map_err(|_| {
            ServiceError::Storage("DB thread has shut down".into())
        })?;
        rx.await
            .map_err(|_| ServiceError::Storage("DB thread dropped response".into()))?
            .map_err(|e| ServiceError::Storage(format!("SQLite error: {e}")))
    }
}

impl Clone for DbHandle {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}
```

**DB thread lifecycle:**

```rust
fn spawn_db_thread(db_path: &Path) -> Result<DbHandle, ServiceError> {
    let (tx, rx) = std::sync::mpsc::channel::<DbJob>();
    let (init_tx, init_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let path = db_path.to_owned();
    std::thread::Builder::new()
        .name("db-worker".into())
        .spawn(move || {
            let conn = match rusqlite::Connection::open(&path) {
                Ok(c) => c,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("failed to open database: {e}")));
                    return;
                }
            };
            match initialize_schema(&conn) {
                Ok(()) => { let _ = init_tx.send(Ok(())); }
                Err(e) => {
                    let _ = init_tx.send(Err(format!("failed to initialize schema: {e}")));
                    return;
                }
            }
            while let Ok(job) = rx.recv() {
                let _ = std::panic::catch_unwind(
                    std::panic::AssertUnwindSafe(|| job(&conn))
                );
                // If job panicked, its captured oneshot tx was dropped → caller gets RecvError
            }

            // Connection dropped here — SQLite flushes WAL.
        })
        .map_err(|e| ServiceError::Storage(format!("failed to spawn DB thread: {e}")))?;
    // Block until the DB thread reports init success or failure.
    init_rx.blocking_recv()
        .map_err(|_| ServiceError::Storage("DB thread died during init".into()))?
        .map_err(|e| ServiceError::Storage(e))?;
    Ok(DbHandle { sender: tx })
}
```

**Shutdown:** When all `DbHandle` clones are dropped, `sender` is dropped, `rx.recv()` returns `Err(RecvError)`, the loop exits, and `Connection` is dropped (WAL flushed). No explicit shutdown command needed.

**Panic safety:** Each job is wrapped in `catch_unwind`. If a job panics, the panic is caught, logged at error level, and the worker loop continues. The panicked job's oneshot sender is consumed and dropped without sending, so the caller receives `RecvError` mapped to `ServiceError::Storage("DB thread dropped response")`. This prevents one bad query from killing persistence for the entire session. The DB thread only dies if `rx.recv()` returns `Err(RecvError)` (all senders dropped), which is the normal shutdown path. Closures should still avoid panicking — `catch_unwind` is a safety net, not an expected control-flow mechanism.

**Error mapping from rusqlite to ServiceError:**

| `rusqlite::Error` | `ServiceError` |
|---|---|
| `QueryReturnedNoRows` | `ServiceError::NotFound(...)` |
| `SqliteFailure` with `CONSTRAINT_FOREIGNKEY` | `ServiceError::NotFound("conversation not found")` |
| `SqliteFailure` with `CONSTRAINT_CHECK` | `ServiceError::Validation(...)` |
| All other variants | `ServiceError::Storage(format!("{e}"))` |

---

## 2. Service Trait Changes

### 2.1 `ConversationService` Trait (Revised)

```rust
#[async_trait]
pub trait ConversationService: Send + Sync {
    // Unchanged signatures
    async fn create(&self, title: Option<String>, model_profile_id: Uuid)
        -> ServiceResult<Conversation>;
    async fn load(&self, id: Uuid) -> ServiceResult<Conversation>;
    async fn delete(&self, id: Uuid) -> ServiceResult<()>;
    async fn rename(&self, id: Uuid, new_title: String) -> ServiceResult<()>;
    async fn set_active(&self, id: Uuid) -> ServiceResult<()>;
    async fn get_active(&self) -> ServiceResult<Option<Uuid>>;
    async fn get_messages(&self, conversation_id: Uuid) -> ServiceResult<Vec<Message>>;
    async fn update(&self, id: Uuid, title: Option<String>, model_profile_id: Option<Uuid>)
        -> ServiceResult<Conversation>;

    // REPLACED: list() → list_metadata()
    async fn list_metadata(&self, limit: Option<usize>, offset: Option<usize>)
        -> ServiceResult<Vec<ConversationMetadata>>;

    // REPLACED: add_user_message() + add_assistant_message() → add_message()
    async fn add_message(&self, conversation_id: Uuid, message: Message)
        -> ServiceResult<Message>;

    // NEW
    async fn search(&self, query: &str, limit: Option<usize>, offset: Option<usize>)
        -> ServiceResult<Vec<SearchResult>>;
    async fn message_count(&self, conversation_id: Uuid) -> ServiceResult<usize>;
    async fn update_context_state(&self, id: Uuid, state: &ContextState) -> ServiceResult<()>;
    async fn get_context_state(&self, id: Uuid) -> ServiceResult<Option<ContextState>>;
}
```

**Pagination defaults and bounds:** For `list_metadata` and `search`, `limit` and `offset` are converted to SQL `LIMIT :limit OFFSET :offset`. Defaults: `limit` defaults to 100 if `None`, `offset` defaults to 0 if `None`. Values are clamped to reasonable bounds: limit is clamped to max 1000, offset is clamped to min 0 (negative values treated as 0).

**Removed methods:**
- `list()` — replaced by `list_metadata()` which returns lightweight metadata without loading messages
- `add_user_message()` — replaced by unified `add_message()`
- `add_assistant_message()` — replaced by unified `add_message()`

### 2.2 Unified `add_message` Rationale

The old API had separate `add_user_message(id, content)` and `add_assistant_message(id, content, thinking)`. These are replaced by a single `add_message(id, Message)` because:

1. The `Message` struct already carries role, content, and thinking_content — the separate methods just reconstructed what the caller already had
2. For #87 (background streaming), the stream task builds a `Message` and needs to persist it to any conversation_id — a generic append is the natural API
3. Reduces the trait surface from 2 methods to 1

### 2.3 New Types

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
    pub match_type: SearchMatchType,
    pub match_context: String,
    pub score: f64,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

pub enum SearchMatchType {
    Title,
    Content,
}

/// Forward-looking context state for conversation compression/summarization.
pub struct ContextState {
    pub strategy: Option<String>,
    pub summary: Option<String>,
    pub visible_range: Option<(usize, usize)>,
}
```

**Nullable title handling:** The `conversations.title` column is nullable TEXT. `ConversationMetadata.title` is `Option<String>` — it correctly preserves the `NULL` as `None`. `SearchResult.title` is `String` (non-optional) because search results always need a displayable title; when mapping from DB, a `NULL` title is rendered as `"Untitled"` (matching current presenter behavior for display).

**ContextState serialization:** `ContextState` is stored in the `context_state` TEXT column as JSON via `serde_json::to_string()` / `serde_json::from_str()`. No schema version is needed — this is an internal-only blob that evolves with the Rust struct definition. If the struct changes (fields added/removed), deserialization of old blobs will fail and `get_context_state()` treats the failure as `None` (equivalent to no saved state). Deserialization failures are logged at `warn` level with the conversation ID and parse error before returning `None`. Callers are expected to handle `None` gracefully, which they already must since context state is optional.

---

## 3. Model Changes

### 3.1 Message Struct Extensions

Add optional fields to `Message` for DB storage. These are not used by existing code but are stored when present:

```rust
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub thinking_content: Option<String>,
    pub timestamp: DateTime<Utc>,
    // New fields
    pub model_id: Option<String>,
    pub tool_calls: Option<String>,    // JSON blob
    pub tool_results: Option<String>,  // JSON blob
}
```

Existing constructors (`Message::user()`, `Message::assistant()`, etc.) initialize new fields to `None`.

### 3.2 Conversation Struct

The `Conversation` struct is unchanged. `load()` continues to return a full `Conversation` with all messages populated. The `messages: Vec<Message>` field stays — it's used by `ChatServiceImpl::build_llm_messages()` to construct the context window.

---

## 4. Search

### 4.1 FTS5 Query

Search aggregates per-message FTS hits to conversation level. Title matches rank higher than content matches due to the 10x BM25 weight on the `title` FTS column (see §1.4):

```sql
WITH hits AS (
    SELECT
        si.conversation_id,
        bm25(search_index, 10.0, 1.0) AS score,
        snippet(search_index, 1, '[', ']', '...', 24) AS ctx
    FROM search_index si
    WHERE search_index MATCH :fts_query
)
SELECT
    c.id, c.title, c.updated_at,
    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
    MIN(h.score) AS rank,
    (SELECT h2.ctx FROM hits h2
     WHERE h2.conversation_id = c.id
     ORDER BY h2.score ASC LIMIT 1) AS match_context
FROM hits h
JOIN conversations c ON c.id = h.conversation_id
GROUP BY c.id, c.title, c.updated_at
ORDER BY rank ASC, c.updated_at DESC
LIMIT :limit OFFSET :offset;
```

**`SearchMatchType` classification is determined post-query in Rust**, not in SQL. The SQL query returns only the BM25 rank — title matches already sort higher due to the 10x weight. After fetching results, the service checks whether the conversation title contains any of the search terms (case-insensitive). If so, `match_type` is `SearchMatchType::Title`; otherwise `SearchMatchType::Content`. This avoids the unreliable `instr(lower(si.title), lower(:raw_query))` approach, which breaks for multi-term queries where only a subset of terms appear in the title.

SearchMatchType classification is best-effort. For multi-term queries where terms appear in both title and content, the classification may not perfectly distinguish the primary match source. The BM25 ranking already handles ordering correctly regardless of classification accuracy.

### 4.2 Query Sanitization

User input is transformed to a safe FTS5 query before MATCH:

1. **Trim** — leading/trailing whitespace removed. If empty after trim, return empty results.
2. **Replace FTS5 structural operators with spaces** — replace characters that alter FTS5 query structure with a space: `*`, `(`, `)`, `{`, `}`, `^`, `~`, `:`. This ensures adjacent tokens separated only by operators become distinct terms (e.g., `foo(bar)` → `foo bar`).
3. **Tokenize** — split on whitespace into individual terms
4. **Quote each term** — wrap each term in double quotes. Inside the quotes, `+`, `-`, `"`, `#`, `.` are treated as literal characters by FTS5, preserving programming terms like `C++`, `C#`, `.NET`. Any embedded `"` within a term is escaped as `""` (FTS5 double-quote escaping).
5. **Join with implicit AND** — FTS5 default is AND between quoted terms
6. **Append prefix wildcard** — add `*` to the last term for prefix matching: `"hello" "world"*`

Example transforms:
- `hello world` → `"hello" "world"*`
- `C++ errors` → `"C++" "errors"*` (operators preserved inside quotes)
- `C# generics` → `"C#" "generics"*` (hash preserved inside quotes)
- `.NET framework` → `".NET" "framework"*` (dot preserved inside quotes)
- `"exact phrase"` → `"exact" "phrase"*` (quotes stripped and re-quoted per-term)
- `foo(bar)` → `"foo" "bar"*` (structural parens replaced with spaces, then tokenized into two terms)
- `` (empty) → no query executed, return `vec![]`

---

## 5. Key Implementation Details

### 5.1 Message Sequence Allocation

When `add_message` inserts a new message, `seq` is computed atomically, and both the INSERT and UPDATE are wrapped in an explicit transaction:

```sql
BEGIN;

INSERT INTO messages (conversation_id, role, content, thinking_content, model_id,
                      tool_calls, tool_results, created_at, seq)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
        COALESCE((SELECT MAX(seq) + 1 FROM messages WHERE conversation_id = ?1), 0));

UPDATE conversations SET updated_at = ?8 WHERE id = ?1;

COMMIT;
```

In Rust this is `conn.execute_batch()` or, more idiomatically, `let tx = conn.transaction()?; ... tx.commit()?;`.

Both statements execute in a single closure on the DB thread. The DB thread serializes all access, so there is no race on `MAX(seq)` even when multiple background streams are persisting messages to different (or the same) conversations. The explicit transaction ensures that a crash between the INSERT and the UPDATE cannot leave `conversations.updated_at` stale — either both succeed or neither does.

### 5.2 Active Conversation State

`active_id` remains in-memory only (`Mutex<Option<Uuid>>`) on `SqliteConversationService`, matching current behavior. It is not persisted to the database. On app restart, `restore_startup_conversation` picks the most-recently-updated conversation via `list_metadata(Some(1), Some(0))`.

### 5.3 Timestamp Format

All timestamps stored as **RFC 3339 / ISO 8601 UTC** text, e.g. `2026-04-03T17:33:52.000Z`. Serialized via `chrono::DateTime<Utc>::to_rfc3339_opts(chrono::SecondsFormat::Millis, true)`, which guarantees millisecond precision and the `Z` suffix (not `+00:00`). Parsed via `DateTime::parse_from_rfc3339()`. This matches the existing `chrono` usage throughout the codebase.

### 5.4 Message Timestamp Contract

`add_message` stores the caller-provided `Message.timestamp` as the `created_at` DB column value. It does **not** override the timestamp with `Utc::now()`. The caller is responsible for constructing the `Message` with the desired timestamp — the existing constructors (`Message::user()`, `Message::assistant()`, `Message::assistant_with_thinking()`, `Message::system()`) already set `timestamp` to `Utc::now()` at construction time.

On read, `get_messages()` maps the DB `created_at` column back to `Message.timestamp`. Round-trip fidelity is exact (RFC 3339 text serialization, per §5.3).

### 5.5 ChatServiceImpl Double-Load Pattern

`prepare_message_context` currently does: `load(id)` → `add_user_message(id, content)` → `load(id)` again. With the SQLite backend, this becomes: `load(id)` → `add_message(id, msg)` → `load(id)`. The double-load is a pre-existing inefficiency. It still works correctly with SQLite (the second load sees the just-inserted message). Optimizing it is out of scope for this issue.

### 5.5.1 Pre-existing bug: prepare_message_context orphaned conversation (out of scope)

`ChatServiceImpl::prepare_message_context` has a pre-existing bug in its fallback path. When `load(conversation_id)` fails, it creates a new conversation via `create(None, default_profile.id)` — but the new conversation's ID is discarded (stored in `_conversation`). The subsequent `add_user_message(conversation_id, content)` and `load(conversation_id)` still use the **original** (non-existent) `conversation_id`, not the newly created one. This means the fallback path creates an orphaned empty conversation and then fails when trying to add a message to the original ID. This bug is pre-existing in the current code and is **out of scope** for this issue.

### 5.6 updated_at Semantics

The following defines which operations bump `conversations.updated_at`:

| Operation | Bumps `updated_at`? | Notes |
|-----------|-------------------|-------|
| `create` | Yes | Sets both `created_at` and `updated_at` to `Utc::now()` |
| `rename` | Yes | Bumps `updated_at` to `Utc::now()` |
| `update` (profile change) | Yes | Bumps `updated_at` to `Utc::now()` |
| `add_message` | Yes | Bumps `updated_at` within the same transaction as the INSERT (see §5.1) |
| `update_context_state` | Yes | Bumps `updated_at` to `Utc::now()` |
| `load` | No | Read-only |
| `get_messages` | No | Read-only |
| `get_context_state` | No | Read-only |
| `list_metadata` | No | Read-only |
| `search` | No | Read-only |

---

## 6. Caller Updates

The ConversationService remains pure storage — it does not publish events. Event emission continues to be the callers' responsibility (presenters emit ConversationEvent after service calls, ChatServiceImpl emits ChatEvent during streaming). This pattern is unchanged from current behavior.

### 6.0 list_metadata SQL Query

```sql
SELECT
    c.id,
    c.title,
    c.profile_id,
    c.created_at,
    c.updated_at,
    (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
    (SELECT SUBSTR(m2.content, 1, 100)
     FROM messages m2
     WHERE m2.conversation_id = c.id
     ORDER BY m2.seq DESC
     LIMIT 1) AS last_message_preview
FROM conversations c
ORDER BY c.updated_at DESC
LIMIT :limit OFFSET :offset;
```

The correlated subqueries for `message_count` and `last_message_preview` are efficient given the `idx_messages_ordering` index on `(conversation_id, seq)`. For a single-user desktop app with a modest number of conversations, this is simpler and fast enough versus a JOIN-based approach.

### 6.1 ChatPresenter

| Current | New |
|---------|-----|
| `conversation_service.list(None, None)` → builds `ConversationSummary` from full `Conversation` objects | `conversation_service.list_metadata(None, None)` → maps `ConversationMetadata` directly to `ConversationSummary` |
| `conversation_service.list(Some(1), Some(0))` in `restore_startup_conversation` | `conversation_service.list_metadata(Some(1), Some(0))` |
| `conversation.messages.len()` for message_count | `metadata.message_count` (already computed by DB) |

### 6.2 ChatServiceImpl

| Current | New |
|---------|-----|
| `conversation_service.add_user_message(id, content)` | `conversation_service.add_message(id, Message::user(content))` |
| `conversation_service.add_assistant_message(id, text, thinking)` | `conversation_service.add_message(id, Message::assistant_with_thinking(text, thinking))` or `conversation_service.add_message(id, Message::assistant(text))` |

### 6.3 HistoryPresenter

Does **not** call `list()` or any listing method. It only handles `DeleteConversation` events (calling `conversation_service.delete()`) and forwards `ConversationEvent` domain events. No changes needed for the history presenter.

### 6.4 App Wiring (app.rs and main_gpui.rs)

There are **three** call sites that currently construct `ConversationServiceImpl::new(conversations_dir)`. All must be updated to construct `SqliteConversationService`:

1. **`app.rs` → `initialize_services()`** — constructs `ConversationServiceImpl::new(conversations_dir.clone())`. Replace with `SqliteConversationService::new(db_handle)`. Also remove the `conversations_dir` variable and the `create_dir_all(&conversations_dir)` call, since the conversations directory is no longer needed.

2. **`main_gpui.rs` → `create_services()`** — constructs `ConversationServiceImpl::new(runtime_paths.conversations_dir.clone())`. Replace with `SqliteConversationService::new(db_handle)`. The `conversations_dir` field on `RuntimePaths` is no longer used for conversation storage.

3. **`main_gpui/startup.rs` → `build_startup_inputs_async()` and `load_startup_transcript()`** — both construct `ConversationServiceImpl::new(conversations_dir)` for startup bootstrap and transcript loading. Replace with `SqliteConversationService::new(db_handle)`.

For each call site, the replacement follows the same pattern:
1. Construct DB path: `{base_dir}/personalagent.db`
2. Spawn DB worker thread, get `DbHandle`
3. Initialize schema via `DbHandle`
4. Construct `SqliteConversationService::new(db_handle)`
5. Wire as `Arc<dyn ConversationService>`

---

## 7. What Gets Deleted

- `src/services/conversation_impl.rs` — the JSON-file-based `ConversationServiceImpl`, entirely replaced
- `src/storage/` — `ConversationStorage` and all JSON file I/O, no longer needed
- `src/migration.rs` — the old migration runner that only did JSON-to-JSON verification, irrelevant now
- All test code that references the deleted types gets rewritten against `SqliteConversationService`

The `conversations/` directory under `data_local_dir` is simply ignored. If it exists from previous test runs, it sits inert. No migration, no backup, no rename.

---

## 8. Dependency: rusqlite

Add to `Cargo.toml`:

```toml
rusqlite = { version = "0.33", features = ["bundled"] }
```

`bundled` compiles SQLite from source (includes FTS5). No need for `blob` or `modern_sqlite` features — the schema uses no blob columns and `bundled` already builds a modern SQLite.

---

## 9. Validation Rules

Carried forward from existing implementation, now enforced at DB + service layer:

| Rule | Enforcement |
|------|-------------|
| Message role ∈ {user, assistant, system} | `CHECK` constraint on `messages.role` |
| Message content non-empty after trim | Service layer validation before INSERT |
| Title ≤ 120 characters | Service layer validation before INSERT/UPDATE |
| conversation_id references valid conversation | Foreign key constraint with `ON DELETE CASCADE` |

---

## 10. Design Considerations for Downstream Issues

### 10.1 Issue #87: Background Streaming on Conversation Switch

**The problem:** When user switches conversations during an active stream, the stream should continue in the background and persist its final response. With JSON files, this meant load-all → append → rewrite-all for each message, with race conditions if two streams are active.

**How SQLite solves it:**
- `add_message()` is a single `INSERT INTO messages` + `UPDATE conversations SET updated_at`. Atomic, fast, no read-modify-write cycle.
- The DB thread serializes all writes. Two concurrent streams calling `add_message()` for different conversations queue up and execute in order. No races, no locks needed at the service layer.
- `get_messages()` for switch-back is a simple SELECT — loads only the requested conversation, unaffected by concurrent writes to other conversations.

**Duplicate messages on stream cancellation:** Stream completion may fire after cancellation, potentially producing duplicate assistant messages (two identical messages in sequence). This is accepted behavior — the double-write is harmless. If dedup is needed later, add a `stream_id` column to `messages` as a uniqueness key. No action needed in this issue.

### 10.2 Issue #57: Popout Mode with Sidebar and Search

**The problem:** The sidebar needs a conversation list with metadata (title, date, count, preview) and a search bar with ranked full-text results.

**How the new trait solves it:**
- `list_metadata()` returns exactly what the sidebar needs without loading any message content. One SQL query with correlated subqueries for count and preview.
- `search()` returns `SearchResult` with BM25-ranked results grouped by conversation, match type classification, and context snippets. The sidebar renders these directly.
- Pagination via `limit`/`offset` supports lazy-loading as the user scrolls.

---

## 11. Module Structure

```
src/
  db/
    mod.rs              — public exports (DbHandle, init functions)
    worker.rs           — DB thread, closure dispatch, shutdown
    schema.rs           — DDL constants, schema initialization, PRAGMA setup
  services/
    mod.rs              — add export: SqliteConversationService; remove: ConversationServiceImpl
    conversation.rs     — updated trait definition
    conversation_sqlite.rs — SqliteConversationService implementation
    (conversation_impl.rs DELETED)
  models/
    mod.rs              — add exports: SearchResult, SearchMatchType, ConversationMetadata, ContextState
    conversation.rs     — updated Message struct with new optional fields
    search.rs           — SearchResult, SearchMatchType
    context_state.rs    — ContextState
  (storage/ DELETED)
  (migration.rs DELETED)
```

---

## 12. Required Integration Test Scenarios

The following test scenarios must pass before the implementation is considered complete. All tests run against a real in-memory SQLite database (`:memory:` or temp file), not mocks.

### 12.1 Concurrent add_message Ordering

- Insert N messages (e.g., 20) to the same conversation from multiple tasks concurrently via `add_message()`.
- Verify all messages are persisted, `seq` values are unique and contiguous (0..N-1), and no messages are lost or duplicated.
- Verify `conversations.updated_at` reflects the last inserted message's timestamp.

### 12.2 FTS Sync on Insert / Delete / Rename

- **Insert:** Add a message, verify the FTS index contains a matching row (search returns the conversation).
- **Delete conversation:** Delete a conversation with messages, verify all FTS rows for that conversation are removed (search returns nothing).
- **CASCADE delete:** Delete a conversation, verify all its messages AND their FTS rows are removed (no orphaned `search_index` or `messages` rows remain).
- **Rename:** Rename a conversation, verify that searching by the new title matches existing messages and searching by the old title does not.

### 12.3 list_metadata Correctness

- Create multiple conversations with varying message counts. Verify `list_metadata()` returns correct `message_count` and `last_message_preview` for each.
- Verify ordering is by `updated_at DESC`.
- Verify `limit` and `offset` pagination works correctly.
- Verify a conversation with zero messages returns `message_count: 0` and `last_message_preview: None`.

### 12.4 Startup Restore via Metadata Ordering

- Create conversations with different `updated_at` timestamps. Call `list_metadata(Some(1), Some(0))`.
- Verify it returns the single most recently updated conversation.

### 12.5 Search Ranking: Title > Content

- Create two conversations: one where the search term appears only in the title, one where it appears only in message content.
- Verify the title-match conversation ranks higher in search results (lower/better BM25 score due to 10x weight).

### 12.6 Search Returns Only Conversations with Messages

- Create a conversation with a title matching a search term but add no messages.
- Verify `search()` does **not** return it.
- Add a message, verify `search()` now returns it.

### 12.7 ContextState Round-Trip

- Store a `ContextState` via `update_context_state()`, retrieve via `get_context_state()`, verify all fields match.
- Verify `get_context_state()` returns `None` for a conversation with no saved state.

