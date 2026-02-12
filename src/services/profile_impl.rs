//! Profile service implementation

use super::{ProfileService, ServiceResult};
use crate::models::{AuthConfig, ModelParameters, ModelProfile};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// File-based implementation of ProfileService
pub struct ProfileServiceImpl {
    profiles_dir: PathBuf,
    profiles: Arc<RwLock<Vec<ModelProfile>>>,
}

impl ProfileServiceImpl {
    /// Create a new ProfileServiceImpl
    ///
    /// # Errors
    ///
    /// Returns an error if the profiles directory cannot be created.
    pub fn new(profiles_dir: PathBuf) -> Result<Self, super::ServiceError> {
        // Ensure profiles directory exists
        fs::create_dir_all(&profiles_dir)
            .map_err(|e| super::ServiceError::Io(format!("Failed to create profiles directory: {e}")))?;

        let service = Self {
            profiles_dir,
            profiles: Arc::new(RwLock::new(Vec::new())),
        };

        // Don't load existing profiles in new() to avoid blocking_write()
        // The profiles will be loaded on-demand or through a separate init method

        Ok(service)
    }

    /// Initialize the service by loading existing profiles from disk
    ///
    /// # Errors
    ///
    /// Returns an error if the profiles cannot be loaded from disk.
    pub async fn initialize(&self) -> Result<(), super::ServiceError> {
        let profiles = self.load_profiles_from_disk()?;
        tracing::info!("ProfileService: loaded {} profiles from disk", profiles.len());
        for p in &profiles {
            tracing::info!("  Profile: {} ({}) id={}", p.name, p.model_id, p.id);
        }
        let mut profiles_lock = self.profiles.write().await;
        *profiles_lock = profiles;
        Ok(())
    }

    /// Load all profiles from disk
    fn load_profiles_from_disk(&self) -> Result<Vec<ModelProfile>, super::ServiceError> {
        let mut profiles = Vec::new();

        tracing::info!("load_profiles_from_disk: looking in {:?}", self.profiles_dir);
        if !self.profiles_dir.exists() {
            tracing::warn!("load_profiles_from_disk: directory does not exist");
            return Ok(profiles);
        }

        let entries = fs::read_dir(&self.profiles_dir).map_err(|e| {
            super::ServiceError::Io(format!("Failed to read profiles directory: {e}"))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| super::ServiceError::Io(format!("Failed to read directory entry: {e}")))?;

            let path = entry.path();

            // Only process JSON files
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            // Skip default.json (it's just a UUID reference, not a profile)
            if path.file_name().and_then(|n| n.to_str()) == Some("default.json") {
                continue;
            }

            // Read and parse profile
            let content = fs::read_to_string(&path).map_err(|e| {
                super::ServiceError::Io(format!("Failed to read profile file {}: {e}", path.display()))
            })?;

            let profile: ModelProfile = match serde_json::from_str(&content) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Skipping invalid profile {}: {}", path.display(), e);
                    continue;
                }
            };

