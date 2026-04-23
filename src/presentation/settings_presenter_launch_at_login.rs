//! Launch-at-Login handlers for `SettingsPresenter`.
//!
//! Extracted from `settings_presenter.rs` to keep the main presenter below
//! the 1000-line file-length gate enforced by structural CI. Contains the
//! user-event dispatcher, the set-state action, and the startup snapshot
//! that combines the persisted preference with the actual OS registration
//! status so a user who revoked approval in System Settings while the app
//! was closed still sees the truth reflected in the toggle.

use std::sync::Arc;

use tokio::sync::broadcast;

use super::settings_presenter::SettingsPresenter;
use super::view_command::ViewCommand;
use crate::events::types::UserEvent;
use crate::services::login_item::{LoginItemService, LoginItemStatus};
use crate::services::AppSettingsService;

const REQUIRES_APPROVAL_MSG: &str = "Approval required: open System Settings -> General -> \
     Login Items to allow PersonalAgent.";
const NOT_FOUND_MSG: &str = "macOS could not find PersonalAgent.app. Launch-at-login \
     only works for the packaged .app bundle (not raw `cargo run` builds).";
const NOT_FOUND_SNAPSHOT_MSG: &str = "macOS could not find PersonalAgent.app. Launch-at-login \
     only works for the packaged .app bundle.";
const UNSUPPORTED_MSG: &str = "Launch-at-login is only supported on macOS 13+.";

impl SettingsPresenter {
    /// Dispatch launch-at-login related user events. Returns `true` if the
    /// event was handled so the caller can short-circuit further dispatch.
    pub(super) async fn handle_launch_at_login_user_event(
        app_settings_service: &Arc<dyn AppSettingsService>,
        login_item_service: &Arc<dyn LoginItemService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        event: &UserEvent,
    ) -> bool {
        match event {
            UserEvent::SetLaunchAtLogin { enabled } => {
                Self::on_set_launch_at_login(
                    app_settings_service,
                    login_item_service,
                    view_tx,
                    *enabled,
                )
                .await;
                true
            }
            _ => false,
        }
    }

    /// Persist the requested preference, then attempt to register or
    /// unregister the OS-level login item. On failure, roll back the
    /// persisted preference and surface the error in the view command so the
    /// settings UI can display it.
    async fn on_set_launch_at_login(
        app_settings_service: &Arc<dyn AppSettingsService>,
        login_item_service: &Arc<dyn LoginItemService>,
        view_tx: &broadcast::Sender<ViewCommand>,
        requested: bool,
    ) {
        let previous = app_settings_service
            .get_launch_at_login()
            .await
            .ok()
            .flatten()
            .unwrap_or(false);

        if let Err(e) = app_settings_service.set_launch_at_login(requested).await {
            tracing::warn!("Failed to persist launch_at_login={}: {}", requested, e);
            let _ = view_tx.send(ViewCommand::SetLaunchAtLoginState {
                enabled: previous,
                error: Some(format!("Could not save launch-at-login preference: {e}")),
            });
            return;
        }

        let os_result = if requested {
            login_item_service.register()
        } else {
            login_item_service.unregister()
        };

        match os_result {
            Ok(status) => {
                let effective = matches!(
                    status,
                    LoginItemStatus::Enabled | LoginItemStatus::RequiresApproval
                );
                let error = match status {
                    LoginItemStatus::RequiresApproval => Some(REQUIRES_APPROVAL_MSG.to_string()),
                    LoginItemStatus::NotFound => Some(NOT_FOUND_MSG.to_string()),
                    LoginItemStatus::Unsupported => Some(UNSUPPORTED_MSG.to_string()),
                    _ => None,
                };
                // Mirror effective OS state back into the persisted setting,
                // so a "RequiresApproval" registration is still reflected as
                // requested-on while NotFound/etc roll the toggle back off.
                if effective != requested {
                    let _ = app_settings_service.set_launch_at_login(effective).await;
                }
                let _ = view_tx.send(ViewCommand::SetLaunchAtLoginState {
                    enabled: effective,
                    error,
                });
            }
            Err(err) => {
                // Roll the persisted preference back to the previous value so
                // the toggle does not "stick" on an unrecoverable failure.
                let _ = app_settings_service.set_launch_at_login(previous).await;
                let _ = view_tx.send(ViewCommand::SetLaunchAtLoginState {
                    enabled: previous,
                    error: Some(err.0),
                });
            }
        }
    }

    /// Emit the initial launch-at-login state on startup. Combines the
    /// persisted preference with the actual OS registration status so the UI
    /// can disagree (e.g. user revoked it in System Settings while the app
    /// was closed).
    pub(super) async fn emit_launch_at_login_snapshot(
        app_settings_service: &Arc<dyn AppSettingsService>,
        login_item_service: &Arc<dyn LoginItemService>,
        view_tx: &broadcast::Sender<ViewCommand>,
    ) {
        let stored = app_settings_service
            .get_launch_at_login()
            .await
            .ok()
            .flatten()
            .unwrap_or(false);

        let (enabled, error) = match login_item_service.status() {
            Ok(LoginItemStatus::Enabled) => (true, None),
            Ok(LoginItemStatus::RequiresApproval) => {
                (true, Some(REQUIRES_APPROVAL_MSG.to_string()))
            }
            // NotRegistered is the ground-truth "off" state — trust the OS
            // over any stale preference we may have persisted.
            Ok(LoginItemStatus::NotRegistered) => (false, None),
            Ok(LoginItemStatus::NotFound) => (false, Some(NOT_FOUND_SNAPSHOT_MSG.to_string())),
            Ok(LoginItemStatus::Unsupported) => (false, Some(UNSUPPORTED_MSG.to_string())),
            Err(err) => (stored, Some(err.0)),
        };

        let _ = view_tx.send(ViewCommand::SetLaunchAtLoginState { enabled, error });
    }
}
