//! Tests for navigation stack operations
//!
//! These tests verify the NavigationState behavior without requiring GPUI.

use personal_agent::ui_gpui::navigation::NavigationState;
use personal_agent::presentation::view_command::ViewId;

#[test]
fn test_initial_state_is_chat() {
    let nav = NavigationState::new();
    assert_eq!(nav.current(), ViewId::Chat);
    assert_eq!(nav.stack_depth(), 1);
}

#[test]
fn test_navigate_pushes_to_stack() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);

    assert_eq!(nav.current(), ViewId::Settings);
    assert_eq!(nav.stack_depth(), 2);
}

#[test]
fn test_navigate_back_pops_stack() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    nav.navigate(ViewId::ModelSelector);

    assert_eq!(nav.stack_depth(), 3);

    nav.navigate_back();
    assert_eq!(nav.current(), ViewId::Settings);
    assert_eq!(nav.stack_depth(), 2);
}

#[test]
fn test_navigate_back_at_root_stays_at_root() {
    let mut nav = NavigationState::new();
    nav.navigate_back(); // Already at Chat

    assert_eq!(nav.current(), ViewId::Chat);
    assert_eq!(nav.stack_depth(), 1);
}

#[test]
fn test_navigate_to_same_view_does_nothing() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Chat); // Already at Chat

    assert_eq!(nav.stack_depth(), 1);
}

#[test]
fn test_can_go_back_returns_false_at_root() {
    let nav = NavigationState::new();
    assert!(!nav.can_go_back());
}

#[test]
fn test_can_go_back_returns_true_when_stacked() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    assert!(nav.can_go_back());
}

#[test]
fn test_multiple_navigation_and_back() {
    let mut nav = NavigationState::new();
    
    // Navigate forward
    nav.navigate(ViewId::Settings);
    assert_eq!(nav.current(), ViewId::Settings);
    assert_eq!(nav.stack_depth(), 2);
    
    nav.navigate(ViewId::ModelSelector);
    assert_eq!(nav.current(), ViewId::ModelSelector);
    assert_eq!(nav.stack_depth(), 3);
    
    nav.navigate(ViewId::History);
    assert_eq!(nav.current(), ViewId::History);
    assert_eq!(nav.stack_depth(), 4);
    
    // Navigate back
    nav.navigate_back();
    assert_eq!(nav.current(), ViewId::ModelSelector);
    assert_eq!(nav.stack_depth(), 3);
    
    nav.navigate_back();
    assert_eq!(nav.current(), ViewId::Settings);
    assert_eq!(nav.stack_depth(), 2);
    
    nav.navigate_back();
    assert_eq!(nav.current(), ViewId::Chat);
    assert_eq!(nav.stack_depth(), 1);
    
    // At root now
    assert!(!nav.can_go_back());
}

#[test]
fn test_navigate_to_current_does_not_increase_stack() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    let depth = nav.stack_depth();
    
    // Navigate to same view
    nav.navigate(ViewId::Settings);
    
    // Stack should not have grown
    assert_eq!(nav.stack_depth(), depth);
}

#[test]
fn test_navigate_back_returns_true_when_successful() {
    let mut nav = NavigationState::new();
    nav.navigate(ViewId::Settings);
    
    let result = nav.navigate_back();
    
    assert!(result);
    assert_eq!(nav.current(), ViewId::Chat);
}

#[test]
fn test_navigate_back_returns_false_at_root() {
    let mut nav = NavigationState::new();
    
    let result = nav.navigate_back();
    
    assert!(!result);
    assert_eq!(nav.current(), ViewId::Chat);
}
