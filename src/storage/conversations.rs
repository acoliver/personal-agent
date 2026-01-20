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
