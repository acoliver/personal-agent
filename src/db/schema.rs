//! `SQLite` schema DDL and initialization logic.
//!
//! `initialize_schema` is called once by the DB worker thread immediately after
//! opening the connection. It applies all PRAGMAs and, when `PRAGMA user_version`
//! is 0, runs the full DDL to create tables, indexes, the FTS5 virtual table, and
//! all triggers.

use rusqlite::Connection;

// ---------------------------------------------------------------------------
// PRAGMAs applied on every connection open
// ---------------------------------------------------------------------------

const PRAGMA_WAL: &str = "PRAGMA journal_mode = WAL";
const PRAGMA_FOREIGN_KEYS: &str = "PRAGMA foreign_keys = ON";
const PRAGMA_BUSY_TIMEOUT: &str = "PRAGMA busy_timeout = 5000";
const PRAGMA_SYNCHRONOUS: &str = "PRAGMA synchronous = NORMAL";

// ---------------------------------------------------------------------------
// Full DDL — applied only when user_version == 0
// ---------------------------------------------------------------------------

const CREATE_CONVERSATIONS: &str = "
CREATE TABLE IF NOT EXISTS conversations (
    id              TEXT PRIMARY KEY,
    title           TEXT,
    profile_id      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    context_state   TEXT
)";

const CREATE_IDX_CONVERSATIONS_UPDATED: &str =
    "CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at DESC)";

const CREATE_IDX_CONVERSATIONS_PROFILE: &str =
    "CREATE INDEX IF NOT EXISTS idx_conversations_profile ON conversations(profile_id)";

const CREATE_MESSAGES: &str = "
CREATE TABLE IF NOT EXISTS messages (
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
)";

const CREATE_IDX_MESSAGES_ORDERING: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_ordering ON messages(conversation_id, seq)";

const CREATE_IDX_MESSAGES_CONVERSATION_TS: &str =
    "CREATE INDEX IF NOT EXISTS idx_messages_conversation_ts ON messages(conversation_id, created_at)";

const CREATE_SEARCH_INDEX: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
    title,
    content,
    conversation_id UNINDEXED,
    message_rowid UNINDEXED
)";

const CREATE_TRIGGER_MESSAGES_AI: &str = "
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO search_index(title, content, conversation_id, message_rowid)
    SELECT c.title, NEW.content, NEW.conversation_id, NEW.id
    FROM conversations c WHERE c.id = NEW.conversation_id;
END";

const CREATE_TRIGGER_MESSAGES_AU: &str = "
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE OF content ON messages BEGIN
    DELETE FROM search_index WHERE message_rowid = OLD.id;
    INSERT INTO search_index(title, content, conversation_id, message_rowid)
    SELECT c.title, NEW.content, NEW.conversation_id, NEW.id
    FROM conversations c WHERE c.id = NEW.conversation_id;
END";

const CREATE_TRIGGER_MESSAGES_AD: &str = "
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    DELETE FROM search_index WHERE message_rowid = OLD.id;
END";

const CREATE_TRIGGER_CONVERSATIONS_TITLE_AU: &str = "
CREATE TRIGGER IF NOT EXISTS conversations_title_au AFTER UPDATE OF title ON conversations BEGIN
    UPDATE search_index SET title = NEW.title WHERE conversation_id = NEW.id;
END";

const CREATE_TRIGGER_CONVERSATIONS_AD: &str = "
CREATE TRIGGER IF NOT EXISTS conversations_ad AFTER DELETE ON conversations BEGIN
    DELETE FROM search_index WHERE conversation_id = OLD.id;
END";

const SET_USER_VERSION_1: &str = "PRAGMA user_version = 1";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply all PRAGMAs and, if the database is uninitialized (`user_version = 0`),
/// run the full DDL to create all tables, indexes, the FTS5 virtual table, and
/// all synchronization triggers.
///
/// Idempotent: subsequent calls with `user_version = 1` are no-ops.
///
/// # Errors
///
/// Returns a `rusqlite::Error` if any PRAGMA, DDL statement, or version query fails.
pub fn initialize_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Always apply connection-level PRAGMAs.
    conn.execute_batch(PRAGMA_WAL)?;
    conn.execute_batch(PRAGMA_FOREIGN_KEYS)?;
    conn.execute_batch(PRAGMA_BUSY_TIMEOUT)?;
    conn.execute_batch(PRAGMA_SYNCHRONOUS)?;

    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version == 0 {
        // Tables
        conn.execute_batch(CREATE_CONVERSATIONS)?;
        conn.execute_batch(CREATE_MESSAGES)?;
        // FTS5 virtual table
        conn.execute_batch(CREATE_SEARCH_INDEX)?;
        // Triggers
        conn.execute_batch(CREATE_TRIGGER_MESSAGES_AI)?;
        conn.execute_batch(CREATE_TRIGGER_MESSAGES_AU)?;
        conn.execute_batch(CREATE_TRIGGER_MESSAGES_AD)?;
        conn.execute_batch(CREATE_TRIGGER_CONVERSATIONS_TITLE_AU)?;
        conn.execute_batch(CREATE_TRIGGER_CONVERSATIONS_AD)?;
        // Indexes
        conn.execute_batch(CREATE_IDX_CONVERSATIONS_UPDATED)?;
        conn.execute_batch(CREATE_IDX_CONVERSATIONS_PROFILE)?;
        conn.execute_batch(CREATE_IDX_MESSAGES_ORDERING)?;
        conn.execute_batch(CREATE_IDX_MESSAGES_CONVERSATION_TS)?;
        // Mark schema as initialized
        conn.execute_batch(SET_USER_VERSION_1)?;
    }
    // user_version == 1: schema already current, nothing to do.

    Ok(())
}
