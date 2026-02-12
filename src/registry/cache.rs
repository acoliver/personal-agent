//! Cache management for model registry

use crate::error::{AppError, Result};
use crate::registry::types::ModelRegistry;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Cache metadata and data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRegistry {
    /// Timestamp when the cache was created
    pub cached_at: DateTime<Utc>,
    /// The cached registry data
    pub data: ModelRegistry,
}

/// Cache manager for model registry
pub struct RegistryCache {
    cache_path: PathBuf,
    expiry_duration: Duration,
}

impl RegistryCache {
    /// Create a new cache manager
    ///
    /// # Arguments
    ///
    /// * `cache_path` - Path to the cache file
    /// * `expiry_hours` - Number of hours before cache expires
    #[must_use]
    pub const fn new(cache_path: PathBuf, expiry_hours: i64) -> Self {
        Self {
            cache_path,
            expiry_duration: Duration::hours(expiry_hours),
        }
    }

    /// Get the default cache path
    ///
    /// # Errors
    ///
    /// Returns `AppError::Storage` if the local data directory cannot be determined
    pub fn default_path() -> Result<PathBuf> {
        let app_support = dirs::data_local_dir()
            .ok_or_else(|| AppError::Storage("Cannot determine data directory".to_string()))?;

        let cache_dir = app_support.join("PersonalAgent").join("cache");
        Ok(cache_dir.join("models.json"))
    }

    /// Load the cached registry if it exists and is not expired
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be read or parsed
    pub fn load(&self) -> Result<Option<ModelRegistry>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.cache_path)?;
        let cached: CachedRegistry = serde_json::from_str(&content)?;

        if self.is_expired(&cached.cached_at) {
            Ok(None)
        } else {
            Ok(Some(cached.data))
        }
    }

    /// Save the registry to cache
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be created, the registry cannot be serialized, or the cache file cannot be written
    pub fn save(&self, registry: &ModelRegistry) -> Result<()> {
        let cached = CachedRegistry {
            cached_at: Utc::now(),
            data: registry.clone(),
        };

        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&cached)?;
        fs::write(&self.cache_path, content)?;

        Ok(())
    }

    /// Delete the cache file
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be deleted
    pub fn clear(&self) -> Result<()> {
        if self.cache_path.exists() {
            fs::remove_file(&self.cache_path)?;
        }
        Ok(())
    }

    /// Check if the cache is expired
    fn is_expired(&self, cached_at: &DateTime<Utc>) -> bool {
        let now = Utc::now();
        now.signed_duration_since(*cached_at) > self.expiry_duration
    }

    /// Get cache metadata (age, size)
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be read or parsed
    pub fn metadata(&self) -> Result<Option<CacheMetadata>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let file_metadata = fs::metadata(&self.cache_path)?;
        let content = fs::read_to_string(&self.cache_path)?;
        let cached: CachedRegistry = serde_json::from_str(&content)?;

        Ok(Some(CacheMetadata {
            cached_at: cached.cached_at,
            size_bytes: file_metadata.len(),
            is_expired: self.is_expired(&cached.cached_at),
        }))
    }
}

/// Cache metadata information
#[derive(Debug, Clone)]
pub struct CacheMetadata {
    /// When the cache was created
    pub cached_at: DateTime<Utc>,
    /// Size of the cache file in bytes
    pub size_bytes: u64,
    /// Whether the cache is expired
    pub is_expired: bool,
}
