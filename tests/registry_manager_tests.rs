use personal_agent::registry::RegistryManager;
use tempfile::TempDir;

#[test]
fn registry_manager_clear_cache_removes_file() -> personal_agent::Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models.json");
    let manager = RegistryManager::with_cache(cache_path.clone(), 24);

    manager.clear_cache()?;
    assert!(!cache_path.exists());

    Ok(())
}

#[test]
fn registry_manager_cache_metadata_is_none_when_missing() -> personal_agent::Result<()> {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("models.json");
    let manager = RegistryManager::with_cache(cache_path.clone(), 24);

    let metadata = manager.cache_metadata()?;
    assert!(metadata.is_none());

    Ok(())
}
