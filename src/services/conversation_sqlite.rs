//! SQLite-backed `ConversationService` implementation.
//!
//! `SqliteConversationService` delegates all persistence to a dedicated DB
//! worker thread via `DbHandle::execute`.  In-memory state is limited to the
//! currently-active conversation ID, which is not persisted across restarts.
//!
//! See spec §2, §4, §5, §6 in project-plans/issue56/overview.md for the
//! full contract.

use async_trait::async_trait;
use chrono::SecondsFormat;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::db::worker::DbHandle;
use crate::models::{
    ContextState, Conversation, ConversationMetadata, Message, MessageRole, SearchMatchType,
    SearchResult,
};
use crate::services::conversation::ConversationService;
use crate::services::{ServiceError, ServiceResult};

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

pub struct SqliteConversationService {
    db: DbHandle,
    active_id: Mutex<Option<Uuid>>,
}

impl SqliteConversationService {
    #[must_use]
    pub const fn new(db: DbHandle) -> Self {
        Self {
            db,
            active_id: Mutex::new(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper – map rusqlite errors to ServiceError (per spec §5.3 error table)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn map_db_err(e: &rusqlite::Error) -> ServiceError {
    match e {
        rusqlite::Error::QueryReturnedNoRows => ServiceError::NotFound("record not found".into()),
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            ServiceError::Validation(format!("constraint violation: {e}"))
        }
        _ => ServiceError::Storage(format!("{e}")),
    }
}

// ---------------------------------------------------------------------------
// Helper – current UTC timestamp in RFC 3339 / ISO 8601 with Z suffix
// ---------------------------------------------------------------------------

fn now_ts() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

// ---------------------------------------------------------------------------
// Helper – parse a stored RFC 3339 timestamp string into DateTime<Utc>
// ---------------------------------------------------------------------------

fn parse_ts(s: &str) -> Result<chrono::DateTime<chrono::Utc>, ServiceError> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| ServiceError::Storage(format!("invalid timestamp '{s}': {e}")))
}

fn parse_uuid_sql(s: &str, col: usize) -> Result<Uuid, rusqlite::Error> {
    Uuid::parse_str(s).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            col,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid uuid: {e}"),
            )),
        )
    })
}

fn parse_ts_sql(s: &str, col: usize) -> Result<chrono::DateTime<chrono::Utc>, rusqlite::Error> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                col,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{e}"),
                )),
            )
        })
}

// ---------------------------------------------------------------------------
// Helper – convert MessageRole to its DB string representation
// ---------------------------------------------------------------------------

const fn role_to_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
    }
}

// ---------------------------------------------------------------------------
// Helper – parse a DB role string back into MessageRole
// ---------------------------------------------------------------------------

fn str_to_role(s: &str) -> Result<MessageRole, ServiceError> {
    match s {
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "system" => Ok(MessageRole::System),
        other => Err(ServiceError::Storage(format!("unknown role: {other}"))),
    }
}

// ---------------------------------------------------------------------------
// Helper – build a Message from a messages row
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn row_to_message(
    role: &str,
    content: String,
    thinking_content: Option<String>,
    model_id: Option<String>,
    tool_calls: Option<String>,
    tool_results: Option<String>,
    created_at: &str,
) -> Result<Message, ServiceError> {
    Ok(Message {
        role: str_to_role(role)?,
        content,
        thinking_content,
        timestamp: parse_ts(created_at)?,
        model_id,
        tool_calls,
        tool_results,
    })
}

// ---------------------------------------------------------------------------
// FTS5 query sanitization (spec §4.2)
// ---------------------------------------------------------------------------

