use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use crate::models::{Conversation, Message};
use crate::services::conversation::ConversationService;
use crate::services::{ServiceError, ServiceResult};

pub struct ConversationServiceImpl {
    storage_dir: PathBuf,
    active_id: Mutex<Option<Uuid>>,
}

impl ConversationServiceImpl {
    /// # Errors
    ///
    /// Returns `ServiceError` if the conversation storage directory cannot be created.
    pub fn new(storage_dir: PathBuf) -> Result<Self, ServiceError> {
        fs::create_dir_all(&storage_dir).map_err(|e| {
            ServiceError::Storage(format!("Failed to create storage directory: {e}"))
        })?;

        Ok(Self {
            storage_dir,
            active_id: Mutex::new(None),
        })
    }

    fn get_conversation_path(&self, id: Uuid) -> PathBuf {
        self.storage_dir.join(format!("{id}.json"))
    }

    fn load_conversation(&self, id: Uuid) -> Result<Conversation, ServiceError> {
        let direct_path = self.get_conversation_path(id);
        if direct_path.exists() {
            let content = fs::read_to_string(&direct_path)
                .map_err(|e| ServiceError::NotFound(format!("Failed to read conversation: {e}")))?;

            let conversation: Conversation = serde_json::from_str(&content).map_err(|e| {
                ServiceError::Serialization(format!("Failed to parse conversation JSON: {e}"))
            })?;

            return Ok(conversation);
        }

        // Compatibility path: older storage names files by timestamp (created_at),
        // while newer storage uses UUID filenames. Scan all conversation files and
        // match by in-file conversation.id so selection works across migrations.
        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to read storage directory: {e}")))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ServiceError::Storage(format!("Failed to read directory entry: {e}"))
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let conversation: Conversation = match serde_json::from_str(&content) {
                Ok(conversation) => conversation,
                Err(_) => continue,
            };

            if conversation.id == id {
                return Ok(conversation);
            }
        }

        Err(ServiceError::NotFound(format!(
            "Failed to read conversation: Conversation not found: {id}"
        )))
    }

    fn save_conversation(&self, conversation: &Conversation) -> Result<(), ServiceError> {
        // Remove any pre-migration timestamp-named file for this same conversation id
        // so we converge to UUID filenames and avoid stale duplicates.
        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to read storage directory: {e}")))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ServiceError::Storage(format!("Failed to read directory entry: {e}"))
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let is_uuid_named = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .is_some();
            if is_uuid_named {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let parsed: Conversation = match serde_json::from_str(&content) {
                Ok(conversation) => conversation,
                Err(_) => continue,
            };
            if parsed.id == conversation.id {
                let _ = fs::remove_file(&path);
            }
        }

        let path = self.get_conversation_path(conversation.id);
        let content = serde_json::to_string_pretty(&conversation).map_err(|e| {
            ServiceError::Serialization(format!("Failed to serialize conversation: {e}"))
        })?;

        fs::write(&path, content)
            .map_err(|e| ServiceError::Storage(format!("Failed to write conversation: {e}")))
    }
}

#[async_trait]
impl ConversationService for ConversationServiceImpl {
    async fn create(
        &self,
        title: Option<String>,
        model_profile_id: Uuid,
    ) -> ServiceResult<Conversation> {
        let mut conversation = Conversation::new(model_profile_id);

        if let Some(t) = title {
            conversation.set_title(t);
        }

        self.save_conversation(&conversation)?;
        Ok(conversation)
    }

    async fn load(&self, id: Uuid) -> ServiceResult<Conversation> {
        self.load_conversation(id)
    }

