//! Search result types for full-text conversation search

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A ranked search result from full-text conversation search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub conversation_id: Uuid,
    pub title: String,
    pub match_type: SearchMatchType,
    pub match_context: String,
    pub score: f64,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

/// How the search query matched a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchMatchType {
    Title,
    Content,
}
