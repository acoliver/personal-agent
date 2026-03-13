//! Profile service implementation

use super::{ProfileService, ServiceResult};
use crate::config::default_api_base_url_for_provider;
use crate::models::{AuthConfig, ModelParameters, ModelProfile};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

/// File-based implementation of ProfileService
pub struct ProfileServiceImpl {
    profiles_dir: PathBuf,
    profiles: Arc<RwLock<Vec<ModelProfile>>>,
}

impl ProfileServiceImpl {
    fn legacy_profile_id_for_path(path: &Path) -> Uuid {
        let identifier = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("legacy-profile:{name}"))
            .unwrap_or_else(|| format!("legacy-profile:{}", path.to_string_lossy()));

        Uuid::new_v5(&Uuid::NAMESPACE_URL, identifier.as_bytes())
    }

    fn normalize_api_base_url(provider: &str, base_url: Option<String>) -> String {
        match base_url {
            Some(candidate) if !candidate.trim().is_empty() => candidate.trim().to_string(),
            _ => default_api_base_url_for_provider(provider),
        }
    }

    fn normalize_system_prompt(system_prompt: Option<String>) -> String {
        match system_prompt {
            Some(candidate) if !candidate.trim().is_empty() => candidate.trim().to_string(),
            _ => "You are a helpful assistant, be direct and to the point. Respond in English."
                .to_string(),
        }
    }

    /// Create a new ProfileServiceImpl
    ///
    /// # Errors
    ///
    /// Returns an error if the profiles directory cannot be created.
    pub fn new(profiles_dir: PathBuf) -> Result<Self, super::ServiceError> {
        // Ensure profiles directory exists
        fs::create_dir_all(&profiles_dir).map_err(|e| {
            super::ServiceError::Io(format!("Failed to create profiles directory: {e}"))
        })?;

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
        tracing::info!(
            "ProfileService: loaded {} profiles from disk",
            profiles.len()
        );
        for p in &profiles {
            tracing::info!("  Profile: {} ({}) id={}", p.name, p.model_id, p.id);
        }
        let mut profiles_lock = self.profiles.write().await;
        *profiles_lock = profiles;
        Ok(())
    }

    fn parse_auth_from_legacy(value: Option<&Value>, keyfile_hint: Option<String>) -> AuthConfig {
        if let Some(Value::Object(auth_obj)) = value {
            if let Some(Value::String(kind)) = auth_obj.get("type") {
                match kind.as_str() {
                    "key" => {
                        if let Some(Value::String(v)) = auth_obj.get("value") {
                            return AuthConfig::Key { value: v.clone() };
                        }
                    }
                    "keyfile" => {
                        if let Some(Value::String(path)) = auth_obj.get("path") {
                            return AuthConfig::Keyfile { path: path.clone() };
                        }
                    }
                    "oauth" => {
                        // OAuth cannot be represented in current AuthConfig, so degrade to empty key.
                        tracing::warn!("Legacy oauth auth config encountered; mapping to empty key auth for compatibility");
                        return AuthConfig::Key {
                            value: String::new(),
                        };
                    }
                    _ => {}
                }
            }
        }

        if let Some(path) = keyfile_hint {
            return AuthConfig::Keyfile { path };
        }

        AuthConfig::Key {
            value: String::new(),
        }
    }

    fn resolve_key_name_to_path(key_name: &str) -> Option<String> {
        let trimmed = key_name.trim();
        if trimmed.is_empty() {
            return None;
        }

        let direct = Path::new(trimmed);
        if direct.is_absolute() && direct.exists() {
            return Some(trimmed.to_string());
        }

        if let Some(home) = dirs::home_dir() {
            let normalized = trimmed.trim_end_matches(|c: char| c.is_ascii_digit());
            let mut key_names = vec![trimmed.to_string()];
            if !normalized.is_empty() && normalized != trimmed {
                key_names.push(normalized.to_string());
            }

            for key_name in key_names {
                let candidates = [
                    home.join(".keys").join(format!(".{}_key", key_name)),
                    home.join(".keys").join(&key_name),
                    home.join(".llxprt")
                        .join("keys")
                        .join(format!(".{}_key", key_name)),
                    home.join(".llxprt").join("keys").join(&key_name),
                ];

                for candidate in candidates {
                    if candidate.exists() {
                        return Some(candidate.to_string_lossy().to_string());
                    }
                }
            }
        }

        None
    }

    fn parse_legacy_auth_key_name(ephemeral: Option<&Value>) -> Option<String> {
        let key_name = ephemeral
            .and_then(Value::as_object)
            .and_then(|e| e.get("auth-key-name"))
            .and_then(Value::as_str)?;

        Self::resolve_key_name_to_path(key_name)
    }

    fn parse_parameters_from_legacy(
        value: Option<&Value>,
        ephemeral: Option<&Value>,
    ) -> ModelParameters {
        let mut params = ModelParameters::default();

        if let Some(Value::Object(obj)) = value {
            if let Some(Value::Number(n)) = obj.get("temperature").or_else(|| obj.get("temp")) {
                if let Some(v) = n.as_f64() {
                    params.temperature = v;
                }
            }
            if let Some(Value::Number(n)) = obj.get("top_p") {
                if let Some(v) = n.as_f64() {
                    params.top_p = v;
                }
            }
            if let Some(Value::Number(n)) =
                obj.get("max_tokens").or_else(|| obj.get("maxOutputTokens"))
            {
                if let Some(v) = n.as_u64() {
                    params.max_tokens = v.min(u32::MAX as u64) as u32;
                }
            }
            if let Some(Value::Bool(v)) = obj
                .get("enable_thinking")
                .or_else(|| obj.get("reasoning.enabled"))
            {
                params.enable_thinking = *v;
            }
            if let Some(Value::Bool(v)) = obj
                .get("show_thinking")
                .or_else(|| obj.get("reasoning.includeInResponse"))
            {
                params.show_thinking = *v;
            }
            if let Some(Value::Number(n)) = obj.get("thinking_budget") {
                params.thinking_budget = n.as_u64().map(|v| v.min(u32::MAX as u64) as u32);
            }
        }

        if let Some(Value::Object(obj)) = ephemeral {
            if let Some(Value::Number(n)) =
                obj.get("maxOutputTokens").or_else(|| obj.get("max_tokens"))
            {
                if let Some(v) = n.as_u64() {
                    params.max_tokens = v.min(u32::MAX as u64) as u32;
                }
            }
            if let Some(Value::Bool(v)) = obj.get("reasoning.enabled") {
                params.enable_thinking = *v;
            }
            if let Some(Value::Bool(v)) = obj.get("reasoning.includeInResponse") {
                params.show_thinking = *v;
            }
        }

        params
    }

    fn parse_legacy_profile(value: &Value, path: &std::path::Path) -> Option<ModelProfile> {
        let obj = value.as_object()?;

        // Already in modern schema but missing / invalid id
        if obj.contains_key("provider_id") || obj.contains_key("model_id") {
            let name = obj
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Legacy Profile")
                        .to_string()
                });
            let provider_id = obj
                .get("provider_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "openai".to_string());
            let model_id = obj
                .get("model_id")
                .or_else(|| obj.get("model"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "gpt-4".to_string());
            let base_url = obj
                .get("base_url")
                .or_else(|| obj.get("api_base_url"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let ephemeral = obj.get("ephemeralSettings");
            let keyfile_hint = ephemeral
                .and_then(Value::as_object)
                .and_then(|e| e.get("auth-keyfile"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| Self::parse_legacy_auth_key_name(ephemeral));
            let auth = Self::parse_auth_from_legacy(obj.get("auth"), keyfile_hint);
            let parameters = Self::parse_parameters_from_legacy(obj.get("parameters"), ephemeral);
            let system_prompt = obj
                .get("system_prompt")
                .and_then(Value::as_str)
                .unwrap_or(
                    "You are a helpful assistant, be direct and to the point. Respond in English.",
                )
                .to_string();

            return Some(ModelProfile {
                id: Self::legacy_profile_id_for_path(path),
                name,
                provider_id,
                model_id,
                base_url,
                auth,
                parameters,
                system_prompt,
            });
        }

        // Legacy schema: { provider, model, modelParams, ephemeralSettings, auth? }
        if obj.contains_key("provider") && obj.contains_key("model") {
            let provider_id = obj
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or("openai")
                .to_string();
            let model_id = obj
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("gpt-4")
                .to_string();
            let name = format!("{}:{}", provider_id, model_id);

            let ephemeral = obj.get("ephemeralSettings");
            let base_url = ephemeral
                .and_then(Value::as_object)
                .and_then(|e| e.get("base-url"))
                .and_then(Value::as_str)
                .unwrap_or("https://api.openai.com/v1")
                .to_string();
            let keyfile_hint = ephemeral
                .and_then(Value::as_object)
                .and_then(|e| e.get("auth-keyfile"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| Self::parse_legacy_auth_key_name(ephemeral));

            let auth = Self::parse_auth_from_legacy(obj.get("auth"), keyfile_hint);
            let parameters = Self::parse_parameters_from_legacy(obj.get("modelParams"), ephemeral);

            return Some(ModelProfile {
                id: Self::legacy_profile_id_for_path(path),
                name,
                provider_id,
                model_id,
                base_url,
                auth,
                parameters,
                system_prompt:
                    "You are a helpful assistant, be direct and to the point. Respond in English."
                        .to_string(),
            });
        }

        None
    }

    /// Load all profiles from disk, with compatibility support for legacy schemas.
    fn load_profiles_from_disk(&self) -> Result<Vec<ModelProfile>, super::ServiceError> {
        let mut profiles = Vec::new();
        let mut seen = HashSet::<Uuid>::new();

        tracing::info!(
            "load_profiles_from_disk: looking in {:?}",
            self.profiles_dir
        );
        if !self.profiles_dir.exists() {
            tracing::warn!("load_profiles_from_disk: directory does not exist");
            return Ok(profiles);
        }

        let entries = fs::read_dir(&self.profiles_dir).map_err(|e| {
            super::ServiceError::Io(format!("Failed to read profiles directory: {e}"))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                super::ServiceError::Io(format!("Failed to read directory entry: {e}"))
            })?;

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
                super::ServiceError::Io(format!(
                    "Failed to read profile file {}: {e}",
                    path.display()
                ))
            })?;

            let mut profile: ModelProfile = match serde_json::from_str(&content) {
                Ok(p) => p,
                Err(e) => {
                    // Try legacy compatibility parse before skipping.
                    let value = match serde_json::from_str::<Value>(&content) {
                        Ok(v) => v,
                        Err(_) => {
                            tracing::warn!("Skipping invalid profile {}: {}", path.display(), e);
                            continue;
                        }
                    };

                    match Self::parse_legacy_profile(&value, &path) {
                        Some(p) => {
                            tracing::info!(
                                "Loaded legacy profile {} as {} ({}) auth={}",
                                path.display(),
                                p.name,
                                p.model_id,
                                match &p.auth {
                                    AuthConfig::Key { .. } => "key",
                                    AuthConfig::Keyfile { .. } => "keyfile",
                                }
                            );
                            p
                        }
                        None => {
                            tracing::warn!("Skipping invalid profile {}: {}", path.display(), e);
                            continue;
                        }
                    }
                }
            };

            // Guarantee non-empty critical fields.
            if profile.name.trim().is_empty() {
                profile.name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Recovered Profile")
                    .to_string();
            }
            if profile.provider_id.trim().is_empty() {
                profile.provider_id = "openai".to_string();
            }
            if profile.model_id.trim().is_empty() {
                profile.model_id = "gpt-4".to_string();
            }
            if profile.base_url.trim().is_empty() {
                profile.base_url = Self::normalize_api_base_url(&profile.provider_id, None);
            }

            // Avoid duplicate IDs if legacy conversion generated conflicting entries.
            while seen.contains(&profile.id) {
                profile.id = Uuid::new_v4();
            }
            seen.insert(profile.id);
            profiles.push(profile);
        }

        Ok(profiles)
    }

    /// Save a profile to disk
    fn save_profile_to_disk(&self, profile: &ModelProfile) -> Result<(), super::ServiceError> {
        let filename = format!("{}.json", profile.id);
        let path = self.profiles_dir.join(filename);

        let content = serde_json::to_string_pretty(profile).map_err(|e| {
            super::ServiceError::Serialization(format!(
                "Failed to serialize profile {}: {e}",
                profile.id
            ))
        })?;

        fs::write(&path, content).map_err(|e| {
            super::ServiceError::Io(format!(
                "Failed to write profile file {}: {e}",
                path.display()
            ))
        })?;

        Ok(())
    }

    /// Delete a profile from disk
    fn delete_profile_from_disk(&self, id: Uuid) -> Result<(), super::ServiceError> {
        let filename = format!("{}.json", id);
        let path = self.profiles_dir.join(filename);

        if path.exists() {
            fs::remove_file(&path).map_err(|e| {
                super::ServiceError::Io(format!(
                    "Failed to delete profile file {}: {e}",
                    path.display()
                ))
            })?;
            return Ok(());
        }

        // Backward-compatibility: legacy profiles may still be stored under non-UUID file names
        // (for example, `synthetic.json`). Those profiles now use a deterministic derived ID,
        // so we can resolve and delete the backing legacy file by matching that derived ID.
        let entries = fs::read_dir(&self.profiles_dir).map_err(|e| {
            super::ServiceError::Io(format!("Failed to read profiles directory: {e}"))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                super::ServiceError::Io(format!("Failed to read directory entry: {e}"))
            })?;

            let candidate_path = entry.path();
            if candidate_path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if candidate_path.file_name().and_then(|n| n.to_str()) == Some("default.json") {
                continue;
            }

            if Self::legacy_profile_id_for_path(&candidate_path) == id {
                fs::remove_file(&candidate_path).map_err(|e| {
                    super::ServiceError::Io(format!(
                        "Failed to delete legacy profile file {}: {e}",
                        candidate_path.display()
                    ))
                })?;
                break;
            }
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

        serde_json::from_str(&content).map(Some).map_err(|e| {
            super::ServiceError::Serialization(format!("Failed to parse default profile ID: {e}"))
        })
    }

    /// Save the default profile ID to disk
    fn save_default_id(&self, id: Uuid) -> Result<(), super::ServiceError> {
        let path = self.default_profile_path();

        let content = serde_json::to_string(&id).map_err(|e| {
            super::ServiceError::Serialization(format!(
                "Failed to serialize default profile ID: {e}"
            ))
        })?;

        fs::write(&path, content).map_err(|e| {
            super::ServiceError::Io(format!("Failed to write default profile file: {e}"))
        })
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
        base_url: Option<String>,
        auth: AuthConfig,
        parameters: ModelParameters,
        system_prompt: Option<String>,
    ) -> ServiceResult<ModelProfile> {
        let normalized_provider = provider.trim().to_string();
        let normalized_base_url = Self::normalize_api_base_url(&normalized_provider, base_url);
        let normalized_system_prompt = Self::normalize_system_prompt(system_prompt);

        let mut profile =
            ModelProfile::new(name, normalized_provider, model, normalized_base_url, auth)
                .with_parameters(parameters);
        profile.system_prompt = normalized_system_prompt;

        // Check if profile with same ID already exists (unlikely but possible)
        {
            let profiles = self.profiles.read().await;
            if profiles.iter().any(|p| p.id == profile.id) {
                return Err(super::ServiceError::Internal(format!(
                    "Profile {} already exists",
                    profile.id
                )));
            }
        }

        // Save to disk
        self.save_profile_to_disk(&profile)?;

        // Add to in-memory cache and auto-select first created profile as default.
        let mut profiles = self.profiles.write().await;
        let was_empty = profiles.is_empty();
        profiles.push(profile.clone());
        drop(profiles);

        if was_empty {
            if let Err(e) = self.save_default_id(profile.id) {
                tracing::warn!(
                    error = %e,
                    profile_id = %profile.id,
                    "Failed to persist default for first created profile"
                );
            }
        }

        Ok(profile)
    }

    /// Update an existing profile
    async fn update(
        &self,
        id: Uuid,
        name: Option<String>,
        provider: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
        auth: Option<AuthConfig>,
        parameters: Option<ModelParameters>,
        system_prompt: Option<String>,
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

        if let Some(provider) = provider {
            let trimmed = provider.trim();
            if !trimmed.is_empty() {
                profile.provider_id = trimmed.to_string();
            }
        }

        if let Some(model) = model {
            let trimmed = model.trim();
            if !trimmed.is_empty() {
                profile.model_id = trimmed.to_string();
            }
        }

        if base_url.is_some() {
            profile.base_url = Self::normalize_api_base_url(&profile.provider_id, base_url);
        }

        if let Some(auth) = auth {
            profile.set_auth(auth);
        }

        if let Some(parameters) = parameters {
            profile.set_parameters(parameters);
        }

        if system_prompt.is_some() {
            profile.system_prompt = Self::normalize_system_prompt(system_prompt);
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
                return Err(super::ServiceError::NotFound(format!(
                    "Profile {id} not found"
                )));
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

        let profiles = self.profiles.read().await;

        if profiles.is_empty() {
            tracing::info!("get_default: no profiles available");
            return Ok(None);
        }

        if let Some(id) = default_id {
            tracing::info!(
                "get_default: searching {} profiles for id {}",
                profiles.len(),
                id
            );
            if let Some(found) = profiles.iter().find(|p| p.id == id).cloned() {
                tracing::info!("get_default: found = {:?}", Some(&found.name));
                return Ok(Some(found));
            }

            tracing::warn!(
                stale_default_id = %id,
                "get_default: default profile id not found in loaded profiles; falling back"
            );
        } else {
            tracing::info!("get_default: no default id set; falling back to first profile");
        }

        let fallback = profiles[0].clone();
        drop(profiles);

        if let Err(e) = self.save_default_id(fallback.id) {
            tracing::warn!(error = %e, "get_default: failed to persist fallback default profile id");
        }

        tracing::info!(
            fallback_id = %fallback.id,
            fallback_name = %fallback.name,
            "get_default: using fallback default profile"
        );
        Ok(Some(fallback))
    }

    /// Set a profile as the default
    async fn set_default(&self, id: Uuid) -> ServiceResult<()> {
        // Check if profile exists
        {
            let profiles = self.profiles.read().await;
            if !profiles.iter().any(|p| p.id == id) {
                return Err(super::ServiceError::NotFound(format!(
                    "Profile {id} not found"
                )));
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
        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        let params = ModelParameters::default();

        let _profile = service
            .create(
                "Profile 1".to_string(),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
                auth,
                params,
                None,
            )
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

        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        let params = ModelParameters::default();

        let profile = service
            .create(
                "Profile 1".to_string(),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
                auth,
                params,
                None,
            )
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

        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        let params = ModelParameters::default();

        let profile = service
            .create(
                "Profile 1".to_string(),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
                auth,
                params,
                None,
            )
            .await
            .unwrap();

        service
            .update(
                profile.id,
                Some("Updated Profile".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
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

        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        let params = ModelParameters::default();

        let profile = service
            .create(
                "Profile 1".to_string(),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
                auth,
                params,
                None,
            )
            .await
            .unwrap();

        service.delete(profile.id).await.unwrap();

        let profiles = service.list().await.unwrap();
        assert_eq!(profiles.len(), 0);

        assert!(service.get(profile.id).await.is_err());
    }

    #[tokio::test]
    async fn test_delete_legacy_named_profile_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let legacy_path = temp_dir.path().join("synthetic.json");
        let payload = serde_json::json!({
            "version": 1,
            "provider": "openai",
            "model": "gpt-4o-mini",
            "modelParams": {
                "temperature": 1
            },
            "ephemeralSettings": {
                "base-url": "https://api.openai.com/v1"
            }
        });

        std::fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&payload).unwrap(),
        )
        .unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let profiles = service.list().await.unwrap();
        assert_eq!(profiles.len(), 1);

        service.delete(profiles[0].id).await.unwrap();

        assert!(!legacy_path.exists());
        assert!(service.list().await.unwrap().is_empty());
    }

    #[test]
    fn test_parse_legacy_profile_uses_stable_file_derived_id() {
        let payload = serde_json::json!({
            "version": 1,
            "provider": "openai",
            "model": "gpt-4o-mini",
            "modelParams": {
                "temperature": 1
            },
            "ephemeralSettings": {
                "base-url": "https://api.openai.com/v1"
            }
        });

        let path_a = std::path::Path::new("/tmp/synthetic.json");
        let path_b = std::path::Path::new("/tmp/synthetic.json");
        let path_c = std::path::Path::new("/tmp/zai.json");

        let profile_a = ProfileServiceImpl::parse_legacy_profile(&payload, path_a).unwrap();
        let profile_b = ProfileServiceImpl::parse_legacy_profile(&payload, path_b).unwrap();
        let profile_c = ProfileServiceImpl::parse_legacy_profile(&payload, path_c).unwrap();

        assert_eq!(profile_a.id, profile_b.id);
        assert_ne!(profile_a.id, profile_c.id);
    }

    #[tokio::test]
    async fn test_set_default_profile() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        let service = ProfileServiceImpl::new(temp_dir.path().to_path_buf()).unwrap();
        service.initialize().await.unwrap();

        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        let params = ModelParameters::default();

        let profile = service
            .create(
                "Profile 1".to_string(),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
                auth,
                params,
                None,
            )
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

        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        let params = ModelParameters::default();

        let profile = service
            .create(
                "Profile 1".to_string(),
                "openai".to_string(),
                "gpt-4".to_string(),
                None,
                auth,
                params,
                None,
            )
            .await
            .unwrap();

        // Should return Ok(()) for now
        service.test_connection(profile.id).await.unwrap();
    }

    #[test]
    fn test_resolve_key_name_to_path_prefers_dot_keys() {
        let home = dirs::home_dir().expect("home directory should exist for test");
        let synthetic_path = home.join(".keys").join(".synthetic1_key");

        if !synthetic_path.exists() {
            // Environment-specific test; skip when key file is absent.
            return;
        }

        let resolved = ProfileServiceImpl::resolve_key_name_to_path("synthetic1");
        assert_eq!(resolved, Some(synthetic_path.to_string_lossy().to_string()));
    }

    #[test]
    fn test_parse_legacy_auth_key_name_reads_ephemeral_setting() {
        let payload = serde_json::json!({
            "auth-key-name": "synthetic1"
        });

        let parsed = ProfileServiceImpl::parse_legacy_auth_key_name(Some(&payload));
        if let Some(path) = parsed {
            assert!(
                std::path::Path::new(&path).exists(),
                "resolved legacy auth key path should exist"
            );
        }
    }
}
