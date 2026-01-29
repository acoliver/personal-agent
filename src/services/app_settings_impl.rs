use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;

use crate::services::{ServiceError, ServiceResult};
use crate::services::app_settings::AppSettingsService;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettingsStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    default_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hotkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    #[serde(flatten)]
    extra_settings: HashMap<String, String>,
}

impl Default for AppSettingsStorage {
    fn default() -> Self {
        Self {
            default_profile_id: None,
            current_conversation_id: None,
            hotkey: None,
            theme: None,
            extra_settings: HashMap::new(),
        }
    }
}

pub struct AppSettingsServiceImpl {
    file_path: PathBuf,
    settings: Mutex<AppSettingsStorage>,
}

impl AppSettingsServiceImpl {
    pub fn new(file_path: PathBuf) -> Result<Self, ServiceError> {
        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ServiceError::Storage(format!("Failed to create parent directory: {}", e)))?;
        }

        let storage = if file_path.exists() {
            let content = fs::read_to_string(&file_path)
                .map_err(|e| ServiceError::Storage(format!("Failed to read settings file: {}", e)))?;
            
            serde_json::from_str(&content)
                .map_err(|e| ServiceError::Validation(format!("Failed to parse settings JSON: {}", e)))?
        } else {
            AppSettingsStorage::default()
        };

        Ok(Self {
            file_path,
            settings: Mutex::new(storage),
        })
    }

    fn load(&self) -> Result<AppSettingsStorage, ServiceError> {
        if !self.file_path.exists() {
            return Ok(AppSettingsStorage::default());
        }

        let content = fs::read_to_string(&self.file_path)
            .map_err(|e| ServiceError::Storage(format!("Failed to read settings file: {}", e)))?;
        
        serde_json::from_str(&content)
            .map_err(|e| ServiceError::Validation(format!("Failed to parse settings JSON: {}", e)))
    }

    fn save(&self, storage: &AppSettingsStorage) -> Result<(), ServiceError> {
        let content = serde_json::to_string_pretty(storage)
            .map_err(|e| ServiceError::Serialization(format!("Failed to serialize settings: {}", e)))?;
        
        fs::write(&self.file_path, content)
            .map_err(|e| ServiceError::Storage(format!("Failed to write settings file: {}", e)))
    }
}

#[async_trait]
impl AppSettingsService for AppSettingsServiceImpl {
    async fn get_default_profile_id(&self) -> ServiceResult<Option<Uuid>> {
        let storage = self.load()?;
        match storage.default_profile_id {
            None => Ok(None),
            Some(ref s) => {
                let uuid = Uuid::parse_str(s)
                    .map_err(|_| ServiceError::Validation("Invalid profile ID UUID".to_string()))?;
                Ok(Some(uuid))
            }
        }
    }

    async fn set_default_profile_id(&self, id: Uuid) -> ServiceResult<()> {
        let mut storage = self.load()?;
        storage.default_profile_id = Some(id.to_string());
        self.save(&storage)?;
        
        let mut current = self.settings.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *current = storage;
        Ok(())
    }

    async fn get_current_conversation_id(&self) -> ServiceResult<Option<Uuid>> {
        let storage = self.load()?;
        match storage.current_conversation_id {
            None => Ok(None),
            Some(ref s) => {
                let uuid = Uuid::parse_str(s)
                    .map_err(|_| ServiceError::Validation("Invalid conversation ID UUID".to_string()))?;
                Ok(Some(uuid))
            }
        }
    }

    async fn set_current_conversation_id(&self, id: Uuid) -> ServiceResult<()> {
        let mut storage = self.load()?;
        storage.current_conversation_id = Some(id.to_string());
        self.save(&storage)?;
        
        let mut current = self.settings.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *current = storage;
        Ok(())
    }

    async fn get_hotkey(&self) -> ServiceResult<Option<String>> {
        let storage = self.load()?;
        Ok(storage.hotkey)
    }

    async fn set_hotkey(&self, hotkey: String) -> ServiceResult<()> {
        let mut storage = self.load()?;
        storage.hotkey = Some(hotkey);
        self.save(&storage)?;
        
        let mut current = self.settings.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *current = storage;
        Ok(())
    }

    async fn get_theme(&self) -> ServiceResult<Option<String>> {
        let storage = self.load()?;
        Ok(storage.theme)
    }

    async fn set_theme(&self, theme: String) -> ServiceResult<()> {
        let mut storage = self.load()?;
        storage.theme = Some(theme);
        self.save(&storage)?;
        
        let mut current = self.settings.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *current = storage;
        Ok(())
    }

    async fn get_setting(&self, key: &str) -> ServiceResult<Option<String>> {
        let storage = self.load()?;
        Ok(storage.extra_settings.get(key).cloned())
    }

    async fn set_setting(&self, key: &str, value: String) -> ServiceResult<()> {
        let mut storage = self.load()?;
        storage.extra_settings.insert(key.to_string(), value);
        self.save(&storage)?;
        
        let mut current = self.settings.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *current = storage;
        Ok(())
    }

    async fn reset_to_defaults(&self) -> ServiceResult<()> {
        let storage = AppSettingsStorage::default();
        self.save(&storage)?;
        
        let mut current = self.settings.lock()
            .map_err(|e| ServiceError::Storage(format!("Failed to acquire lock: {}", e)))?;
        *current = storage;
        Ok(())
    }
}
