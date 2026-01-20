use personal_agent::registry::{ModelRegistry, RegistryCache};
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn registry_cache_clear_removes_file() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models_dev_cache.json");
    let cache = RegistryCache::new(cache_path.clone(), 24);

    let registry = ModelRegistry {
        providers: HashMap::new(),
    };
    cache.save(&registry).unwrap();

    assert!(cache.clear().is_ok());
    assert!(!cache_path.exists());
}

#[test]
fn registry_cache_metadata_is_none_when_missing() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models_dev_cache.json");
    let cache = RegistryCache::new(cache_path, 24);

    let metadata = cache.metadata().unwrap();
    assert!(metadata.is_none());
}

#[test]
fn registry_cache_metadata_populates_when_present() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models_dev_cache.json");
    let cache = RegistryCache::new(cache_path, 24);

    let registry = ModelRegistry {
        providers: HashMap::new(),
    };
    cache.save(&registry).unwrap();

    let metadata = cache.metadata().unwrap().unwrap();
    assert!(metadata.size_bytes > 0);
    assert!(!metadata.is_expired);
}
