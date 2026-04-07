//! Backup-related actions for `SettingsView`.

use super::SettingsView;
use crate::events::types::UserEvent;

impl SettingsView {
    pub(super) fn emit_set_backup_enabled(&self, enabled: bool) {
        self.emit(&UserEvent::SetBackupEnabled { enabled });
    }

    pub(super) fn emit_set_backup_interval_hours(&self, hours: u32) {
        self.emit(&UserEvent::SetBackupIntervalHours { hours });
    }

    pub(super) fn emit_set_backup_max_copies(&self, copies: u32) {
        self.emit(&UserEvent::SetBackupMaxCopies { copies });
    }

    pub(super) fn emit_set_backup_directory(&self, path: Option<String>) {
        self.emit(&UserEvent::SetBackupDirectory { path });
    }

    pub(super) fn emit_trigger_backup_now(&self) {
        self.emit(&UserEvent::TriggerBackupNow);
    }

    pub(super) fn emit_restore_backup(&self, path: String) {
        // Mark restore as in progress
        self.emit(&UserEvent::RestoreBackup { path });
    }

    pub(super) fn emit_refresh_backup_list(&self) {
        self.emit(&UserEvent::RefreshBackupList);
    }

    #[allow(clippy::unused_self)]
    pub(super) fn browse_backup_directory(&mut self, cx: &mut gpui::Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select Backup Directory".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.first() {
                    let path_str = path.to_string_lossy().to_string();
                    cx.update(|cx| {
                        this.update(cx, |view, _cx| {
                            view.emit_set_backup_directory(Some(path_str));
                        })
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}