            profiles.push(profile);
        }

        Ok(profiles)
    }

    /// Save a profile to disk
    fn save_profile_to_disk(&self, profile: &ModelProfile) -> Result<(), super::ServiceError> {
        let filename = format!("{}.json", profile.id);
        let path = self.profiles_dir.join(filename);

        let content = serde_json::to_string_pretty(profile).map_err(|e| {
            super::ServiceError::Serialization(format!("Failed to serialize profile {}: {e}", profile.id))
        })?;

        fs::write(&path, content)
            .map_err(|e| super::ServiceError::Io(format!("Failed to write profile file {}: {e}", path.display())))?;

        Ok(())
    }

    /// Delete a profile from disk
    fn delete_profile_from_disk(&self, id: Uuid) -> Result<(), super::ServiceError> {
        let filename = format!("{}.json", id);
        let path = self.profiles_dir.join(filename);

        if path.exists() {
            fs::remove_file(&path).map_err(|e| {
                super::ServiceError::Io(format!("Failed to delete profile file {}: {e}", path.display()))
            })?;
        }

        Ok(())
    }

    /// Get the default profile file path
    fn default_profile_path(&self) -> PathBuf {
        self.profiles_dir.join("default.json")
    }

    /// Load the default profile ID from disk
    fn load_default_id(&self) -> Result<Option<Uuid>, super::ServiceError> {
        let path = self.default_profile_path();

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            super::ServiceError::Io(format!("Failed to read default profile file: {e}"))
        })?;

        serde_json::from_str(&content)
            .map(Some)
            .map_err(|e| super::ServiceError::Serialization(format!("Failed to parse default profile ID: {e}")))
    }

    /// Save the default profile ID to disk
    fn save_default_id(&self, id: Uuid) -> Result<(), super::ServiceError> {
        let path = self.default_profile_path();

        let content = serde_json::to_string(&id).map_err(|e| {
            super::ServiceError::Serialization(format!("Failed to serialize default profile ID: {e}"))
        })?;

        fs::write(&path, content)
            .map_err(|e| super::ServiceError::Io(format!("Failed to write default profile file: {e}")))
    }
}

#[async_trait::async_trait]
impl ProfileService for ProfileServiceImpl {
    /// List all profiles
    async fn list(&self) -> ServiceResult<Vec<ModelProfile>> {
        let profiles = self.profiles.read().await;
        Ok(profiles.clone())
    }

