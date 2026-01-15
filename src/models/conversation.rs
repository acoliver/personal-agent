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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_conversation() {
        let profile_id = Uuid::new_v4();
        let conversation = Conversation::new(profile_id);

        assert_eq!(conversation.profile_id, profile_id);
        assert_eq!(conversation.messages.len(), 0);
        // Title defaults to timestamp format YYYYMMDDHHMMSSmmm
        assert!(conversation.title.is_some());
        let title = conversation.title.as_ref().unwrap();
        assert_eq!(title.len(), 17); // YYYYMMDDHHMMSSmmm = 17 chars
    }

    #[test]
    fn test_add_message() {
        let mut conversation = Conversation::new(Uuid::new_v4());
        let message = Message::user("Hello".to_string());

        conversation.add_message(message.clone());

        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(conversation.messages[0], message);
    }

    #[test]
    fn test_set_title() {
        let mut conversation = Conversation::new(Uuid::new_v4());
        conversation.set_title("Test Conversation".to_string());

        assert_eq!(conversation.title, Some("Test Conversation".to_string()));
    }

    #[test]
    fn test_filename() {
        let mut conversation = Conversation::new(Uuid::new_v4());
        conversation.created_at = DateTime::parse_from_rfc3339("2026-01-14T12:34:56.789Z")
            .unwrap()
            .with_timezone(&Utc);

        let filename = conversation.filename();
        assert_eq!(filename, "20260114123456789.json");
    }

    #[test]
    fn test_user_message() {
        let message = Message::user("Hello".to_string());
        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello");
        assert!(message.thinking_content.is_none());
    }

    #[test]
    fn test_assistant_message() {
        let message = Message::assistant("Hi there".to_string());
        assert_eq!(message.role, MessageRole::Assistant);
        assert_eq!(message.content, "Hi there");
        assert!(message.thinking_content.is_none());
    }

    #[test]
    fn test_assistant_with_thinking() {
        let message = Message::assistant_with_thinking(
            "Answer".to_string(),
            "Thinking...".to_string(),
        );
        assert_eq!(message.role, MessageRole::Assistant);
        assert_eq!(message.content, "Answer");
        assert_eq!(message.thinking_content, Some("Thinking...".to_string()));
    }

    #[test]
    fn test_system_message() {
        let message = Message::system("You are a helpful assistant".to_string());
        assert_eq!(message.role, MessageRole::System);
        assert_eq!(message.content, "You are a helpful assistant");
    }

    #[test]
    fn test_message_serialization() {
        let message = Message::user("Test".to_string());
        let json = serde_json::to_string(&message).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_conversation_serialization() {
        let mut conversation = Conversation::new(Uuid::new_v4());
        conversation.add_message(Message::user("Hello".to_string()));
        conversation.add_message(Message::assistant("Hi".to_string()));

        let json = serde_json::to_string(&conversation).unwrap();
        let deserialized: Conversation = serde_json::from_str(&json).unwrap();
        assert_eq!(conversation, deserialized);
    }

    #[test]
    fn test_updated_at_changes() {
        let mut conversation = Conversation::new(Uuid::new_v4());
        let initial_updated = conversation.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        conversation.add_message(Message::user("Test".to_string()));

        assert!(conversation.updated_at > initial_updated);
    }
}
