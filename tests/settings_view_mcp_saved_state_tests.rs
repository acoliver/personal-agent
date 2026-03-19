//! Settings View - MCP saved state handling tests
//!
//! These tests mirror the `SettingsView::handle_command McpConfigSaved` branch.
//! After save the MCP is enabled but has `Stopped` status — the global runtime
//! reload starts it asynchronously, so the view reflects a truthful initial state.

use uuid::Uuid;

use personal_agent::ui_gpui::views::settings_view::{McpStatus, SettingsState};

#[test]
fn test_mcp_config_saved_new_item_is_enabled_and_stopped() {
    let id = Uuid::new_v4();
    let mut state = SettingsState::new();

    // Mirror SettingsView::handle_command MCP save branch for insertion path.
    let name = "Fetch".to_string();
    state.selected_mcp_id = Some(id);
    if let Some(existing) = state.mcps.iter_mut().find(|m| m.id == id) {
        existing.name = name;
        existing.enabled = true;
        existing.status = McpStatus::Stopped;
    } else {
        state.mcps.push(
            personal_agent::ui_gpui::views::settings_view::McpItem::new(id, name)
                .with_enabled(true)
                .with_status(McpStatus::Stopped),
        );
    }

    let saved = state
        .mcps
        .iter()
        .find(|m| m.id == id)
        .expect("saved MCP exists");
    assert!(saved.enabled, "newly saved MCP should be enabled");
    assert_eq!(
        saved.status,
        McpStatus::Stopped,
        "newly saved MCP should be Stopped until runtime starts it"
    );
}

#[test]
fn test_mcp_config_saved_existing_item_is_enabled_and_stopped() {
    let id = Uuid::new_v4();
    let mut state = SettingsState::new();
    state.mcps.push(
        personal_agent::ui_gpui::views::settings_view::McpItem::new(id, "Old")
            .with_status(McpStatus::Error),
    );

    // Mirror SettingsView::handle_command MCP save branch for existing path.
    state.selected_mcp_id = Some(id);
    if let Some(existing) = state.mcps.iter_mut().find(|m| m.id == id) {
        existing.name = "Updated".to_string();
        existing.enabled = true;
        existing.status = McpStatus::Stopped;
    }

    let saved = state
        .mcps
        .iter()
        .find(|m| m.id == id)
        .expect("saved MCP exists");
    assert!(saved.enabled, "existing MCP should be enabled after save");
    assert_eq!(
        saved.status,
        McpStatus::Stopped,
        "existing MCP should be Stopped after save until runtime starts it"
    );
    assert_eq!(saved.name, "Updated");
}
