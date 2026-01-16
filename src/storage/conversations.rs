//! Conversation storage operations

use std::fs;
use std::path::PathBuf;

use crate::error::{AppError, Result};
use crate::models::Conversation;

pub struct ConversationStorage {
    base_path: PathBuf,
}

impl ConversationStorage {
    /// Create a new storage instance with the given base path
    pub fn new<P: Into<PathBuf>>(base_path: P) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Create storage with default path
    ///
    /// # Errors
    /// Returns error if default path cannot be determined
    pub fn with_default_path() -> Result<Self> {
        let path = Self::default_path()?;
        Ok(Self::new(path))
    }

    /// Get the default conversations directory path
    ///
    /// # Errors
    /// Returns error if application support directory cannot be determined
    pub fn default_path() -> Result<PathBuf> {
        let app_support = dirs::data_local_dir().ok_or_else(|| {
            AppError::Storage("Could not determine application support directory".to_string())
        })?;

        Ok(app_support.join("PersonalAgent").join("conversations"))
    }

    /// Ensure the storage directory exists
    fn ensure_directory(&self) -> Result<()> {
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path)?;
        }
        Ok(())
    }

    /// Save a conversation to disk
    ///
    /// # Errors
    /// Returns error if directory cannot be created or file cannot be written
    pub fn save(&self, conversation: &Conversation) -> Result<()> {
        self.ensure_directory()?;

        let filename = conversation.filename();
        let path = self.base_path.join(&filename);

        let contents = serde_json::to_string_pretty(conversation)?;
        fs::write(&path, contents)?;

        Ok(())
    }

    /// Load a conversation by filename
    ///
    /// # Errors
    /// Returns error if file does not exist or cannot be parsed
    pub fn load(&self, filename: &str) -> Result<Conversation> {
        let path = self.base_path.join(filename);

        if !path.exists() {
            return Err(AppError::ConversationNotFound(filename.to_string()));
        }

        let contents = fs::read_to_string(&path)?;
        let conversation = serde_json::from_str(&contents)?;

        Ok(conversation)
    }

    /// List all conversation filenames
    ///
    /// # Errors
    /// Returns error if directory cannot be read
    pub fn list(&self) -> Result<Vec<String>> {
        if !self.base_path.exists() {
            return Ok(Vec::new());
        }

        let mut filenames = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if let Some(filename) = path.file_name() {
                    filenames.push(filename.to_string_lossy().to_string());
                }
            }
        }

        // Sort in reverse chronological order (newest first)
        filenames.sort_by(|a, b| b.cmp(a));

        Ok(filenames)
    }

    /// Delete a conversation by filename
    ///
    /// # Errors
    /// Returns error if file does not exist or cannot be deleted
    pub fn delete(&self, filename: &str) -> Result<()> {
        let path = self.base_path.join(filename);

        if !path.exists() {
            return Err(AppError::ConversationNotFound(filename.to_string()));
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    /// Load all conversations
    ///
    /// # Errors
    /// Returns error if directory cannot be read
    pub fn load_all(&self) -> Result<Vec<Conversation>> {
        let filenames = self.list()?;
        let mut conversations = Vec::new();

        for filename in filenames {
            match self.load(&filename) {
                Ok(conv) => conversations.push(conv),
                Err(e) => {
                    eprintln!("Warning: Failed to load conversation {filename}: {e}");
                }
            }
        }

        Ok(conversations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Message;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_save_and_load() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let mut conversation = Conversation::new(Uuid::new_v4());
        conversation.add_message(Message::user("Hello".to_string()));
        conversation.add_message(Message::assistant("Hi there".to_string()));

        storage.save(&conversation)?;

        let filename = conversation.filename();
        let loaded = storage.load(&filename)?;

        assert_eq!(loaded.id, conversation.id);
        assert_eq!(loaded.messages.len(), 2);

        Ok(())
    }

    #[test]
    fn test_load_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let result = storage.load("nonexistent.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_empty() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let filenames = storage.list()?;
        assert_eq!(filenames.len(), 0);

        Ok(())
    }

    #[test]
    fn test_list_conversations() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let conv1 = Conversation::new(Uuid::new_v4());
        storage.save(&conv1)?;

        // Sleep briefly to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(2));

        let conv2 = Conversation::new(Uuid::new_v4());
        storage.save(&conv2)?;

        let filenames = storage.list()?;
        assert_eq!(filenames.len(), 2);

        Ok(())
    }

    #[test]
    fn test_list_sorted() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        // Create conversations with different timestamps
        let mut conv1 = Conversation::new(Uuid::new_v4());
        conv1.created_at = chrono::DateTime::parse_from_rfc3339("2026-01-14T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        let mut conv2 = Conversation::new(Uuid::new_v4());
        conv2.created_at = chrono::DateTime::parse_from_rfc3339("2026-01-14T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        storage.save(&conv1)?;
        storage.save(&conv2)?;

        let filenames = storage.list()?;

        // Newest (conv2) should be first
        assert_eq!(filenames[0], conv2.filename());
        assert_eq!(filenames[1], conv1.filename());

        Ok(())
    }

    #[test]
    fn test_delete() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let conversation = Conversation::new(Uuid::new_v4());
        storage.save(&conversation)?;

        let filename = conversation.filename();
        storage.delete(&filename)?;

        let result = storage.load(&filename);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_delete_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let result = storage.delete("nonexistent.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_all() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage = ConversationStorage::new(temp_dir.path());

        let conv1 = Conversation::new(Uuid::new_v4());
        storage.save(&conv1)?;

        // Sleep briefly to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(2));

        let conv2 = Conversation::new(Uuid::new_v4());
        storage.save(&conv2)?;

        let conversations = storage.load_all()?;
        assert_eq!(conversations.len(), 2);

        Ok(())
    }

    #[test]
    fn test_ensure_directory_creates() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("nested").join("path");
        let storage = ConversationStorage::new(&storage_path);

        assert!(!storage_path.exists());

        let conversation = Conversation::new(Uuid::new_v4());
        storage.save(&conversation)?;

        assert!(storage_path.exists());

        Ok(())
    }
}
