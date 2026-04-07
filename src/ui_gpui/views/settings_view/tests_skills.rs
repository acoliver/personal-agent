use super::*;
use crate::presentation::view_command::ViewCommand;
use gpui::TestAppContext;

fn clear_navigation_requests() {
    while crate::ui_gpui::navigation_channel()
        .take_pending()
        .is_some()
    {}
}

#[gpui::test]
fn handle_command_applies_skills_loaded(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::SkillsLoaded {
                skills: vec![
                    crate::presentation::view_command::SkillSummary {
                        name: "docx".to_string(),
                        description: "Word docs".to_string(),
                        source: crate::models::SkillSource::Bundled,
                        enabled: true,
                        path: "/skills/docx".to_string(),
                    },
                    crate::presentation::view_command::SkillSummary {
                        name: "meeting-notes".to_string(),
                        description: "Meeting notes".to_string(),
                        source: crate::models::SkillSource::User,
                        enabled: false,
                        path: "/user/meeting-notes".to_string(),
                    },
                ],
                watched_directories: vec!["/home/skills".to_string(), "/extra".to_string()],
                default_directory: "/default/skills".to_string(),
            },
            cx,
        );

        let state = view.get_state();
        assert_eq!(state.skills.len(), 2);
        assert_eq!(state.skills[0].name, "docx");
        assert!(state.skills[0].enabled);
        assert_eq!(state.skills[1].name, "meeting-notes");
        assert!(!state.skills[1].enabled);
        assert_eq!(
            state.watched_skill_directories,
            vec!["/home/skills", "/extra"]
        );
        assert_eq!(state.default_skill_directory, "/default/skills");
    });

    clear_navigation_requests();
}

#[gpui::test]
fn handle_command_skills_loaded_empty_list(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::SkillsLoaded {
                skills: vec![],
                watched_directories: vec![],
                default_directory: String::new(),
            },
            cx,
        );

        let state = view.get_state();
        assert!(state.skills.is_empty());
        assert!(state.watched_skill_directories.is_empty());
        assert!(state.default_skill_directory.is_empty());
    });

    clear_navigation_requests();
}

fn make_skill_item(name: &str, source: crate::models::SkillSource, enabled: bool) -> SkillItem {
    SkillItem {
        name: name.to_string(),
        description: format!("{name} description"),
        source,
        enabled,
        path: format!("/skills/{name}"),
    }
}

#[gpui::test]
fn set_skill_items_auto_selects_first(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        let skills = vec![
            make_skill_item("alpha", crate::models::SkillSource::Bundled, true),
            make_skill_item("beta", crate::models::SkillSource::User, false),
        ];
        view.set_skill_items(skills);

        assert_eq!(
            view.state.selected_skill_name.as_deref(),
            Some("alpha"),
            "first skill should be auto-selected"
        );
    });

    clear_navigation_requests();
}

#[gpui::test]
fn set_skill_items_preserves_valid_selection(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        let skills = vec![
            make_skill_item("alpha", crate::models::SkillSource::Bundled, true),
            make_skill_item("beta", crate::models::SkillSource::User, true),
        ];
        view.set_skill_items(skills);
        view.select_skill("beta".to_string());

        // Re-set items that still include beta
        let skills = vec![
            make_skill_item("alpha", crate::models::SkillSource::Bundled, true),
            make_skill_item("beta", crate::models::SkillSource::User, true),
        ];
        view.set_skill_items(skills);

        assert_eq!(
            view.state.selected_skill_name.as_deref(),
            Some("beta"),
            "valid selection should be preserved"
        );
    });

    clear_navigation_requests();
}

#[gpui::test]
fn set_skill_items_resets_stale_selection(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        let skills = vec![
            make_skill_item("alpha", crate::models::SkillSource::Bundled, true),
            make_skill_item("beta", crate::models::SkillSource::User, true),
        ];
        view.set_skill_items(skills);
        view.select_skill("beta".to_string());

        // Re-set items without beta
        let skills = vec![make_skill_item(
            "alpha",
            crate::models::SkillSource::Bundled,
            true,
        )];
        view.set_skill_items(skills);

        assert_eq!(
            view.state.selected_skill_name.as_deref(),
            Some("alpha"),
            "stale selection should fall back to first"
        );
    });

    clear_navigation_requests();
}

