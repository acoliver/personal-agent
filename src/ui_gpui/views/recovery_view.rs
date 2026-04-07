//! Recovery view for database corruption recovery
//!
//! Provides a standalone GPUI view for database recovery that:
//! - Shows error message explaining the DB could not be loaded
//! - Lists available backups with timestamps and sizes
//! - Provides "Restore Latest" button (visible only if backups exist)
//! - Provides backup selection mechanism (dropdown/list)
//! - Provides "Quit" button
//! - Shows "No backups found" message when list is empty
//!
//! @requirement REQ-BACKUP-001
//! @requirement REQ-BACKUP-002

use gpui::{
    div, prelude::*, px, FocusHandle, FontWeight, IntoElement, MouseButton, ParentElement,
    ScrollHandle, SharedString, Styled,
};
use std::sync::Arc;

use crate::backup::BackupInfo;
use crate::events::types::UserEvent;
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::theme::Theme;

/// Result of the recovery startup check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryResult {
    /// Database loaded successfully, no recovery needed
    Success,
    /// Database failed to load, recovery is required
    Required {
        /// Error message explaining why the database could not be loaded
        error: String,
        /// Available backups for recovery
        available_backups: Vec<BackupInfo>,
    },
}

/// State for the recovery view
#[derive(Clone, Debug)]
pub struct RecoveryState {
    /// Error message explaining why the database could not be loaded
    pub error_message: String,
    /// List of available backups
    pub backups: Vec<BackupInfo>,
    /// Index of the currently selected backup
    pub selected_backup_index: Option<usize>,
    /// Status message to display (e.g., restore progress)
    pub status_message: Option<String>,
    /// Whether the status is an error
    pub status_is_error: bool,
    /// Whether a restore operation is in progress
    pub restore_in_progress: bool,
}

impl RecoveryState {
    #[must_use]
    pub fn new(error_message: impl Into<String>) -> Self {
        Self {
            error_message: error_message.into(),
            backups: Vec::new(),
            selected_backup_index: None,
            status_message: None,
            status_is_error: false,
            restore_in_progress: false,
        }
    }

    #[must_use]
    pub fn with_backups(mut self, backups: Vec<BackupInfo>) -> Self {
        self.backups = backups;
        // Select the most recent backup by default if any exist
        if !self.backups.is_empty() {
            self.selected_backup_index = Some(0);
        }
        self
    }

    /// Get the currently selected backup info
    #[must_use]
    pub fn selected_backup(&self) -> Option<&BackupInfo> {
        self.selected_backup_index
            .and_then(|idx| self.backups.get(idx))
    }

    /// Check if any backups are available
    #[must_use]
    pub const fn has_backups(&self) -> bool {
        !self.backups.is_empty()
    }

    /// Select the latest (most recent) backup
    pub const fn select_latest(&mut self) {
        if !self.backups.is_empty() {
            self.selected_backup_index = Some(0);
        }
    }

    /// Select a backup by index
    pub const fn select_backup(&mut self, index: usize) {
        if index < self.backups.len() {
            self.selected_backup_index = Some(index);
        }
    }

    /// Set a status message
    pub fn set_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = Some(message.into());
        self.status_is_error = is_error;
    }

    /// Clear the status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
        self.status_is_error = false;
    }
}

/// Recovery view for database corruption recovery
pub struct RecoveryView {
    state: RecoveryState,
    bridge: Option<Arc<GpuiBridge>>,
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
}

