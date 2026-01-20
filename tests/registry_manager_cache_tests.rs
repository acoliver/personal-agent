use personal_agent::registry::{ModelRegistry, RegistryCache, RegistryManager};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn registry_manager_reads_cache_without_network() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models_dev_cache.json");
    let cache = RegistryCache::new(cache_path.clone(), 24);

    let registry = ModelRegistry {
        providers: HashMap::new(),
    };
    cache.save(&registry).unwrap();

    let manager = RegistryManager::with_cache(cache_path, 24);
    let loaded = manager.get_registry().await.unwrap();

    assert!(loaded.providers.is_empty());
}
