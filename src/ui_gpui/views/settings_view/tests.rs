#![allow(clippy::future_not_send)]

#[path = "tests_category.rs"]
mod tests_category;

use super::*;
use crate::presentation::view_command::{ViewCommand, ViewId};
use gpui::{AppContext, Bounds, EntityInputHandler, Pixels, TestAppContext};

fn clear_navigation_requests() {
    while crate::ui_gpui::navigation_channel()
        .take_pending()
        .is_some()
    {}
}

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

use flume;

fn profile_summary(
    id: Uuid,
    name: &str,
    provider: &str,
    model: &str,
    is_default: bool,
) -> ProfileSummary {
    ProfileSummary {
        id,
        name: name.to_string(),
        provider_id: provider.to_string(),
        model_id: model.to_string(),
        is_default,
    }
}

#[gpui::test]
async fn handle_command_applies_profile_summaries_and_selection_fallbacks(
    cx: &mut gpui::TestAppContext,
) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let profile_c = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, cx| {
        view.handle_command(
            ViewCommand::ShowSettings {
                profiles: vec![
                    profile_summary(profile_a, "Alpha", "openai", "gpt-4o", false),
                    profile_summary(profile_b, "Beta", "anthropic", "claude", true),
                ],
                selected_profile_id: None,
            },
            cx,
        );

        assert_eq!(view.state.selected_profile_id, Some(profile_a));
        assert_eq!(view.state.profiles.len(), 2);
        assert_eq!(
            view.state.profiles[0].display_text(),
            "Alpha (openai:gpt-4o)"
        );
        assert_eq!(
            view.state.profiles[1].display_text(),
            "Beta (anthropic:claude)"
        );
        assert!(view.state.profiles[1].is_default);

        view.handle_command(
            ViewCommand::ChatProfilesUpdated {
                profiles: vec![
                    profile_summary(profile_b, "Beta", "anthropic", "claude", true),
                    profile_summary(profile_c, "Gamma", "openai", "gpt-4.1", false),
                ],
                selected_profile_id: Some(profile_b),
            },
            cx,
        );

        assert_eq!(view.state.selected_profile_id, Some(profile_b));
        assert_eq!(view.state.profiles.len(), 2);

        view.handle_command(
            ViewCommand::ShowSettings {
                profiles: vec![profile_summary(
                    profile_c, "Gamma", "openai", "gpt-4.1", false,
                )],
                selected_profile_id: Some(profile_b),
            },
            cx,
        );

        assert_eq!(view.state.selected_profile_id, Some(profile_c));
        assert_eq!(view.state.profiles.len(), 1);
        assert_eq!(view.state.profiles[0].name, "Gamma");
    });
}

#[gpui::test]
async fn scroll_profiles_clamps_and_emits_selection_events(cx: &mut gpui::TestAppContext) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, _cx| {
        view.set_bridge(Arc::clone(&bridge));
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
            ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
        ]);
        view.state.selected_profile_id = Some(profile_a);

        view.scroll_profiles(1);
        assert_eq!(view.state.selected_profile_id, Some(profile_b));
        view.scroll_profiles(20);
        assert_eq!(view.state.selected_profile_id, Some(profile_b));
        view.scroll_profiles(-20);
        assert_eq!(view.state.selected_profile_id, Some(profile_a));
    });

    assert_eq!(
        user_rx.recv().expect("profile scroll selects beta"),
        UserEvent::SelectProfile { id: profile_b }
    );
    assert_eq!(
        user_rx.recv().expect("profile scroll returns to alpha"),
        UserEvent::SelectProfile { id: profile_b }
    );
    assert_eq!(
        user_rx.recv().expect("profile scroll selects alpha"),
        UserEvent::SelectProfile { id: profile_a }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "settings view test should emit only expected bridge events"
    );
}