#[gpui::test]
fn set_skill_items_empty_clears_selection(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        let skills = vec![make_skill_item(
            "alpha",
            crate::models::SkillSource::Bundled,
            true,
        )];
        view.set_skill_items(skills);
        assert!(view.state.selected_skill_name.is_some());

        view.set_skill_items(vec![]);
        assert!(
            view.state.selected_skill_name.is_none(),
            "empty list should clear selection"
        );
    });

    clear_navigation_requests();
}

#[gpui::test]
fn select_skill_sets_selected_name(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.select_skill("my-skill".to_string());
        assert_eq!(view.state.selected_skill_name.as_deref(), Some("my-skill"));
    });

    clear_navigation_requests();
}

#[gpui::test]
fn selected_skill_returns_matching_item(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        let skills = vec![
            make_skill_item("alpha", crate::models::SkillSource::Bundled, true),
            make_skill_item("beta", crate::models::SkillSource::User, false),
        ];
        view.set_skill_items(skills);
        view.select_skill("beta".to_string());

        let selected = view.selected_skill();
        assert!(selected.is_some(), "should find selected skill");
        assert_eq!(selected.unwrap().name, "beta");
    });

    clear_navigation_requests();
}

#[gpui::test]
fn selected_skill_returns_none_when_no_selection(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.selected_skill_name = None;
        assert!(view.selected_skill().is_none());
    });

    clear_navigation_requests();
}

#[gpui::test]
fn install_skill_url_input_field_append_and_backspace(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.active_field = Some(ActiveField::InstallSkillUrlInput);

        view.append_to_active_field("https://");
        assert_eq!(view.state.install_skill_url_input, "https://");

        view.append_to_active_field("example.com/SKILL.md");
        assert_eq!(
            view.state.install_skill_url_input,
            "https://example.com/SKILL.md"
        );

        view.backspace_active_field();
        assert_eq!(
            view.state.install_skill_url_input,
            "https://example.com/SKILL.m"
        );
    });

    clear_navigation_requests();
}

#[gpui::test]
fn cycle_active_field_includes_install_skill_url(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        // Cycling from DenylistInput should reach InstallSkillUrlInput
        view.state.active_field = Some(ActiveField::DenylistInput);
        view.cycle_active_field();
        assert_eq!(
            view.state.active_field,
            Some(ActiveField::InstallSkillUrlInput),
            "should cycle from denylist to install URL input"
        );

        // Cycling from InstallSkillUrlInput should wrap to ExportDirInput
        view.cycle_active_field();
        assert_eq!(
            view.state.active_field,
            Some(ActiveField::ExportDirInput),
            "should cycle from install URL to export dir"
        );
    });

    clear_navigation_requests();
}

#[gpui::test]
fn handle_command_skills_loaded_updates_watched_directories(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::SkillsLoaded {
                skills: vec![],
                watched_directories: vec![
                    "/home/user/skills".to_string(),
                    "/opt/skills".to_string(),
                ],
                default_directory: "/default/skills".to_string(),
            },
            cx,
        );

        assert_eq!(view.state.watched_skill_directories.len(), 2);
        assert_eq!(view.state.watched_skill_directories[0], "/home/user/skills");
        assert_eq!(view.state.watched_skill_directories[1], "/opt/skills");
    });

    clear_navigation_requests();
}

#[gpui::test]
fn handle_command_show_notification_sets_status_message(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::ShowNotification {
                message: "Installed skill successfully".to_string(),
            },
            cx,
        );

        assert_eq!(
            view.state.status_message.as_deref(),
            Some("Installed skill successfully")
        );
        assert!(!view.state.status_is_error);
    });

    clear_navigation_requests();
}

