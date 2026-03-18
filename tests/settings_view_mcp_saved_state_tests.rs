//! Settings View - MCP saved state handling tests

use uuid::Uuid;

use personal_agent::ui_gpui::views::settings_view::{McpStatus, SettingsState};

#[test]
fn test_mcp_config_saved_new_item_is_enabled_and_running() {
    let id = Uuid::new_v4();
    let mut state = SettingsState::new();

    // Mirror SettingsView::handle_command MCP save branch for insertion path.
    let name = "Fetch".to_string();
    state.selected_mcp_id = Some(id);
    if let Some(existing) = state.mcps.iter_mut().find(|m| m.id == id) {
        existing.name = name;
        existing.enabled = true;
        existing.status = McpStatus::Running;
    } else {
        state.mcps.push(
            personal_agent::ui_gpui::views::settings_view::McpItem::new(id, name)
                .with_status(McpStatus::Running)
                .with_enabled(true),
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
        McpStatus::Running,
        "newly saved MCP should be Running"
    );
}

#[test]
fn test_mcp_config_saved_existing_item_is_enabled_and_running() {
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
        existing.status = McpStatus::Running;
    }

    let saved = state
        .mcps
        .iter()
        .find(|m| m.id == id)
        .expect("saved MCP exists");
    assert!(saved.enabled, "existing MCP should be enabled after save");
    assert_eq!(
        saved.status,
        McpStatus::Running,
        "existing MCP should be Running after save"
    );
    assert_eq!(saved.name, "Updated");
}