#[gpui::test]
async fn mcp_commands_update_status_lifecycle_and_selection(cx: &mut gpui::TestAppContext) {
    let mcp_existing = Uuid::new_v4();
    let mcp_new = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, cx| {
        view.set_mcps(vec![
            McpItem::new(mcp_existing, "Existing").with_status(McpStatus::Stopped)
        ]);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_existing));

        view.handle_command(
            ViewCommand::McpStatusChanged {
                id: mcp_existing,
                status: crate::presentation::view_command::McpStatus::Running,
            },
            cx,
        );
        let existing = view
            .state
            .mcps
            .iter()
            .find(|m| m.id == mcp_existing)
            .unwrap();
        assert_eq!(existing.status, McpStatus::Running);
        assert!(existing.enabled);

        view.handle_command(
            ViewCommand::McpServerStarted {
                id: mcp_new,
                name: Some("Fetch".to_string()),
                tool_count: 3,
                enabled: Some(false),
            },
            cx,
        );
        let inserted = view.state.mcps.iter().find(|m| m.id == mcp_new).unwrap();
        assert_eq!(inserted.name, "Fetch");
        assert!(!inserted.enabled);
        assert_eq!(inserted.status, McpStatus::Stopped);

        view.handle_command(
            ViewCommand::McpServerFailed {
                id: mcp_new,
                error: "boom".to_string(),
            },
            cx,
        );
        let failed = view.state.mcps.iter().find(|m| m.id == mcp_new).unwrap();
        assert_eq!(failed.status, McpStatus::Error);
        assert!(!failed.enabled);

        view.handle_command(
            ViewCommand::McpConfigSaved {
                id: mcp_new,
                name: Some("Saved MCP".to_string()),
            },
            cx,
        );
        let saved = view.state.mcps.iter().find(|m| m.id == mcp_new).unwrap();
        assert_eq!(view.state.selected_mcp_id, Some(mcp_new));
        assert_eq!(saved.name, "Saved MCP");
        assert!(saved.enabled);
        assert_eq!(saved.status, McpStatus::Stopped);

        view.handle_command(ViewCommand::McpDeleted { id: mcp_new }, cx);
        assert!(view.state.mcps.iter().all(|m| m.id != mcp_new));
    });
}

#[gpui::test]
async fn profile_and_mcp_setters_enforce_selection_fallbacks_without_bridge(
    cx: &mut gpui::TestAppContext,
) {
    let profile_a = Uuid::new_v4();
    let profile_b = Uuid::new_v4();
    let profile_c = Uuid::new_v4();
    let mcp_a = Uuid::new_v4();
    let mcp_b = Uuid::new_v4();
    let mcp_c = Uuid::new_v4();
    let view = cx.new(SettingsView::new);

    view.update(cx, |view: &mut SettingsView, _cx| {
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
            ProfileItem::new(profile_b, "Beta").with_model("anthropic", "claude"),
        ]);
        assert_eq!(view.state.selected_profile_id, None);

        view.state.selected_profile_id = Some(profile_a);
        view.set_profiles(vec![
            ProfileItem::new(profile_a, "Alpha").with_model("openai", "gpt-4o"),
            ProfileItem::new(profile_c, "Gamma").with_model("openai", "gpt-4.1"),
        ]);
        assert_eq!(view.state.selected_profile_id, Some(profile_a));
        assert_eq!(view.state.profiles.len(), 2);
        assert_eq!(
            view.state.profiles[1].display_text(),
            "Gamma (openai:gpt-4.1)"
        );

        view.state.selected_profile_id = Some(profile_b);
        view.set_profiles(vec![
            ProfileItem::new(profile_c, "Gamma").with_model("openai", "gpt-4.1")
        ]);
        assert_eq!(view.state.selected_profile_id, Some(profile_b));

        view.set_mcps(vec![
            McpItem::new(mcp_a, "Existing").with_status(McpStatus::Stopped),
            McpItem::new(mcp_b, "Runner").with_enabled(true),
        ]);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_a));

        view.state.selected_mcp_id = Some(mcp_b);
        view.set_mcps(vec![
            McpItem::new(mcp_b, "Runner").with_enabled(true),
            McpItem::new(mcp_c, "Fetcher").with_status(McpStatus::Error),
        ]);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_b));
        assert_eq!(view.state.mcps[0].status, McpStatus::Running);
        assert_eq!(view.state.mcps[1].status, McpStatus::Error);

        view.state.selected_mcp_id = Some(mcp_a);
        view.set_mcps(vec![
            McpItem::new(mcp_c, "Fetcher").with_status(McpStatus::Error)
        ]);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_c));
        assert_eq!(view.state.mcps.len(), 1);
        assert!(!view.state.mcps[0].enabled);
    });
}

fn settings_key_event(key: &str) -> gpui::KeyDownEvent {
    gpui::KeyDownEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
        is_held: false,
        prefer_character_input: false,
    }
}

