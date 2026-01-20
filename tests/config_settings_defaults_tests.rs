use personal_agent::config::Config;
use tempfile::TempDir;

#[test]
fn config_load_default_has_empty_fields() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");
    let config = Config::load(config_path).unwrap();

    assert!(config.profiles.is_empty());
    assert!(config.mcps.is_empty());
    assert!(config.default_profile.is_none());
}
