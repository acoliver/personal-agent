//! Context state for conversation compression and summarization

use serde::{Deserialize, Serialize};

/// Persisted state tracking how a conversation's context window is managed.
///
/// Stored as a JSON blob in the `context_state` column of the conversations table.
/// Deserialization failures (e.g., from struct evolution) are treated as `None` by
/// the service layer — callers must handle `None` gracefully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextState {
    pub strategy: Option<String>,
    pub summary: Option<String>,
    pub visible_range: Option<(usize, usize)>,
}
