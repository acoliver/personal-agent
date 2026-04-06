//! Action helpers for `SettingsView`.

use super::SettingsView;
use crate::events::types::UserEvent;

impl SettingsView {
    pub(super) fn save_export_directory(&self) {
        let path = self.state.export_dir_input.trim().to_string();
        self.emit(&UserEvent::SetExportDirectory { path });
    }

    pub(super) fn apply_selected_theme(&mut self, cx: &mut gpui::Context<Self>) {
        if self.state.available_themes.is_empty() {
            return;
        }
        let selected_slug = self
            .state
            .available_themes
            .iter()
            .find(|theme| theme.slug == self.state.selected_theme_slug)
            .map(|theme| theme.slug.clone())
            .or_else(|| {
                self.state
                    .available_themes
                    .first()
                    .map(|theme| theme.slug.clone())
            });
        if let Some(slug) = selected_slug {
            self.select_theme(slug, cx);
        }
    }
}