#[gpui::test]
async fn helper_actions_emit_expected_bridge_events(cx: &mut TestAppContext) {
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
        view.state.selected_profile_id = Some(profile_a);
        view.state.selected_mcp_id = Some(mcp_a);

        view.select_profile(profile_b, cx);
        assert_eq!(view.state.selected_profile_id, Some(profile_b));

        view.delete_selected_profile();
        view.edit_selected_profile();

        view.toggle_mcp(mcp_a, false);
        view.select_mcp(mcp_a, cx);
        assert_eq!(view.state.selected_mcp_id, Some(mcp_a));
        view.delete_selected_mcp();
        view.edit_selected_mcp();
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(ViewId::McpConfigure)
        );
    });

    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::SelectProfile { id: profile_b }
    );
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::DeleteProfile { id: profile_b }
    );
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::EditProfile { id: profile_b }
    );
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::ToggleMcp {
            id: mcp_a,
            enabled: false
        }
    );
    assert_eq!(user_rx.recv().unwrap(), UserEvent::DeleteMcp { id: mcp_a });
    assert_eq!(
        user_rx.recv().unwrap(),
        UserEvent::ConfigureMcp { id: mcp_a }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "unexpected additional settings events"
    );
}

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

#[gpui::test]
async fn static_navigation_helpers_route_to_expected_views(_cx: &mut TestAppContext) {
    clear_navigation_requests();

    SettingsView::navigate_to_profile_editor();
    assert_eq!(
        crate::ui_gpui::navigation_channel().take_pending(),
        Some(ViewId::ProfileEditor)
    );

    SettingsView::navigate_to_mcp_add();
    assert_eq!(
        crate::ui_gpui::navigation_channel().take_pending(),
        Some(ViewId::McpAdd)
    );

    SettingsView::navigate_to_chat();
    assert_eq!(
        crate::ui_gpui::navigation_channel().take_pending(),
        Some(ViewId::Chat)
    );
}

#[gpui::test]
async fn tool_approval_policy_updated_applies_all_fields(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::ToolApprovalPolicyUpdated {
                yolo_mode: true,
                auto_approve_reads: true,
                skills_auto_approve: false,
                mcp_approval_mode: crate::agent::McpApprovalMode::PerServer,
                persistent_allowlist: vec!["git".to_string(), "ls".to_string()],
                persistent_denylist: vec!["rm".to_string()],
            },
            cx,
        );
    });

    view.read_with(cx, |view, _cx| {
        let state = view.get_state();
        assert!(state.yolo_mode);
        assert!(state.auto_approve_reads);
        assert_eq!(
            state.mcp_approval_mode,
            crate::agent::McpApprovalMode::PerServer
        );
        assert_eq!(state.persistent_allowlist, vec!["git", "ls"]);
        assert_eq!(state.persistent_denylist, vec!["rm"]);
    });
}

#[gpui::test]
async fn tool_approval_policy_updated_clears_input_buffers(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.allowlist_input = "pending-entry".to_string();
        view.state.denylist_input = "another-pending".to_string();
    });

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::ToolApprovalPolicyUpdated {
                yolo_mode: false,
                auto_approve_reads: false,
                skills_auto_approve: false,
                mcp_approval_mode: crate::agent::McpApprovalMode::PerTool,
                persistent_allowlist: vec![],
                persistent_denylist: vec![],
            },
            cx,
        );
    });

    view.read_with(cx, |view, _cx| {
        assert!(view.state.allowlist_input.is_empty());
        assert!(view.state.denylist_input.is_empty());
    });
}

#[gpui::test]
async fn yolo_mode_changed_updates_state(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.read_with(cx, |view, _cx| {
        assert!(!view.get_state().yolo_mode);
    });

    view.update(cx, |view, cx| {
        view.handle_command(ViewCommand::YoloModeChanged { active: true }, cx);
    });

    view.read_with(cx, |view, _cx| {
        assert!(view.get_state().yolo_mode);
    });
}

#[gpui::test]
async fn show_error_sets_status_message_as_error(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::ShowError {
                title: "Tool Approval Settings".to_string(),
                message: "Failed to persist allowlist".to_string(),
                severity: crate::presentation::view_command::ErrorSeverity::Warning,
            },
            cx,
        );
    });

    view.read_with(cx, |view, _cx| {
        assert!(view.state.status_is_error);
        assert!(view
            .state
            .status_message
            .as_ref()
            .unwrap()
            .contains("Failed to persist allowlist"));
    });
}