impl RecoveryView {
    pub fn new(cx: &mut gpui::Context<Self>, error_message: impl Into<String>) -> Self {
        Self {
            state: RecoveryState::new(error_message),
            bridge: None,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// Create a new recovery view with pre-populated backups
    pub fn with_backups(
        cx: &mut gpui::Context<Self>,
        error_message: impl Into<String>,
        backups: Vec<BackupInfo>,
    ) -> Self {
        Self {
            state: RecoveryState::new(error_message).with_backups(backups),
            bridge: None,
            focus_handle: cx.focus_handle(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// Set the event bridge for cross-runtime communication
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Get a reference to the current state
    #[must_use]
    pub const fn state(&self) -> &RecoveryState {
        &self.state
    }

    /// Get a mutable reference to the current state
    pub const fn state_mut(&mut self) -> &mut RecoveryState {
        &mut self.state
    }

    /// Update the backups list
    pub fn set_backups(&mut self, backups: Vec<BackupInfo>, cx: &mut gpui::Context<Self>) {
        self.state.backups = backups;
        self.state.selected_backup_index = if self.state.has_backups() {
            Some(0)
        } else {
            None
        };
        cx.notify();
    }

    /// Set restore in progress state
    pub fn set_restore_in_progress(&mut self, in_progress: bool, cx: &mut gpui::Context<Self>) {
        self.state.restore_in_progress = in_progress;
        cx.notify();
    }

    /// Emit a `UserEvent` through the bridge
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    /// Handle restore latest backup request
    fn restore_latest(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(backup) = self.state.selected_backup().cloned() {
            self.start_restore(&backup, cx);
        }
    }

    /// Handle restore from selected backup
    fn restore_selected(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(backup) = self.state.selected_backup().cloned() {
            self.start_restore(&backup, cx);
        }
    }

    /// Start a restore operation
    fn start_restore(&mut self, backup: &BackupInfo, cx: &mut gpui::Context<Self>) {
        self.state.restore_in_progress = true;
        self.state.set_status(
            format!("Restoring from {}...", backup.formatted_timestamp()),
            false,
        );
        self.emit(&UserEvent::RestoreDatabaseBackup {
            backup_path: backup.path.clone(),
        });
        cx.notify();
    }

    /// Handle restore completed successfully
    pub fn handle_restore_success(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.restore_in_progress = false;
        self.state.set_status(
            "Restore completed successfully. Please restart the application.",
            false,
        );
        cx.notify();
    }

    /// Handle restore failed
    pub fn handle_restore_failed(
        &mut self,
        error: impl Into<String>,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state.restore_in_progress = false;
        self.state
            .set_status(format!("Restore failed: {}", error.into()), true);
        cx.notify();
    }

    /// Handle quit request
    fn quit_application(&self) {
        self.emit(&UserEvent::QuitApplication);
    }

    /// Render the error header explaining the DB could not be loaded
    fn render_error_header(&self) -> impl IntoElement {
        div()
            .id("recovery-error-header")
            .w_full()
            .p(px(Theme::SPACING_LG))
            .bg(Theme::bg_darker())
            .border_1()
            .border_color(Theme::error())
            .rounded(px(Theme::RADIUS_MD))
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_SM))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::SPACING_SM))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_h3()))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::error())
                            .child("Database Error"),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_muted())
                            .child(""),
                    ),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_body()))
                    .text_color(Theme::text_secondary())
                    .child(self.state.error_message.clone()),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(
                        "Your conversation database could not be loaded. You can restore from an \
                         automatic backup below, or quit and try starting again.",
                    ),
            )
    }

    /// Render the status message if present
    fn render_status_message(&self) -> Option<gpui::AnyElement> {
        let message = self.state.status_message.as_ref()?;
        let text_color = if self.state.status_is_error {
            Theme::error()
        } else {
            Theme::text_secondary()
        };

        Some(
            div()
                .id("recovery-status-message")
                .w_full()
                .p(px(Theme::SPACING_MD))
                .bg(if self.state.status_is_error {
                    let mut bg = Theme::error();
                    bg.a = 0.1;
                    bg
                } else {
                    let mut bg = Theme::accent();
                    bg.a = 0.1;
                    bg
                })
                .border_1()
                .border_color(if self.state.status_is_error {
                    let mut c = Theme::error();
                    c.a = 0.3;
                    c
                } else {
                    let mut c = Theme::accent();
                    c.a = 0.3;
                    c
                })
                .rounded(px(Theme::RADIUS_SM))
                .child(
                    div()
                        .text_size(px(Theme::font_size_body()))
                        .text_color(text_color)
                        .child(message.clone()),
                )
                .into_any_element(),
        )
    }

    /// Render the "No backups found" message
    fn render_no_backups() -> impl IntoElement {
        div()
            .id("recovery-no-backups")
            .w_full()
            .p(px(Theme::SPACING_XL))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Theme::SPACING_MD))
            .child(
                div()
                    .text_size(px(Theme::font_size_h2()))
                    .text_color(Theme::text_muted())
                    .child(""),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_h3()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("No Backups Found"),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_body()))
                    .text_color(Theme::text_secondary())
                    .child("There are no automatic backups available to restore from."),
            )
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(
                        "You may need to delete the corrupted database file and start fresh, \
                         or contact support.",
                    ),
            )
    }

    /// Render a single backup item
    fn render_backup_item(
        &self,
        index: usize,
        backup: &BackupInfo,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        let is_selected = self.state.selected_backup_index == Some(index);
        let index_for_closure = index;
        let timestamp = backup.formatted_timestamp();
        let size = backup.formatted_size();

        let mut container = div()
            .id(SharedString::from(format!("backup-item-{index}")))
            .w_full()
            .p(px(Theme::SPACING_MD))
            .rounded(px(Theme::RADIUS_MD))
            .cursor_pointer()
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_XS));

        if is_selected {
            container = container
                .bg(Theme::accent())
                .border_1()
                .border_color(Theme::border());
        } else {
            container = container
                .bg(Theme::bg_darker())
                .hover(|s| s.bg(Theme::bg_dark()));
        }

        let text_color = if is_selected {
            Theme::accent_fg()
        } else {
            Theme::text_primary()
        };

        let secondary_color = if is_selected {
            Theme::bg_darker()
        } else {
            Theme::text_secondary()
        };

        container
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::SPACING_SM))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_mono()))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .child(format!("Backup #{}", index + 1)),
                    )
                    .when(is_selected, |d| {
                        d.child(
                            div()
                                .text_size(px(Theme::font_size_ui()))
                                .text_color(text_color)
                                .child("[OK] Selected"),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(Theme::SPACING_MD))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(secondary_color)
                            .child(timestamp),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(secondary_color)
                            .child(format!("({size})")),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.state.select_backup(index_for_closure);
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// Render the backup list
    fn render_backup_list(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let backups = self.state.backups.clone();

        div()
            .id("recovery-backup-list")
            .w_full()
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_SM))
            .children(
                backups
                    .iter()
                    .enumerate()
                    .map(|(index, backup)| self.render_backup_item(index, backup, cx)),
            )
    }

    /// Render the "Restore Latest" button (only visible if backups exist)
    fn render_restore_latest_button(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.state.has_backups() || self.state.restore_in_progress {
            return None;
        }

        Some(
            div()
                .id("btn-restore-latest")
                .h(px(40.0))
                .px(px(Theme::SPACING_LG))
                .rounded(px(Theme::RADIUS_MD))
                .bg(Theme::accent())
                .cursor_pointer()
                .hover(|s| s.bg(Theme::accent_hover()))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(Theme::font_size_body()))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::accent_fg())
                        .child("Restore Latest Backup"),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.restore_latest(cx);
                    }),
                )
                .into_any_element(),
        )
    }

    /// Render the "Restore Selected" button
    fn render_restore_selected_button(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.state.has_backups() || self.state.restore_in_progress {
            return None;
        }

        Some(
            div()
                .id("btn-restore-selected")
                .h(px(40.0))
                .px(px(Theme::SPACING_LG))
                .rounded(px(Theme::RADIUS_MD))
                .bg(Theme::bg_darker())
                .border_1()
                .border_color(Theme::border())
                .cursor_pointer()
                .hover(|s| s.bg(Theme::bg_dark()))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(Theme::font_size_body()))
                        .text_color(Theme::text_primary())
                        .child("Restore Selected Backup"),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.restore_selected(cx);
                    }),
                )
                .into_any_element(),
        )
    }

    /// Render the "Quit" button
    fn render_quit_button(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let is_in_progress = self.state.restore_in_progress;

        div()
            .id("btn-quit")
            .h(px(40.0))
            .px(px(Theme::SPACING_LG))
            .rounded(px(Theme::RADIUS_MD))
            .bg(if is_in_progress {
                Theme::bg_darkest()
            } else {
                Theme::error()
            })
            .cursor_pointer()
            .hover(|s| {
                if is_in_progress {
                    s
                } else {
                    s.bg(Theme::danger())
                }
            })
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(Theme::font_size_body()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(if is_in_progress {
                        Theme::text_muted()
                    } else {
                        Theme::error_fg()
                    })
                    .child(if is_in_progress {
                        "Restoring..."
                    } else {
                        "Quit Application"
                    }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, _cx| {
                    if !this.state.restore_in_progress {
                        this.quit_application();
                    }
                }),
            )
    }

    /// Render the action buttons section
    fn render_action_buttons(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("recovery-action-buttons")
            .w_full()
            .p(px(Theme::SPACING_LG))
            .bg(Theme::bg_darkest())
            .border_t_1()
            .border_color(Theme::border())
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_MD))
            .children(self.render_restore_latest_button(cx))
            .children(self.render_restore_selected_button(cx))
            .child(self.render_quit_button(cx))
    }

    /// Render the backups section content
    fn render_backups_content(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        if self.state.has_backups() {
            div()
                .id("recovery-backups-scroll")
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&self.scroll_handle)
                .child(
                    div()
                        .p(px(Theme::SPACING_MD))
                        .child(self.render_backup_list(cx)),
                )
                .into_any_element()
        } else {
            div()
                .id("recovery-no-backups-container")
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(Self::render_no_backups())
                .into_any_element()
        }
    }

    /// Render the top bar with title
    fn render_top_bar() -> impl IntoElement {
        div()
            .id("recovery-top-bar")
            .h(px(56.0))
            .w_full()
            .bg(Theme::bg_darkest())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(Theme::SPACING_LG))
            .flex()
            .items_center()
            .child(
                div()
                    .text_size(px(Theme::font_size_h3()))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Database Recovery"),
            )
    }

    /// Render the backup count indicator
    fn render_backup_count(&self) -> impl IntoElement {
        let count = self.state.backups.len();
        let label = if count == 1 {
            "1 backup available".to_string()
        } else {
            format!("{count} backups available")
        };

        div()
            .id("recovery-backup-count")
            .w_full()
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .bg(Theme::bg_dark())
            .border_b_1()
            .border_color(Theme::border())
            .child(
                div()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(Theme::text_muted())
                    .child(label),
            )
    }
}

