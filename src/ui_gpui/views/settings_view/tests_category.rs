//! Tests for category sidebar, theme dropdown, and scoped keyboard navigation.

#![allow(clippy::future_not_send)]

use super::*;
use crate::events::types::UserEvent;
use gpui::TestAppContext;
use std::sync::Arc;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn settings_key_event(key: &str) -> gpui::KeyDownEvent {
    gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
async fn settings_state_default_has_category_and_dropdown_fields(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.read_with(cx, |view, _cx| {
        assert_eq!(
            view.state.selected_category,
            SettingsCategory::General,
            "default category should be General"
        );
        assert!(
            !view.state.theme_dropdown_open,
            "dropdown should be closed by default"
        );
    });
}

#[gpui::test]
async fn select_category_updates_state_and_closes_dropdown(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.theme_dropdown_open = true;

        view.select_category(SettingsCategory::Models);
        assert_eq!(view.state.selected_category, SettingsCategory::Models);
        assert!(
            !view.state.theme_dropdown_open,
            "selecting a category should close dropdown"
        );

        view.select_category(SettingsCategory::Security);
        assert_eq!(view.state.selected_category, SettingsCategory::Security);

        view.select_category(SettingsCategory::McpTools);
        assert_eq!(view.state.selected_category, SettingsCategory::McpTools);

        view.select_category(SettingsCategory::General);
        assert_eq!(view.state.selected_category, SettingsCategory::General);
    });
}

#[gpui::test]
async fn toggle_theme_dropdown_toggles_state(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        assert!(!view.state.theme_dropdown_open);
        view.toggle_theme_dropdown();
        assert!(view.state.theme_dropdown_open);
        view.toggle_theme_dropdown();
        assert!(!view.state.theme_dropdown_open);
    });
}

#[gpui::test]
async fn close_theme_dropdown_sets_false(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.theme_dropdown_open = true;
        view.close_theme_dropdown();
        assert!(!view.state.theme_dropdown_open);

        view.close_theme_dropdown();
        assert!(!view.state.theme_dropdown_open);
    });
}

#[gpui::test]
async fn select_theme_from_dropdown_selects_and_closes(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, cx| {
        view.state.available_themes = vec![
            ThemeOption {
                name: "Green Screen".to_string(),
                slug: "green-screen".to_string(),
            },
            ThemeOption {
                name: "Midnight Nebula".to_string(),
                slug: "default".to_string(),
            },
        ];
        view.state.theme_dropdown_open = true;
        view.select_theme_from_dropdown("default".to_string(), cx);
        assert_eq!(view.state.selected_theme_slug, "default");
        assert!(!view.state.theme_dropdown_open);
    });

    assert_eq!(
        user_rx.try_recv().unwrap(),
        UserEvent::SelectTheme {
            slug: "default".to_string()
        }
    );
}

#[gpui::test]
async fn arrow_keys_only_scroll_profiles_on_models_category(cx: &mut TestAppContext) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let mcp_a = Uuid::new_v4();
    let mcp_b = Uuid::new_v4();
    let (bridge, _user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, cx| {
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha"),
            ProfileItem::new(profile_b, "Beta"),
        ]);
        view.set_mcps(vec![
            McpItem::new(mcp_a, "First"),
            McpItem::new(mcp_b, "Second"),
        ]);
        view.state.selected_profile_id = Some(profile_a);
        view.state.selected_mcp_id = Some(mcp_a);

        view.select_category(SettingsCategory::Models);
        view.handle_key_down(&settings_key_event("down"), cx);
        assert_eq!(view.state.selected_profile_id, Some(profile_b));
        assert_eq!(
            view.state.selected_mcp_id,
            Some(mcp_a),
            "MCP selection unchanged when on Models"
        );
    });
}

#[gpui::test]
async fn arrow_keys_only_scroll_mcps_on_mcp_tools_category(cx: &mut TestAppContext) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let mcp_a = Uuid::new_v4();
    let mcp_b = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha"),
            ProfileItem::new(profile_b, "Beta"),
        ]);
        view.set_mcps(vec![
            McpItem::new(mcp_a, "First"),
            McpItem::new(mcp_b, "Second"),
        ]);
        view.state.selected_profile_id = Some(profile_a);
        view.state.selected_mcp_id = Some(mcp_a);

        view.select_category(SettingsCategory::McpTools);
        view.handle_key_down(&settings_key_event("down"), cx);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_b));
        assert_eq!(
            view.state.selected_profile_id,
            Some(profile_a),
            "Profile selection unchanged when on McpTools"
        );
    });
}

#[gpui::test]
async fn arrow_keys_do_nothing_on_general_without_dropdown(cx: &mut TestAppContext) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha"),
            ProfileItem::new(profile_b, "Beta"),
        ]);
        view.state.selected_profile_id = Some(profile_a);

        view.select_category(SettingsCategory::General);
        view.handle_key_down(&settings_key_event("down"), cx);
        assert_eq!(
            view.state.selected_profile_id,
            Some(profile_a),
            "profiles unchanged on General"
        );
    });
}

#[gpui::test]
async fn arrow_keys_do_nothing_on_security_category(cx: &mut TestAppContext) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha"),
            ProfileItem::new(profile_b, "Beta"),
        ]);
        view.state.selected_profile_id = Some(profile_a);

        view.select_category(SettingsCategory::Security);
        view.handle_key_down(&settings_key_event("down"), cx);
        assert_eq!(
            view.state.selected_profile_id,
            Some(profile_a),
            "profiles unchanged on Security"
        );
    });
}

#[gpui::test]
async fn settings_category_display_names(cx: &mut TestAppContext) {
    let _ = cx;
    assert_eq!(SettingsCategory::General.display_name(), "General");
    assert_eq!(SettingsCategory::Models.display_name(), "Models");
    assert_eq!(SettingsCategory::Security.display_name(), "Security");
    assert_eq!(SettingsCategory::McpTools.display_name(), "MCP Tools");
}

#[gpui::test]
async fn scroll_mcps_clamps_correctly(cx: &mut TestAppContext) {
    let mcp_a = Uuid::new_v4();
    let mcp_b = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.set_mcps(vec![
            McpItem::new(mcp_a, "First"),
            McpItem::new(mcp_b, "Second"),
        ]);
        view.state.selected_mcp_id = Some(mcp_a);

        view.scroll_mcps(1);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_b));
        view.scroll_mcps(20);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_b));
        view.scroll_mcps(-20);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_a));
    });
}
