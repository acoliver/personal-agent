//! Skills state and event-emitting actions for `SettingsView`.

use super::types::SkillItem;
use super::SettingsView;
use crate::events::types::UserEvent;

impl SettingsView {
    pub(super) fn emit_set_skill_enabled(&self, name: String, enabled: bool) {
        self.emit(&UserEvent::SetSkillEnabled { name, enabled });
    }

    pub(super) fn emit_refresh_skills(&self) {
        self.emit(&UserEvent::RefreshSkills);
    }

    pub(super) fn emit_add_skills_directory(&self, path: String) {
        self.emit(&UserEvent::AddSkillsDirectory { path });
    }

    pub(super) fn emit_remove_skills_directory(&self, path: String) {
        self.emit(&UserEvent::RemoveSkillsDirectory { path });
    }

    pub(super) fn emit_install_skill_from_url(&self, url: String) {
        self.emit(&UserEvent::InstallSkillFromUrl { url });
    }

    pub(super) fn selected_skill(&self) -> Option<&SkillItem> {
        self.state
            .selected_skill_name
            .as_ref()
            .and_then(|selected_name| {
                self.state
                    .skills
                    .iter()
                    .find(|skill| &skill.name == selected_name)
            })
    }

    pub(super) fn set_skill_items(&mut self, skills: Vec<SkillItem>) {
        self.state.skills = skills;

        if self.state.selected_skill_name.is_none() {
            self.state.selected_skill_name =
                self.state.skills.first().map(|skill| skill.name.clone());
        }

        if let Some(selected_name) = self.state.selected_skill_name.as_ref() {
            if self
                .state
                .skills
                .iter()
                .all(|skill| &skill.name != selected_name)
            {
                self.state.selected_skill_name =
                    self.state.skills.first().map(|skill| skill.name.clone());
            }
        }
    }

    pub(super) fn select_skill(&mut self, name: String) {
        self.state.selected_skill_name = Some(name);
    }
}
