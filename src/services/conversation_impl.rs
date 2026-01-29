use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;
use async_trait::async_trait;

use crate::models::{Conversation, Message, MessageRole};
use crate::services::{ServiceError, ServiceResult};
use crate::services::conversation::ConversationService;

pub struct ConversationServiceImpl {
    storage_dir: PathBuf,
    active_id: Mutex<Option<Uuid>>,
}

impl ConversationServiceImpl {
    pub fn new(storage_dir: PathBuf) -> Result<Self, ServiceError> {
        fs::create_dir_all(&storage_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to create storage directory: {}", e)))?;
        
        Ok(Self {
            storage_dir,
            active_id: Mutex::new(None),
        })
    }

    fn get_conversation_path(&self, id: Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.json", id))
    }

    fn load_conversation(&self, id: Uuid) -> Result<Conversation, ServiceError> {
        let path = self.get_conversation_path(id);
        let content = fs::read_to_string(&path)
            .map_err(|e| ServiceError::NotFound(format!("Failed to read conversation: {}", e)))?;
        
        let conversation: Conversation = serde_json::from_str(&content)
            .map_err(|e| ServiceError::Serialization(format!("Failed to parse conversation JSON: {}", e)))?;
        
        Ok(conversation)
    }

    fn save_conversation(&self, conversation: &Conversation) -> Result<(), ServiceError> {
        let path = self.get_conversation_path(conversation.id);
        let content = serde_json::to_string_pretty(&conversation)
            .map_err(|e| ServiceError::Serialization(format!("Failed to serialize conversation: {}", e)))?;
        
        fs::write(&path, content)
            .map_err(|e| ServiceError::Storage(format!("Failed to write conversation: {}", e)))
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

    async fn list(&self, limit: Option<usize>, offset: Option<usize>) -> ServiceResult<Vec<Conversation>> {
        let mut conversations = Vec::new();
        
        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| ServiceError::Storage(format!("Failed to read storage directory: {}", e)))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| ServiceError::Storage(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)
                .map_err(|e| ServiceError::Storage(format!("Failed to read conversation file: {}", e)))?;
            
            let conversation: Conversation = serde_json::from_str(&content)
                .map_err(|e| ServiceError::Serialization(format!("Failed to parse conversation JSON: {}", e)))?;
            
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

    async fn add_user_message(&self, conversation_id: Uuid, content: String) -> ServiceResult<Message> {
        let mut conversation = self.load_conversation(conversation_id)?;
        
        let message = Message::user(content);
        conversation.add_message(message.clone());
        
        self.save_conversation(&conversation)?;
        Ok(message)
    }

    async fn add_assistant_message(&self, conversation_id: Uuid, content: String) -> ServiceResult<Message> {
        let mut conversation = self.load_conversation(conversation_id)?;
        
        let message = Message::assistant(content);
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
        let path = self.get_conversation_path(id);
        
        if !path.exists() {
            return Err(ServiceError::NotFound(format!("Conversation not found: {}", id)));
        }
        
        fs::remove_file(&path)
            .map_err(|e| ServiceError::Storage(format!("Failed to delete conversation: {}", e)))
    }

    async fn set_active(&self, id: Uuid) -> ServiceResult<()> {
        self.load_conversation(id)?;
        
        let mut active = self.active_id.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *active = Some(id);
        Ok(())
    }

    async fn get_active(&self) -> ServiceResult<Option<Uuid>> {
        let active = self.active_id.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
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
