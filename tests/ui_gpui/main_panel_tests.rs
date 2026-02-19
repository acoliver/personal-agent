//! Tests for navigation handling behavior used by MainPanel.
//!
//! MainPanel construction requires a live GPUI context, so these tests verify
//! the underlying NavigationState behavior directly.

use personal_agent::ui_gpui::navigation::NavigationState;
use personal_agent::presentation::view_command::ViewId;

#[test]
fn test_main_panel_starts_with_chat_view() {
    let navigation = NavigationState::new();

    assert_eq!(navigation.current(), ViewId::Chat);
}

#[test]
fn test_main_panel_handles_navigate_command() {
    let mut navigation = NavigationState::new();

    navigation.navigate(ViewId::History);
    assert_eq!(navigation.current(), ViewId::History);
}

#[test]
fn test_main_panel_navigation_state_exists() {
    let mut navigation = NavigationState::new();

    navigation.navigate(ViewId::Settings);
    assert_eq!(navigation.current(), ViewId::Settings);
    assert!(navigation.can_go_back());
}
