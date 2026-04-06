use super::SettingsView;
use crate::models::SkillSource;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, SharedString};

impl SettingsView {
    fn render_skill_row(
        skill: &crate::ui_gpui::views::settings_view::SkillItem,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let source_label = match skill.source {
            SkillSource::Bundled => "bundled",
            SkillSource::User => "user",
        };
        let source_color = match skill.source {
            SkillSource::Bundled => Theme::text_muted(),
            SkillSource::User => Theme::accent(),
        };
        let skill_name = skill.name.clone();
        let enabled = skill.enabled;

        div()
            .id(SharedString::from(format!("skill-{}", skill.name)))
            .w_full()
            .px(px(8.0))
            .py(px(6.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(source_color)
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(source_color)
                    .child(source_label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(Theme::text_primary())
                            .child(skill.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(skill.description.clone()),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("toggle-skill-{}", skill.name)))
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(if enabled {
                        Theme::selection_bg()
                    } else {
                        Theme::bg_dark()
                    })
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(if enabled {
                        Theme::selection_fg()
                    } else {
                        Theme::text_muted()
                    })
                    .cursor_pointer()
                    .child(if enabled { "ON" } else { "OFF" })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, _cx| {
                            this.emit_set_skill_enabled(skill_name.clone(), !enabled);
                        }),
                    ),
            )
    }

    pub(super) fn render_skills_section(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("SKILLS"),
            )
            .child(Self::render_toggle(
                "toggle-skills-auto-approve",
                "Auto-approve skill activation",
                self.state.skills_auto_approve,
                cx,
                |this, _cx| this.emit_set_skills_auto_approve(!this.state.skills_auto_approve),
            ))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .when(self.state.skills.is_empty(), |d| {
                        d.child(
                            div()
                                .px(px(8.0))
                                .py(px(8.0))
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_muted())
                                .child("No skills found. Add skills to ~/Library/Application Support/PersonalAgent/skills/"),
                        )
                    })
                    .children(self.state.skills.iter().map(|skill| Self::render_skill_row(skill, cx))),
            )
    }
}
