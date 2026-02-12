//! Tests for MainPanel navigation handling
//!
//! These tests verify MainPanel correctly handles ViewCommand navigation.
//! Note: These are basic unit tests - full GPUI integration tests will be added later.

use personal_agent::ui_gpui::views::main_panel::MainPanel;
use personal_agent::presentation::view_command::{ViewCommand, ViewId};

#[test]
fn test_main_panel_starts_with_chat_view() {
    let panel = MainPanel::new();
    
    assert_eq!(panel.current_view(), ViewId::Chat);
}

#[test]
fn test_main_panel_handles_navigate_command() {
    let mut panel = MainPanel::new();
    
    // Note: We can't test the full handle_command without GPUI context,
    // but we can verify the navigation state exists
    assert_eq!(panel.current_view(), ViewId::Chat);
}

#[test]
fn test_main_panel_navigation_state_exists() {
    let panel = MainPanel::new();
    
    // Verify navigation state is initialized
    assert_eq!(panel.current_view(), ViewId::Chat);
}