/// Sanitize a raw user query into a safe FTS5 MATCH expression.
///
/// Returns `None` if the query is empty after sanitization (caller should
/// return `vec![]` immediately without querying the DB).
fn sanitize_fts_query(raw: &str) -> Option<String> {
    // Step 1: trim leading/trailing whitespace.
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Step 2: replace FTS5 structural operators with spaces so adjacent tokens
    // separated by operators become distinct terms (e.g., `foo(bar)` → `foo bar`).
    // Characters: * ( ) { } ^ ~ :
    // Note: + - " # . are preserved here because they are treated as literal
    // characters inside double-quoted FTS5 terms (spec §4.2 step 4).
    let replaced: String = trimmed
        .chars()
        .map(|c| {
            if matches!(c, '*' | '(' | ')' | '{' | '}' | '^' | '~' | ':') {
                ' '
            } else {
                c
            }
        })
        .collect();

    // Step 3: tokenize on whitespace.
    let terms: Vec<&str> = replaced.split_whitespace().collect();
    if terms.is_empty() {
        return None;
    }

    // Step 4 & 5: quote each term (escaping embedded double-quotes as ""),
    // join with implicit AND, append prefix wildcard to last term.
    let last_idx = terms.len() - 1;
    let quoted: Vec<String> = terms
        .iter()
        .enumerate()
        .map(|(i, term)| {
            let escaped = term.replace('"', "\"\"");
            if i == last_idx {
                format!("\"{escaped}\"*")
            } else {
                format!("\"{escaped}\"")
            }
        })
        .collect();

    Some(quoted.join(" "))
}

// ---------------------------------------------------------------------------
// Helper – query all messages for a conversation (used by load + get_messages)
// ---------------------------------------------------------------------------

fn select_messages(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Vec<Message>, rusqlite::Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT role, content, thinking_content, model_id, tool_calls, tool_results, created_at
         FROM messages
         WHERE conversation_id = ?1
         ORDER BY seq ASC",
    )?;

    let rows = stmt.query_map([conversation_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;

    let mut messages = Vec::new();
    for row in rows {
        let (role, content, thinking_content, model_id, tool_calls, tool_results, created_at) =
            row?;
        let msg = row_to_message(
            &role,
            content,
            thinking_content,
            model_id,
            tool_calls,
            tool_results,
            &created_at,
        )
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("{e}"),
                )),
            )
        })?;
        messages.push(msg);
    }

    Ok(messages)
}

// ---------------------------------------------------------------------------
// Per-conversation aggregation state used by search
// ---------------------------------------------------------------------------

struct ConvAgg {
    id_str: String,
    title_opt: Option<String>,
    updated_at_str: String,
    message_count: i64,
    best_score: f64, // minimum (most negative) = best
    best_ctx: String,
}

/// Aggregate per-FTS-hit rows into one `ConvAgg` per conversation,
/// keeping the best (most-negative) BM25 score and its snippet.
fn aggregate_hits(
    per_hit_rows: Vec<(String, Option<String>, String, i64, f64, String)>,
) -> Vec<ConvAgg> {
    let mut agg: HashMap<String, ConvAgg> = HashMap::new();
    for (id_str, title_opt, updated_at_str, message_count, score, ctx) in per_hit_rows {
        let entry = agg.entry(id_str.clone()).or_insert_with(|| ConvAgg {
            id_str: id_str.clone(),
            title_opt: title_opt.clone(),
            updated_at_str: updated_at_str.clone(),
            message_count,
            best_score: score,
            best_ctx: ctx.clone(),
        });
        // BM25 scores are negative; lower (more negative) is a better match.
        if score < entry.best_score {
            entry.best_score = score;
            entry.best_ctx = ctx;
        }
    }

    // Sort by best score ascending (most negative first), then updated_at DESC.
    let mut agg_vec: Vec<ConvAgg> = agg.into_values().collect();
    agg_vec.sort_by(|a, b| {
        a.best_score
            .partial_cmp(&b.best_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.updated_at_str.cmp(&a.updated_at_str))
    });
    agg_vec
}