#[gpui::test]
async fn show_notification_sets_status_without_error(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::ShowNotification {
                message: "Settings saved".to_string(),
            },
            cx,
        );
    });

    view.read_with(cx, |view, _cx| {
        assert!(!view.state.status_is_error);
        assert_eq!(view.state.status_message.as_deref(), Some("Settings saved"));
    });
}

#[gpui::test]
async fn add_allowlist_entry_emits_event_without_clearing_input(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.state.allowlist_input = "git".to_string();
        view.add_allowlist_entry();
    });

    let event = user_rx.try_recv().unwrap();
    assert_eq!(
        event,
        UserEvent::AddToolApprovalAllowlistPrefix {
            prefix: "git".to_string()
        }
    );

    view.read_with(cx, |view, _cx| {
        assert_eq!(view.state.allowlist_input, "git");
    });
}

#[gpui::test]
async fn add_denylist_entry_emits_event_without_clearing_input(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.state.denylist_input = "rm".to_string();
        view.add_denylist_entry();
    });

    let event = user_rx.try_recv().unwrap();
    assert_eq!(
        event,
        UserEvent::AddToolApprovalDenylistPrefix {
            prefix: "rm".to_string()
        }
    );

    view.read_with(cx, |view, _cx| {
        assert_eq!(view.state.denylist_input, "rm");
    });
}

#[gpui::test]
async fn add_entry_ignores_empty_input(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.state.allowlist_input = "   ".to_string();
        view.add_allowlist_entry();
        view.state.denylist_input = String::new();
        view.add_denylist_entry();
    });

    assert!(user_rx.try_recv().is_err());
}

#[gpui::test]
async fn remove_allowlist_entry_emits_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.remove_allowlist_entry("git".to_string());
    });

    assert_eq!(
        user_rx.try_recv().unwrap(),
        UserEvent::RemoveToolApprovalAllowlistPrefix {
            prefix: "git".to_string()
        }
    );
}

#[gpui::test]
async fn remove_denylist_entry_emits_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.remove_denylist_entry("rm".to_string());
    });

    assert_eq!(
        user_rx.try_recv().unwrap(),
        UserEvent::RemoveToolApprovalDenylistPrefix {
            prefix: "rm".to_string()
        }
    );
}

#[gpui::test]
async fn cycle_active_field_rotates_through_fields(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        assert!(view.state.active_field.is_none());

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::ExportDirInput));

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::AllowlistInput));

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::DenylistInput));

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::ExportDirInput));
    });
}

#[gpui::test]
async fn set_active_field_resets_ime_marked_bytes(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.ime_marked_byte_count = 5;
        view.set_active_field(Some(ActiveField::DenylistInput));
        assert_eq!(view.ime_marked_byte_count, 0);
        assert_eq!(view.state.active_field, Some(ActiveField::DenylistInput));
    });
}

#[gpui::test]
async fn emit_set_yolo_mode_sends_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.emit_set_yolo_mode(true);
    });

    assert_eq!(
        user_rx.try_recv().unwrap(),
        UserEvent::SetToolApprovalYoloMode { enabled: true }
    );
}

#[gpui::test]
async fn emit_set_auto_approve_reads_sends_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.emit_set_auto_approve_reads(true);
    });

    assert_eq!(
        user_rx.try_recv().unwrap(),
        UserEvent::SetToolApprovalAutoApproveReads { enabled: true }
    );
}

#[gpui::test]
async fn emit_set_mcp_approval_mode_sends_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.emit_set_mcp_approval_mode(crate::agent::McpApprovalMode::PerServer);
    });

    assert_eq!(
        user_rx.try_recv().unwrap(),
        UserEvent::SetToolApprovalMcpApprovalMode {
            mode: crate::agent::McpApprovalMode::PerServer
        }
    );
}

#[gpui::test]
async fn append_to_active_field_adds_text_to_correct_buffer(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.set_active_field(Some(ActiveField::AllowlistInput));
        view.append_to_active_field("hello");
        assert_eq!(view.state.allowlist_input, "hello");
        assert!(view.state.denylist_input.is_empty());

        view.set_active_field(Some(ActiveField::DenylistInput));
        view.append_to_active_field("world");
        assert_eq!(view.state.denylist_input, "world");
        assert_eq!(view.state.allowlist_input, "hello");
    });
}

