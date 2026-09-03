//! `SQLite` schema DDL and initialization logic.
//!
//! `initialize_schema` is called once by the DB worker thread immediately after
//! opening the connection. It applies all PRAGMAs and, when `PRAGMA user_version`
//! is 0, runs the full DDL to create tables, indexes, the FTS5 virtual table, and
//! all triggers. Older schema versions are migrated forward to the current
//! version (v1 → v2 adds the messages `interrupted` column).

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
    seq                 INTEGER NOT NULL,
    interrupted         INTEGER NOT NULL DEFAULT 0
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

/// v1 → v2 migration: add the assistant `interrupted` marker (issue #193).
/// Existing rows pick up the column default (`0`). The ALTER and the version
/// bump share one transaction so a failure rolls both back together instead
/// of leaving a schema that still reports v1 while already carrying the
/// column.
const MIGRATE_V1_TO_V2: &str = "
BEGIN;
ALTER TABLE messages ADD COLUMN interrupted INTEGER NOT NULL DEFAULT 0;
PRAGMA user_version = 2;
COMMIT;";

const SET_USER_VERSION_2: &str = "PRAGMA user_version = 2";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply all PRAGMAs and bring the schema to the current version.
///
/// - `user_version = 0` (uninitialized): run the full DDL to create all
///   tables, indexes, the FTS5 virtual table, and all synchronization
///   triggers, then set `user_version = 2`.
/// - `user_version = 1`: run the v1 → v2 migration, atomically adding the
///   messages `interrupted` column and setting `user_version = 2` in one
///   transaction.
/// - `user_version = 2`: schema already current, nothing to do.
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
        conn.execute_batch(SET_USER_VERSION_2)?;
    } else if version == 1 {
        conn.execute_batch(MIGRATE_V1_TO_V2)?;
    }
    // user_version == 2: schema already current, nothing to do.

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("user_version query should succeed")
    }

    /// (notnull, `dflt_value`) for the `interrupted` column, or None when absent.
    fn interrupted_column(conn: &Connection) -> Option<(i64, Option<String>)> {
        let mut stmt = conn
            .prepare("PRAGMA table_info(messages)")
            .expect("table_info should prepare");
        let columns: Vec<(String, i64, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(1)?, row.get(3)?, row.get(4)?)))
            .expect("table_info should query")
            .collect::<Result<_, _>>()
            .expect("table_info rows should map");
        columns
            .into_iter()
            .find(|(name, _, _)| name == "interrupted")
            .map(|(_, notnull, default)| (notnull, default))
    }

    #[test]
    fn fresh_schema_sets_version_2_with_interrupted_column() {
        let conn = Connection::open_in_memory().expect("in-memory database should open");
        initialize_schema(&conn).expect("fresh schema initialization should succeed");

        assert_eq!(user_version(&conn), 2);

        let (notnull, default) =
            interrupted_column(&conn).expect("fresh schema must include the interrupted column");
        assert_eq!(notnull, 1, "interrupted must be NOT NULL");
        assert_eq!(
            default.as_deref(),
            Some("0"),
            "interrupted must default to 0"
        );

        initialize_schema(&conn).expect("re-initialization on v2 should succeed");
        assert_eq!(user_version(&conn), 2);
    }

    #[test]
    fn v1_schema_migrates_to_v2_and_defaults_existing_rows_to_false() {
        let conn = Connection::open_in_memory().expect("in-memory database should open");
        conn.execute_batch(
            "
            CREATE TABLE conversations (
                id              TEXT PRIMARY KEY,
                title           TEXT,
                profile_id      TEXT,
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL,
                context_state   TEXT
            );
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
            PRAGMA user_version = 1;
            ",
        )
        .expect("v1 schema should build");
        conn.execute(
            "INSERT INTO conversations (id, created_at, updated_at)
             VALUES ('conv-1', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')",
            [],
        )
        .expect("v1 conversation row should insert");
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, seq)
             VALUES ('conv-1', 'user', 'old row', '2026-01-01T00:00:00.000Z', 0)",
            [],
        )
        .expect("v1 message row should insert");

        initialize_schema(&conn).expect("v1 to v2 migration should succeed");

        assert_eq!(user_version(&conn), 2);
        let (notnull, default) =
            interrupted_column(&conn).expect("migration must add the interrupted column");
        assert_eq!(notnull, 1);
        assert_eq!(default.as_deref(), Some("0"));

        let interrupted: i64 = conn
            .query_row(
                "SELECT interrupted FROM messages WHERE conversation_id = 'conv-1'",
                [],
                |row| row.get(0),
            )
            .expect("existing v1 row should read interrupted after migration");
        assert_eq!(
            interrupted, 0,
            "rows created before the migration must default to interrupted = 0"
        );
    }
}