/// Convert a `ConvAgg` into a `SearchResult`, classifying the match type by
/// checking whether any raw query term appears in the title (case-insensitive).
fn conv_agg_to_search_result(
    agg: ConvAgg,
    raw_terms: &[String],
) -> Result<SearchResult, ServiceError> {
    let conversation_id = Uuid::parse_str(&agg.id_str)
        .map_err(|e| ServiceError::Storage(format!("invalid uuid: {e}")))?;
    let title = agg.title_opt.unwrap_or_else(|| "Untitled".to_string());
    let updated_at = parse_ts(&agg.updated_at_str)?;
    let title_lower = title.to_lowercase();
    let match_type = if raw_terms
        .iter()
        .any(|term| title_lower.contains(term.as_str()))
    {
        SearchMatchType::Title
    } else {
        SearchMatchType::Content
    };
    Ok(SearchResult {
        conversation_id,
        title,
        match_type,
        match_context: agg.best_ctx,
        score: agg.best_score,
        updated_at,
        message_count: usize::try_from(agg.message_count).unwrap_or(0),
    })
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ConversationService for SqliteConversationService {
    // -----------------------------------------------------------------------
    // create
    // -----------------------------------------------------------------------

    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> ServiceResult<Conversation> {
        let id = Uuid::new_v4();
        let now = now_ts();
        let id_str = id.to_string();
        let profile_str = model_profile_id.to_string();
        let title_clone = title.clone();
        // Clone now before moving it into the closure so we can use it afterward.
        let now_for_return = now.clone();

        let now_db = now_for_return.clone();
        let now_db2 = now_for_return.clone();
        self.db
            .execute(move |conn| {
                conn.execute(
                    "INSERT INTO conversations (id, title, profile_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![id_str, title_clone, profile_str, now_db, now_db2],
                )?;
                Ok(())
            })
            .await?;

        let created = parse_ts(&now_for_return)?;

        Ok(Conversation {
            id,
            created_at: created,
            updated_at: created,
            title,
            profile_id: model_profile_id,
            messages: Vec::new(),
        })
    }

    // -----------------------------------------------------------------------
    // load
    // -----------------------------------------------------------------------

    async fn load(&self, id: Uuid) -> ServiceResult<Conversation> {
        let id_str = id.to_string();

        let (title, profile_id_str, created_at_str, updated_at_str, messages) = self
            .db
            .execute(move |conn| {
                let meta: (Option<String>, String, String, String) = conn.query_row(
                    "SELECT title, profile_id, created_at, updated_at
                     FROM conversations WHERE id = ?1",
                    [&id_str],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )?;
                let msgs = select_messages(conn, &id_str)?;
                Ok((meta.0, meta.1, meta.2, meta.3, msgs))
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })?;

        let profile_id = Uuid::parse_str(&profile_id_str)
            .map_err(|e| ServiceError::Storage(format!("invalid profile_id: {e}")))?;
        let created_at = parse_ts(&created_at_str)?;
        let updated_at = parse_ts(&updated_at_str)?;

        Ok(Conversation {
            id,
            created_at,
            updated_at,
            title,
            profile_id,
            messages,
        })
    }

    // -----------------------------------------------------------------------
    // list_metadata
    // -----------------------------------------------------------------------

    async fn list_metadata(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<ConversationMetadata>> {
        // Spec §2: defaults and clamps.
        let limit = i64::try_from(limit.unwrap_or(100).min(1000)).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset.unwrap_or(0)).unwrap_or(i64::MAX);

        self.db
            .execute(move |conn| {
                let mut stmt = conn.prepare_cached(
                    "SELECT
                         c.id,
                         c.title,
                         c.profile_id,
                         c.created_at,
                         c.updated_at,
                         (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id)
                             AS message_count,
                         (SELECT SUBSTR(m2.content, 1, 100)
                          FROM messages m2
                          WHERE m2.conversation_id = c.id
                          ORDER BY m2.seq DESC
                          LIMIT 1) AS last_message_preview
                     FROM conversations c
                     ORDER BY c.updated_at DESC
                     LIMIT ?1 OFFSET ?2",
                )?;

                let rows = stmt.query_map(rusqlite::params![limit, offset], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                })?;

                let mut results = Vec::new();
                for row in rows {
                    let (id_str, title, profile_id_str, ca_str, ua_str, mc, preview) = row?;
                    results.push(ConversationMetadata {
                        id: parse_uuid_sql(&id_str, 0)?,
                        title,
                        profile_id: profile_id_str
                            .as_deref()
                            .and_then(|s| Uuid::parse_str(s).ok()),
                        created_at: parse_ts_sql(&ca_str, 3)?,
                        updated_at: parse_ts_sql(&ua_str, 4)?,
                        message_count: usize::try_from(mc).unwrap_or(0),
                        last_message_preview: preview,
                    });
                }

                Ok(results)
            })
            .await
    }

    // -----------------------------------------------------------------------
    // add_message  (spec §5.1 – transaction wrapping INSERT + UPDATE)
    // -----------------------------------------------------------------------

    async fn add_message(&self, conversation_id: Uuid, message: Message) -> ServiceResult<Message> {
        let conv_id_str = conversation_id.to_string();
        let role_str = role_to_str(&message.role).to_string();
        let content = message.content.clone();
        let thinking = message.thinking_content.clone();
        let model_id = message.model_id.clone();
        let tool_calls = message.tool_calls.clone();
        let tool_results = message.tool_results.clone();
        let created_at = message
            .timestamp
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let updated_at = created_at.clone();

        self.db
            .execute(move |conn| {
                let tx = conn.unchecked_transaction()?;

                tx.execute(
                    "INSERT INTO messages
                         (conversation_id, role, content, thinking_content,
                          model_id, tool_calls, tool_results, created_at, seq)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                             COALESCE(
                                 (SELECT MAX(seq) + 1 FROM messages WHERE conversation_id = ?1),
                                 0
                             ))",
                    rusqlite::params![
                        conv_id_str,
                        role_str,
                        content,
                        thinking,
                        model_id,
                        tool_calls,
                        tool_results,
                        created_at,
                    ],
                )?;

                tx.execute(
                    "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![updated_at, conv_id_str],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await?;

        Ok(message)
    }

    // -----------------------------------------------------------------------
    // search  (spec §4)
    // -----------------------------------------------------------------------

    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<SearchResult>> {
        // Sanitize the query before any DB interaction.
        let Some(fts_query) = sanitize_fts_query(query) else {
            return Ok(Vec::new());
        };

        // Keep the raw terms for post-query SearchMatchType classification.
        let raw_terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();

        let limit = i64::try_from(limit.unwrap_or(100).min(1000)).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset.unwrap_or(0)).unwrap_or(i64::MAX);

        // SQLite FTS5 auxiliary functions (bm25, snippet) cannot be used in
        // aggregate context (e.g., MIN(bm25(...)) GROUP BY) — they require the
        // FTS virtual table to be the driving query with no aggregation layer
        // between the FTS scan and the auxiliary function call.
        //
        // Strategy: collect one row per FTS hit (no GROUP BY in SQL), then
        // aggregate per conversation_id in Rust. This preserves the per-hit
        // BM25 score and snippet while allowing us to pick the best score and
        // context for each conversation.
        let per_hit_rows: Vec<(String, Option<String>, String, i64, f64, String)> = self
            .db
            .execute(move |conn| {
                let mut stmt = conn.prepare_cached(
                    "SELECT
                         c.id, c.title, c.updated_at,
                         (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id)
                             AS message_count,
                         bm25(search_index, 10.0, 1.0) AS score,
                         snippet(search_index, 1, '[', ']', '...', 24) AS ctx
                     FROM search_index
                     JOIN conversations c ON c.id = search_index.conversation_id
                     WHERE search_index MATCH ?1",
                )?;

                let rows = stmt.query_map(rusqlite::params![fts_query], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, f64>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?;

                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .await?;

        // Aggregate per-hit rows into per-conversation best scores in Rust.
        // BM25 scores are negative (more negative = better match).
        let agg_vec = aggregate_hits(per_hit_rows);

        // Apply limit/offset after in-Rust aggregation.
        let skip = usize::try_from(offset).unwrap_or(0);
        let take = usize::try_from(limit).unwrap_or(usize::MAX);

        agg_vec
            .into_iter()
            .skip(skip)
            .take(take)
            .map(|agg| conv_agg_to_search_result(agg, &raw_terms))
            .collect()
    }

    // -----------------------------------------------------------------------
    // message_count
    // -----------------------------------------------------------------------

    async fn message_count(&self, conversation_id: Uuid) -> ServiceResult<usize> {
        let id_str = conversation_id.to_string();

        let count: i64 = self
            .db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                    [&id_str],
                    |row| row.get(0),
                )
            })
            .await?;

        Ok(usize::try_from(count).unwrap_or(0))
    }

    // -----------------------------------------------------------------------
    // rename
    // -----------------------------------------------------------------------

    async fn rename(&self, id: Uuid, new_title: String) -> ServiceResult<()> {
        let id_str = id.to_string();
        let now = now_ts();

        self.db
            .execute(move |conn| {
                let changed = conn.execute(
                    "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![new_title, now, id_str],
                )?;
                if changed == 0 {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
                Ok(())
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })
    }

    // -----------------------------------------------------------------------
    // delete
    // -----------------------------------------------------------------------

    async fn delete(&self, id: Uuid) -> ServiceResult<()> {
        let id_str = id.to_string();

        self.db
            .execute(move |conn| {
                let changed = conn.execute("DELETE FROM conversations WHERE id = ?1", [&id_str])?;
                if changed == 0 {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
                Ok(())
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })?;

        // Clear active_id if the deleted conversation was the active one.
        let mut active = self
            .active_id
            .lock()
            .map_err(|e| ServiceError::Storage(format!("mutex poisoned: {e}")))?;
        if *active == Some(id) {
            *active = None;
        }
        drop(active);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // set_active
    // -----------------------------------------------------------------------

    async fn set_active(&self, id: Uuid) -> ServiceResult<()> {
        // Verify the conversation exists before setting active.
        let id_str = id.to_string();
        self.db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT 1 FROM conversations WHERE id = ?1",
                    [&id_str],
                    |_| Ok(()),
                )
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })?;

        *self
            .active_id
            .lock()
            .map_err(|e| ServiceError::Storage(format!("mutex poisoned: {e}")))? = Some(id);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // get_active
    // -----------------------------------------------------------------------

    async fn get_active(&self) -> ServiceResult<Option<Uuid>> {
        let guard = self
            .active_id
            .lock()
            .map_err(|e| ServiceError::Storage(format!("mutex poisoned: {e}")))?;
        Ok(*guard)
    }

    // -----------------------------------------------------------------------
    // get_messages
    // -----------------------------------------------------------------------

    async fn get_messages(&self, conversation_id: Uuid) -> ServiceResult<Vec<Message>> {
        let id_str = conversation_id.to_string();
        self.db
            .execute(move |conn| select_messages(conn, &id_str))
            .await
    }

    // -----------------------------------------------------------------------
    // update
    // -----------------------------------------------------------------------

    async fn update(
        &self,
        id: Uuid,
        title: Option<String>,
        model_profile_id: Option<Uuid>,
    ) -> ServiceResult<Conversation> {
        let id_str = id.to_string();
        let now = now_ts();
        let profile_str = model_profile_id.map(|u| u.to_string());

        self.db
            .execute(move |conn| {
                let changed = conn.execute(
                    "UPDATE conversations
                     SET title        = COALESCE(?1, title),
                         profile_id   = COALESCE(?2, profile_id),
                         updated_at   = ?3
                     WHERE id = ?4",
                    rusqlite::params![title, profile_str, now, id_str],
                )?;
                if changed == 0 {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
                Ok(())
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })?;

        // Load and return the full updated conversation.
        self.load(id).await
    }

    // -----------------------------------------------------------------------
    // update_context_state
    // -----------------------------------------------------------------------

    async fn update_context_state(&self, id: Uuid, state: &ContextState) -> ServiceResult<()> {
        let json = serde_json::to_string(state).map_err(|e| {
            ServiceError::Serialization(format!("failed to serialize context state: {e}"))
        })?;

        let id_str = id.to_string();
        let now = now_ts();

        self.db
            .execute(move |conn| {
                let changed = conn.execute(
                    "UPDATE conversations SET context_state = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![json, now, id_str],
                )?;
                if changed == 0 {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
                Ok(())
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })
    }

    // -----------------------------------------------------------------------
    // get_context_state
    // -----------------------------------------------------------------------

    async fn get_context_state(&self, id: Uuid) -> ServiceResult<Option<ContextState>> {
        let id_str = id.to_string();

        let json_opt: Option<String> = self
            .db
            .execute(move |conn| {
                conn.query_row(
                    "SELECT context_state FROM conversations WHERE id = ?1",
                    [&id_str],
                    |row| row.get(0),
                )
            })
            .await
            .map_err(|e| {
                if matches!(e, ServiceError::NotFound(_)) {
                    ServiceError::NotFound(format!("conversation not found: {id}"))
                } else {
                    e
                }
            })?;

        json_opt.map_or(Ok(None), |json| {
            match serde_json::from_str::<ContextState>(&json) {
                Ok(state) => Ok(Some(state)),
                Err(e) => {
                    // Deserialization failures are treated as None (spec §2.3).
                    tracing::warn!(
                        conversation_id = %id,
                        error = %e,
                        "failed to deserialize context_state, treating as None"
                    );
                    Ok(None)
                }
            }
        })
    }
}
