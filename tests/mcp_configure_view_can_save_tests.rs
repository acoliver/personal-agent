//! MCP Configure View - Save Eligibility Tests
//!
//! Validates local can-save gating for MCP configure drafts.

use personal_agent::ui_gpui::views::mcp_configure_view::{McpAuthMethod, McpConfigureData, OAuthStatus};

#[test]
fn test_can_save_requires_name_and_command_for_none_auth() {
    let mut data = McpConfigureData::new();
    data.auth_method = McpAuthMethod::None;

    // Missing both name and command
    assert!(!data.can_save());

    // Name only is insufficient
    data.name = "Filesystem".to_string();
    assert!(!data.can_save());

    // Name + command enables save for None auth
    data.command = "npx".to_string();
    assert!(data.can_save());
}

#[test]
fn test_can_save_requires_command_for_api_key_auth() {
    let mut data = McpConfigureData::new();
    data.name = "Fetch".to_string();
    data.auth_method = McpAuthMethod::ApiKey;
    data.api_key = "secret".to_string();

    // Missing command blocks save even with auth
    assert!(!data.can_save());

    data.command = "npx".to_string();
    assert!(data.can_save());
}

#[test]
fn test_can_save_requires_command_for_oauth_auth() {
    let mut data = McpConfigureData::new();
    data.name = "GitHub".to_string();
    data.auth_method = McpAuthMethod::OAuth;
    data.oauth_status = OAuthStatus::Connected {
        username: "alice".to_string(),
    };

    // Connected OAuth still requires command payload
    assert!(!data.can_save());

    data.command = "https://example.com/mcp".to_string();
    assert!(data.can_save());
}