#[gpui::test]
fn handle_command_show_error_sets_status_as_error(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::ShowError {
                title: "Skills".to_string(),
                message: "Failed to install".to_string(),
                severity: crate::presentation::view_command::ErrorSeverity::Warning,
            },
            cx,
        );

        assert_eq!(
            view.state.status_message.as_deref(),
            Some("Skills: Failed to install")
        );
        assert!(view.state.status_is_error);
    });

    clear_navigation_requests();
}

#[gpui::test]
fn skill_item_from_summary_maps_all_fields(cx: &mut TestAppContext) {
    let _ = cx;
    let summary = crate::presentation::view_command::SkillSummary {
        name: "my-skill".to_string(),
        description: "A test skill".to_string(),
        source: crate::models::SkillSource::User,
        enabled: false,
        path: "/path/to/skill".to_string(),
    };

    let item: SkillItem = summary.into();
    assert_eq!(item.name, "my-skill");
    assert_eq!(item.description, "A test skill");
    assert_eq!(item.source, crate::models::SkillSource::User);
    assert!(!item.enabled);
    assert_eq!(item.path, "/path/to/skill");
}

#[gpui::test]
fn settings_category_skills_display_name(cx: &mut TestAppContext) {
    let _ = cx;
    assert_eq!(SettingsCategory::Skills.display_name(), "Skills");
}

#[gpui::test]
fn settings_category_all_includes_skills(cx: &mut TestAppContext) {
    let _ = cx;
    assert!(
        SettingsCategory::ALL.contains(&SettingsCategory::Skills),
        "ALL should include Skills category"
    );
}

#[gpui::test]
fn handle_command_skills_loaded_with_empty_list_clears_state(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    // First load some skills
    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::SkillsLoaded {
                skills: vec![crate::presentation::view_command::SkillSummary {
                    name: "docx".to_string(),
                    description: "Word".to_string(),
                    source: crate::models::SkillSource::Bundled,
                    enabled: true,
                    path: "/skills/docx".to_string(),
                }],
                watched_directories: vec!["/home/skills".to_string()],
                default_directory: "/default/skills".to_string(),
            },
            cx,
        );
        assert_eq!(view.state.skills.len(), 1);
        assert!(view.state.selected_skill_name.is_some());
    });

    // Then load empty list
    view.update(cx, |view, cx| {
        view.handle_command(
            ViewCommand::SkillsLoaded {
                skills: vec![],
                watched_directories: vec![],
                default_directory: "/default/skills".to_string(),
            },
            cx,
        );
        assert!(view.state.skills.is_empty());
        assert!(
            view.state.selected_skill_name.is_none(),
            "selection should be cleared when skills list becomes empty"
        );
        assert!(view.state.watched_skill_directories.is_empty());
    });

    clear_navigation_requests();
}

#[gpui::test]
fn install_skill_url_input_clear(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, _cx| {
        view.state.active_field = Some(ActiveField::InstallSkillUrlInput);
        view.state
            .install_skill_url_input
            .push_str("https://example.com/SKILL.md");

        // Simulate clearing via backspace
        while !view.state.install_skill_url_input.is_empty() {
            view.state.install_skill_url_input.pop();
        }

        assert!(view.state.install_skill_url_input.is_empty());
    });

    clear_navigation_requests();
}

#[gpui::test]
fn default_skill_directory_set_by_skills_loaded(cx: &mut TestAppContext) {
    let view = cx.new(SettingsView::new);

    view.update(cx, |view, cx| {
        assert!(
            view.state.default_skill_directory.is_empty(),
            "initially empty"
        );

        view.handle_command(
            ViewCommand::SkillsLoaded {
                skills: vec![],
                watched_directories: vec![],
                default_directory: "/home/user/.config/skills".to_string(),
            },
            cx,
        );

        assert_eq!(
            view.state.default_skill_directory,
            "/home/user/.config/skills"
        );
    });

    clear_navigation_requests();
}
