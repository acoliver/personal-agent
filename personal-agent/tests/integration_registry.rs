//! Integration tests for the registry module

use personal_agent::RegistryManager;
use tempfile::TempDir;

#[tokio::test]
async fn test_full_registry_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("test_cache.json");

    let manager = RegistryManager::with_cache(cache_path.clone(), 24);

    let registry = manager.get_registry().await;
    assert!(registry.is_ok(), "Should fetch registry successfully");

    let registry = registry.unwrap();
    let provider_ids = registry.get_provider_ids();
    assert!(
        !provider_ids.is_empty(),
        "Registry should have at least one provider"
    );

    assert!(
        cache_path.exists(),
        "Cache file should exist after first fetch"
    );

    let cached_registry = manager.get_registry().await.unwrap();
    assert_eq!(
        cached_registry.get_provider_ids().len(),
        provider_ids.len(),
        "Cached registry should match original"
    );

    let metadata = manager.cache_metadata().unwrap();
    assert!(metadata.is_some(), "Cache metadata should exist");
    let metadata = metadata.unwrap();
    assert!(!metadata.is_expired, "Cache should not be expired");
}

#[tokio::test]
async fn test_registry_search_capabilities() {
    let manager = RegistryManager::new().unwrap();
    let registry = manager.get_registry().await.unwrap();

    let tool_models = registry.get_tool_call_models();
    println!("Found {} models with tool calling", tool_models.len());
    assert!(
        !tool_models.is_empty(),
        "Should find at least one model with tool calling"
    );

    let reasoning_models = registry.get_reasoning_models();
    println!("Found {} models with reasoning", reasoning_models.len());

    let multimodal_models = registry.search_models(|model| {
        model
            .modalities
            .as_ref()
            .map(|m| m.input.len() > 1)
            .unwrap_or(false)
    });
    println!(
        "Found {} multimodal models",
        multimodal_models.len()
    );
}

#[tokio::test]
async fn test_provider_lookup() {
    let manager = RegistryManager::new().unwrap();
    let registry = manager.get_registry().await.unwrap();

    let provider_ids = registry.get_provider_ids();
    assert!(
        !provider_ids.is_empty(),
        "Should have at least one provider"
    );

    for provider_id in provider_ids.iter().take(3) {
        let provider = registry.get_provider(provider_id);
        assert!(provider.is_some(), "Provider {} should exist", provider_id);

        let provider = provider.unwrap();
        println!("Provider: {} ({})", provider.name, provider.id);
        println!("  Models: {}", provider.models.len());

        let models = registry.get_models_for_provider(provider_id);
        assert!(models.is_some(), "Models should exist for provider");
        let models = models.unwrap();
        assert!(
            !models.is_empty(),
            "Provider should have at least one model"
        );

        if let Some(first_model) = models.first() {
            println!(
                "  First model: {} ({})",
                first_model.name, first_model.id
            );

            let retrieved_model = registry.get_model(provider_id, &first_model.id);
            assert!(
                retrieved_model.is_some(),
                "Should be able to retrieve model by ID"
            );
        }
    }
}
