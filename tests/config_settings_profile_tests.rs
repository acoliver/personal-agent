use personal_agent::{AuthConfig, Config, ModelProfile};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn config_profile_lifecycle_updates_default() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut config = Config::load(&config_path).unwrap();

    let mut profile = ModelProfile::default();
    profile.id = Uuid::new_v4();
    config.add_profile(profile.clone());
    config.default_profile = Some(profile.id);

    let mut updated = profile.clone();
    updated.name = "Updated".to_string();
    config.update_profile(updated).unwrap();
    assert_eq!(config.get_profile(&profile.id).unwrap().name, "Updated");

    config.remove_profile(&profile.id).unwrap();
    assert!(config.default_profile.is_none());
}

#[test]
fn config_profile_lookup_errors_when_missing() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let config = Config::load(&config_path).unwrap();

    let missing_id = Uuid::new_v4();
    assert!(config.get_profile(&missing_id).is_err());
}

#[test]
fn config_profile_mut_lookup_errors_when_missing() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut config = Config::load(&config_path).unwrap();

    let missing_id = Uuid::new_v4();
    assert!(config.get_profile_mut(&missing_id).is_err());
}

#[test]
fn config_remove_mcp_errors_when_missing() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut config = Config::load(&config_path).unwrap();

    let missing_id = Uuid::new_v4();
    assert!(config.remove_mcp(&missing_id).is_err());
}

#[test]
fn config_load_rejects_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    std::fs::write(&config_path, "not-json").unwrap();

    let result = Config::load(&config_path);
    assert!(result.is_err());
}

#[test]
fn config_keyfile_profile_requires_valid_key() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let mut config = Config::load(&config_path).unwrap();

    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Keyfile {
            path: temp_dir.path().join("missing.key").to_string_lossy().to_string(),
        },
    );

    config.add_profile(profile);
    assert!(config.save(&config_path).is_ok());
}
