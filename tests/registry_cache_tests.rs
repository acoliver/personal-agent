use personal_agent::registry::{ModelRegistry, Provider, RegistryCache};
use tempfile::TempDir;

#[test]
fn registry_cache_round_trip_and_metadata() -> personal_agent::Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models.json");
    let cache = RegistryCache::new(cache_path.clone(), 24);

    let mut providers = std::collections::HashMap::new();
    providers.insert(
        "test-provider".to_string(),
        Provider {
            id: "test-provider".to_string(),
            name: "Test Provider".to_string(),
            env: vec!["TEST_API_KEY".to_string()],
            npm: Some("@ai-sdk/openai".to_string()),
            api: Some("https://api.example.com".to_string()),
            doc: None,
            models: std::collections::HashMap::new(),
        },
    );

    let registry = ModelRegistry { providers };
    cache.save(&registry)?;

    let loaded = cache.load()?.expect("cache to load");
    assert_eq!(loaded.providers.len(), 1);

    let metadata = cache.metadata()?.expect("metadata to exist");
    assert!(!metadata.is_expired);
    assert!(metadata.size_bytes > 0);

    cache.clear()?;
    assert!(cache.load()?.is_none());

    Ok(())
}
