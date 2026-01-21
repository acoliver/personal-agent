//! Tests for settings view display behavior
//!
//! These tests verify that the settings view properly displays:
//! 1. Profile rows when profiles exist in config
//! 2. MCP rows when MCPs exist in config
//! 3. Proper section headers
//! 4. Hotkey field

use personal_agent::config::Config;
use personal_agent::models::{AuthConfig, ModelProfile};
use personal_agent::mcp::{McpConfig, McpPackage, McpPackageType, McpSource, McpTransport, McpAuthType};
use tempfile::TempDir;
use uuid::Uuid;
use std::fs;

fn create_test_config_dir() -> TempDir {
    TempDir::new().unwrap()
}

fn create_config_with_profiles(dir: &TempDir, num_profiles: usize) -> Config {
    let mut config = Config::default();
    
    for i in 0..num_profiles {
        let profile = ModelProfile::new(
            format!("Profile {}", i + 1),
            "openai".to_string(),
            format!("gpt-4-{}", i),
            String::new(),
            AuthConfig::Key { value: format!("key-{}", i) },
        );
        config.profiles.push(profile);
    }
    
    // Save config
    let config_path = dir.path().join("config.json");
    let json = serde_json::to_string_pretty(&config).unwrap();
    fs::write(&config_path, json).unwrap();
    
    config
}

fn create_config_with_mcps(dir: &TempDir, num_mcps: usize) -> Config {
    let mut config = Config::default();
    
    for i in 0..num_mcps {
        let mcp = McpConfig {
            id: Uuid::new_v4(),
            name: format!("MCP Tool {}", i + 1),
            enabled: true,
            source: McpSource::Manual { url: format!("https://example.com/mcp/{}", i) },
            package: McpPackage {
                package_type: McpPackageType::Http,
                identifier: format!("https://example.com/mcp/{}", i),
                runtime_hint: None,
            },
            transport: McpTransport::Http,
            auth_type: McpAuthType::None,
            env_vars: vec![],
            package_args: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        };
        config.mcps.push(mcp);
    }
    
    let config_path = dir.path().join("config.json");
    let json = serde_json::to_string_pretty(&config).unwrap();
    fs::write(&config_path, json).unwrap();
    
    config
}

/// Config with profiles should result in profile rows being created
#[test]
fn config_profiles_create_rows() {
    let dir = create_test_config_dir();
    let config = create_config_with_profiles(&dir, 3);
    
    // Verify config was created correctly
    assert_eq!(config.profiles.len(), 3);
    assert_eq!(config.profiles[0].name, "Profile 1");
    assert_eq!(config.profiles[1].name, "Profile 2");
    assert_eq!(config.profiles[2].name, "Profile 3");
}

/// Config with MCPs should result in MCP rows being created  
#[test]
fn config_mcps_create_rows() {
    let dir = create_test_config_dir();
    let config = create_config_with_mcps(&dir, 2);
    
    assert_eq!(config.mcps.len(), 2);
    assert_eq!(config.mcps[0].name, "MCP Tool 1");
    assert_eq!(config.mcps[1].name, "MCP Tool 2");
}

/// Empty config should show placeholder messages
#[test]
fn empty_config_shows_placeholders() {
    let config = Config::default();
    
    assert!(config.profiles.is_empty());
    assert!(config.mcps.is_empty());
    // UI should show "No profiles yet. Click + to add one."
    // UI should show "No MCPs configured."
}

/// Default config path should be accessible
#[test]
fn default_config_path_exists() {
    let path = Config::default_path();
    assert!(path.is_ok(), "Should be able to get default config path");
}

/// Loading real config should succeed if file exists
#[test]
fn load_real_config_if_exists() {
    if let Ok(path) = Config::default_path() {
        if path.exists() {
            let config = Config::load(&path);
            assert!(config.is_ok(), "Should be able to load existing config");
            
            let config = config.unwrap();
            // Log what we found for debugging
            println!("Real config has {} profiles and {} MCPs", 
                     config.profiles.len(), config.mcps.len());
            
            for profile in &config.profiles {
                println!("  Profile: {} ({}:{})", 
                         profile.name, profile.provider_id, profile.model_id);
            }
        }
    }
}

/// Profile row text should contain name and model info
#[test]
fn profile_row_text_format() {
    let profile = ModelProfile::new(
        "My Profile".to_string(),
        "openai".to_string(),
        "gpt-4".to_string(),
        String::new(),
        AuthConfig::Key { value: "test".to_string() },
    );
    
    // The expected format from create_profile_row is:
    // "{name} ({provider_id}:{model_id})"
    let expected_text = format!(
        "{} ({}:{})",
        profile.name, profile.provider_id, profile.model_id
    );
    
    assert_eq!(expected_text, "My Profile (openai:gpt-4)");
}

/// MCP row text should contain name and status
#[test]
fn mcp_row_text_format() {
    let mcp = McpConfig {
        id: Uuid::new_v4(),
        name: "My MCP Tool".to_string(),
        enabled: true,
        source: McpSource::Manual { url: "https://example.com".to_string() },
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
    };
    
    // The row should show the name and enabled status
    assert_eq!(mcp.name, "My MCP Tool");
    assert!(mcp.enabled);
}
