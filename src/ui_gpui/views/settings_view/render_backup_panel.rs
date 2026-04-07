//! Backup panel rendering for `SettingsView`.

use super::SettingsView;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, MouseButton, SharedString};

#[allow(clippy::unused_self)]
impl SettingsView {
    /// Backup panel: automatic backup settings, manual backup controls, and restore.
    pub(super) fn render_backup_panel(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let settings = self.state.backup_settings.clone().unwrap_or_default();
        let backups = &self.state.backups;
        let in_progress = self.state.backup_in_progress;
        let last_backup = self.state.last_backup_time;
        let status = self.state.backup_status.clone();
        let selected_backup_id = self.state.selected_backup_id;

        div()
            .id("backup-panel-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(16.0))
            .overflow_y_scroll()
            .child(self.render_backup_settings_section(&settings, cx))
            .child(self.render_backup_status_section(last_backup, backups.len(), status, cx))
            .child(self.render_backup_actions_section(in_progress, cx))
            .child(self.render_restore_section(backups, selected_backup_id, cx))
    }

    /// Backup settings section: enable toggle, interval, max copies, directory.
    fn render_backup_settings_section(
        &self,
        settings: &crate::backup::DatabaseBackupSettings,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let enabled = settings.enabled;
        let interval_hours = settings.interval_hours;
        let max_copies = settings.max_copies;
        let backup_dir = settings.backup_directory.as_ref().map_or_else(
            || "Default location".to_string(),
            |p| p.display().to_string(),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("AUTOMATIC BACKUP"),
            )
            .child(self.render_backup_toggle(enabled, cx))
            .when(enabled, |d| {
                d.child(self.render_backup_interval_selector(interval_hours, cx))
                    .child(self.render_backup_max_copies_selector(max_copies, cx))
                    .child(self.render_backup_directory_row(&backup_dir, cx))
            })
    }

    /// Enable/disable automatic backups toggle.
    fn render_backup_toggle(
        &self,
        enabled: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("backup-toggle-row")
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, _cx| {
                    this.emit_set_backup_enabled(!enabled);
                }),
            )
            .child(self.render_checkbox_indicator(enabled))
            .child(
                div()
                    .text_size(px(Theme::font_size_mono()))
                    .text_color(Theme::text_primary())
                    .child("Enable automatic backups"),
            )
    }

    /// Interval selector (hours between backups).
    fn render_backup_interval_selector(
        &self,
        current_hours: u32,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let hours_options = vec![1, 2, 4, 6, 8, 12, 24, 48];

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("BACKUP INTERVAL"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .children(hours_options.into_iter().map(move |hours| {
                        let is_selected = current_hours == hours;
                        div()
                            .id(SharedString::from(format!("interval-{hours}")))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .border_1()
                            .border_color(if is_selected {
                                Theme::accent()
                            } else {
                                Theme::border()
                            })
                            .bg(if is_selected {
                                Theme::selection_bg()
                            } else {
                                Theme::bg_dark()
                            })
                            .text_color(if is_selected {
                                Theme::selection_fg()
                            } else {
                                Theme::text_primary()
                            })
                            .text_size(px(Theme::font_size_ui()))
                            .child(format!("{hours}h"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, _cx| {
                                    this.emit_set_backup_interval_hours(hours);
                                }),
                            )
                            .into_any_element()
                    }))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .child("hours between backups"),
                    ),
            )
    }

    /// Max copies selector.
    fn render_backup_max_copies_selector(
        &self,
        current_copies: u32,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let copies_options = vec![5, 10, 20, 50, 100];

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("MAX BACKUP COPIES"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .children(copies_options.into_iter().map(move |copies| {
                        let is_selected = current_copies == copies;
                        div()
                            .id(SharedString::from(format!("copies-{copies}")))
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .border_1()
                            .border_color(if is_selected {
                                Theme::accent()
                            } else {
                                Theme::border()
                            })
                            .bg(if is_selected {
                                Theme::selection_bg()
                            } else {
                                Theme::bg_dark()
                            })
                            .text_color(if is_selected {
                                Theme::selection_fg()
                            } else {
                                Theme::text_primary()
                            })
                            .text_size(px(Theme::font_size_ui()))
                            .child(format!("{copies}"))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, _cx| {
                                    this.emit_set_backup_max_copies(copies);
                                }),
                            )
                            .into_any_element()
                    }))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .child("copies to retain"),
                    ),
            )
    }

    /// Backup directory display with Change and Reset buttons.
    fn render_backup_directory_row(
        &self,
        backup_dir: &str,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let dir_text = backup_dir.to_string();

        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("BACKUP DIRECTORY"),
            )
            .child(
                div()
                    .w_full()
                    .p(px(8.0))
                    .bg(Theme::bg_darker())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(Theme::font_size_mono()))
                            .text_color(Theme::text_primary())
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(dir_text),
                    )
                    .child(
                        div()
                            .id("btn-change-backup-dir")
                            .px(px(12.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Change")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.browse_backup_directory(cx);
                                }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("btn-reset-backup-dir")
                    .px(px(12.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_secondary())
                    .child("Reset to Default Directory")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, _cx| {
                            this.emit_set_backup_directory(None);
                        }),
                    ),
            )
    }

    /// Status section showing last backup time and backup count.
    fn render_backup_status_section(
        &self,
        last_backup: Option<chrono::DateTime<chrono::Utc>>,
        backup_count: usize,
        status: Option<String>,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let last_backup_text = last_backup.map_or_else(
            || "Never".to_string(),
            |dt| dt.format("%Y-%m-%d %H:%M UTC").to_string(),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("BACKUP STATUS"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::text_muted())
                                    .child("Last backup:"),
                            )
                            .child(
                                div()
                                    .text_size(px(Theme::font_size_mono()))
                                    .text_color(Theme::text_primary())
                                    .child(last_backup_text),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(Theme::font_size_ui()))
                                    .text_color(Theme::text_muted())
                                    .child("Backup count:"),
                            )
                            .child(
                                div()
                                    .text_size(px(Theme::font_size_mono()))
                                    .text_color(Theme::text_primary())
                                    .child(format!("{backup_count}")),
                            ),
                    ),
            )
            .when_some(status, |d, msg| {
                d.child(
                    div()
                        .w_full()
                        .p(px(8.0))
                        .rounded(px(4.0))
                        .bg(Theme::bg_dark())
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::text_primary())
                        .child(msg),
                )
            })
    }

    /// Action buttons: Back Up Now, Refresh List.
    fn render_backup_actions_section(
        &self,
        in_progress: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("ACTIONS"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("btn-backup-now")
                            .px(px(16.0))
                            .py(px(8.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .when(!in_progress, |d| d.hover(|s| s.bg(Theme::accent())))
                            .bg(if in_progress {
                                Theme::bg_dark()
                            } else {
                                Theme::selection_bg()
                            })
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(if in_progress {
                                Theme::text_muted()
                            } else {
                                Theme::selection_fg()
                            })
                            .child(if in_progress {
                                "Backing up..."
                            } else {
                                "Back Up Now"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, _cx| {
                                    if !in_progress {
                                        this.emit_trigger_backup_now();
                                    }
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-refresh-backups")
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_dark()))
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child("Refresh")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, _cx| {
                                    this.emit_refresh_backup_list();
                                }),
                            ),
                    ),
            )
    }

    /// Restore section with backup list.
    fn render_restore_section(
        &self,
        backups: &[crate::backup::BackupInfo],
        selected_backup_id: Option<usize>,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_primary())
                    .child("RESTORE FROM BACKUP"),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child("Select a backup to restore your database:"),
            )
            .child(self.render_backup_list(backups, selected_backup_id, cx))
            .when(selected_backup_id.is_some(), |d| {
                d.child(self.render_restore_button(selected_backup_id, backups, cx))
            })
    }

    /// Render the list of available backups.
    fn render_backup_list(
        &self,
        backups: &[crate::backup::BackupInfo],
        selected_backup_id: Option<usize>,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("backup-list")
            .w_full()
            .max_h(px(200.0))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::border())
            .rounded(px(4.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .when(backups.is_empty(), |d| {
                d.items_center().justify_center().child(
                    div()
                        .p(px(16.0))
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_muted())
                        .child("No backups available"),
                )
            })
            .children(backups.iter().enumerate().map(move |(idx, backup)| {
                let is_selected = selected_backup_id == Some(idx);
                let timestamp = backup.formatted_timestamp();
                let size = backup.formatted_size();
                let path_str = backup.path.display().to_string();

                div()
                    .id(SharedString::from(format!("backup-{idx}")))
                    .w_full()
                    .h(px(32.0))
                    .px(px(8.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .when(is_selected, |d| {
                        d.bg(Theme::selection_bg())
                            .text_color(Theme::selection_fg())
                    })
                    .when(!is_selected, |d| {
                        d.hover(|s| s.bg(Theme::bg_dark()))
                            .text_color(Theme::text_primary())
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(self.render_checkbox_indicator(is_selected))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(Theme::font_size_mono()))
                                            .child(timestamp),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(Theme::font_size_ui()))
                                            .text_color(if is_selected {
                                                Theme::selection_fg()
                                            } else {
                                                Theme::text_muted()
                                            })
                                            .child(format!("{size} — {path_str}")),
                                    ),
                            ),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _window, cx| {
                            this.state.selected_backup_id = Some(idx);
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            }))
    }

    /// Restore button for selected backup.
    fn render_restore_button(
        &self,
        _selected_backup_id: Option<usize>,
        backups: &[crate::backup::BackupInfo],
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        // Get the path at render time, not in the closure
        let restore_path = self
            .state
            .selected_backup_id
            .and_then(|id| backups.get(id))
            .map(|b| b.path.display().to_string());

        let can_restore = restore_path.is_some();

        div()
            .id("btn-restore-backup")
            .px(px(16.0))
            .py(px(8.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .when(can_restore, |d| d.hover(|s| s.bg(Theme::danger())))
            .bg(Theme::danger())
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::selection_fg())
            .child("Restore Selected Backup")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, _cx| {
                    if let Some(path) = restore_path.clone() {
                        this.emit_restore_backup(path);
                    }
                }),
            )
    }

    /// Helper: render a checkbox indicator (checked/unchecked box).
    fn render_checkbox_indicator(&self, checked: bool) -> impl IntoElement {
        div()
            .size(px(14.0))
            .rounded(px(2.0))
            .border_1()
            .border_color(Theme::border())
            .bg(if checked {
                Theme::accent()
            } else {
                Theme::bg_dark()
            })
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::text_primary())
            .when(checked, |d| {
                d.child(
                    div()
                        .text_size(px(Theme::font_size_small()))
                        .text_color(Theme::selection_fg())
                        .child("v"),
                )
            })
    }
}
