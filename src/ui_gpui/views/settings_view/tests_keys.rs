//! Keyboard handling in the settings panel.
//!
//! Split out of `tests.rs`, which sits at this repo's 1000-line file cap.

use super::super::*;
use super::{clear_navigation_requests, make_bridge, settings_key_event};
use crate::presentation::view_command::ViewId;
use gpui::{AppContext, TestAppContext};

#[gpui::test]
async fn key_handling_navigates_and_emits_profile_events(cx: &mut TestAppContext) {
    clear_navigation_requests();
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let mcp_a = Uuid::new_v4();
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
            ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
        ]);
        view.set_mcps(vec![McpItem::new(mcp_a, "Fetcher").with_enabled(true)]);
        view.state.selected_profile_id = Some(profile_b);

        // Arrow keys on Models category scroll profiles
        view.select_category(SettingsCategory::Models);
        view.handle_key_down(&settings_key_event("up"), cx);
        assert_eq!(view.state.selected_profile_id, Some(profile_a));

        view.handle_key_down(&settings_key_event("down"), cx);
        assert_eq!(view.state.selected_profile_id, Some(profile_b));

        view.handle_key_down(&settings_key_event("e"), cx);

        view.handle_key_down(&settings_key_event("shift-="), cx);
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(ViewId::ProfileEditor)
        );

        view.handle_key_down(&settings_key_event("m"), cx);
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(ViewId::McpAdd)
        );

        // Theme scrolling requires Appearance category with dropdown open
        view.select_category(SettingsCategory::Appearance);
        view.state.theme_dropdown_open = true;
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
        view.state.selected_theme_slug = "green-screen".to_string();
        view.handle_key_down(&settings_key_event("down"), cx);
        assert_eq!(view.state.selected_theme_slug, "default");
        view.handle_key_down(&settings_key_event("enter"), cx);
        assert!(!view.state.theme_dropdown_open, "enter closes dropdown");

        view.handle_key_down(&settings_key_event("cmd-w"), cx);
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(ViewId::Chat)
        );
    });

    assert_eq!(user_rx.recv().unwrap(), UserEvent::ListCodexAccounts);
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::SelectProfile { id: profile_a }
    );
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::SelectProfile { id: profile_b }
    );
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::EditProfile { id: profile_b }
    );
    // `shift-=` now emits `OpenNewProfile` so the presenter can reset the
    // editor view before the `+`/`Shift+=` flow navigates into it. See
    // issue #182.
    assert_eq!(user_rx.recv().unwrap(), UserEvent::OpenNewProfile);
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::SelectTheme {
            slug: "default".to_string()
        }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "unexpected additional settings events"
    );
}
