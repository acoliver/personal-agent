//! Conversation and message types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Conversation {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub title: Option<String>,
    pub profile_id: Uuid,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub thinking_content: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl Conversation {
    /// Create a new conversation
    #[must_use]
    pub fn new(profile_id: Uuid) -> Self {
        let now = Utc::now();
        // Default title to timestamp format YYYYMMDDHHMMSSmmm
        let default_title = now.format("%Y%m%d%H%M%S%3f").to_string();

        Self {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            title: Some(default_title),
            profile_id,
            messages: Vec::new(),
        }
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Set the conversation title
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
        self.updated_at = Utc::now();
    }

    /// Get the timestamp-based filename for this conversation
    #[must_use]
    pub fn filename(&self) -> String {
        format!("{}.json", self.created_at.format("%Y%m%d%H%M%S%3f"))
    }
}

impl Message {
    /// Create a new user message
    #[must_use]
    pub fn user(content: String) -> Self {
        Self {
            role: MessageRole::User,
            content,
            thinking_content: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new assistant message
    #[must_use]
    pub fn assistant(content: String) -> Self {
        Self {
            role: MessageRole::Assistant,
            content,
            thinking_content: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new assistant message with thinking content
    #[must_use]
    pub fn assistant_with_thinking(content: String, thinking: String) -> Self {
        Self {
            role: MessageRole::Assistant,
            content,
            thinking_content: Some(thinking),
            timestamp: Utc::now(),
        }
    }

    /// Create a new system message
    #[must_use]
    pub fn system(content: String) -> Self {
        Self {
            role: MessageRole::System,
            content,
            thinking_content: None,
            timestamp: Utc::now(),
        }
    }
}