    async fn list(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> ServiceResult<Vec<Conversation>> {
        let mut conversations = Vec::new();

        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to read storage directory: {e}")))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ServiceError::Storage(format!("Failed to read directory entry: {e}"))
            })?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path).map_err(|e| {
                ServiceError::Storage(format!("Failed to read conversation file: {e}"))
            })?;

            let conversation: Conversation = serde_json::from_str(&content).map_err(|e| {
                ServiceError::Serialization(format!("Failed to parse conversation JSON: {e}"))
            })?;

            conversations.push(conversation);
        }

        conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(conversations.len());

        let end = std::cmp::min(offset + limit, conversations.len());
        if offset >= conversations.len() {
            return Ok(Vec::new());
        }

        Ok(conversations[offset..end].to_vec())
    }

    async fn add_user_message(
        &self,
        conversation_id: Uuid,
        content: String,
    ) -> ServiceResult<Message> {
        let mut conversation = self.load_conversation(conversation_id)?;

        let message = Message::user(content);
        conversation.add_message(message.clone());

        self.save_conversation(&conversation)?;
        Ok(message)
    }

    async fn add_assistant_message(
        &self,
        conversation_id: Uuid,
        content: String,
        thinking_content: Option<String>,
    ) -> ServiceResult<Message> {
        let mut conversation = self.load_conversation(conversation_id)?;

        let message = if let Some(thinking) = thinking_content {
            Message::assistant_with_thinking(content, thinking)
        } else {
            Message::assistant(content)
        };
        conversation.add_message(message.clone());

        self.save_conversation(&conversation)?;
        Ok(message)
    }

    async fn rename(&self, id: Uuid, new_title: String) -> ServiceResult<()> {
        let mut conversation = self.load_conversation(id)?;
        conversation.set_title(new_title);
        self.save_conversation(&conversation)?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> ServiceResult<()> {
        let direct_path = self.get_conversation_path(id);

        if direct_path.exists() {
            fs::remove_file(&direct_path).map_err(|e| {
                ServiceError::Storage(format!("Failed to delete conversation: {e}"))
            })?;
            return Ok(());
        }

        // Compatibility path for timestamp-named files.
        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to read storage directory: {e}")))?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ServiceError::Storage(format!("Failed to read directory entry: {e}"))
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let conversation: Conversation = match serde_json::from_str(&content) {
                Ok(conversation) => conversation,
                Err(_) => continue,
            };

            if conversation.id == id {
                fs::remove_file(&path).map_err(|e| {
                    ServiceError::Storage(format!("Failed to delete conversation: {e}"))
                })?;
                return Ok(());
            }
        }

        Err(ServiceError::NotFound(format!(
            "Conversation not found: {id}"
        )))
    }

    async fn set_active(&self, id: Uuid) -> ServiceResult<()> {
        self.load_conversation(id)?;

        *self
            .active_id
            .lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {e}")))? = Some(id);
        Ok(())
    }

    async fn get_active(&self) -> ServiceResult<Option<Uuid>> {
        let active = self
            .active_id
            .lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {e}")))?;
        Ok(*active)
    }

    async fn get_messages(&self, conversation_id: Uuid) -> ServiceResult<Vec<Message>> {
        let conversation = self.load_conversation(conversation_id)?;
        Ok(conversation.messages)
    }

    async fn update(
        &self,
        id: Uuid,
        title: Option<String>,
        model_profile_id: Option<Uuid>,
    ) -> ServiceResult<Conversation> {
        let mut conversation = self.load_conversation(id)?;

        if let Some(new_title) = title {
            conversation.set_title(new_title);
        }

        if let Some(new_profile_id) = model_profile_id {
            conversation.profile_id = new_profile_id;
        }

        conversation.updated_at = chrono::Utc::now();

        self.save_conversation(&conversation)?;
        Ok(conversation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_timestamp_named_conversation(
        storage_dir: &std::path::Path,
        conversation: &Conversation,
    ) {
        let filename = format!("{}.json", conversation.created_at.format("%Y%m%d%H%M%S%3f"));
        let path = storage_dir.join(filename);
        let content = serde_json::to_string_pretty(conversation).unwrap();
        fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn load_finds_timestamp_named_conversation_by_id() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = ConversationServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();

        let profile_id = Uuid::new_v4();
        let conversation = Conversation::new(profile_id);
        let id = conversation.id;

        write_timestamp_named_conversation(&service.storage_dir, &conversation);

        let loaded = service.load(id).await.unwrap();
        assert_eq!(loaded.id, id);

        let direct_uuid_file = service.storage_dir.join(format!("{id}.json"));
        assert!(
            !direct_uuid_file.exists(),
            "test precondition: only timestamp file should exist before migration save"
        );
    }

    #[tokio::test]
    async fn save_migrates_timestamp_named_file_to_uuid_filename() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = ConversationServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();

        let profile_id = Uuid::new_v4();
        let mut conversation = Conversation::new(profile_id);
        let id = conversation.id;

        write_timestamp_named_conversation(&service.storage_dir, &conversation);

        conversation.set_title("Migrated".to_string());
        service.save_conversation(&conversation).unwrap();

        let uuid_file = service.storage_dir.join(format!("{id}.json"));
        assert!(uuid_file.exists(), "uuid-named file should be written");

        let files = fs::read_dir(&service.storage_dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect::<Vec<_>>();

        assert_eq!(
            files.len(),
            1,
            "timestamp duplicate should be removed after save"
        );
    }

    #[tokio::test]
    async fn delete_removes_timestamp_named_file_when_uuid_filename_missing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = ConversationServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();

        let profile_id = Uuid::new_v4();
        let conversation = Conversation::new(profile_id);
        let id = conversation.id;

        write_timestamp_named_conversation(&service.storage_dir, &conversation);

        service.delete(id).await.unwrap();

        let remaining = fs::read_dir(&service.storage_dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .count();

        assert_eq!(
            remaining, 0,
            "delete should remove timestamp-named legacy file"
        );
    }
}
