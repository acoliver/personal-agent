//! Behavioral tests for settings panel
//!
//! The settings panel should display:
//! 1. All configured profiles in a list
//! 2. All configured MCPs in a list with enable/disable toggles
//! 3. Global hotkey configuration
//!
//! These tests verify the data loading behavior that feeds the UI.

use personal_agent::config::Config;
use personal_agent::mcp::{
    McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport,
};
use personal_agent::{AuthConfig, ModelProfile};
use tempfile::TempDir;
use uuid::Uuid;

/// Helper to create a test profile
fn test_profile(name: &str) -> ModelProfile {
    ModelProfile::new(
        name.to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        String::new(),
        AuthConfig::Key {
            value: "test-key".to_string(),
        },
    )
}

/// Helper to create a test MCP config
fn test_mcp(name: &str, enabled: bool) -> McpConfig {
    McpConfig {
        id: Uuid::new_v4(),
        name: name.to_string(),
        enabled,
        source: McpSource::Manual {
            url: "https://example.com".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "https://example.com".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Http,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    }
}

/// Settings panel should display all profiles from config
#[test]
fn settings_loads_all_profiles() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();

    // Add profiles
    config.add_profile(test_profile("Profile One"));
    config.add_profile(test_profile("Profile Two"));
    config.add_profile(test_profile("Profile Three"));
    config.save(&config_path).unwrap();

    // Reload and verify
    let loaded = Config::load(&config_path).unwrap();
    assert_eq!(loaded.profiles.len(), 3, "Should have 3 profiles");

    let names: Vec<&str> = loaded.profiles.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Profile One"));
    assert!(names.contains(&"Profile Two"));
    assert!(names.contains(&"Profile Three"));
}

/// Settings panel should display all MCPs from config
#[test]
fn settings_loads_all_mcps() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();

    // Add MCPs
    config.mcps.push(test_mcp("MCP Alpha", true));
    config.mcps.push(test_mcp("MCP Beta", false));
    config.mcps.push(test_mcp("MCP Gamma", true));
    config.save(&config_path).unwrap();

    // Reload and verify
    let loaded = Config::load(&config_path).unwrap();
    assert_eq!(loaded.mcps.len(), 3, "Should have 3 MCPs");

    let names: Vec<&str> = loaded.mcps.iter().map(|m| m.name.as_str()).collect();
    assert!(names.contains(&"MCP Alpha"));
    assert!(names.contains(&"MCP Beta"));
    assert!(names.contains(&"MCP Gamma"));
}

/// Settings panel should reflect MCP enabled/disabled state
#[test]
fn settings_reflects_mcp_enabled_state() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();

    let enabled_mcp = test_mcp("Enabled MCP", true);
    let disabled_mcp = test_mcp("Disabled MCP", false);

    config.mcps.push(enabled_mcp);
    config.mcps.push(disabled_mcp);
    config.save(&config_path).unwrap();

    // Reload and verify enabled states
    let loaded = Config::load(&config_path).unwrap();

    let enabled = loaded.mcps.iter().find(|m| m.name == "Enabled MCP").unwrap();
    let disabled = loaded.mcps.iter().find(|m| m.name == "Disabled MCP").unwrap();

    assert!(enabled.enabled, "Enabled MCP should be enabled");
    assert!(!disabled.enabled, "Disabled MCP should be disabled");
}

/// Settings panel should show the default profile as selected
#[test]
fn settings_shows_default_profile_selected() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();

    let profile1 = test_profile("Not Default");
    let profile2 = test_profile("Is Default");

    config.add_profile(profile1);
    config.add_profile(profile2.clone());
    config.default_profile = Some(profile2.id);
    config.save(&config_path).unwrap();

    // Reload and verify
    let loaded = Config::load(&config_path).unwrap();
    assert_eq!(loaded.default_profile, Some(profile2.id));
}

/// Settings panel should preserve global hotkey setting
#[test]
fn settings_preserves_global_hotkey() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();
    config.global_hotkey = "Cmd+Shift+P".to_string();
    config.save(&config_path).unwrap();

    let loaded = Config::load(&config_path).unwrap();
    assert_eq!(loaded.global_hotkey, "Cmd+Shift+P");
}

/// Empty config should still load without errors (for fresh installs)
#[test]
fn settings_handles_empty_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let config = Config::load(&config_path).unwrap();

    assert!(config.profiles.is_empty(), "New config should have no profiles");
    assert!(config.mcps.is_empty(), "New config should have no MCPs");
    assert!(config.default_profile.is_none(), "New config should have no default profile");
}

/// Config with profiles but no MCPs should load correctly
#[test]
fn settings_handles_profiles_without_mcps() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();
    config.add_profile(test_profile("Solo Profile"));
    config.save(&config_path).unwrap();

    let loaded = Config::load(&config_path).unwrap();
    assert_eq!(loaded.profiles.len(), 1);
    assert!(loaded.mcps.is_empty());
}

/// Config with MCPs but no profiles should load correctly
#[test]
fn settings_handles_mcps_without_profiles() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.json");

    let mut config = Config::load(&config_path).unwrap();
    config.mcps.push(test_mcp("Solo MCP", true));
    config.save(&config_path).unwrap();

    let loaded = Config::load(&config_path).unwrap();
    assert!(loaded.profiles.is_empty());
    assert_eq!(loaded.mcps.len(), 1);
}