#[gpui::test]
async fn backspace_active_field_removes_last_char(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.set_active_field(Some(ActiveField::AllowlistInput));
        view.state.allowlist_input = "abc".to_string();
        view.backspace_active_field();
        assert_eq!(view.state.allowlist_input, "ab");
    });
}

#[gpui::test]
async fn active_field_text_returns_correct_buffer(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.allowlist_input = "allow".to_string();
        view.state.denylist_input = "deny".to_string();

        view.set_active_field(Some(ActiveField::AllowlistInput));
        assert_eq!(view.active_field_text(), "allow");

        view.set_active_field(Some(ActiveField::DenylistInput));
        assert_eq!(view.active_field_text(), "deny");

        view.state.export_dir_input = "/tmp/exports".to_string();
        view.set_active_field(Some(ActiveField::ExportDirInput));
        assert_eq!(view.active_field_text(), "/tmp/exports");

        view.set_active_field(None);
        assert_eq!(view.active_field_text(), "");
    });
}

#[gpui::test]
async fn save_export_directory_emits_set_export_directory_event(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.state.export_dir_input = "/tmp/exports".to_string();
        view.save_export_directory();
    });

    let event = user_rx.try_recv().unwrap();
    assert_eq!(
        event,
        UserEvent::SetExportDirectory {
            path: "/tmp/exports".to_string()
        }
    );
}

#[gpui::test]
async fn reset_export_directory_emits_empty_path(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut v = SettingsView::new(cx);
        v.bridge = Some(bridge);
        v
    });

    view.update(cx, |view, _cx| {
        view.state.export_dir_input = "/tmp/exports".to_string();
        view.state.export_dir_input.clear();
        view.save_export_directory();
    });

    let event = user_rx.try_recv().unwrap();
    assert_eq!(
        event,
        UserEvent::SetExportDirectory {
            path: String::new()
        }
    );
}

#[gpui::test]
async fn export_dir_input_field_append_and_backspace(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.set_active_field(Some(ActiveField::ExportDirInput));
        view.append_to_active_field("/tmp/test");
        assert_eq!(view.state.export_dir_input, "/tmp/test");

        view.backspace_active_field();
        assert_eq!(view.state.export_dir_input, "/tmp/tes");
    });
}

#[gpui::test]
async fn cycle_active_field_includes_export_dir(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        assert_eq!(view.state.active_field, None);

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::ExportDirInput));

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::AllowlistInput));

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::DenylistInput));

        view.cycle_active_field();
        assert_eq!(view.state.active_field, Some(ActiveField::ExportDirInput));
    });
}

#[gpui::test]
async fn input_handler_tracks_marked_text_and_cursor_for_export_directory(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|window, app| {
        view.update(app, |view: &mut SettingsView, cx| {
            view.set_active_field(Some(ActiveField::ExportDirInput));
            view.replace_text_in_range(None, "/tmp", window, cx);
            assert_eq!(view.state.export_dir_input, "/tmp");
            assert_eq!(
                view.text_for_range(0..2, &mut None, window, cx),
                Some("/t".to_string())
            );

            view.replace_and_mark_text_in_range(None, "/ex", None, window, cx);
            assert_eq!(view.state.export_dir_input, "/tmp/ex");
            assert_eq!(view.marked_text_range(window, cx), Some(4..7));

            view.replace_text_in_range(None, "ports", window, cx);
            assert_eq!(view.state.export_dir_input, "/tmpports");
            assert_eq!(view.marked_text_range(window, cx), None);

            let selection = view
                .selected_text_range(false, window, cx)
                .expect("selection range");
            let len = "/tmpports".encode_utf16().count();
            assert_eq!(selection.range, len..len);

            view.replace_and_mark_text_in_range(None, "/exports", None, window, cx);
            assert!(view.marked_text_range(window, cx).is_some());
            view.unmark_text(window, cx);
            assert_eq!(view.marked_text_range(window, cx), None);
            assert!(view
                .bounds_for_range(0..1, Bounds::default(), window, cx)
                .is_none());
            assert!(view
                .character_index_for_point(gpui::point(Pixels::ZERO, Pixels::ZERO), window, cx)
                .is_none());
        });
    });
}
