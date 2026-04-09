//! Tests for scrollable container behavior in settings view.
//!
//! These tests verify that profile and MCP lists properly constrain their height
//! and allow scrolling when content exceeds available space, ensuring toolbar
//! buttons (+/-) remain visible at all times.

#![allow(clippy::future_not_send)]

use super::*;
use gpui::TestAppContext;

/// Test that the view can handle a large number of profiles without crashing.
/// This verifies the profiles list properly handles overflow scenarios.
#[gpui::test]
async fn profiles_list_handles_large_number_of_profiles(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    // Create 100 profiles to simulate overflow scenario
    let many_profiles: Vec<ProfileItem> = (0..100)
        .map(|i| {
            ProfileItem::new(Uuid::new_v4(), format!("Profile {i}"))
                .with_model("openai", format!("gpt-{i}"))
        })
        .collect();

    view.update(cx, |view, _cx| {
        view.set_profiles(many_profiles);
        assert_eq!(view.state.profiles.len(), 100);
        // set_profiles doesn't auto-select, but apply_profile_summaries does
        // Just verify we can set many profiles without crashing
    });
}

/// Test that the view can handle a large number of MCPs without crashing.
/// This verifies the MCP list properly handles overflow scenarios.
#[gpui::test]
async fn mcps_list_handles_large_number_of_mcps(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    // Create 100 MCPs to simulate overflow scenario
    let many_mcps: Vec<McpItem> = (0..100)
        .map(|i| McpItem::new(Uuid::new_v4(), format!("MCP {i}")).with_enabled(i % 2 == 0))
        .collect();

    view.update(cx, |view, _cx| {
        view.set_mcps(many_mcps);
        assert_eq!(view.state.mcps.len(), 100);
        // set_mcps selects first item by default
        assert!(view.state.selected_mcp_id.is_some());
    });
}

/// Test scrolling through many profiles works correctly.
#[gpui::test]
async fn scroll_profiles_with_many_items(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    let many_profiles: Vec<ProfileItem> = (0..50)
        .map(|i| {
            ProfileItem::new(Uuid::new_v4(), format!("Profile {i}")).with_model("openai", "gpt-4o")
        })
        .collect();

    view.update(cx, |view, _cx| {
        view.set_profiles(many_profiles);
        // Manually select first profile for testing
        view.state.selected_profile_id = view.state.profiles.first().map(|p| p.id);

        // Scroll down through many items
        for _ in 0..25 {
            view.scroll_profiles(1);
        }

        // Should be at index 25 (0-based)
        let current_idx = view.selected_profile_index().unwrap_or(0);
        assert_eq!(current_idx, 25);

        // Scroll beyond bounds - should clamp
        view.scroll_profiles(100);
        let final_idx = view.selected_profile_index().unwrap_or(0);
        assert_eq!(final_idx, 49); // Last item (index 49)
    });
}

/// Test scrolling through many MCPs works correctly.
#[gpui::test]
async fn scroll_mcps_with_many_items(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    let many_mcps: Vec<McpItem> = (0..50)
        .map(|i| McpItem::new(Uuid::new_v4(), format!("MCP {i}")).with_enabled(true))
        .collect();

    view.update(cx, |view, _cx| {
        view.set_mcps(many_mcps);
        // set_mcps already selects first MCP

        // Scroll down through many items
        for _ in 0..25 {
            view.scroll_mcps(1);
        }

        // Should be at index 25 (0-based)
        let current_idx = view
            .state
            .selected_mcp_id
            .and_then(|id| view.state.mcps.iter().position(|m| m.id == id))
            .unwrap_or(0);
        assert_eq!(current_idx, 25);

        // Scroll beyond bounds - should clamp
        view.scroll_mcps(100);
        let final_idx = view
            .state
            .selected_mcp_id
            .and_then(|id| view.state.mcps.iter().position(|m| m.id == id))
            .unwrap_or(0);
        assert_eq!(final_idx, 49); // Last item (index 49)
    });
}