impl gpui::Focusable for RecoveryView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for RecoveryView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let mut content = div()
            .id("recovery-view")
            .size_full()
            .bg(Theme::bg_base())
            .flex()
            .flex_col()
            .child(Self::render_top_bar());

        // Add status message if present
        if let Some(status) = self.render_status_message() {
            content = content.child(
                div()
                    .px(px(Theme::SPACING_LG))
                    .pt(px(Theme::SPACING_LG))
                    .child(status),
            );
        }

        content
            .child(
                div()
                    .id("recovery-content")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p(px(Theme::SPACING_LG))
                    .gap(px(Theme::SPACING_LG))
                    .child(self.render_error_header())
                    .when(self.state.has_backups(), |d| {
                        d.child(self.render_backup_count())
                    })
                    .child(self.render_backups_content(cx)),
            )
            .child(self.render_action_buttons(cx))
    }
}

// ============================================================================
// Event handler helpers
// ============================================================================

impl RecoveryView {
    /// Handle a restore completed event from the presenter
    pub fn handle_restore_completed(
        &mut self,
        success: bool,
        message: &str,
        cx: &mut gpui::Context<Self>,
    ) {
        self.state.restore_in_progress = false;
        if success {
            self.state.set_status(message, false);
        } else {
            self.state.set_status(message, true);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use chrono::{TimeZone, Utc};
    use gpui::TestAppContext;
    use std::path::PathBuf;

    fn create_test_backup(id: u64, timestamp: chrono::DateTime<Utc>, size: u64) -> BackupInfo {
        BackupInfo::new(
            PathBuf::from(format!("/backups/personalagent-{id}.db.gz")),
            timestamp,
            size,
        )
    }

    #[gpui::test]
    async fn recovery_view_constructs_with_error_message(cx: &mut TestAppContext) {
        let view = cx.new(|cx| RecoveryView::new(cx, "Database corruption detected"));
        view.update(cx, |view: &mut RecoveryView, _cx| {
            assert_eq!(view.state.error_message, "Database corruption detected");
            assert!(!view.state.has_backups());
            assert!(view.state.selected_backup_index.is_none());
        });
    }

    #[gpui::test]
    async fn recovery_view_with_backups_selects_first_by_default(cx: &mut TestAppContext) {
        let backups = vec![
            create_test_backup(1, Utc::now(), 1024 * 1024),
            create_test_backup(2, Utc::now() - chrono::Duration::hours(1), 1024 * 512),
        ];
        let view = cx.new(|cx| RecoveryView::with_backups(cx, "DB failed", backups));

        view.update(cx, |view: &mut RecoveryView, _cx| {
            assert_eq!(view.state.backups.len(), 2);
            assert_eq!(view.state.selected_backup_index, Some(0));
            assert!(view.state.has_backups());
        });
    }

    #[gpui::test]
    async fn state_select_backup_updates_index(cx: &mut TestAppContext) {
        let backups = vec![
            create_test_backup(1, Utc::now(), 1024 * 1024),
            create_test_backup(2, Utc::now() - chrono::Duration::hours(1), 1024 * 512),
        ];
        let view = cx.new(|cx| RecoveryView::with_backups(cx, "DB failed", backups));

        view.update(cx, |view: &mut RecoveryView, _cx| {
            view.state.select_backup(1);
            assert_eq!(view.state.selected_backup_index, Some(1));

            // Out of bounds should be ignored
            view.state.select_backup(10);
            assert_eq!(view.state.selected_backup_index, Some(1));
        });
    }

    #[gpui::test]
    async fn state_select_latest_chooses_first_backup(cx: &mut TestAppContext) {
        let backups = vec![
            create_test_backup(1, Utc::now(), 1024),
            create_test_backup(2, Utc::now() - chrono::Duration::hours(1), 512),
        ];
        let view = cx.new(|cx| RecoveryView::with_backups(cx, "DB failed", backups));

        view.update(cx, |view: &mut RecoveryView, _cx| {
            view.state.select_backup(1);
            assert_eq!(view.state.selected_backup_index, Some(1));

            view.state.select_latest();
            assert_eq!(view.state.selected_backup_index, Some(0));
        });
    }

    #[gpui::test]
    async fn set_backups_updates_state_and_notifies(cx: &mut TestAppContext) {
        let view = cx.new(|cx| RecoveryView::new(cx, "DB failed"));

        view.update(cx, |view: &mut RecoveryView, _cx| {
            assert!(!view.state.has_backups());
        });

        let new_backups = vec![
            create_test_backup(1, Utc::now(), 1024),
            create_test_backup(2, Utc::now() - chrono::Duration::hours(1), 512),
        ];

        view.update(cx, |view: &mut RecoveryView, cx| {
            view.set_backups(new_backups, cx);
            assert!(view.state.has_backups());
            assert_eq!(view.state.backups.len(), 2);
            assert_eq!(view.state.selected_backup_index, Some(0));
        });
    }

    #[gpui::test]
    async fn handle_restore_success_updates_state(cx: &mut TestAppContext) {
        let view = cx.new(|cx| RecoveryView::new(cx, "DB failed"));

        view.update(cx, |view: &mut RecoveryView, cx| {
            view.state.restore_in_progress = true;
            view.handle_restore_success(cx);
            assert!(!view.state.restore_in_progress);
            assert!(view.state.status_message.is_some());
            assert!(!view.state.status_is_error);
        });
    }

    #[gpui::test]
    async fn handle_restore_failed_updates_state(cx: &mut TestAppContext) {
        let view = cx.new(|cx| RecoveryView::new(cx, "DB failed"));

        view.update(cx, |view: &mut RecoveryView, cx| {
            view.state.restore_in_progress = true;
            view.handle_restore_failed("Permission denied", cx);
            assert!(!view.state.restore_in_progress);
            assert!(view.state.status_message.is_some());
            assert!(view.state.status_is_error);
        });
    }

    #[gpui::test]
    async fn set_restore_in_progress_updates_state(cx: &mut TestAppContext) {
        let view = cx.new(|cx| RecoveryView::new(cx, "DB failed"));

        view.update(cx, |view: &mut RecoveryView, cx| {
            assert!(!view.state.restore_in_progress);
            view.set_restore_in_progress(true, cx);
            assert!(view.state.restore_in_progress);
            view.set_restore_in_progress(false, cx);
            assert!(!view.state.restore_in_progress);
        });
    }

    #[test]
    fn recovery_result_variants() {
        let success = RecoveryResult::Success;
        assert_eq!(success, RecoveryResult::Success);

        let required = RecoveryResult::Required {
            error: "DB corrupted".to_string(),
            available_backups: vec![],
        };
        assert!(matches!(required, RecoveryResult::Required { .. }));
    }

    #[test]
    fn backup_info_formatting() {
        let backup = create_test_backup(
            123,
            Utc.with_ymd_and_hms(2026, 4, 5, 14, 30, 0).unwrap(),
            1024 * 1024 * 5,
        );

        assert_eq!(backup.formatted_timestamp(), "2026-04-05 14:30 UTC");
        assert_eq!(backup.formatted_size(), "5.00 MB");
    }
}
