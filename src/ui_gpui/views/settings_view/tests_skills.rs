use super::*;
use crate::presentation::view_command::ViewCommand;
use gpui::TestAppContext;

fn clear_navigation_requests() {
    while crate::ui_gpui::navigation_channel()
        .take_pending()
        .is_some()
    {}
}

// ---------------------------------------------------------------------------
// BEHAVIORAL TESTS: Command handling
// ---------------------------------------------------------------------------

/// When `SkillsLoaded` command is received, the view should reflect the skills list,
/// watched directories, and default directory in its observable state.
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

/// Loading an empty skills list should clear the view state.
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

/// When skills are loaded, then an empty list is loaded, the selection should be cleared.
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

/// `ShowNotification` command should update the status message without error flag.
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

/// `ShowError` command should update the status message with error flag.
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

/// The default skill directory should be set by `SkillsLoaded` command.
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
