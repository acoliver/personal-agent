use super::{SettingsView, SkillItem};
use crate::models::SkillSource;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, SharedString};

impl SettingsView {
    fn render_skill_source_badge(source: SkillSource) -> impl IntoElement {
        let (label, color) = match source {
            SkillSource::Bundled => ("bundled", Theme::text_muted()),
            SkillSource::User => ("user", Theme::accent()),
        };

        div()
            .px(px(6.0))
            .py(px(2.0))
            .rounded(px(3.0))
            .border_1()
            .border_color(color)
            .text_size(px(Theme::font_size_ui()))
            .text_color(color)
            .child(label)
    }

    fn render_skill_row(
        &self,
        skill: &SkillItem,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let is_selected = self
            .state
            .selected_skill_name
            .as_ref()
            .is_some_and(|name| name == &skill.name);
        let skill_name = skill.name.clone();
        let toggle_name = skill.name.clone();
        let enabled = skill.enabled;

        div()
            .id(SharedString::from(format!("skill-{}", skill.name)))
            .w_full()
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(if is_selected {
                Theme::selection_bg()
            } else {
                Theme::border()
            })
            .bg(if is_selected {
                Theme::bg_dark()
            } else {
                Theme::bg_darker()
            })
            .cursor_pointer()
            .hover(|style| style.bg(Theme::bg_dark()))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(Self::render_skill_source_badge(skill.source))
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
                    .py(px(3.0))
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
                    .child(if enabled { "ON" } else { "OFF" })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.emit_set_skill_enabled(toggle_name.clone(), !enabled);
                            cx.notify();
                        }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.select_skill(skill_name.clone());
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    fn render_refresh_skills_button(cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        div()
            .id("btn-refresh-skills")
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("Refresh")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.emit_refresh_skills();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    fn render_open_skills_folder_button(cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        div()
            .id("btn-open-default-skills-dir")
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("Open Skills Folder")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, _cx| {
                    let target = if this.state.default_skill_directory.is_empty() {
                        dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
                    } else {
                        std::path::PathBuf::from(&this.state.default_skill_directory)
                    };
                    let _ = std::process::Command::new("open").arg(target).spawn();
                }),
            )
            .into_any_element()
    }

    fn render_add_skills_directory_button(cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        div()
            .id("btn-add-watched-skills-dir")
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .child("Add Directory")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.browse_skills_directory(cx);
                }),
            )
            .into_any_element()
    }

    fn render_install_skill_button(
        cx: &mut gpui::Context<Self>,
        install_url: String,
        has_install_url: bool,
    ) -> gpui::AnyElement {
        div()
            .id("btn-install-skill-url")
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .when(has_install_url, |style| {
                style.hover(|hover| hover.bg(Theme::bg_dark()))
            })
            .text_size(px(Theme::font_size_ui()))
            .text_color(if has_install_url {
                Theme::text_primary()
            } else {
                Theme::text_muted()
            })
            .child("Install URL")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    if has_install_url {
                        this.emit_install_skill_from_url(install_url.clone());
                        this.state.install_skill_url_input.clear();
                        cx.notify();
                    }
                }),
            )
            .into_any_element()
    }

    fn render_skills_toolbar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let install_url = self.state.install_skill_url_input.trim().to_string();
        let has_install_url = !install_url.is_empty();

        div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(Self::render_refresh_skills_button(cx))
            .child(Self::render_open_skills_folder_button(cx))
            .child(Self::render_add_skills_directory_button(cx))
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(format!("{} skills", self.state.skills.len())),
            )
            .child(Self::render_install_skill_button(
                cx,
                install_url,
                has_install_url,
            ))
    }

    fn render_watched_directories(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("WATCHED DIRECTORIES"),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_darker())
                            .border_1()
                            .border_color(Theme::border())
                            .child(
                                div()
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::text_muted())
                                    .child(format!(
                                        "Default install directory: {}",
                                        self.state.default_skill_directory
                                    )),
                            ),
                    )
                    .children(self.state.watched_skill_directories.iter().map(|path| {
                        let remove_path = path.clone();
                        div()
                            .w_full()
                            .px(px(8.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(Theme::bg_darker())
                            .border_1()
                            .border_color(Theme::border())
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::text_primary())
                                    .child(path.clone()),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(3.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(Theme::bg_dark()))
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::text_primary())
                                    .child("Remove")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _window, cx| {
                                            this.emit_remove_skills_directory(remove_path.clone());
                                            cx.notify();
                                        }),
                                    ),
                            )
                    }))
                    .when(self.state.watched_skill_directories.is_empty(), |style| {
                        style.child(
                            div()
                                .px(px(8.0))
                                .py(px(6.0))
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_muted())
                                .child("No extra watched directories configured."),
                        )
                    }),
            )
    }

    fn render_skill_details_panel(&self, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let selected_skill = self.selected_skill();

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("SELECTED SKILL"),
            )
            .child(
                div()
                    .w_full()
                    .min_h(px(180.0))
                    .px(px(12.0))
                    .py(px(10.0))
                    .rounded(px(4.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .when_some(selected_skill, |container, skill| {
                        container.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_size(px(Theme::font_size_mono()))
                                                .text_color(Theme::text_primary())
                                                .child(skill.name.clone()),
                                        )
                                        .child(Self::render_skill_source_badge(skill.source)),
                                )
                                .child(
                                    div()
                                        .text_size(px(Theme::font_size_body()))
                                        .text_color(Theme::text_secondary())
                                        .child(skill.description.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(Theme::font_size_ui()))
                                        .text_color(Theme::text_muted())
                                        .child(format!(
                                            "Status: {}",
                                            if skill.enabled { "Enabled" } else { "Disabled" }
                                        )),
                                )
                                .child(
                                    div()
                                        .text_size(px(Theme::font_size_ui()))
                                        .text_color(Theme::text_muted())
                                        .child(format!("Path: {}", skill.path)),
                                ),
                        )
                    })
                    .when(selected_skill.is_none(), |container| {
                        container.child(
                            div()
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_muted())
                                .child("Select a skill to inspect its details."),
                        )
                    }),
            )
    }

    fn render_skill_url_input(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        let is_active = self.state.active_field == Some(super::ActiveField::InstallSkillUrlInput);
        let display_text = if self.state.install_skill_url_input.is_empty() {
            "Paste a SKILL.md URL here\u{2026}".to_string()
        } else {
            self.state.install_skill_url_input.clone()
        };
        let text_color = if self.state.install_skill_url_input.is_empty() {
            Theme::text_muted()
        } else {
            Theme::text_primary()
        };

        div()
            .id("skill-url-input")
            .w_full()
            .h(px(28.0))
            .px(px(8.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(if is_active {
                Theme::accent()
            } else {
                Theme::border()
            })
            .rounded(px(4.0))
            .flex()
            .items_center()
            .overflow_hidden()
            .cursor_text()
            .text_size(px(Theme::font_size_mono()))
            .text_color(text_color)
            .child(display_text)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus_handle, cx);
                    this.set_active_field(Some(super::ActiveField::InstallSkillUrlInput));
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    pub(super) fn render_skills_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("skills-panel-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("SKILLS"),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .child("Manage discovered skills and imports"),
                    ),
            )
            .child(self.render_skills_toolbar(cx))
            .child(self.render_skill_url_input(cx))



            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .when(self.state.skills.is_empty(), |style| {
                        style.child(
                            div()
                                .px(px(8.0))
                                .py(px(8.0))
                                .rounded(px(4.0))
                                .bg(Theme::bg_darker())
                                .border_1()
                                .border_color(Theme::border())
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(Theme::text_muted())
                                .child(format!(
                                    "No skills found yet. Install from a URL or add a watched directory. Default location: {}",
                                    self.state.default_skill_directory
                                )),
                        )
                    })
                    .children(self.state.skills.iter().map(|skill| self.render_skill_row(skill, cx))),
            )
            .child(self.render_skill_details_panel(cx))
            .child(self.render_watched_directories(cx))
    }
}
