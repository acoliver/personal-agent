//! Context state for conversation compression and summarization

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompressionPhase {
    None,
    ObservationMasked,
    Summarized,
    Truncated,
}

/// Persisted state tracking how a conversation's context window is managed.
///
/// Stored as a JSON blob in the `context_state` column of the conversations table.
/// Deserialization failures (e.g., from struct evolution) are treated as `None` by
/// the service layer — callers must handle `None` gracefully.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextState {
    pub strategy: Option<String>,
    pub summary: Option<String>,
    pub visible_range: Option<(usize, usize)>,
    #[serde(default)]
    pub preserved_facts: Option<Vec<String>>,
    #[serde(default)]
    pub summary_range: Option<(usize, usize)>,
    #[serde(default)]
    pub compressed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub masked_tool_seqs: Option<Vec<usize>>,
    #[serde(default)]
    pub compression_phase: Option<CompressionPhase>,
    #[serde(default)]
    pub last_input_tokens: Option<u32>,
    #[serde(default)]
    pub last_output_tokens: Option<u32>,
}