    /// Get a profile by ID
    async fn get(&self, id: Uuid) -> ServiceResult<ModelProfile> {
        let profiles = self.profiles.read().await;

        profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| super::ServiceError::NotFound(format!("Profile {id} not found")))
    }

    /// Create a new profile
    async fn create(
        &self,
        name: String,
        provider: String,
        model: String,
        auth: AuthConfig,
        parameters: ModelParameters,
    ) -> ServiceResult<ModelProfile> {
        let profile = ModelProfile::new(name, provider, model, "https://api.openai.com/v1".to_string(), auth)
            .with_parameters(parameters);

        // Check if profile with same ID already exists (unlikely but possible)
        {
            let profiles = self.profiles.read().await;
            if profiles.iter().any(|p| p.id == profile.id) {
                return Err(super::ServiceError::Internal(format!("Profile {} already exists", profile.id)));
            }
        }

        // Save to disk
        self.save_profile_to_disk(&profile)?;

        // Add to in-memory cache
        let mut profiles = self.profiles.write().await;
        profiles.push(profile.clone());

        Ok(profile)
    }

    /// Update an existing profile
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        model: Option<String>,
        auth: Option<AuthConfig>,
        parameters: Option<ModelParameters>,
    ) -> ServiceResult<ModelProfile> {
        // Check if profile exists
        let mut profiles = self.profiles.write().await;

        let profile = profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| super::ServiceError::NotFound(format!("Profile {id} not found")))?;

        // Update fields
        if let Some(name) = name {
            profile.set_name(name);
        }
        if let Some(model) = model {
            profile.model_id = model;
        }
        if let Some(auth) = auth {
            profile.set_auth(auth);
        }
        if let Some(parameters) = parameters {
            profile.set_parameters(parameters);
        }

        let updated_profile = profile.clone();

        // Save to disk
        drop(profiles); // Release lock before I/O
        self.save_profile_to_disk(&updated_profile)?;

        Ok(updated_profile)
    }

    /// Delete a profile
    async fn delete(&self, id: Uuid) -> ServiceResult<()> {
        // Check if profile exists
        {
            let profiles = self.profiles.read().await;
            if !profiles.iter().any(|p| p.id == id) {
                return Err(super::ServiceError::NotFound(format!("Profile {id} not found")));
            }
        }

        // Delete from disk
        self.delete_profile_from_disk(id)?;

        // Remove from in-memory cache
        let mut profiles = self.profiles.write().await;
        profiles.retain(|p| p.id != id);

        Ok(())
    }

    /// Test connection to a model provider
    async fn test_connection(&self, _id: Uuid) -> ServiceResult<()> {
        // TODO: Implement actual LLM connection test (Phase 09 stretch goal)
        Ok(())
    }

    /// Get the default profile
    async fn get_default(&self) -> ServiceResult<Option<ModelProfile>> {
        let default_id = self.load_default_id()?;
        tracing::info!("get_default: default_id = {:?}", default_id);

        match default_id {
            Some(id) => {
                let profiles = self.profiles.read().await;
                tracing::info!("get_default: searching {} profiles for id {}", profiles.len(), id);
                let found = profiles.iter().find(|p| p.id == id).cloned();
                tracing::info!("get_default: found = {:?}", found.as_ref().map(|p| &p.name));
                Ok(found)
            }
            None => {
                tracing::info!("get_default: no default id set");
                Ok(None)
            }
        }
    }

    /// Set a profile as the default
    async fn set_default(&self, id: Uuid) -> ServiceResult<()> {
        // Check if profile exists
        {
            let profiles = self.profiles.read().await;
            if !profiles.iter().any(|p| p.id == id) {
                return Err(super::ServiceError::NotFound(format!("Profile {id} not found")));
            }
        }

        // Save default ID to disk
        self.save_default_id(id)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuthConfig, ModelParameters};

    #[tokio::test]
    async fn test_create_and_list_profiles() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        // Create profile
        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let params = ModelParameters::default();

        let profile = service
            .create("Profile 1".to_string(), "openai".to_string(), "gpt-4".to_string(), auth, params)
            .await
            .unwrap();

        // List profiles
        let profiles = service.list().await.unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Profile 1");
    }

    #[tokio::test]
    async fn test_get_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let params = ModelParameters::default();

        let profile = service
            .create("Profile 1".to_string(), "openai".to_string(), "gpt-4".to_string(), auth, params)
            .await
            .unwrap();

        let retrieved = service.get(profile.id).await.unwrap();
        assert_eq!(retrieved.name, "Profile 1");
    }

    #[tokio::test]
    async fn test_update_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let params = ModelParameters::default();

        let profile = service
            .create("Profile 1".to_string(), "openai".to_string(), "gpt-4".to_string(), auth, params)
            .await
            .unwrap();

        service
            .update(profile.id, Some("Updated Profile".to_string()), None, None, None)
            .await
            .unwrap();

        let retrieved = service.get(profile.id).await.unwrap();
        assert_eq!(retrieved.name, "Updated Profile");
    }

    #[tokio::test]
    async fn test_delete_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let params = ModelParameters::default();

        let profile = service
            .create("Profile 1".to_string(), "openai".to_string(), "gpt-4".to_string(), auth, params)
            .await
            .unwrap();

        service.delete(profile.id).await.unwrap();

        let profiles = service.list().await.unwrap();
        assert_eq!(profiles.len(), 0);

        assert!(service.get(profile.id).await.is_err());
    }

    #[tokio::test]
    async fn test_set_default_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let params = ModelParameters::default();

        let profile = service
            .create("Profile 1".to_string(), "openai".to_string(), "gpt-4".to_string(), auth, params)
            .await
            .unwrap();

        service.set_default(profile.id).await.unwrap();

        let default = service.get_default().await.unwrap().unwrap();
        assert_eq!(default.id, profile.id);
    }

    #[tokio::test]
    async fn test_test_connection() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let auth = AuthConfig::Key { value: "test-key".to_string() };
        let params = ModelParameters::default();

        let profile = service
            .create("Profile 1".to_string(), "openai".to_string(), "gpt-4".to_string(), auth, params)
            .await
            .unwrap();

        // Should return Ok(()) for now
        service.test_connection(profile.id).await.unwrap();
    }
}
