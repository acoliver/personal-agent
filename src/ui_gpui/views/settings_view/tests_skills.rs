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
